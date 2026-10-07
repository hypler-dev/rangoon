use super::*;
use crate::composition_records_tests::{TestDir, legacy, source};
use rangoon_workflow::records::{prepare_revision, workflow_id_from_nonce};

fn draft(seed: u8, parent: Option<&str>, title: &str) -> WorkflowRevision {
    let definition = serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":title,"nodes":[],"controlEdges":[],"dataEdges":[]});
    prepare_revision(
        &workflow_id_from_nonce(&[seed; 32]),
        parent,
        SaveIntent::Draft,
        &serde_json::to_vec(&definition).unwrap(),
        br#"{"positions":[]}"#,
    )
    .unwrap()
}
fn save(
    db: &mut Connection,
    candidate: &WorkflowRevision,
    state: &str,
) -> Result<WorkflowSaveReceipt, StoreError> {
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let result = commit_save(&tx, candidate, state)?;
    tx.commit()?;
    Ok(result)
}
fn state(db: &Connection) -> String {
    workflow_mutations::read_records(db)
        .unwrap()
        .state_id()
        .unwrap()
}

#[test]
fn incomplete_drafts_append_and_exact_retry_preserves_bytes_time_and_history() {
    let mut db = composition_backup::memory_database().unwrap();
    let root = draft(1, None, "Unfinished 🦀");
    let empty = state(&db);
    let first = save(&mut db, &root, &empty).unwrap();
    assert!(!first.already_saved);
    assert!(!first.workflow.head.structurally_valid);
    assert_eq!(
        first.workflow.revision.serialized_bytes().unwrap(),
        root.serialized_bytes().unwrap()
    );
    let before = workflow_backup::encode(&Records::load_complete(&db).unwrap()).unwrap();
    let retry = save(&mut db, &root, &empty).unwrap();
    assert!(retry.already_saved);
    assert_eq!(retry.workflow, first.workflow);
    assert_eq!(
        workflow_backup::encode(&Records::load_complete(&db).unwrap()).unwrap(),
        before
    );
    let child = draft(1, Some(root.id()), "Next draft");
    let next = save(&mut db, &child, &first.state_id).unwrap();
    assert_eq!(next.workflow.history.len(), 2);
    assert_eq!(next.workflow.history[0].id, root.id());
    assert_eq!(
        save(&mut db, &root, &next.state_id).unwrap_err(),
        StoreError::WorkflowConflict
    );
    assert_eq!(
        save(&mut db, &root, &empty).unwrap_err(),
        StoreError::WorkspaceChanged
    );
    assert_eq!(state(&db), next.state_id);
    assert_eq!(
        save(&mut db, &child, "bad").unwrap_err(),
        StoreError::WorkflowInvalid
    );
}

#[test]
fn stale_conflicts_new_owner_shape_and_deleted_owner_replay_do_not_write() {
    let mut db = composition_backup::memory_database().unwrap();
    let root = draft(2, None, "First");
    let first = save(
        &mut db,
        &root,
        &state(&composition_backup::memory_database().unwrap()),
    )
    .unwrap();
    let competing = draft(2, None, "Competing root");
    assert_eq!(
        save(&mut db, &competing, &first.state_id).unwrap_err(),
        StoreError::WorkflowConflict
    );
    let wrong_owner = draft(3, Some(root.id()), "Cross owner");
    assert_eq!(
        save(&mut db, &wrong_owner, &first.state_id).unwrap_err(),
        StoreError::WorkflowInvalid
    );
    db.execute("DELETE FROM workflow_revisions", []).unwrap();
    db.execute("DELETE FROM workflows", []).unwrap();
    let deleted = state(&db);
    assert_eq!(
        save(&mut db, &root, &first.state_id).unwrap_err(),
        StoreError::WorkspaceChanged
    );
    assert_eq!(state(&db), deleted);
}

fn populated(owners: u8, history: usize) -> Records {
    let mut records = Records::empty();
    records.version = 4;
    for seed in 0..owners {
        let mut parent = None;
        for index in 0..history {
            let record = draft(seed, parent.as_deref(), &format!("History {index}"));
            parent = Some(record.id().to_owned());
            records
                .workflows
                .owners
                .insert(record.workflow_id().into(), record.id().into());
            records.workflows.revisions.insert(
                record.id().into(),
                SavedRevision {
                    record,
                    saved_at_ms: index as i64,
                },
            );
        }
    }
    Records::load_complete(&composition_backup::canonical_database(&records).unwrap()).unwrap()
}

#[test]
fn workflow_quotas_allow_exact_boundaries_and_retries_but_reject_growth() {
    let records = populated(128, 1);
    assert_eq!(
        prepare(&records, &draft(128, None, "Overflow")).unwrap_err(),
        StoreError::WorkflowFull
    );
    let owner = workflow_id_from_nonce(&[0; 32]);
    let head = &records.workflows.owners[&owner];
    assert!(prepare(&records, &draft(0, Some(head), "Existing append")).is_ok());
    let records = populated(1, 31);
    let head = &records.workflows.owners[&owner];
    let last = draft(0, Some(head), "Thirty two");
    let (_, full) = prepare(&records, &last).unwrap();
    assert_eq!(full.workflows.history(&owner).unwrap().len(), 32);
    assert_eq!(
        prepare(&full, &draft(0, Some(last.id()), "Thirty three")).unwrap_err(),
        StoreError::WorkflowFull
    );
    assert!(prepare(&full, &last).unwrap().0.already_saved);
    let records = populated(128, 8);
    let head = &records.workflows.owners[&owner];
    assert_eq!(
        prepare(&records, &draft(0, Some(head), "Total overflow")).unwrap_err(),
        StoreError::WorkflowFull
    );
    assert!(
        prepare(&records, &records.workflows.revisions[head].record)
            .unwrap()
            .0
            .already_saved
    );
}

fn pinned(cap: &str, revision: &str, intent: SaveIntent) -> WorkflowRevision {
    let definition = serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":"Pinned",
        "nodes":[
            {"id":"start","title":"Start","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"text","dataType":"text"}]},
            {"id":"work","title":"Work","operation":{"kind":"capability","capabilityId":cap,"revisionId":revision},"inputs":[{"name":"text","dataType":"text"}],"outputs":[{"name":"text","dataType":"text"}]},
            {"id":"finish","title":"Finish","operation":{"kind":"output"},"inputs":[{"name":"text","dataType":"text"}],"outputs":[]}],
        "controlEdges":[{"fromNode":"start","outlet":"next","toNode":"work"},{"fromNode":"work","outlet":"next","toNode":"finish"}],
        "dataEdges":[{"fromNode":"start","fromPort":"text","toNode":"work","toPort":"text"},{"fromNode":"work","fromPort":"text","toNode":"finish","toPort":"text"}]});
    prepare_revision(
        &workflow_id_from_nonce(&[88; 32]),
        None,
        intent,
        &serde_json::to_vec(&definition).unwrap(),
        br#"{"positions":[]}"#,
    )
    .unwrap()
}

#[test]
fn validated_intent_requires_exact_pins_but_drafts_keep_unresolved_references() {
    let dir = TestDir::new();
    let report = source(&dir);
    let cap = legacy(&dir, &report);
    let current = Records::load(&dir.db()).unwrap();
    let absent = format!("revision:{}", "0".repeat(64));
    assert_eq!(
        prepare(&current, &pinned(&cap.id, &absent, SaveIntent::Validated)).unwrap_err(),
        StoreError::WorkflowDependencyMissing
    );
    let (plan, _) = prepare(&current, &pinned(&cap.id, &absent, SaveIntent::Draft)).unwrap();
    assert_eq!(plan.unresolved_references, 1);
    let candidate = pinned(&cap.id, &cap.latest_revision_id, SaveIntent::Validated);
    let mut db = dir.db();
    let saved = save(&mut db, &candidate, &current.state_id().unwrap()).unwrap();
    assert!(saved.workflow.head.structurally_valid);
    assert_eq!(saved.workflow.head.unresolved_references, 0);
    assert_eq!(saved.workflow.revision.intent(), SaveIntent::Validated);
    let wire = serde_json::to_value(saved.workflow.revision.inspection().report()).unwrap();
    assert_eq!(wire["authority"], "none");
    assert_eq!(wire["executionStatus"], "unavailable");
}

#[test]
fn public_inspection_is_noncreating_and_save_reopens_complete_draft() {
    let dir = TestDir::new();
    let candidate = draft(4, None, "Draft");
    let plan = dir.store().inspect_workflow_save(&candidate).unwrap();
    assert_eq!(plan.expected_state_id, Records::empty().state_id().unwrap());
    assert_eq!(plan.expected_head_id, None);
    assert!(!plan.already_saved);
    assert_eq!(
        serde_json::to_value(&plan)
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        7
    );
    assert!(!dir.0.exists());
    source(&dir);
    let before = fs::read(dir.store().path()).unwrap();
    let plan = dir.store().inspect_workflow_save(&candidate).unwrap();
    assert_eq!(fs::read(dir.store().path()).unwrap(), before);
    let saved = dir
        .store()
        .save_workflow(&candidate, &plan.expected_state_id)
        .unwrap();
    assert!(!saved.already_saved);
    assert_eq!(
        dir.store()
            .open_workflow(candidate.workflow_id(), None)
            .unwrap(),
        saved.workflow
    );
}

#[test]
#[ignore = "isolated child for workflow save crash and capacity failures"]
fn save_fault_child() {
    let directory = PathBuf::from(std::env::var_os("RANGOON_RECOVERY_TEST_WORKSPACE").unwrap());
    let bytes = fs::read(std::env::var_os("RANGOON_RECOVERY_TEST_BACKUP").unwrap()).unwrap();
    let candidate = decode_revision(&bytes).unwrap();
    let mut db = Connection::open(directory.join("workspace.sqlite3")).unwrap();
    let expected = state(&db);
    let result = save(&mut db, &candidate, &expected);
    assert_eq!(
        std::env::var("RANGOON_RECOVERY_TEST_MODE").as_deref(),
        Ok("full")
    );
    assert_eq!(result.unwrap_err(), StoreError::Full);
}

#[test]
fn interrupted_and_full_save_preserve_original_schema_heads_and_histories() {
    for existing in [false, true] {
        for (mode, phase) in [
            ("crash", "before_workflow_save_migration"),
            ("crash", "after_workflow_save_migration"),
            ("crash", "after_workflow_save_revision"),
            ("crash", "after_workflow_save_head"),
            ("full", "after_workflow_save_migration"),
        ] {
            let dir = TestDir::new();
            fs::create_dir_all(&dir.0).unwrap();
            fs::write(dir.store().path(), []).unwrap();
            let root = draft(7, None, "Original");
            let candidate = if existing {
                let mut db = dir.db();
                let expected = state(&db);
                save(&mut db, &root, &expected).unwrap();
                draft(7, Some(root.id()), &"x".repeat(16_384))
            } else {
                draft(7, None, &"x".repeat(16_384))
            };
            let before = state(&dir.db());
            let version: i64 = dir
                .db()
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap();
            let path = dir.0.join("candidate.json");
            fs::write(&path, candidate.serialized_bytes().unwrap()).unwrap();
            crate::composition_recovery_tests::fault_child_for(
                &dir,
                &path,
                phase,
                mode,
                "unused",
                "workflow_saves::tests::save_fault_child",
            );
            let db = dir.db();
            assert_eq!(state(&db), before);
            assert_eq!(
                db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                    .unwrap(),
                version
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
fn successful_migration_restart_and_retry_preserve_layout_and_all_base_records() {
    let definition = serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":"位置 🦀",
        "nodes":[{"id":"input","title":"Input","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"text","dataType":"text"}]}],"controlEdges":[],"dataEdges":[]});
    let candidate = prepare_revision(
        &workflow_id_from_nonce(&[99; 32]),
        None,
        SaveIntent::Draft,
        &serde_json::to_vec(&definition).unwrap(),
        br#"{"positions":[{"nodeIndex":0,"x":-400,"y":200}]}"#,
    )
    .unwrap();
    for version in [1, 2, 3] {
        let dir = TestDir::new();
        let report = source(&dir);
        if version >= 2 {
            legacy(&dir, &report);
        }
        if version == 3 {
            let db = dir.db();
            crate::composition_records_tests::schema3(&db);
            crate::composition_records_tests::retain_outputs(
                &db,
                &crate::composition_records_tests::fixture_request(),
                &crate::composition_records_tests::resolved_source(&report),
                &[],
                &[0, 1],
            );
        }
        let current = Records::load(&dir.db()).unwrap();
        let base = composition_backup::encode(&current).unwrap();
        let saved = save(&mut dir.db(), &candidate, &current.state_id().unwrap()).unwrap();
        let reopened = Records::load_complete(&dir.db()).unwrap();
        assert_eq!(reopened.version, 4);
        assert_eq!(
            composition_backup::encode(&workflow_recovery::base_projection(&reopened)).unwrap(),
            base
        );
        let detail = workflow_views::detail(&reopened, candidate.workflow_id(), None).unwrap();
        assert_eq!(
            detail.revision.serialized_bytes().unwrap(),
            candidate.serialized_bytes().unwrap()
        );
        assert_eq!(detail.revision.layout().positions()[0].x, -400);
        assert_eq!(detail.revision.inspection().definition().title, "位置 🦀");
        let before = fs::read(dir.store().path()).unwrap();
        let retry = save(&mut dir.db(), &candidate, &current.state_id().unwrap()).unwrap();
        assert!(retry.already_saved);
        assert_eq!(retry.workflow, saved.workflow);
        assert_eq!(fs::read(dir.store().path()).unwrap(), before);
        assert_eq!(
            serde_json::to_value(retry)
                .unwrap()
                .as_object()
                .unwrap()
                .len(),
            4
        );
    }
}

#[test]
fn save_plan_and_receipt_have_exact_ordered_camel_case_wire_contracts() {
    let candidate = draft(42, None, "Wire draft");
    let empty = Records::empty();
    let (plan, _) = prepare(&empty, &candidate).unwrap();
    let expected = format!(
        r#"{{"schemaVersion":"rangoon.workflow-save-plan.v1","workflowId":"{}","revisionId":"{}","expectedStateId":"{}","expectedHeadId":null,"alreadySaved":false,"unresolvedReferences":0}}"#,
        candidate.workflow_id(),
        candidate.id(),
        empty.state_id().unwrap()
    );
    assert_eq!(serde_json::to_string(&plan).unwrap(), expected);
    let mut db = composition_backup::memory_database().unwrap();
    let receipt = save(&mut db, &candidate, &empty.state_id().unwrap()).unwrap();
    // WorkflowDetail has its own wire contract; this fixture fixes its exact
    // position and the outer save receipt fields independently of map sorting.
    let expected = format!(
        r#"{{"schemaVersion":"rangoon.workflow-save-receipt.v1","workflow":{},"alreadySaved":false,"stateId":"{}"}}"#,
        serde_json::to_string(&receipt.workflow).unwrap(),
        receipt.state_id
    );
    assert_eq!(serde_json::to_string(&receipt).unwrap(), expected);
    let retry = save(&mut db, &candidate, &empty.state_id().unwrap()).unwrap();
    let expected = format!(
        r#"{{"schemaVersion":"rangoon.workflow-save-receipt.v1","workflow":{},"alreadySaved":true,"stateId":"{}"}}"#,
        serde_json::to_string(&receipt.workflow).unwrap(),
        receipt.state_id
    );
    assert_eq!(serde_json::to_string(&retry).unwrap(), expected);
    let (retry_plan, _) = prepare(&Records::load_complete(&db).unwrap(), &candidate).unwrap();
    let expected = format!(
        r#"{{"schemaVersion":"rangoon.workflow-save-plan.v1","workflowId":"{}","revisionId":"{}","expectedStateId":"{}","expectedHeadId":"{}","alreadySaved":true,"unresolvedReferences":0}}"#,
        candidate.workflow_id(),
        candidate.id(),
        receipt.state_id,
        candidate.id()
    );
    assert_eq!(serde_json::to_string(&retry_plan).unwrap(), expected);
}
