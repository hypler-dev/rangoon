use super::*;
use rangoon_compose::application::{Request, Target};
use rangoon_domain::capability_v1::{Origin, RevisionProvenance};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rangoon-native-composition-{}-{}-{}", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
    fn store(&self) -> Workspace {
        Workspace::new(self.0.clone())
    }
    fn prepare(&self) -> CompositionPreview {
        let source = rangoon_import::analyze(
            "AGENTS.md",
            b"\xef\xbb\xbf# Rules\r\nKeep originals.\r\n# Tests\r\nCheck edits.\r\n",
        )
        .unwrap();
        self.store().save_v1(&source).unwrap();
        self.store().preview_composition(&draft()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn draft() -> Request {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/composition/decompose-v0.json"
    ))
    .unwrap();
    Request {
        schema_version: application::REQUEST_SCHEMA.into(),
        draft: serde_json::from_value(fixture["draft"].clone()).unwrap(),
        targets: vec![Target::New {}, Target::New {}],
    }
}
fn confirmation(result: CompositionResult) -> Confirmation {
    let encoded = serde_json::to_value(&result).unwrap();
    assert_eq!(encoded["outcome"], "ready");
    assert_eq!(encoded["schemaVersion"], SESSION_SCHEMA);
    assert!(encoded.get("request").is_none());
    match result {
        CompositionResult::Ready {
            preview_id,
            expected_state_id,
            ..
        } => Confirmation {
            schema_version: COMMIT_SCHEMA.into(),
            preview_id,
            expected_state_id,
            acknowledged: true,
        },
        _ => panic!("expected retained preview"),
    }
}
fn body(value: serde_json::Value) -> InvokeBody {
    InvokeBody::Raw(serde_json::to_vec(&value).unwrap())
}

#[test]
fn native_decoders_require_bounded_raw_closed_utf8_payloads() {
    let value = serde_json::to_value(draft()).unwrap();
    assert!(decode_preview(&body(value.clone())).is_ok());
    assert!(decode_preview(&InvokeBody::Json(value.clone())).is_err());
    assert!(
        decode_preview(&InvokeBody::Raw(vec![
            b' ';
            application::MAX_REQUEST_BYTES + 1
        ]))
        .is_err()
    );
    assert!(decode_preview(&InvokeBody::Raw(vec![255])).is_err());
    let mut extra = value;
    extra["originalBytes"] = "forged input".into();
    assert!(decode_preview(&body(extra)).is_err());
    let value = serde_json::json!({"schemaVersion": COMMIT_SCHEMA, "previewId": format!("preview:{}", "a".repeat(64)), "expectedStateId": "workspace:x", "acknowledged": true});
    assert!(decode_confirmation(&body(value.clone())).is_ok());
    assert!(decode_confirmation(&InvokeBody::Json(value.clone())).is_err());
    for field in [
        "schemaVersion",
        "previewId",
        "expectedStateId",
        "acknowledged",
    ] {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(decode_confirmation(&body(missing)).is_err());
    }
    let mut forged = value.clone();
    forged["draft"] = serde_json::to_value(draft()).unwrap();
    assert!(decode_confirmation(&body(forged)).is_err());
    let mut wrong = value.clone();
    wrong["schemaVersion"] = "unknown".into();
    assert!(decode_confirmation(&body(wrong)).is_err());
    let mut wrong = value.clone();
    wrong["previewId"] = "composition:fake".into();
    assert!(decode_confirmation(&body(wrong)).is_err());
    let mut duplicate = serde_json::to_string(&value).unwrap();
    duplicate.pop();
    duplicate.push_str(",\"acknowledged\":false}");
    assert!(decode_confirmation(&InvokeBody::Raw(duplicate.into_bytes())).is_err());
    assert!(decode_confirmation(&InvokeBody::Raw(vec![b' '; MAX_CONFIRMATION_BYTES + 1])).is_err());
}

#[test]
fn native_session_rejects_unissued_replaced_wrong_state_and_unacknowledged_handles() {
    let dir = Fixture::new();
    let prepared = dir.prepare();
    let session = CompositionSession::default();
    let mut first = confirmation(session.stage(prepared.clone()).unwrap());
    assert_eq!(first.preview_id.len(), 72);
    assert!(CompositionSession::default().selected(&first).is_err());
    first.acknowledged = false;
    assert!(matches!(
        session.selected(&first),
        Err(StoreError::CompositionAcknowledgmentRequired)
    ));
    first.acknowledged = true;
    let expected = first.expected_state_id.clone();
    first.expected_state_id = "workspace:wrong".into();
    assert!(session.selected(&first).is_err());
    first.expected_state_id = expected;
    assert!(session.selected(&first).is_ok());
    let second = confirmation(session.stage(prepared).unwrap());
    assert_ne!(first.preview_id, second.preview_id);
    assert!(session.selected(&first).is_err());
    assert!(session.selected(&second).is_ok());
}

#[test]
fn random_failure_preserves_previous_preview_and_never_issues_predictable_fallback() {
    let dir = Fixture::new();
    let prepared = dir.prepare();
    let session = CompositionSession::default();
    let first = confirmation(session.stage(prepared.clone()).unwrap());
    assert!(
        session
            .stage_with(prepared, |_| Err(StoreError::Unavailable))
            .is_err()
    );
    assert!(session.selected(&first).is_ok());
}

#[test]
fn native_commit_consumes_exact_preview_and_versioned_flows_survive_schema_three() {
    let dir = Fixture::new();
    let prepared = dir.prepare();
    let session = CompositionSession::default();
    let confirm = confirmation(session.stage(prepared).unwrap());
    let receipt = session.apply(&dir.store(), &confirm).unwrap();
    assert_eq!(receipt.capabilities.len(), 2);
    assert!(session.selected(&confirm).is_err());
    assert!(session.apply(&dir.store(), &confirm).is_err());
    let mut detail = receipt.capabilities[0].clone();
    assert!(matches!(detail.origin, Origin::Composition { .. }));
    assert!(matches!(
        detail.revision.provenance,
        RevisionProvenance::Composition { .. }
    ));
    assert!(detail.revision.review.is_none());
    detail = dir
        .store()
        .revise_capability_v1(
            &detail.id,
            &detail.latest_revision_id,
            "Edited",
            "Exact edit\r\n",
        )
        .unwrap()
        .capability;
    assert!(matches!(
        detail.revision.provenance,
        RevisionProvenance::Ordinary {}
    ));
    assert!(matches!(detail.origin, Origin::Composition { .. }));
    dir.store()
        .review_capability_v1(&detail.id, &detail.latest_revision_id)
        .unwrap();
    let later =
        rangoon_import::analyze("later.md", b"# Later\nSaved after composition.\n").unwrap();
    dir.store().save_v1(&later).unwrap();
    assert_eq!(dir.store().open(&later.source.id).unwrap(), later);
    assert_eq!(dir.store().list().unwrap().len(), 2);
    assert_eq!(dir.store().list_capabilities_v1().unwrap().len(), 2);
    let backup =
        rangoon_store::CompositionBackup::decode(&dir.store().export_composition_backup().unwrap())
            .unwrap();
    let restored = Fixture::new();
    let plan = restored
        .store()
        .prepare_composition_restore(&backup)
        .unwrap();
    restored
        .store()
        .restore_composition_backup(&backup, &plan)
        .unwrap();
    let reopened = restored
        .store()
        .open_capability_v1(&detail.id, None)
        .unwrap();
    assert_eq!(reopened.revision.content, "Exact edit\r\n");
    assert!(reopened.revision.review.is_some());
    assert_eq!(
        restored
            .store()
            .composition_data()
            .unwrap()
            .records
            .derivations,
        2
    );
    let plan = restored
        .store()
        .inspect_composition_deletion(rangoon_store::RecordKind::Capability, &detail.id)
        .unwrap();
    restored.store().delete_composition_record(&plan).unwrap();
    assert_eq!(restored.store().list_capabilities_v1().unwrap().len(), 1);
}

#[test]
fn changed_workspace_rejects_retained_preview_without_any_composition_write() {
    let dir = Fixture::new();
    let prepared = dir.prepare();
    let session = CompositionSession::default();
    let confirm = confirmation(session.stage(prepared).unwrap());
    let other = rangoon_import::analyze("other.md", b"Changed workspace\n").unwrap();
    dir.store().save_v1(&other).unwrap();
    let before = dir.store().export_composition_backup().unwrap();
    assert!(matches!(
        session.apply(&dir.store(), &confirm),
        Err(StoreError::WorkspaceChanged)
    ));
    assert_eq!(dir.store().export_composition_backup().unwrap(), before);
    assert!(session.selected(&confirm).is_err());
}

#[test]
fn failed_or_unknown_commit_retires_confirmation_even_when_state_is_unchanged() {
    let dir = Fixture::new();
    let session = CompositionSession::default();
    for uncertain in [false, true] {
        let prepared = dir
            .store()
            .preview_composition(&draft())
            .unwrap_or_else(|_| dir.prepare());
        let confirm = confirmation(session.stage(prepared).unwrap());
        let before = dir.store().composition_data().unwrap();
        let result = session.apply_with(&confirm, |prepared| {
            if uncertain {
                dir.store().apply_composition(prepared, true)?;
            }
            Err(StoreError::Unavailable)
        });
        assert!(matches!(result, Err(StoreError::Unavailable)));
        assert!(session.selected(&confirm).is_err());
        assert!(
            session
                .apply_with(&confirm, |_| panic!("spent handle must not reach storage"))
                .is_err()
        );
        if !uncertain {
            assert_eq!(
                before.state_id,
                dir.store().composition_data().unwrap().state_id
            );
            let fresh = confirmation(
                session
                    .stage(dir.store().preview_composition(&draft()).unwrap())
                    .unwrap(),
            );
            assert_ne!(fresh.preview_id, confirm.preview_id);
            assert!(session.selected(&fresh).is_ok());
        } else {
            // Simulate a committed transaction whose confirmation was lost.
            assert_eq!(dir.store().list_capabilities_v1().unwrap().len(), 2);
        }
    }
}

#[test]
fn local_command_registration_covers_composition() {
    for command in ["preview_composition", "commit_composition"] {
        assert!(include_str!("../build.rs").contains(&format!("\"{command}\"")));
        assert!(
            include_str!("../capabilities/source-analysis.json")
                .contains(&format!("\"allow-{}\"", command.replace('_', "-")))
        );
        assert!(include_str!("main.rs").contains(&format!("            {command},")));
    }
}
