//! Real public workflow lifecycle over disposable on-disk workspaces.
use super::*;
use composition_records::Records;
use composition_records_tests::{TestDir, fixture_request, legacy, source};
use rangoon_workflow::records::{
    SaveIntent, WorkflowRevision, prepare_revision, workflow_id_from_nonce,
};

fn draft(
    seed: u8,
    parent: Option<&str>,
    title: &str,
    reference: Option<(&str, &str)>,
) -> WorkflowRevision {
    let nodes = reference.map(|(cap, revision)| serde_json::json!([
        {"id":"skill","title":"Skill","operation":{"kind":"capability","capabilityId":cap,"revisionId":revision},"inputs":[],"outputs":[]}
    ])).unwrap_or_else(||serde_json::json!([]));
    let definition = serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":title,"nodes":nodes,"controlEdges":[],"dataEdges":[]});
    let layout = if reference.is_some() {
        br#"{"positions":[{"nodeIndex":0,"x":-40,"y":20}]}"#.as_slice()
    } else {
        br#"{"positions":[]}"#.as_slice()
    };
    prepare_revision(
        &workflow_id_from_nonce(&[seed; 32]),
        parent,
        SaveIntent::Draft,
        &serde_json::to_vec(&definition).unwrap(),
        layout,
    )
    .unwrap()
}
fn saved(store: &Workspace, candidate: &WorkflowRevision) -> WorkflowSaveReceipt {
    let plan = store.inspect_workflow_save(candidate).unwrap();
    store
        .save_workflow(candidate, &plan.expected_state_id)
        .unwrap()
}

#[test]
fn first_save_restart_append_historical_open_and_exact_retry_preserve_bytes() {
    let dir = TestDir::new();
    let store = dir.store();
    let root = draft(1, None, "Unfinished 🦀\r\n", None);
    let inspected = store.inspect_workflow_save(&root).unwrap();
    assert!(!dir.0.exists());
    assert_eq!(
        store
            .save_workflow(&root, &format!("workspace:{}", "0".repeat(64)))
            .unwrap_err(),
        StoreError::WorkspaceChanged
    );
    assert!(!dir.0.exists());
    let first = store
        .save_workflow(&root, &inspected.expected_state_id)
        .unwrap();
    assert!(!first.already_saved);
    assert!(!first.workflow.head.structurally_valid);
    assert_eq!(
        first.workflow.revision.serialized_bytes().unwrap(),
        root.serialized_bytes().unwrap()
    );
    assert_eq!(
        dir.store().open_workflow(root.workflow_id(), None).unwrap(),
        first.workflow
    );
    let before = fs::read(store.path()).unwrap();
    let retried = store
        .save_workflow(&root, &inspected.expected_state_id)
        .unwrap();
    assert!(retried.already_saved);
    assert_eq!(retried.workflow, first.workflow);
    assert_eq!(fs::read(store.path()).unwrap(), before);
    let child = draft(1, Some(root.id()), "Still editing", None);
    let next = saved(&store, &child);
    assert_eq!(next.workflow.history.len(), 2);
    assert_eq!(
        store
            .open_workflow(root.workflow_id(), Some(root.id()))
            .unwrap()
            .revision,
        root
    );
    assert_eq!(
        store.list_workflows().unwrap()[0].latest_revision_id,
        child.id()
    );
    let data = store.workflow_data().unwrap();
    assert_eq!(
        (data.records.workflows, data.records.workflow_revisions),
        (1, 2)
    );
    assert_eq!(data.state_id, next.state_id);
    assert_eq!(
        store.save_workflow(&root, &next.state_id).unwrap_err(),
        StoreError::WorkflowConflict
    );
}

#[test]
fn public_backup_restore_roundtrip_preserves_old_schemas_and_complete_records() {
    for version in 1..=3 {
        let dir = TestDir::new();
        let report = source(&dir);
        if version >= 2 {
            legacy(&dir, &report);
        }
        if version == 3 {
            let preview = dir.store().preview_composition(&fixture_request()).unwrap();
            dir.store().apply_composition(&preview, true).unwrap();
        }
        let before = dir.store().export_workflow_backup().unwrap();
        assert!(before.starts_with(b"RANGOON-BACKUP-V2\n"));
        let root = draft(version as u8 + 10, None, "Imported workflow", None);
        saved(&dir.store(), &root);
        let complete = Records::load_complete(&dir.db()).unwrap();
        let projected = workflow_recovery::base_projection(&complete);
        assert_eq!(composition_backup::encode(&projected).unwrap(), before);
        let archive_bytes = dir.store().export_workflow_backup().unwrap();
        assert!(archive_bytes.starts_with(b"RANGOON-BACKUP-V3\n"));
        let archive = WorkspaceBackup::decode(&archive_bytes).unwrap();
        let dest = TestDir::new();
        let plan = dest.store().prepare_workflow_restore(&archive).unwrap();
        assert!(!dest.0.exists());
        let data = dest
            .store()
            .restore_workflow_backup(&archive, &plan)
            .unwrap();
        assert_eq!(data.records.workflows, 1);
        assert_eq!(
            dest.store().export_workflow_backup().unwrap(),
            archive_bytes
        );
        assert_eq!(dest.store().open(&report.source.id).unwrap(), report);
        assert_eq!(
            dest.store()
                .open_workflow(root.workflow_id(), None)
                .unwrap()
                .revision,
            root
        );
        let plan = dest.store().prepare_workflow_restore(&archive).unwrap();
        let before = fs::read(dest.store().path()).unwrap();
        dest.store()
            .restore_workflow_backup(&archive, &plan)
            .unwrap();
        assert_eq!(fs::read(dest.store().path()).unwrap(), before);
    }
}

#[test]
fn public_restore_keeps_exact_prefix_and_rejects_newer_or_stale_plan() {
    let dir = TestDir::new();
    let store = dir.store();
    let root = draft(20, None, "Root", None);
    saved(&store, &root);
    let older = WorkspaceBackup::decode(&store.export_workflow_backup().unwrap()).unwrap();
    let retained = store.prepare_workflow_restore(&older).unwrap();
    let child = draft(20, Some(root.id()), "Child", None);
    saved(&store, &child);
    let before = fs::read(store.path()).unwrap();
    assert_eq!(
        store
            .restore_workflow_backup(&older, &retained)
            .unwrap_err(),
        StoreError::WorkspaceChanged
    );
    assert_eq!(fs::read(store.path()).unwrap(), before);
    let plan = store.prepare_workflow_restore(&older).unwrap();
    store.restore_workflow_backup(&older, &plan).unwrap();
    assert_eq!(fs::read(store.path()).unwrap(), before);
    assert_eq!(
        store
            .open_workflow(root.workflow_id(), None)
            .unwrap()
            .revision,
        child
    );
    let newer = WorkspaceBackup::decode(&store.export_workflow_backup().unwrap()).unwrap();
    let dest = TestDir::new();
    let plan = dest.store().prepare_workflow_restore(&older).unwrap();
    dest.store().restore_workflow_backup(&older, &plan).unwrap();
    let before = fs::read(dest.store().path()).unwrap();
    assert_eq!(
        dest.store().prepare_workflow_restore(&newer).unwrap_err(),
        StoreError::BackupInvalid
    );
    assert_eq!(fs::read(dest.store().path()).unwrap(), before);
}

#[test]
fn historical_draft_dependency_blocks_skill_deletion_until_workflow_is_deleted() {
    let dir = TestDir::new();
    let report = source(&dir);
    let cap = legacy(&dir, &report);
    let store = dir.store();
    let root = draft(30, None, "Pinned draft", Some((&cap.id, "bad revision")));
    let first = saved(&store, &root);
    assert_eq!(first.workflow.head.unresolved_references, 1);
    let child = draft(30, Some(root.id()), "No current reference", None);
    let next = saved(&store, &child);
    let blocked = store
        .inspect_workflow_deletion(WorkflowRecordKind::Capability, &cap.id)
        .unwrap();
    let wire = serde_json::to_value(&blocked).unwrap();
    assert_eq!(wire["workflowDependencies"].as_array().unwrap().len(), 1);
    let before = fs::read(store.path()).unwrap();
    assert_eq!(
        store.delete_workflow_record(&blocked).unwrap_err(),
        StoreError::RecordInUse
    );
    assert_eq!(fs::read(store.path()).unwrap(), before);
    let plan = store
        .inspect_workflow_deletion(WorkflowRecordKind::Workflow, root.workflow_id())
        .unwrap();
    let deleted = store.delete_workflow_record(&plan).unwrap();
    assert_eq!(
        (
            deleted.records.workflows,
            deleted.records.workflow_revisions
        ),
        (0, 0)
    );
    assert_eq!(deleted.records.capabilities, 1);
    assert_eq!(Records::load_complete(&dir.db()).unwrap().version, 4);
    assert_eq!(
        store.save_workflow(&child, &next.state_id).unwrap_err(),
        StoreError::WorkspaceChanged
    );
    let plan = store
        .inspect_workflow_deletion(WorkflowRecordKind::Capability, &cap.id)
        .unwrap();
    store.delete_workflow_record(&plan).unwrap();
    let plan = store
        .inspect_workflow_deletion(WorkflowRecordKind::Source, &report.source.id)
        .unwrap();
    let cleared = store.delete_workflow_record(&plan).unwrap();
    assert_eq!(cleared.records, WorkflowRecoveryCounts::default());
    assert_eq!(Records::load_complete(&dir.db()).unwrap().version, 4);
    assert!(
        store
            .export_workflow_backup()
            .unwrap()
            .starts_with(b"RANGOON-BACKUP-V3\n")
    );
}

#[test]
fn competing_public_saves_commit_once_and_exact_concurrent_retries_are_noops() {
    use std::sync::{Arc, Barrier};
    for (existing, same) in [(false, false), (false, true), (true, false), (true, true)] {
        let dir = TestDir::new();
        if existing {
            source(&dir);
        }
        let root = draft(40, None, "First", None);
        let state = dir
            .store()
            .inspect_workflow_save(&root)
            .unwrap()
            .expected_state_id;
        let barrier = Arc::new(Barrier::new(2));
        let handles: Vec<_> = [
            root.clone(),
            if same {
                root.clone()
            } else {
                draft(41, None, "Other", None)
            },
        ]
        .into_iter()
        .map(|candidate| {
            let store = dir.store();
            let state = state.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store.save_workflow(&candidate, &state)
            })
        })
        .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        if same {
            assert!(results.iter().all(Result::is_ok));
            assert_eq!(
                results
                    .iter()
                    .filter(|r| !r.as_ref().unwrap().already_saved)
                    .count(),
                1
            );
        } else {
            assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
            assert!(
                results
                    .iter()
                    .any(|r| matches!(r, Err(StoreError::WorkspaceChanged)))
            );
        }
        let data = dir.store().workflow_data().unwrap();
        assert_eq!(
            (data.records.workflows, data.records.workflow_revisions),
            (1, 1)
        );
    }
}

#[test]
fn stale_deletion_plan_rejects_reset_empty_file_without_initializing_it() {
    let dir = TestDir::new();
    let root = draft(50, None, "Saved", None);
    saved(&dir.store(), &root);
    let plan = dir
        .store()
        .inspect_workflow_deletion(WorkflowRecordKind::Workflow, root.workflow_id())
        .unwrap();
    fs::write(dir.store().path(), []).unwrap();
    assert_eq!(
        dir.store().delete_workflow_record(&plan).unwrap_err(),
        StoreError::WorkspaceChanged
    );
    assert!(fs::read(dir.store().path()).unwrap().is_empty());
}
