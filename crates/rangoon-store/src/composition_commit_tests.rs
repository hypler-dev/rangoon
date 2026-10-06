use super::*;
use composition_records_tests::{
    TestDir, assert_unchanged, fixture_request, legacy, source, split_request,
};
use rangoon_compose::{
    Piece,
    application::{Request, Target},
};
use rangoon_domain::{
    Authority,
    capability_v1::{Origin, RevisionProvenance},
};

pub(super) fn fault_checkpoint(db: &Connection, phase: &str) -> Result<(), StoreError> {
    composition_recovery_tests::fault_checkpoint(db, phase)
}

fn apply(dir: &TestDir, request: &Request) -> CompositionReceipt {
    let preview = dir.store().preview_composition(request).unwrap();
    dir.store().apply_composition(&preview, true).unwrap()
}

#[test]
fn mixed_commit_preserves_sources_birth_and_reviewed_history_after_reopen() {
    let dir = TestDir::new();
    let saved = source(&dir);
    let old = legacy(&dir, &saved);
    dir.store()
        .review_capability(&old.id, &old.latest_revision_id)
        .unwrap();
    let mut request = fixture_request();
    request.targets[0] = Target::Append {
        capability_id: old.id.clone(),
        expected_revision_id: old.latest_revision_id.clone(),
    };
    let receipt = apply(&dir, &request);
    assert_eq!(receipt.authority, Authority::None);
    assert_eq!(receipt.capabilities.len(), 2);
    assert_eq!(receipt.capabilities[0].id, old.id);
    assert!(matches!(
        receipt.capabilities[0].origin,
        Origin::Source { .. }
    ));
    assert!(matches!(
        receipt.capabilities[1].origin,
        Origin::Composition { .. }
    ));
    for detail in &receipt.capabilities {
        assert_eq!(detail.authority, Authority::None);
        assert!(detail.revision.review.is_none());
        assert!(matches!(
            detail.revision.provenance,
            RevisionProvenance::Composition { .. }
        ));
        assert_eq!(
            dir.store().open_capability_v1(&detail.id, None).unwrap(),
            *detail
        );
    }
    assert!(
        dir.store()
            .open_capability_v1(&old.id, Some(&old.latest_revision_id))
            .unwrap()
            .revision
            .review
            .is_some()
    );
    assert_eq!(dir.store().open(&saved.source.id).unwrap(), saved);
    assert_eq!(
        dir.store().composition_data().unwrap().state_id,
        receipt.committed_state_id
    );
    let bytes = dir.store().export_composition_backup().unwrap();
    let backup = CompositionBackup::decode(&bytes).unwrap();
    let reopened = TestDir::new();
    let plan = reopened
        .store()
        .prepare_composition_restore(&backup)
        .unwrap();
    reopened
        .store()
        .restore_composition_backup(&backup, &plan)
        .unwrap();
    assert_eq!(reopened.store().export_composition_backup().unwrap(), bytes);
}

#[test]
fn acknowledgment_forged_preview_and_stale_state_fail_without_migration() {
    let dir = TestDir::new();
    source(&dir);
    let preview = dir.store().preview_composition(&fixture_request()).unwrap();
    let before = fs::read(dir.store().path()).unwrap();
    assert!(matches!(
        dir.store().apply_composition(&preview, false),
        Err(StoreError::CompositionAcknowledgmentRequired)
    ));
    let mut forged = preview.clone();
    forged.preview.core.outputs[0].content.push_str("forged");
    assert!(matches!(
        dir.store().apply_composition(&forged, true),
        Err(StoreError::CompositionInvalid)
    ));
    assert_unchanged(&dir, &before);
    let other = analyze("other.md", b"# Other\nAnother source.\n").unwrap();
    dir.store().save_v1(&other).unwrap();
    let changed = fs::read(dir.store().path()).unwrap();
    assert!(matches!(
        dir.store().apply_composition(&preview, true),
        Err(StoreError::WorkspaceChanged)
    ));
    assert_unchanged(&dir, &changed);
    assert_eq!(
        dir.db()
            .pragma_query_value::<i64, _>(None, "user_version", |r| r.get(0))
            .unwrap(),
        1
    );
    let fresh = dir.store().preview_composition(&fixture_request()).unwrap();
    dir.store().apply_composition(&fresh, true).unwrap();
    let committed = fs::read(dir.store().path()).unwrap();
    assert!(matches!(
        dir.store().apply_composition(&fresh, true),
        Err(StoreError::WorkspaceChanged)
    ));
    assert_unchanged(&dir, &committed);
}

#[test]
fn incomplete_draft_remains_editable_but_cannot_commit() {
    let dir = TestDir::new();
    source(&dir);
    let mut request = fixture_request();
    request.draft.outputs[0].title.clear();
    let preview = dir.store().preview_composition(&request).unwrap();
    assert!(!preview.preview.saveable);
    let before = fs::read(dir.store().path()).unwrap();
    assert!(matches!(
        dir.store().apply_composition(&preview, true),
        Err(StoreError::CompositionInvalid)
    ));
    assert_unchanged(&dir, &before);
}

#[test]
fn ordinary_edit_and_review_after_composition_keep_provenance_and_allow_new_sources() {
    let dir = TestDir::new();
    source(&dir);
    let initial = apply(&dir, &fixture_request()).capabilities.remove(0);
    let reviewed = dir
        .store()
        .review_capability_v1(&initial.id, &initial.latest_revision_id)
        .unwrap();
    assert!(!reviewed.already_applied);
    assert!(
        dir.store()
            .review_capability_v1(&initial.id, &initial.latest_revision_id)
            .unwrap()
            .already_applied
    );
    let edited = dir
        .store()
        .revise_capability_v1(
            &initial.id,
            &initial.latest_revision_id,
            "Edited",
            "New content\n",
        )
        .unwrap()
        .capability;
    assert_eq!(edited.origin, initial.origin);
    assert!(edited.revision.review.is_none());
    assert!(matches!(
        edited.revision.provenance,
        RevisionProvenance::Ordinary {}
    ));
    assert!(matches!(
        dir.store()
            .open_capability_v1(&initial.id, Some(&initial.latest_revision_id))
            .unwrap()
            .revision
            .provenance,
        RevisionProvenance::Composition { .. }
    ));
    assert!(
        dir.store()
            .revise_capability_v1(
                &initial.id,
                &initial.latest_revision_id,
                "Edited",
                "New content\n"
            )
            .unwrap()
            .already_applied
    );
    assert!(matches!(
        dir.store()
            .review_capability_v1(&initial.id, &initial.latest_revision_id),
        Err(StoreError::CapabilityConflict)
    ));
    let extra = analyze("extra.md", b"# Extra\nNew local input.\n").unwrap();
    dir.store().save_v1(&extra).unwrap();
    let created = dir
        .store()
        .create_capability_v1(&extra.source.id, &extra.fragments[0].id, "Extra")
        .unwrap();
    assert!(matches!(created.capability.origin, Origin::Source { .. }));
    assert!(
        dir.store()
            .create_capability_v1(&extra.source.id, &extra.fragments[0].id, "Extra")
            .unwrap()
            .already_applied
    );
    assert!(matches!(
        dir.store().save(&extra),
        Err(StoreError::UnsupportedSchema)
    ));
    assert!(matches!(
        dir.store()
            .revise_capability(&initial.id, &edited.latest_revision_id, "Old API", "No"),
        Err(StoreError::UnsupportedSchema)
    ));
    let bytes = dir.store().export_composition_backup().unwrap();
    CompositionBackup::decode(&bytes).unwrap();
}

#[test]
fn versioned_source_save_rejects_corrupt_history_before_writing() {
    let dir = TestDir::new();
    let saved = source(&dir);
    legacy(&dir, &saved);
    dir.db()
        .execute("UPDATE revisions SET content=?1", [b"altered".as_slice()])
        .unwrap();
    let before = fs::read(dir.store().path()).unwrap();
    let incoming = analyze("later.md", b"# Later\nKeep input.\n").unwrap();
    assert!(matches!(
        dir.store().save_v1(&incoming),
        Err(StoreError::Corrupt)
    ));
    assert_unchanged(&dir, &before);
}

#[test]
fn historical_input_can_append_to_current_head_without_changing_birth() {
    let dir = TestDir::new();
    let saved = source(&dir);
    let old = dir
        .store()
        .create_capability_v1(&saved.source.id, &saved.fragments[0].id, "Input")
        .unwrap()
        .capability;
    let current = dir
        .store()
        .revise_capability_v1(
            &old.id,
            &old.latest_revision_id,
            "Current",
            "Current revision\n",
        )
        .unwrap()
        .capability;
    let (mut request, _) = split_request(&old, 1);
    request.targets[0] = Target::Append {
        capability_id: old.id.clone(),
        expected_revision_id: current.latest_revision_id.clone(),
    };
    let receipt = apply(&dir, &request);
    let appended = &receipt.capabilities[0];
    assert_eq!(appended.origin, old.origin);
    assert_eq!(
        appended.revision.parent_revision_id,
        Some(current.latest_revision_id)
    );
    assert_eq!(appended.history.len(), 3);
    assert_eq!(
        dir.store()
            .open_capability_v1(&old.id, Some(&old.latest_revision_id))
            .unwrap()
            .revision
            .content,
        old.revision.content
    );
    assert!(matches!(
        dir.store().preview_composition(&request),
        Err(StoreError::CapabilityConflict)
    ));
}

#[test]
fn fresh_confirmed_application_can_recreate_deleted_output_without_implicit_resurrection() {
    let dir = TestDir::new();
    source(&dir);
    let original = apply(&dir, &fixture_request());
    let missing = &original.capabilities[0];
    let survivor = &original.capabilities[1];
    let plan = dir
        .store()
        .inspect_composition_deletion(RecordKind::Capability, &missing.id)
        .unwrap();
    dir.store().delete_composition_record(&plan).unwrap();
    assert_eq!(dir.store().list_capabilities_v1().unwrap().len(), 1);
    let mut request = fixture_request();
    request.targets[1] = Target::Append {
        capability_id: survivor.id.clone(),
        expected_revision_id: survivor.latest_revision_id.clone(),
    };
    let before = fs::read(dir.store().path()).unwrap();
    let preview = dir.store().preview_composition(&request).unwrap();
    assert_unchanged(&dir, &before);
    let receipt = dir.store().apply_composition(&preview, true).unwrap();
    assert_eq!(receipt.capabilities[0].id, missing.id);
    assert_eq!(
        receipt.capabilities[0].latest_revision_id,
        missing.latest_revision_id
    );
    assert_ne!(receipt.application_id, original.application_id);
    assert_eq!(receipt.capabilities[1].history.len(), 2);
    CompositionBackup::decode(&dir.store().export_composition_backup().unwrap()).unwrap();
}

#[test]
fn per_skill_capacity_rejects_entire_mixed_application_without_writes() {
    let dir = TestDir::new();
    let saved = source(&dir);
    let mut current = dir
        .store()
        .create_capability_v1(&saved.source.id, &saved.fragments[0].id, "Full")
        .unwrap()
        .capability;
    for index in 1..MAX_REVISIONS {
        current = dir
            .store()
            .revise_capability_v1(
                &current.id,
                &current.latest_revision_id,
                "Full",
                &format!("Revision {index}\n"),
            )
            .unwrap()
            .capability;
    }
    let mut request = fixture_request();
    request.targets[0] = Target::Append {
        capability_id: current.id,
        expected_revision_id: current.latest_revision_id,
    };
    let before = fs::read(dir.store().path()).unwrap();
    assert!(matches!(
        dir.store().preview_composition(&request),
        Err(StoreError::CapabilityFull)
    ));
    assert_unchanged(&dir, &before);
}

#[test]
fn concurrent_exact_previews_have_one_committed_winner() {
    use std::{
        sync::{Arc, Barrier},
        thread,
    };
    let dir = TestDir::new();
    source(&dir);
    let prepared = dir.store().preview_composition(&fixture_request()).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let mut tasks = Vec::new();
    for _ in 0..2 {
        let workspace = dir.store();
        let prepared = prepared.clone();
        let barrier = barrier.clone();
        tasks.push(thread::spawn(move || {
            barrier.wait();
            workspace.apply_composition(&prepared, true)
        }));
    }
    let results = tasks
        .into_iter()
        .map(|t| t.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(StoreError::WorkspaceChanged)))
            .count(),
        1
    );
    assert_eq!(dir.store().list_capabilities_v1().unwrap().len(), 2);
}

#[test]
fn merge_saved_derived_revisions_into_existing_skill_preserves_exact_inputs() {
    use rangoon_compose::{Draft, InputRange, InputReference, Operation, OutputRecipe};
    let dir = TestDir::new();
    source(&dir);
    let originals = apply(&dir, &fixture_request()).capabilities;
    let inputs = originals
        .iter()
        .map(|detail| InputReference::Revision {
            capability_id: detail.id.clone(),
            revision_id: detail.latest_revision_id.clone(),
            sha256: detail.revision.sha256.clone(),
        })
        .collect();
    let pieces = originals
        .iter()
        .enumerate()
        .map(|(index, detail)| Piece::Copy {
            range: InputRange {
                input_index: index as u32,
                start_byte: 0,
                end_byte: detail.revision.content.len() as u64,
            },
        })
        .collect();
    let request = Request {
        schema_version: rangoon_compose::application::REQUEST_SCHEMA.into(),
        draft: Draft {
            schema_version: "rangoon.composition-draft.v0".into(),
            operation: Operation::Merge,
            inputs,
            outputs: vec![OutputRecipe {
                title: "Merged".into(),
                pieces,
            }],
            exclusions: vec![],
            duplications: vec![],
            conflicts: vec![],
        },
        targets: vec![Target::Append {
            capability_id: originals[0].id.clone(),
            expected_revision_id: originals[0].latest_revision_id.clone(),
        }],
    };
    let merged = apply(&dir, &request).capabilities.remove(0);
    assert_eq!(
        merged.revision.content,
        originals
            .iter()
            .map(|d| d.revision.content.as_str())
            .collect::<String>()
    );
    assert_eq!(merged.origin, originals[0].origin);
    for original in &originals {
        assert_eq!(
            dir.store()
                .open_capability_v1(&original.id, Some(&original.latest_revision_id))
                .unwrap()
                .revision,
            original.revision
        );
    }
    let dependency = dir
        .store()
        .inspect_composition_deletion(RecordKind::Capability, &originals[1].id)
        .unwrap();
    assert_eq!(dependency.dependencies.len(), 1);
    assert_eq!(dependency.dependencies[0].id, merged.id);
    CompositionBackup::decode(&dir.store().export_composition_backup().unwrap()).unwrap();
}

#[test]
fn interrupted_first_composition_keeps_source_only_schema() {
    for phase in ["after_migration", "after_output"] {
        let dir = TestDir::new();
        source(&dir);
        let request = fixture_request();
        let path = dir.0.join("request.json");
        fs::write(&path, serde_json::to_vec(&request).unwrap()).unwrap();
        let before = dir.store().export_composition_backup().unwrap();
        composition_recovery_tests::fault_child_for(
            &dir,
            &path,
            phase,
            "crash",
            "",
            "composition_commit_tests::commit_fault_child",
        );
        assert_eq!(dir.store().export_composition_backup().unwrap(), before);
        assert_eq!(
            dir.db()
                .pragma_query_value::<i64, _>(None, "user_version", |r| r.get(0))
                .unwrap(),
            1
        );
        assert_eq!(apply(&dir, &request).capabilities.len(), 2);
    }
}

#[test]
fn interrupted_or_full_mixed_commit_preserves_prior_schema_reviews_and_heads() {
    for (mode, phase) in [
        ("crash", "after_migration"),
        ("crash", "after_output"),
        ("full", "before_migration"),
        ("full", "after_migration"),
    ] {
        let dir = TestDir::new();
        let saved = source(&dir);
        let old = legacy(&dir, &saved);
        dir.store()
            .review_capability(&old.id, &old.latest_revision_id)
            .unwrap();
        let mut request = fixture_request();
        request.targets[0] = Target::Append {
            capability_id: old.id.clone(),
            expected_revision_id: old.latest_revision_id.clone(),
        };
        request.draft.outputs[0].pieces.push(Piece::Authored {
            content: "x".repeat(200 * 1024),
            reason: "Allocation and interruption fixture".into(),
        });
        let path = dir.0.join("request.json");
        fs::write(&path, serde_json::to_vec(&request).unwrap()).unwrap();
        let before = dir.store().export_composition_backup().unwrap();
        composition_recovery_tests::fault_child_for(
            &dir,
            &path,
            phase,
            mode,
            "",
            "composition_commit_tests::commit_fault_child",
        );
        assert_eq!(dir.store().export_composition_backup().unwrap(), before);
        assert_eq!(
            dir.db()
                .pragma_query_value::<i64, _>(None, "user_version", |r| r.get(0))
                .unwrap(),
            2
        );
        let receipt = apply(&dir, &request);
        assert_eq!(receipt.capabilities.len(), 2);
        assert!(
            dir.store()
                .open_capability_v1(&old.id, Some(&old.latest_revision_id))
                .unwrap()
                .revision
                .review
                .is_some()
        );
    }
}

#[test]
#[ignore = "isolated child used by public composition commit crash and SQLite-full tests"]
fn commit_fault_child() {
    let workspace = Workspace::new(PathBuf::from(
        std::env::var_os("RANGOON_RECOVERY_TEST_WORKSPACE").unwrap(),
    ));
    let bytes = fs::read(std::env::var_os("RANGOON_RECOVERY_TEST_BACKUP").unwrap()).unwrap();
    let request = rangoon_compose::application::decode_request_json(&bytes).unwrap();
    let prepared = workspace.preview_composition(&request).unwrap();
    let result = workspace.apply_composition(&prepared, true);
    assert_eq!(
        std::env::var("RANGOON_RECOVERY_TEST_MODE").as_deref(),
        Ok("full")
    );
    assert!(matches!(result, Err(StoreError::Unavailable)));
}
