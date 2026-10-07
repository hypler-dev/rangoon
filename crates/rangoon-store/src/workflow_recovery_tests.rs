use super::*;
use crate::composition_records_tests::{TestDir, legacy, source};
use rangoon_workflow::records::{SaveIntent, prepare_revision, workflow_id_from_nonce};
use workflow_records::SavedRevision;

fn workflow(base: &mut Records, count: usize) {
    base.version = 4;
    let owner = workflow_id_from_nonce(&[31; 32]);
    let mut parent = None;
    for index in 0..count {
        let definition = serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":format!("Draft {index}"),"nodes":[],"controlEdges":[],"dataEdges":[]});
        let record = prepare_revision(
            &owner,
            parent.as_deref(),
            SaveIntent::Draft,
            &serde_json::to_vec(&definition).unwrap(),
            br#"{"positions":[]}"#,
        )
        .unwrap();
        parent = Some(record.id().to_owned());
        base.workflows
            .owners
            .insert(owner.clone(), record.id().into());
        base.workflows.revisions.insert(
            record.id().into(),
            SavedRevision {
                record,
                saved_at_ms: index as i64,
            },
        );
    }
}

fn backup(records: &Records) -> WorkspaceBackup {
    WorkspaceBackup::decode(&workflow_backup::encode(records).unwrap()).unwrap()
}

#[test]
fn absent_restore_previews_and_exports_do_not_create_destination() {
    let dir = TestDir::new();
    let mut records = Records::empty();
    workflow(&mut records, 2);
    let archive = backup(&records);
    let plan = dir.store().prepare_workflow_restore(&archive).unwrap();
    assert_eq!((plan.add.workflows, plan.add.workflow_revisions), (1, 2));
    assert_eq!(plan.backup_id(), archive.id());
    assert_eq!(
        plan.expected_state_id(),
        Records::empty().state_id().unwrap()
    );
    assert_eq!(
        dir.store().prepare_workflow_restore(&archive).unwrap(),
        plan
    );
    let exported = dir.store().export_workflow_backup().unwrap();
    assert_eq!(
        exported,
        composition_backup::encode(&Records::empty()).unwrap()
    );
    assert!(!dir.0.exists());
    let wire = serde_json::to_value(&plan).unwrap();
    assert_eq!(wire["schemaVersion"], "rangoon.restore-plan.v2");
    assert_eq!(wire["add"]["workflowRevisions"], 2);
    assert_eq!(wire["add"].as_object().unwrap().len(), 9);
}

#[test]
fn empty_v3_preview_preserves_absent_and_existing_schemas_without_writes() {
    let mut empty = Records::empty();
    empty.version = 4;
    let archive = backup(&empty);
    let dir = TestDir::new();
    let (plan, candidate) = restore_plan(&Records::empty(), &archive).unwrap();
    assert_eq!(plan.add, WorkflowRecoveryCounts::default());
    assert_eq!(candidate.version, 1);
    assert_eq!(
        dir.store().prepare_workflow_restore(&archive).unwrap(),
        plan
    );
    assert!(!dir.0.exists());
    source(&dir);
    for version in [1, 3] {
        if version == 3 {
            crate::composition_records_tests::schema3(&dir.db());
        }
        let before = fs::read(dir.store().path()).unwrap();
        let current = Records::load(&dir.db()).unwrap();
        let (expected, candidate) = restore_plan(&current, &archive).unwrap();
        assert_eq!(candidate.version, version);
        assert_eq!(
            dir.store().prepare_workflow_restore(&archive).unwrap(),
            expected
        );
        assert_eq!(fs::read(dir.store().path()).unwrap(), before);
    }
}

#[test]
fn complete_loader_state_and_legacy_export_fences_preserve_workflow_rows() {
    let dir = TestDir::new();
    let source = source(&dir);
    legacy(&dir, &source);
    let mut records = Records::load(&dir.db()).unwrap();
    workflow(&mut records, 2);
    let db = composition_backup::canonical_database(&records).unwrap();
    let loaded = Records::load_complete(&db).unwrap();
    assert_eq!(loaded.version, 4);
    assert_eq!(loaded.workflows.revisions.len(), 2);
    assert_eq!(loaded.state_id().unwrap(), records.state_id().unwrap());
    assert_eq!(
        Records::load(&db).unwrap_err(),
        StoreError::UnsupportedSchema
    );
    assert_eq!(
        composition_backup::encode(&loaded).unwrap_err(),
        StoreError::UnsupportedSchema
    );
    db.execute("UPDATE workflow_revisions SET saved_at_ms=-1", [])
        .unwrap();
    assert_eq!(
        Records::load_complete(&db).unwrap_err(),
        StoreError::Corrupt
    );
}

#[test]
fn restore_plan_keeps_full_local_history_and_rejects_newer_incoming_history() {
    let mut current = Records::empty();
    workflow(&mut current, 3);
    let current =
        Records::load_complete(&composition_backup::canonical_database(&current).unwrap()).unwrap();
    let mut older = Records::empty();
    workflow(&mut older, 2);
    let (plan, candidate) = restore_plan(&current, &backup(&older)).unwrap();
    assert_eq!(plan.kept_workflows, 1);
    assert_eq!(plan.add, WorkflowRecoveryCounts::default());
    assert_eq!(candidate.state_id().unwrap(), current.state_id().unwrap());
    let mut newer = Records::empty();
    workflow(&mut newer, 4);
    assert_eq!(
        restore_plan(&current, &backup(&newer)).unwrap_err(),
        StoreError::BackupInvalid
    );
}

#[test]
fn source_capability_and_workflow_preview_reports_complete_additions() {
    let dir = TestDir::new();
    let source = source(&dir);
    legacy(&dir, &source);
    let mut records = Records::load(&dir.db()).unwrap();
    workflow(&mut records, 2);
    let archive = backup(&records);
    let target = TestDir::new();
    let plan = target.store().prepare_workflow_restore(&archive).unwrap();
    assert_eq!(
        (
            plan.add.sources,
            plan.add.capabilities,
            plan.add.revisions,
            plan.add.workflows,
            plan.add.workflow_revisions
        ),
        (1, 1, 1, 1, 2)
    );
    let before = fs::read(dir.store().path()).unwrap();
    let existing = dir.store().prepare_workflow_restore(&archive).unwrap();
    assert_eq!((existing.kept_sources, existing.kept_capabilities), (1, 1));
    assert_eq!(
        (
            existing.add.sources,
            existing.add.capabilities,
            existing.add.workflows
        ),
        (0, 0, 1)
    );
    assert_eq!(fs::read(dir.store().path()).unwrap(), before);
    assert!(!target.0.exists());
}

#[test]
fn final_union_refuses_validated_pin_missing_from_kept_local_capability_history() {
    let dir = TestDir::new();
    let source = source(&dir);
    let first = legacy(&dir, &source);
    let current = Records::load(&dir.db()).unwrap();
    let old_state = current.state_id().unwrap();
    dir.store()
        .revise_capability(
            &first.id,
            &first.latest_revision_id,
            "Updated",
            "Updated instructions",
        )
        .unwrap();
    let mut incoming = Records::load(&dir.db()).unwrap();
    incoming.version = 4;
    let revision = &incoming.owners[&first.id].latest_revision_id;
    let definition = serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":"Pinned",
        "nodes":[
            {"id":"start","title":"Start","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"text","dataType":"text"}]},
            {"id":"work","title":"Work","operation":{"kind":"capability","capabilityId":first.id,"revisionId":revision},"inputs":[{"name":"text","dataType":"text"}],"outputs":[{"name":"text","dataType":"text"}]},
            {"id":"finish","title":"Finish","operation":{"kind":"output"},"inputs":[{"name":"text","dataType":"text"}],"outputs":[]}],
        "controlEdges":[{"fromNode":"start","outlet":"next","toNode":"work"},{"fromNode":"work","outlet":"next","toNode":"finish"}],
        "dataEdges":[{"fromNode":"start","fromPort":"text","toNode":"work","toPort":"text"},{"fromNode":"work","fromPort":"text","toNode":"finish","toPort":"text"}]});
    let record = prepare_revision(
        &workflow_id_from_nonce(&[99; 32]),
        None,
        SaveIntent::Validated,
        &serde_json::to_vec(&definition).unwrap(),
        br#"{"positions":[]}"#,
    )
    .unwrap();
    incoming
        .workflows
        .owners
        .insert(record.workflow_id().into(), record.id().into());
    incoming.workflows.revisions.insert(
        record.id().into(),
        SavedRevision {
            record,
            saved_at_ms: 0,
        },
    );
    let archive = backup(&incoming);
    assert_eq!(
        restore_plan(&current, &archive).unwrap_err(),
        StoreError::WorkflowDependencyMissing
    );
    assert_eq!(current.state_id().unwrap(), old_state);
    let (plan, result) = restore_plan(&Records::empty(), &archive).unwrap();
    assert_eq!(
        (
            plan.add.capabilities,
            plan.add.revisions,
            plan.add.workflows
        ),
        (1, 2, 1)
    );
    assert_eq!(result.state_id().unwrap(), incoming.state_id().unwrap());
}

#[test]
fn existing_empty_files_remain_byte_identical_after_read_only_operations() {
    let archive = backup(&Records::empty());
    for initialized in [false, true] {
        let dir = TestDir::new();
        fs::create_dir_all(&dir.0).unwrap();
        fs::write(dir.store().path(), []).unwrap();
        if initialized {
            let db = dir.db();
            db.execute_batch("PRAGMA page_size=4096; VACUUM;").unwrap();
        }
        let before = fs::read(dir.store().path()).unwrap();
        dir.store().prepare_workflow_restore(&archive).unwrap();
        dir.store().export_workflow_backup().unwrap();
        assert_eq!(fs::read(dir.store().path()).unwrap(), before);
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }
}

#[test]
fn legacy_archives_preview_against_absent_and_existing_workspaces() {
    let source_dir = TestDir::new();
    let report = source(&source_dir);
    let v1 = source_dir.store().export_backup().unwrap();
    legacy(&source_dir, &report);
    let v2 = source_dir.store().export_workflow_backup().unwrap();
    for bytes in [v1, v2] {
        let archive = WorkspaceBackup::decode(&bytes).unwrap();
        let destination = TestDir::new();
        let expected = restore_plan(&Records::empty(), &archive).unwrap().0;
        assert_eq!(
            destination
                .store()
                .prepare_workflow_restore(&archive)
                .unwrap(),
            expected
        );
        assert!(!destination.0.exists());
        source(&destination);
        let before = fs::read(destination.store().path()).unwrap();
        let current = Records::load(&destination.db()).unwrap();
        let expected = restore_plan(&current, &archive).unwrap().0;
        assert_eq!(
            destination
                .store()
                .prepare_workflow_restore(&archive)
                .unwrap(),
            expected
        );
        assert_eq!(fs::read(destination.store().path()).unwrap(), before);
    }
}
