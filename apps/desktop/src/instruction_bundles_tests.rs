use super::*;
use rangoon_domain::capability_v1::CapabilityDetail;
use serde_json::{Value, json};
use std::{
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rangoon-bundle-native-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> Workspace {
        Workspace::new(self.0.join("workspace"))
    }
    fn database(&self) -> PathBuf {
        self.0.join("workspace/workspace.sqlite3")
    }
    fn output(&self) -> PathBuf {
        self.0.join("Exact 🦀.rangoon-instructions")
    }
    fn skill(&self) -> CapabilityDetail {
        let source =
            rangoon_import::analyze("AGENTS.md", "\u{feff}# Rules\r\nKeep 🦀 exactly".as_bytes())
                .unwrap();
        let store = self.store();
        store.save_v1(&source).unwrap();
        store
            .create_capability_v1(&source.source.id, &source.fragments[0].id, "Exact")
            .unwrap()
            .capability
    }
    fn request(&self, skill: &CapabilityDetail) -> ExportRequest {
        let result = self
            .store()
            .compile_capability(&skill.id, &skill.revision.id, Profile::AgentsMdV1)
            .unwrap();
        ExportRequest {
            schema_version: EXPORT_REQUEST.into(),
            capability_id: skill.id.clone(),
            revision_id: skill.revision.id.clone(),
            profile: Profile::AgentsMdV1,
            expected_candidate_id: result.compilation.candidate.unwrap().candidate_id,
        }
    }
    fn reviewed(&self) -> (CapabilityDetail, ExportRequest) {
        let skill = self.skill();
        self.store()
            .review_capability_v1(&skill.id, &skill.revision.id)
            .unwrap();
        let request = self.request(&skill);
        (skill, request)
    }
    fn export(&self, request: &ExportRequest) -> Value {
        value(export_selected(
            request,
            || Ok(Some(self.output())),
            || Ok(self.store()),
            write_selected_instruction_bundle,
        ))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn value(result: BundleResult) -> Value {
    serde_json::to_value(result).unwrap()
}
fn raw(value: &Value) -> InvokeBody {
    InvokeBody::Raw(serde_json::to_vec(value).unwrap())
}
fn export_body() -> Value {
    json!({"schemaVersion":EXPORT_REQUEST,"capabilityId":format!("capability:{}","a".repeat(64)),"revisionId":format!("revision:{}","b".repeat(64)),"profile":"agents_md_v1","expectedCandidateId":format!("candidate:{}","c".repeat(64))})
}

#[test]
fn raw_requests_are_bounded_closed_and_path_free() {
    let expected = export_body();
    assert!(decode_export(&raw(&expected)).is_ok());
    assert!(decode_export(&InvokeBody::Json(expected.clone())).is_err());
    assert!(decode_export(&InvokeBody::Raw(vec![255])).is_err());
    for field in expected.as_object().unwrap().keys() {
        let mut missing = expected.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(decode_export(&raw(&missing)).is_err());
        let mut wrong = expected.clone();
        wrong[field] = Value::Null;
        assert!(decode_export(&raw(&wrong)).is_err());
        let mut future = expected.clone();
        future[field] = "future".into();
        assert!(decode_export(&raw(&future)).is_err());
    }
    for field in [
        "path",
        "content",
        "review",
        "bytes",
        "manifest",
        "requirements",
    ] {
        let mut extra = expected.clone();
        extra[field] = true.into();
        assert!(decode_export(&raw(&extra)).is_err());
    }
    let mut duplicate = serde_json::to_string(&expected).unwrap();
    duplicate.pop();
    duplicate.push_str(",\"profile\":\"agents_md_v1\"}");
    assert!(decode_export(&InvokeBody::Raw(duplicate.into_bytes())).is_err());
    let mut bytes = serde_json::to_vec(&expected).unwrap();
    bytes.resize(MAX_REQUEST_BYTES, b' ');
    assert!(decode_export(&InvokeBody::Raw(bytes.clone())).is_ok());
    bytes.push(b' ');
    assert!(decode_export(&InvokeBody::Raw(bytes)).is_err());
    let inspect = json!({"schemaVersion":INSPECT_REQUEST});
    assert!(decode_inspect(&raw(&inspect)).is_ok());
    for invalid in [
        json!({}),
        json!({"schemaVersion":null}),
        json!({"schemaVersion":"future"}),
        json!({"schemaVersion":INSPECT_REQUEST,"path":"private"}),
    ] {
        assert!(decode_inspect(&raw(&invalid)).is_err());
    }
    assert!(decode_inspect(&InvokeBody::Json(inspect)).is_err());
    let duplicate = format!(
        "{{\"schemaVersion\":\"{INSPECT_REQUEST}\",\"schemaVersion\":\"{INSPECT_REQUEST}\"}}"
    );
    assert!(decode_inspect(&InvokeBody::Raw(duplicate.into_bytes())).is_err());
    assert!(decode_inspect(&InvokeBody::Raw(vec![b' '; MAX_REQUEST_BYTES + 1])).is_err());
}

#[test]
fn cancel_and_picker_failure_do_not_resolve_workspace_or_write() {
    let request = decode_export(&raw(&export_body())).unwrap();
    let cancelled = export_selected(
        &request,
        || Ok(None),
        || panic!("store opened on cancel"),
        |_, _| panic!("write on cancel"),
    );
    assert_eq!(value(cancelled), json!({"outcome":"cancelled"}));
    let failed = export_selected(
        &request,
        || Err(unavailable()),
        || panic!("store opened on picker failure"),
        |_, _| panic!("write on picker failure"),
    );
    assert_eq!(value(failed)["outputState"], "not_created");
    assert_eq!(
        value(inspect_selected(|| Ok(None))),
        json!({"outcome":"cancelled"})
    );
}

#[test]
fn selected_export_preserves_exact_bytes_and_returns_correlated_receipt() {
    let fixture = Fixture::new();
    let (_, request) = fixture.reviewed();
    let before = std::fs::read(fixture.database()).unwrap();
    let picked = Cell::new(false);
    let result = value(export_selected(
        &request,
        || {
            picked.set(true);
            Ok(Some(fixture.output()))
        },
        || {
            assert!(picked.get());
            Ok(fixture.store())
        },
        write_selected_instruction_bundle,
    ));
    assert_eq!(result["outcome"], "exported");
    assert_eq!(result.as_object().unwrap().len(), 7);
    assert_eq!(
        result["schemaVersion"],
        "rangoon.instruction-bundle-export.v1"
    );
    assert_eq!(result["authority"], "none");
    let bytes = std::fs::read(fixture.output()).unwrap();
    let inspection = inspect_bundle(&bytes).unwrap();
    assert_eq!(result["candidateId"], request.expected_candidate_id);
    assert_eq!(result["bundleId"], inspection.bundle_id);
    assert_eq!(result["sha256"], inspection.sha256);
    assert_eq!(result["byteLength"], bytes.len());
    assert_eq!(
        inspection.compilation.artifact.content,
        "\u{feff}# Rules\r\nKeep 🦀 exactly"
    );
    assert_eq!(before, std::fs::read(fixture.database()).unwrap());
    let collision = fixture.export(&request);
    assert_eq!(collision["error"]["code"], "instruction_bundle_exists");
    assert_eq!(collision["outputState"], "not_created");
    assert_eq!(bytes, std::fs::read(fixture.output()).unwrap());
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains(fixture.0.to_str().unwrap())
    );
}

#[test]
fn post_picker_resolution_rejects_lost_review_or_missing_records_before_write() {
    let fixture = Fixture::new();
    let skill = fixture.skill();
    let unreviewed = std::fs::read(fixture.database()).unwrap();
    fixture
        .store()
        .review_capability_v1(&skill.id, &skill.revision.id)
        .unwrap();
    let request = fixture.request(&skill);
    let result = value(export_selected(
        &request,
        || {
            // A second process restored an older, valid database while the picker was open.
            std::fs::write(fixture.database(), &unreviewed).unwrap();
            Ok(Some(fixture.output()))
        },
        || Ok(fixture.store()),
        |_, _| panic!("unreviewed candidate reached writer"),
    ));
    assert_eq!(result["error"]["code"], "instruction_candidate_changed");
    assert_eq!(result["outputState"], "not_created");
    assert_eq!(std::fs::read(fixture.database()).unwrap(), unreviewed);
    assert!(!fixture.output().exists());
    let missing = value(export_selected(
        &request,
        || Ok(Some(fixture.output())),
        || Ok(Workspace::new(fixture.0.join("missing"))),
        |_, _| panic!("missing candidate reached writer"),
    ));
    assert_eq!(missing["error"]["code"], "instruction_candidate_changed");
    assert!(!fixture.0.join("missing").exists());
}

#[test]
fn historical_revision_remains_exportable_and_wrong_candidate_cannot_write() {
    let fixture = Fixture::new();
    let (skill, mut request) = fixture.reviewed();
    fixture
        .store()
        .revise_capability_v1(
            &skill.id,
            &skill.latest_revision_id,
            "New head",
            "Different",
        )
        .unwrap();
    let before = std::fs::read(fixture.database()).unwrap();
    assert_eq!(fixture.export(&request)["outcome"], "exported");
    assert_eq!(before, std::fs::read(fixture.database()).unwrap());
    request.expected_candidate_id = format!("candidate:{}", "a".repeat(64));
    let result = value(export_selected(
        &request,
        || Ok(Some(fixture.0.join("other.rangoon-instructions"))),
        || Ok(fixture.store()),
        |_, _| panic!("wrong candidate reached writer"),
    ));
    assert_eq!(result["error"]["code"], "instruction_candidate_changed");
    request.capability_id = format!("capability:{}", "b".repeat(64));
    assert_eq!(
        fixture.export(&request)["error"]["code"],
        "instruction_candidate_changed"
    );
}

#[test]
fn existing_wrong_owner_revision_and_blocked_profile_never_reach_writer() {
    let fixture = Fixture::new();
    let (first, _) = fixture.reviewed();
    let store = fixture.store();
    let source = rangoon_import::analyze("Second.md", b"# Different\nSee @external").unwrap();
    store.save_v1(&source).unwrap();
    let second = store
        .create_capability_v1(&source.source.id, &source.fragments[0].id, "Other")
        .unwrap()
        .capability;
    store
        .review_capability_v1(&second.id, &second.revision.id)
        .unwrap();
    let mut request = fixture.request(&second);
    request.capability_id = first.id;
    let before = std::fs::read(fixture.database()).unwrap();
    let wrong_owner = value(export_selected(
        &request,
        || Ok(Some(fixture.output())),
        || Ok(fixture.store()),
        |_, _| panic!("wrong owner reached writer"),
    ));
    assert_eq!(
        wrong_owner["error"]["code"],
        "instruction_candidate_changed"
    );
    assert_eq!(wrong_owner["outputState"], "not_created");
    request.capability_id = second.id;
    request.profile = Profile::ClaudeMdV1;
    let blocked = value(export_selected(
        &request,
        || Ok(Some(fixture.output())),
        || Ok(fixture.store()),
        |_, _| panic!("blocked profile reached writer"),
    ));
    assert_eq!(blocked["error"]["code"], "instruction_candidate_changed");
    assert_eq!(blocked["outputState"], "not_created");
    assert!(!fixture.output().exists());
    assert_eq!(before, std::fs::read(fixture.database()).unwrap());
}

#[test]
fn uncertain_writer_and_join_results_never_claim_no_output() {
    let fixture = Fixture::new();
    let (_, request) = fixture.reviewed();
    let result = value(export_selected(
        &request,
        || Ok(Some(fixture.output())),
        || Ok(fixture.store()),
        |path, bytes| {
            std::fs::write(path, &bytes[..20]).unwrap();
            Err(public(
                "instruction_bundle_write_failed",
                "A file may remain.",
            ))
        },
    ));
    assert_eq!(result["outcome"], "failed");
    assert_eq!(result["outputState"], "uncertain");
    assert_eq!(std::fs::metadata(fixture.output()).unwrap().len(), 20);
    assert_eq!(value(join_failure(true))["outputState"], "uncertain");
    assert_eq!(value(join_failure(false))["outputState"], "not_created");
    assert_eq!(value(busy())["outputState"], "not_created");
}

#[test]
fn external_inspection_is_canonical_and_needs_no_workspace() {
    let source = Fixture::new();
    let (_, request) = source.reviewed();
    source.export(&request);
    let external = Fixture::new();
    std::fs::copy(source.output(), external.output()).unwrap();
    let before = std::fs::read(external.output()).unwrap();
    let inspected = value(inspect_selected(|| Ok(Some(external.output()))));
    assert_eq!(inspected["outcome"], "inspected");
    assert_eq!(inspected.as_object().unwrap().len(), 4);
    assert_eq!(
        inspected["schemaVersion"],
        "rangoon.instruction-bundle-file.v1"
    );
    assert_eq!(
        inspected["inspection"]["verification"],
        "internal_consistency_only"
    );
    assert_eq!(inspected["inspection"]["authority"], "none");
    let parsed: Value =
        serde_json::from_str(inspected["candidateManifestJson"].as_str().unwrap()).unwrap();
    assert_eq!(
        parsed,
        inspected["inspection"]["compilation"]["candidate"]["manifest"]
    );
    assert!(!external.0.join("workspace").exists());
    assert_eq!(before, std::fs::read(external.output()).unwrap());
    std::fs::write(external.output(), b"hostile invalid bytes").unwrap();
    let invalid = value(inspect_selected(|| Ok(Some(external.output()))));
    assert_eq!(invalid["outputState"], "not_created");
    assert_eq!(invalid["error"]["code"], "bundle_invalid");
    assert!(!invalid.to_string().contains("hostile"));
    assert!(!external.0.join("workspace").exists());
}

#[test]
fn instruction_commands_are_registered_only_for_local_main() {
    let capability: Value =
        serde_json::from_str(include_str!("../capabilities/source-analysis.json")).unwrap();
    assert_eq!(capability["local"], true);
    assert_eq!(capability["windows"], json!(["main"]));
    let build = include_str!("../build.rs");
    let main = include_str!("main.rs");
    for (permission, command) in [
        (
            "allow-export-instruction-bundle",
            "export_instruction_bundle",
        ),
        (
            "allow-inspect-instruction-bundle",
            "inspect_instruction_bundle",
        ),
    ] {
        assert!(
            capability["permissions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p == permission)
        );
        assert!(build.contains(command));
        assert!(main.contains(command));
    }
}
