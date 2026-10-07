use super::*;
use crate::composition_records_tests::{TestDir, legacy, source};
use rangoon_workflow::records::{SaveIntent, prepare_revision, workflow_id_from_nonce};
use workflow_records::SavedRevision;

fn fixture() -> (Records, String) {
    let dir = TestDir::new();
    let report = source(&dir);
    let capability = legacy(&dir, &report);
    let mut records = Records::load(&dir.db()).unwrap();
    records.version = 4;
    for seed in [21, 11] {
        let owner = workflow_id_from_nonce(&[seed; 32]);
        let mut parent = None;
        for index in 0..2 {
            let nodes = if index == 0 {
                serde_json::json!([
                    {"id":"duplicate","title":"Old pin","operation":{"kind":"capability","capabilityId":capability.id,"revisionId":"bad"},"inputs":[],"outputs":[]},
                    {"id":"duplicate","title":"Old pin","operation":{"kind":"capability","capabilityId":capability.id,"revisionId":"also-bad"},"inputs":[],"outputs":[]}
                ])
            } else {
                serde_json::json!([])
            };
            let definition = serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":"\nOld workflow\u{2028}","nodes":nodes,"controlEdges":[],"dataEdges":[]});
            let record = prepare_revision(
                &owner,
                parent.as_deref(),
                SaveIntent::Draft,
                &serde_json::to_vec(&definition).unwrap(),
                br#"{"positions":[]}"#,
            )
            .unwrap();
            parent = Some(record.id().to_owned());
            records
                .workflows
                .owners
                .insert(owner.clone(), record.id().into());
            records.workflows.revisions.insert(
                record.id().into(),
                SavedRevision {
                    record,
                    saved_at_ms: index,
                },
            );
        }
    }
    let records =
        Records::load_complete(&composition_backup::canonical_database(&records).unwrap()).unwrap();
    (records, capability.id)
}

#[test]
fn all_history_draft_pins_block_skill_deletion_with_sorted_unique_dependents() {
    let (records, cap) = fixture();
    let (plan, _) = deletion_plan(&records, WorkflowRecordKind::Capability, &cap).unwrap();
    assert_eq!(plan.workflow_dependencies.len(), 2);
    assert!(plan.workflow_dependencies[0].id < plan.workflow_dependencies[1].id);
    assert_eq!(plan.remove.capabilities, 1);
    assert_eq!(plan.remove.workflows, 0);
    assert_eq!(plan.remove.workflow_revisions, 0);
    assert_eq!(plan.kind(), WorkflowRecordKind::Capability);
    assert_eq!(plan.id(), cap);
    assert_eq!(plan.expected_state_id(), records.state_id().unwrap());
    let mut db = composition_backup::canonical_database(&records).unwrap();
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    assert_eq!(
        commit_deletion(&tx, &plan).unwrap_err(),
        StoreError::RecordInUse
    );
    tx.rollback().unwrap();
    assert_eq!(
        Records::load_complete(&db).unwrap().state_id().unwrap(),
        records.state_id().unwrap()
    );
    let wire = serde_json::to_value(plan).unwrap();
    assert_eq!(wire["schemaVersion"], "rangoon.deletion-plan.v2");
    assert_eq!(wire["kind"], "capability");
    assert_eq!(wire.as_object().unwrap().len(), 8);
}

#[test]
fn workflow_deletion_removes_only_selected_history_and_never_downgrades() {
    let (records, cap) = fixture();
    let mut db = composition_backup::canonical_database(&records).unwrap();
    for owner in records.workflows.owners.keys() {
        let current = Records::load_complete(&db).unwrap();
        let (plan, _) = deletion_plan(&current, WorkflowRecordKind::Workflow, owner).unwrap();
        assert_eq!(plan.label, "Old workflow");
        assert_eq!(
            (
                plan.remove.workflows,
                plan.remove.workflow_revisions,
                plan.remove.capabilities
            ),
            (1, 2, 0)
        );
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        let receipt = commit_deletion(&tx, &plan).unwrap();
        assert_eq!(receipt.records.capabilities, 1);
        tx.commit().unwrap();
        let after = Records::load_complete(&db).unwrap();
        assert_eq!(after.version, 4);
        assert!(!after.workflows.owners.contains_key(owner));
        assert_eq!(
            after.owners[&cap].latest_revision_id,
            records.owners[&cap].latest_revision_id
        );
    }
    let current = Records::load_complete(&db).unwrap();
    let (plan, _) = deletion_plan(&current, WorkflowRecordKind::Capability, &cap).unwrap();
    assert!(plan.workflow_dependencies.is_empty());
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    assert_eq!(commit_deletion(&tx, &plan).unwrap().records.capabilities, 0);
    tx.commit().unwrap();
    assert_eq!(Records::load_complete(&db).unwrap().version, 4);
}

#[test]
fn retained_plan_all_fields_and_full_state_are_rechecked() {
    let (records, _) = fixture();
    let owner = records.workflows.owners.keys().next().unwrap();
    let (original, _) = deletion_plan(&records, WorkflowRecordKind::Workflow, owner).unwrap();
    for field in 0..6 {
        let mut plan = original.clone();
        match field {
            0 => plan.schema_version = "wrong",
            1 => plan.label.push('!'),
            2 => plan.remove.workflow_revisions += 1,
            3 => plan.remove.sources += 1,
            4 => plan
                .workflow_dependencies
                .push(workflow_views::summary(&records, owner).unwrap()),
            _ => plan.capability_dependencies.push(
                records
                    .summary(records.owners.keys().next().unwrap())
                    .unwrap(),
            ),
        }
        let mut db = composition_backup::canonical_database(&records).unwrap();
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        assert_eq!(
            commit_deletion(&tx, &plan).unwrap_err(),
            StoreError::WorkspaceChanged
        );
        tx.rollback().unwrap();
        assert_eq!(
            Records::load_complete(&db).unwrap().state_id().unwrap(),
            original.expected_state_id
        );
    }
    let mut db = composition_backup::canonical_database(&records).unwrap();
    db.execute(
        "UPDATE workflow_revisions SET saved_at_ms=saved_at_ms+1",
        [],
    )
    .unwrap();
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    assert_eq!(
        commit_deletion(&tx, &original).unwrap_err(),
        StoreError::WorkspaceChanged
    );
    tx.rollback().unwrap();
}

#[test]
fn dropping_transaction_rolls_back_full_workflow_history_deletion() {
    let (records, _) = fixture();
    let mut db = composition_backup::canonical_database(&records).unwrap();
    let (plan, _) = deletion_plan(
        &records,
        WorkflowRecordKind::Workflow,
        records.workflows.owners.keys().next().unwrap(),
    )
    .unwrap();
    {
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        commit_deletion(&tx, &plan).unwrap();
        assert_eq!(
            Records::load_complete(&tx).unwrap().workflows.owners.len(),
            1
        );
        // Cancellation/failure before commit must leave no partial history.
    }
    assert_eq!(
        Records::load_complete(&db).unwrap().state_id().unwrap(),
        records.state_id().unwrap()
    );
}

#[test]
fn public_source_skill_deletion_preserves_dependencies_stale_checks_and_noncreation() {
    let dir = TestDir::new();
    assert_eq!(
        dir.store()
            .inspect_workflow_deletion(WorkflowRecordKind::Source, "invalid")
            .unwrap_err(),
        StoreError::WorkflowInvalid
    );
    let missing = format!("source:{}", "0".repeat(64));
    assert_eq!(
        dir.store()
            .inspect_workflow_deletion(WorkflowRecordKind::Source, &missing)
            .unwrap_err(),
        StoreError::NotFound
    );
    assert!(!dir.0.exists());
    let report = source(&dir);
    let cap = legacy(&dir, &report);
    let before = fs::read(dir.store().path()).unwrap();
    let source_plan = dir
        .store()
        .inspect_workflow_deletion(WorkflowRecordKind::Source, &report.source.id)
        .unwrap();
    assert_eq!(source_plan.capability_dependencies.len(), 1);
    assert_eq!(fs::read(dir.store().path()).unwrap(), before);
    assert_eq!(
        dir.store()
            .delete_workflow_record(&source_plan)
            .unwrap_err(),
        StoreError::RecordInUse
    );
    let cap_plan = dir
        .store()
        .inspect_workflow_deletion(WorkflowRecordKind::Capability, &cap.id)
        .unwrap();
    let receipt = dir.store().delete_workflow_record(&cap_plan).unwrap();
    assert_eq!(
        (receipt.records.sources, receipt.records.capabilities),
        (1, 0)
    );
    assert_eq!(
        dir.store()
            .delete_workflow_record(&source_plan)
            .unwrap_err(),
        StoreError::WorkspaceChanged
    );
    assert_eq!(
        dir.store().delete_workflow_record(&cap_plan).unwrap_err(),
        StoreError::WorkspaceChanged
    );
    let new_plan = dir
        .store()
        .inspect_workflow_deletion(WorkflowRecordKind::Source, &report.source.id)
        .unwrap();
    assert_eq!(
        dir.store()
            .delete_workflow_record(&new_plan)
            .unwrap()
            .records
            .sources,
        0
    );
    fs::remove_file(dir.store().path()).unwrap();
    assert_eq!(
        dir.store().delete_workflow_record(&new_plan).unwrap_err(),
        StoreError::WorkspaceChanged
    );
    assert!(!dir.store().path().exists());
}

#[test]
fn public_workflow_delete_commits_complete_plan_and_absent_commit_creates_nothing() {
    let (records, _) = fixture();
    let owner = records.workflows.owners.keys().next().unwrap();
    let (plan, _) = deletion_plan(&records, WorkflowRecordKind::Workflow, owner).unwrap();
    let dir = TestDir::new();
    assert_eq!(
        dir.store().delete_workflow_record(&plan).unwrap_err(),
        StoreError::WorkspaceChanged
    );
    assert!(!dir.0.exists());
    fs::create_dir_all(&dir.0).unwrap();
    let db = composition_backup::canonical_database(&records).unwrap();
    db.execute("VACUUM INTO ?1", [dir.store().path().to_str().unwrap()])
        .unwrap();
    let before = fs::read(dir.store().path()).unwrap();
    let inspected = dir
        .store()
        .inspect_workflow_deletion(WorkflowRecordKind::Workflow, owner)
        .unwrap();
    assert_eq!(inspected, plan);
    assert_eq!(fs::read(dir.store().path()).unwrap(), before);
    let deleted = dir.store().delete_workflow_record(&plan).unwrap();
    assert_eq!(
        deleted.records.workflows as usize,
        records.workflows.owners.len() - 1
    );
    assert_eq!(Records::load_complete(&dir.db()).unwrap().version, 4);
}

#[test]
#[ignore = "isolated child killed at transactional deletion checkpoints"]
fn deletion_crash_child() {
    let directory = PathBuf::from(std::env::var_os("RANGOON_RECOVERY_TEST_WORKSPACE").unwrap());
    let owner = std::env::var("RANGOON_RECOVERY_TEST_DELETE_ID").unwrap();
    let mut db = Connection::open(directory.join("workspace.sqlite3")).unwrap();
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let current = Records::load_complete(&tx).unwrap();
    let (plan, _) = deletion_plan(&current, WorkflowRecordKind::Workflow, &owner).unwrap();
    commit_deletion(&tx, &plan).unwrap();
    panic!("child must stop at its selected pre-commit checkpoint");
}

#[test]
fn interrupted_workflow_deletion_recovers_complete_history_and_heads() {
    let (records, _) = fixture();
    let owner = records.workflows.owners.keys().next().unwrap();
    for phase in [
        "before_workflow_delete",
        "after_workflow_history_delete",
        "after_workflow_delete_rows",
    ] {
        let dir = TestDir::new();
        fs::create_dir_all(&dir.0).unwrap();
        let db = composition_backup::canonical_database(&records).unwrap();
        db.execute("VACUUM INTO ?1", [dir.store().path().to_str().unwrap()])
            .unwrap();
        crate::composition_recovery_tests::fault_child_for(
            &dir,
            &dir.0.join("unused-backup"),
            phase,
            "crash",
            owner,
            "workflow_deletion::tests::deletion_crash_child",
        );
        let reopened = dir.db();
        assert_eq!(
            Records::load_complete(&reopened)
                .unwrap()
                .state_id()
                .unwrap(),
            records.state_id().unwrap()
        );
        let integrity: String = reopened
            .pragma_query_value(None, "integrity_check", |r| r.get(0))
            .unwrap();
        assert_eq!(integrity, "ok");
    }
}
