use super::*;
use crate::composition_records_tests::{TestDir, source};
use rangoon_workflow::records::{prepare_revision, workflow_id_from_nonce};
use workflow_records::SavedRevision;

fn fixture() -> Records {
    let mut records = Records::empty();
    records.version = 4;
    for seed in [12, 9] {
        let owner = workflow_id_from_nonce(&[seed; 32]);
        let mut parent = None;
        for time in [90, 5] {
            let definition = serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":format!("  Draft\n {time}\u{2028} "),"nodes":[],"controlEdges":[],"dataEdges":[]});
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
    }
    Records::load_complete(&composition_backup::canonical_database(&records).unwrap()).unwrap()
}

#[test]
fn display_labels_preserve_record_bytes_and_utf8_boundaries() {
    assert_eq!(label("\n\t\u{2028}\u{2029}\0"), "Untitled workflow");
    assert_eq!(label("  A\nB\u{2028} C\u{2029}  "), "AB C");
    assert_eq!(label(&format!("{}é", "a".repeat(159))), "a".repeat(159));
    assert_eq!(label(&"🦀".repeat(41)), "🦀".repeat(40));
    assert_eq!(
        label(&format!("{}   suffix", "a".repeat(158))),
        "a".repeat(158)
    );
    let records = fixture();
    let owner = records.workflows.owners.keys().next().unwrap();
    let result = detail(&records, owner, None).unwrap();
    assert_eq!(result.head.label, "Draft 5");
    assert!(result.head.label_adjusted);
    assert_eq!(
        result.revision.inspection().definition().title,
        "  Draft\n 5\u{2028} "
    );
}

#[test]
fn historical_selection_preserves_current_head_and_parent_order() {
    let records = fixture();
    let owner = records.workflows.owners.keys().next().unwrap();
    let history = records.workflows.history(owner).unwrap();
    let current = detail(&records, owner, None).unwrap();
    let previous = detail(&records, owner, Some(&history[0])).unwrap();
    assert_eq!(previous.head, current.head);
    assert_eq!(previous.revision.id(), history[0]);
    assert_eq!(previous.saved_at_ms, 90);
    assert_eq!(
        previous
            .history
            .iter()
            .map(|row| row.saved_at_ms)
            .collect::<Vec<_>>(),
        [90, 5]
    );
    assert_eq!(
        previous.history[1].parent_revision_id.as_deref(),
        Some(history[0].as_str())
    );
    assert!(!current.head.structurally_valid);
    assert_eq!(current.head.revision_count, 2);
    let other = records.workflows.owners.keys().nth(1).unwrap();
    assert_eq!(
        detail(&records, other, Some(&history[0])).unwrap_err(),
        StoreError::WorkflowNotFound
    );
    let wire = serde_json::to_value(previous).unwrap();
    assert_eq!(wire["schemaVersion"], "rangoon.workflow-detail.v1");
    assert_eq!(wire.as_object().unwrap().len(), 6);
}

#[test]
fn complete_inventory_keeps_separate_counts_and_sorted_workflows() {
    let records = fixture();
    let result = data(&records, 123, 45).unwrap();
    assert_eq!(result.schema_version, "rangoon.workspace-data.v2");
    assert_eq!(
        (
            result.records.workflows,
            result.records.workflow_revisions,
            result.records.revisions
        ),
        (2, 4, 0)
    );
    assert_eq!((result.database_bytes, result.reusable_bytes), (123, 45));
    assert!(result.workflows[0].id < result.workflows[1].id);
    assert_eq!(result.state_id, records.state_id().unwrap());
    assert_eq!(
        serde_json::to_value(result)
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        8
    );
}

#[test]
fn public_reads_are_noncreating_and_existing_source_bytes_unchanged() {
    let dir = TestDir::new();
    let owner = workflow_id_from_nonce(&[1; 32]);
    assert_eq!(
        dir.store().open_workflow("invalid", None).unwrap_err(),
        StoreError::WorkflowInvalid
    );
    assert_eq!(
        dir.store()
            .open_workflow(&owner, Some("invalid"))
            .unwrap_err(),
        StoreError::WorkflowInvalid
    );
    assert_eq!(
        dir.store().open_workflow(&owner, None).unwrap_err(),
        StoreError::WorkflowNotFound
    );
    assert!(dir.store().list_workflows().unwrap().is_empty());
    let empty = dir.store().workflow_data().unwrap();
    assert_eq!(empty.database_bytes, 0);
    assert_eq!(empty.state_id, Records::empty().state_id().unwrap());
    assert!(!dir.0.exists());
    source(&dir);
    let before = fs::read(dir.store().path()).unwrap();
    let result = dir.store().workflow_data().unwrap();
    assert_eq!(result.records.sources, 1);
    assert!(result.workflows.is_empty());
    assert!(dir.store().list_workflows().unwrap().is_empty());
    assert_eq!(
        dir.store().open_workflow(&owner, None).unwrap_err(),
        StoreError::WorkflowNotFound
    );
    assert_eq!(fs::read(dir.store().path()).unwrap(), before);
}

#[test]
fn reference_observations_keep_duplicate_node_indices_and_do_not_grant_authority() {
    let dir = TestDir::new();
    let report = source(&dir);
    crate::composition_records_tests::legacy(&dir, &report);
    let mut records = Records::load(&dir.db()).unwrap();
    let (cap, owner) = records.owners.first_key_value().unwrap();
    let missing_cap = format!("capability:{}", "0".repeat(64));
    let missing_rev = format!("revision:{}", "0".repeat(64));
    let pairs = [
        (cap.as_str(), owner.latest_revision_id.as_str()),
        (cap.as_str(), "bad"),
        (missing_cap.as_str(), missing_rev.as_str()),
        (cap.as_str(), missing_rev.as_str()),
    ];
    let nodes: Vec<_> = pairs.iter().map(|(capability, revision)| serde_json::json!({"id":"duplicate","title":"Capability","operation":{"kind":"capability","capabilityId":capability,"revisionId":revision},"inputs":[],"outputs":[]})).collect();
    let definition = serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":"References","nodes":nodes,"controlEdges":[],"dataEdges":[]});
    let owner = workflow_id_from_nonce(&[78; 32]);
    let record = prepare_revision(
        &owner,
        None,
        SaveIntent::Draft,
        &serde_json::to_vec(&definition).unwrap(),
        br#"{"positions":[]}"#,
    )
    .unwrap();
    records.version = 4;
    records
        .workflows
        .owners
        .insert(owner.clone(), record.id().into());
    records.workflows.revisions.insert(
        record.id().into(),
        SavedRevision {
            record,
            saved_at_ms: 1,
        },
    );
    let records =
        Records::load_complete(&composition_backup::canonical_database(&records).unwrap()).unwrap();
    let inspected = detail(&records, &owner, None).unwrap();
    assert_eq!(inspected.head.unresolved_references, 3);
    assert_eq!(
        inspected
            .references
            .iter()
            .map(|r| (r.node_index, r.status))
            .collect::<Vec<_>>(),
        [
            (0, WorkflowReferenceStatus::Resolved),
            (1, WorkflowReferenceStatus::InvalidReference),
            (2, WorkflowReferenceStatus::MissingCapability),
            (3, WorkflowReferenceStatus::MissingRevision)
        ]
    );
    let wire = serde_json::to_value(inspected.revision.inspection().report()).unwrap();
    assert_eq!(wire["authority"], "none");
    assert_eq!(wire["referenceStatus"], "unverified");
    assert_eq!(wire["executionStatus"], "unavailable");
    assert_eq!(
        serde_json::to_value(&inspected.references[2]).unwrap()["status"],
        "missing_capability"
    );
}

#[test]
fn new_reads_preserve_empty_files_and_accept_complete_schema_four() {
    let dir = TestDir::new();
    fs::create_dir_all(&dir.0).unwrap();
    fs::write(dir.store().path(), []).unwrap();
    let owner = workflow_id_from_nonce(&[1; 32]);
    for initialized in [false, true] {
        if initialized {
            dir.db()
                .execute_batch("PRAGMA page_size=4096; VACUUM;")
                .unwrap();
        }
        let before = fs::read(dir.store().path()).unwrap();
        assert!(dir.store().list_workflows().unwrap().is_empty());
        assert_eq!(
            dir.store().workflow_data().unwrap().records,
            WorkflowRecoveryCounts::default()
        );
        assert_eq!(
            dir.store().open_workflow(&owner, None).unwrap_err(),
            StoreError::WorkflowNotFound
        );
        assert_eq!(fs::read(dir.store().path()).unwrap(), before);
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }
    source(&dir);
    crate::composition_records_tests::schema3(&dir.db());
    for (_, schema) in workflow_records::SCHEMAS {
        dir.db().execute_batch(schema).unwrap();
    }
    dir.db().pragma_update(None, "user_version", 4).unwrap();
    let before = fs::read(dir.store().path()).unwrap();
    assert!(dir.store().list_workflows().unwrap().is_empty());
    assert_eq!(dir.store().workflow_data().unwrap().records.workflows, 0);
    assert_eq!(
        dir.store().open_workflow(&owner, None).unwrap_err(),
        StoreError::WorkflowNotFound
    );
    assert_eq!(fs::read(dir.store().path()).unwrap(), before);
}
