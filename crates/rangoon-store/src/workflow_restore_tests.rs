use super::*;
use crate::composition_records_tests::{
    TestDir, fixture_request, resolved_source, retain_outputs, schema3, source,
};
use rangoon_workflow::records::{SaveIntent, prepare_revision, workflow_id_from_nonce};
use workflow_records::SavedRevision;

fn incoming() -> WorkspaceBackup {
    let dir = TestDir::new();
    let report = source(&dir);
    let db = dir.db();
    schema3(&db);
    retain_outputs(
        &db,
        &fixture_request(),
        &resolved_source(&report),
        &[],
        &[0, 1],
    );
    let mut records = Records::load(&db).unwrap();
    records.version = 4;
    let owner = workflow_id_from_nonce(&[61; 32]);
    let mut parent = None;
    for time in [8, 2] {
        let definition = serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":format!("Restore {time} {}", "x".repeat(16_384)),"nodes":[],"controlEdges":[],"dataEdges":[]});
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
                saved_at_ms: time,
            },
        );
    }
    WorkspaceBackup::decode(&workflow_backup::encode(&records).unwrap()).unwrap()
}

#[test]
fn complete_restore_migrates_all_records_and_exact_retry_plan_is_noop() {
    let archive = incoming();
    let mut db = composition_backup::memory_database().unwrap();
    let retained = restore_plan(&Records::empty(), &archive).unwrap().0;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let receipt = commit_restore(&tx, &archive, &retained).unwrap();
    assert_eq!(
        (
            receipt.records.sources,
            receipt.records.capabilities,
            receipt.records.recipes,
            receipt.records.applications,
            receipt.records.workflows,
            receipt.records.workflow_revisions
        ),
        (1, 2, 1, 1, 1, 2)
    );
    tx.commit().unwrap();
    let actual = Records::load_complete(&db).unwrap();
    assert_eq!(actual.version, 4);
    assert_eq!(
        actual.state_id().unwrap(),
        archive.records.state_id().unwrap()
    );
    let before = workflow_backup::encode(&actual).unwrap();
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    assert_eq!(
        commit_restore(&tx, &archive, &retained).unwrap_err(),
        StoreError::WorkspaceChanged
    );
    tx.rollback().unwrap();
    let no_op = restore_plan(&actual, &archive).unwrap().0;
    assert_eq!(no_op.add, WorkflowRecoveryCounts::default());
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    assert_eq!(
        commit_restore(&tx, &archive, &no_op).unwrap().state_id,
        actual.state_id().unwrap()
    );
    tx.commit().unwrap();
    assert_eq!(
        workflow_backup::encode(&Records::load_complete(&db).unwrap()).unwrap(),
        before
    );
}

#[test]
fn every_retained_plan_field_is_bound_before_schema_or_rows_change() {
    let archive = incoming();
    let original = restore_plan(&Records::empty(), &archive).unwrap().0;
    for field in 0..8 {
        let mut plan = original.clone();
        match field {
            0 => plan.schema_version = "wrong",
            1 => plan.backup_id.push('0'),
            2 => plan.expected_state_id.push('0'),
            3 => plan.byte_length += 1,
            4 => plan.add.workflow_revisions += 1,
            5 => plan.kept_sources += 1,
            6 => plan.kept_capabilities += 1,
            _ => plan.kept_workflows += 1,
        }
        let mut db = composition_backup::memory_database().unwrap();
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        assert_eq!(
            commit_restore(&tx, &archive, &plan).unwrap_err(),
            StoreError::WorkspaceChanged
        );
        tx.rollback().unwrap();
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            workflow_mutations::read_records(&db)
                .unwrap()
                .state_id()
                .unwrap(),
            Records::empty().state_id().unwrap()
        );
    }
}

#[test]
fn public_noop_v3_restore_preserves_absence_empty_files_and_original_schema() {
    let mut records = Records::empty();
    records.version = 4;
    let empty = WorkspaceBackup::decode(&workflow_backup::encode(&records).unwrap()).unwrap();
    let dir = TestDir::new();
    let retained = dir.store().prepare_workflow_restore(&empty).unwrap();
    let receipt = dir
        .store()
        .restore_workflow_backup(&empty, &retained)
        .unwrap();
    assert_eq!(receipt.database_bytes, 0);
    assert!(!dir.0.exists());
    for version in [0, 1, 3] {
        if version == 0 {
            fs::create_dir_all(&dir.0).unwrap();
            fs::write(dir.store().path(), []).unwrap();
        }
        if version == 1 {
            source(&dir);
        }
        if version == 3 {
            schema3(&dir.db());
        }
        let before = fs::read(dir.store().path()).unwrap();
        let retained = dir.store().prepare_workflow_restore(&empty).unwrap();
        dir.store()
            .restore_workflow_backup(&empty, &retained)
            .unwrap();
        assert_eq!(fs::read(dir.store().path()).unwrap(), before);
        assert_eq!(
            dir.db()
                .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            version
        );
    }
}

#[test]
fn public_v3_and_legacy_restore_commit_after_noncreating_plan_checks() {
    let archive = incoming();
    let dir = TestDir::new();
    let retained = dir.store().prepare_workflow_restore(&archive).unwrap();
    assert!(!dir.0.exists());
    let mut forged = retained.clone();
    forged.kept_sources += 1;
    assert_eq!(
        dir.store()
            .restore_workflow_backup(&archive, &forged)
            .unwrap_err(),
        StoreError::WorkspaceChanged
    );
    assert!(!dir.0.exists());
    let restored = dir
        .store()
        .restore_workflow_backup(&archive, &retained)
        .unwrap();
    assert_eq!(restored.records.workflows, 1);
    assert_eq!(restored.records.workflow_revisions, 2);
    assert_eq!(Records::load_complete(&dir.db()).unwrap().version, 4);
    let dir = TestDir::new();
    let base = base_projection(&archive.records);
    let legacy = WorkspaceBackup::decode(&composition_backup::encode(&base).unwrap()).unwrap();
    let retained = dir.store().prepare_workflow_restore(&legacy).unwrap();
    let result = dir
        .store()
        .restore_workflow_backup(&legacy, &retained)
        .unwrap();
    assert_eq!(
        (
            result.records.capabilities,
            result.records.recipes,
            result.records.workflows
        ),
        (2, 1, 0)
    );
    assert_eq!(Records::load(&dir.db()).unwrap().version, 3);
    let before = fs::read(dir.store().path()).unwrap();
    assert_eq!(
        dir.store()
            .restore_workflow_backup(&legacy, &retained)
            .unwrap_err(),
        StoreError::WorkspaceChanged
    );
    assert_eq!(fs::read(dir.store().path()).unwrap(), before);
}

#[test]
fn sqlite_full_rolls_back_schema_migration_and_all_record_additions() {
    let archive = incoming();
    let dir = TestDir::new();
    source(&dir);
    let mut db = dir.db();
    let current = Records::load(&db).unwrap();
    let retained = restore_plan(&current, &archive).unwrap().0;
    let before = fs::read(dir.store().path()).unwrap();
    let pages: u32 = db
        .pragma_query_value(None, "page_count", |r| r.get(0))
        .unwrap();
    db.pragma_update(None, "max_page_count", pages).unwrap();
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    assert_eq!(
        commit_restore(&tx, &archive, &retained).unwrap_err(),
        StoreError::Full
    );
    tx.rollback().unwrap();
    drop(db);
    assert_eq!(
        Records::load(&dir.db()).unwrap().state_id().unwrap(),
        current.state_id().unwrap()
    );
    assert_eq!(fs::read(dir.store().path()).unwrap(), before);
}

#[test]
#[ignore = "isolated child killed during complete workflow restore"]
fn restore_crash_child() {
    let directory = PathBuf::from(std::env::var_os("RANGOON_RECOVERY_TEST_WORKSPACE").unwrap());
    let bytes = fs::read(std::env::var_os("RANGOON_RECOVERY_TEST_BACKUP").unwrap()).unwrap();
    let archive = WorkspaceBackup::decode(&bytes).unwrap();
    let mut db = Connection::open(directory.join("workspace.sqlite3")).unwrap();
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let current = workflow_mutations::read_records(&tx).unwrap();
    let retained = restore_plan(&current, &archive).unwrap().0;
    if std::env::var("RANGOON_RECOVERY_TEST_MODE").as_deref() == Ok("full") {
        let outcome = commit_restore(&tx, &archive, &retained);
        assert_eq!(outcome.unwrap_err(), StoreError::Full);
        drop(tx);
        return;
    }
    commit_restore(&tx, &archive, &retained).unwrap();
    panic!("child must stop at its selected pre-commit checkpoint");
}

#[test]
fn interrupted_restore_recovers_schema_and_records_at_each_write_phase() {
    let archive = incoming();
    for initialized in [false, true] {
        for phase in [
            "before_workflow_restore_migration",
            "after_workflow_restore_migration",
            "after_workflow_restore_base",
            "after_workflow_restore_rows",
        ] {
            let dir = TestDir::new();
            if initialized {
                source(&dir);
            } else {
                fs::create_dir_all(&dir.0).unwrap();
                fs::write(dir.store().path(), []).unwrap();
            }
            let current = workflow_mutations::read_records(&dir.db()).unwrap();
            let original_version: i64 = dir
                .db()
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap();
            let backup = dir.0.join("restore.rangoon");
            fs::write(&backup, workflow_backup::encode(&archive.records).unwrap()).unwrap();
            crate::composition_recovery_tests::fault_child_for(
                &dir,
                &backup,
                phase,
                "crash",
                "unused",
                "workflow_recovery::restore_tests::restore_crash_child",
            );
            let db = dir.db();
            assert_eq!(
                workflow_mutations::read_records(&db)
                    .unwrap()
                    .state_id()
                    .unwrap(),
                current.state_id().unwrap()
            );
            assert_eq!(
                db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                    .unwrap(),
                original_version
            );
            assert_eq!(
                db.pragma_query_value(None, "integrity_check", |r| r.get::<_, String>(0))
                    .unwrap(),
                "ok"
            );
        }
    }
}

#[test]
fn public_v1_restore_preserves_source_bytes_and_keeps_schema_one() {
    let input = TestDir::new();
    let report = source(&input);
    let archive = WorkspaceBackup::decode(&input.store().export_backup().unwrap()).unwrap();
    let destination = TestDir::new();
    let retained = destination
        .store()
        .prepare_workflow_restore(&archive)
        .unwrap();
    let receipt = destination
        .store()
        .restore_workflow_backup(&archive, &retained)
        .unwrap();
    assert_eq!(receipt.records.sources, 1);
    assert_eq!(receipt.records.capabilities, 0);
    let actual = Records::load(&destination.db()).unwrap();
    assert_eq!(actual.version, 1);
    assert_eq!(
        actual.sources[&report.source.id].report.source,
        report.source
    );
    let before = fs::read(destination.store().path()).unwrap();
    let no_op = destination
        .store()
        .prepare_workflow_restore(&archive)
        .unwrap();
    destination
        .store()
        .restore_workflow_backup(&archive, &no_op)
        .unwrap();
    assert_eq!(fs::read(destination.store().path()).unwrap(), before);
}

#[test]
fn sqlite_full_after_migration_or_partial_insert_rolls_back_every_record() {
    let archive = incoming();
    for initialized in [false, true] {
        for phase in [
            "after_workflow_restore_migration",
            "after_workflow_restore_base",
        ] {
            let dir = TestDir::new();
            if initialized {
                source(&dir);
            } else {
                fs::create_dir_all(&dir.0).unwrap();
                fs::write(dir.store().path(), []).unwrap();
            }
            let current = workflow_mutations::read_records(&dir.db()).unwrap();
            let original_version: i64 = dir
                .db()
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap();
            let backup = dir.0.join("restore.rangoon");
            fs::write(&backup, workflow_backup::encode(&archive.records).unwrap()).unwrap();
            crate::composition_recovery_tests::fault_child_for(
                &dir,
                &backup,
                phase,
                "full",
                "unused",
                "workflow_recovery::restore_tests::restore_crash_child",
            );
            let db = dir.db();
            assert_eq!(
                workflow_mutations::read_records(&db)
                    .unwrap()
                    .state_id()
                    .unwrap(),
                current.state_id().unwrap()
            );
            assert_eq!(
                db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                    .unwrap(),
                original_version
            );
            assert_eq!(
                db.pragma_query_value(None, "integrity_check", |r| r.get::<_, String>(0))
                    .unwrap(),
                "ok"
            );
        }
    }
}
