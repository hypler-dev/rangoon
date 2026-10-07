//! Complete-state routing and legacy schema rejection fixtures.
use super::*;
use composition_records::Records;
use composition_records_tests::{TestDir, fixture_request, legacy, source};
use rangoon_workflow::records::{SaveIntent, prepare_revision, workflow_id_from_nonce};

fn install_workflow(dir: &TestDir, cap: &str, revision: &str) -> String {
    let definition = serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":"Pinned workflow",
        "nodes":[
            {"id":"start","title":"Start","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"text","dataType":"text"}]},
            {"id":"work","title":"Work","operation":{"kind":"capability","capabilityId":cap,"revisionId":revision},"inputs":[{"name":"text","dataType":"text"}],"outputs":[{"name":"text","dataType":"text"}]},
            {"id":"finish","title":"Finish","operation":{"kind":"output"},"inputs":[{"name":"text","dataType":"text"}],"outputs":[]}],
        "controlEdges":[{"fromNode":"start","outlet":"next","toNode":"work"},{"fromNode":"work","outlet":"next","toNode":"finish"}],
        "dataEdges":[{"fromNode":"start","fromPort":"text","toNode":"work","toPort":"text"},{"fromNode":"work","fromPort":"text","toNode":"finish","toPort":"text"}]});
    let id = workflow_id_from_nonce(&[55; 32]);
    let mut db = dir.db();
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    workflow_mutations::ensure_schema(&tx, 4).unwrap();
    let mut parent = None;
    for time in [2, 1] {
        let record = prepare_revision(
            &id,
            parent.as_deref(),
            SaveIntent::Validated,
            &serde_json::to_vec(&definition).unwrap(),
            br#"{"positions":[]}"#,
        )
        .unwrap();
        tx.execute(
            "INSERT INTO workflow_revisions VALUES (?1,?2,?3,?4)",
            params![record.id(), id, record.serialized_bytes().unwrap(), time],
        )
        .unwrap();
        parent = Some(record.id().to_owned());
    }
    tx.execute(
        "INSERT INTO workflows VALUES (?1,?2)",
        params![id, parent.unwrap()],
    )
    .unwrap();
    Records::load_complete(&tx).unwrap();
    tx.commit().unwrap();
    id
}
fn workflow_bytes(dir: &TestDir) -> Vec<u8> {
    let records = Records::load_complete(&dir.db()).unwrap();
    assert_eq!(records.version, 4);
    let rows: Vec<_> = records
        .workflows
        .revisions
        .iter()
        .map(|(id, row)| (id, row.record.serialized_bytes().unwrap(), row.saved_at_ms))
        .collect();
    serde_json::to_vec(&(records.workflows.owners, rows)).unwrap()
}

#[test]
fn aware_source_skill_composition_and_compile_preserve_pins_and_history() {
    let dir = TestDir::new();
    let report = source(&dir);
    let cap = legacy(&dir, &report);
    let id = install_workflow(&dir, &cap.id, &cap.latest_revision_id);
    let before = workflow_bytes(&dir);
    let store = dir.store();
    assert_eq!(store.list().unwrap().len(), 1);
    assert_eq!(store.open(&report.source.id).unwrap(), report);
    assert_eq!(store.list_capabilities_v1().unwrap().len(), 1);
    let extra = analyze("CLAUDE.md", b"# Extra\nMore source.\n").unwrap();
    store.save_v1(&extra).unwrap();
    assert!(store.save_v1(&extra).unwrap().already_saved);
    let new_cap = store
        .create_capability_v1(&extra.source.id, &extra.fragments[0].id, "Extra")
        .unwrap()
        .capability;
    let next = store
        .revise_capability_v1(&cap.id, &cap.latest_revision_id, "Edited", "Edited bytes")
        .unwrap()
        .capability;
    store
        .review_capability_v1(&cap.id, &next.latest_revision_id)
        .unwrap();
    assert_eq!(
        store
            .open_capability_v1(&cap.id, Some(&cap.latest_revision_id))
            .unwrap()
            .revision
            .content,
        cap.revision.content
    );
    assert_eq!(
        store.open_capability_v1(&new_cap.id, None).unwrap(),
        new_cap
    );
    let mut request = fixture_request();
    request.targets[0] = rangoon_compose::application::Target::Append {
        capability_id: cap.id.clone(),
        expected_revision_id: next.latest_revision_id.clone(),
    };
    let preview = store.preview_composition(&request).unwrap();
    let result = store.apply_composition(&preview, true).unwrap();
    assert_eq!(result.capabilities.len(), 2);
    assert_eq!(
        result.committed_state_id,
        Records::load_complete(&dir.db())
            .unwrap()
            .state_id()
            .unwrap()
    );
    let compiled = store
        .compile_capability(
            &cap.id,
            &cap.latest_revision_id,
            rangoon_compile::Profile::AgentsMdV1,
        )
        .unwrap();
    assert_eq!(
        compiled.observed_current_head,
        result.capabilities[0].latest_revision_id
    );
    assert_eq!(
        compiled.compilation.selected_revision.revision_id,
        cap.latest_revision_id
    );
    assert_eq!(compiled.compilation.artifact.content, cap.revision.content);
    assert_eq!(workflow_bytes(&dir), before);
    assert_eq!(
        Records::load_complete(&dir.db())
            .unwrap()
            .workflows
            .history(&id)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn every_legacy_api_rejects_schema_four_without_projection_or_mutation() {
    let dir = TestDir::new();
    let report = source(&dir);
    let cap = legacy(&dir, &report);
    let store = dir.store();
    let old_backup = Backup::decode(&store.export_backup().unwrap()).unwrap();
    let old_state = store.data().unwrap().state_id;
    let composed_backup =
        CompositionBackup::decode(&store.export_composition_backup().unwrap()).unwrap();
    let restore_plan = store.prepare_composition_restore(&composed_backup).unwrap();
    let delete_plan = store
        .inspect_composition_deletion(RecordKind::Capability, &cap.id)
        .unwrap();
    install_workflow(&dir, &cap.id, &cap.latest_revision_id);
    let before = fs::read(store.path()).unwrap();
    macro_rules! refused {
        ($call:expr) => {
            assert_eq!($call.unwrap_err(), StoreError::UnsupportedSchema);
        };
    }
    refused!(store.save(&report));
    refused!(store.list_capabilities());
    refused!(store.open_capability(&cap.id, None));
    refused!(store.create_capability(&report.source.id, &report.fragments[0].id, "Other"));
    refused!(store.revise_capability(&cap.id, &cap.latest_revision_id, "Other", "Other"));
    refused!(store.review_capability(&cap.id, &cap.latest_revision_id));
    refused!(store.data());
    refused!(store.export_backup());
    refused!(store.prepare_restore(&old_backup));
    refused!(store.restore_backup(&old_backup, &old_state));
    refused!(store.inspect_deletion(RecordKind::Capability, &cap.id));
    refused!(store.delete_record(RecordKind::Capability, &cap.id, &old_state));
    refused!(store.composition_data());
    refused!(store.export_composition_backup());
    refused!(store.prepare_composition_restore(&composed_backup));
    refused!(store.restore_composition_backup(&composed_backup, &restore_plan));
    refused!(store.inspect_composition_deletion(RecordKind::Capability, &cap.id));
    refused!(store.delete_composition_record(&delete_plan));
    assert_eq!(fs::read(store.path()).unwrap(), before);
}

#[test]
fn complete_state_rejects_corrupt_workflow_before_any_aware_operation() {
    let dir = TestDir::new();
    let report = source(&dir);
    let cap = legacy(&dir, &report);
    let id = install_workflow(&dir, &cap.id, &cap.latest_revision_id);
    let store = dir.store();
    let preview = store.preview_composition(&fixture_request()).unwrap();
    dir.db()
        .execute(
            "UPDATE workflows SET latest_revision_id=?1 WHERE workflow_id=?2",
            params![format!("workflow-revision:{}", "0".repeat(64)), id],
        )
        .unwrap();
    let before = fs::read(store.path()).unwrap();
    macro_rules! corrupt {
        ($call:expr) => {
            assert_eq!($call.unwrap_err(), StoreError::Corrupt);
        };
    }
    corrupt!(store.list());
    corrupt!(store.open(&report.source.id));
    corrupt!(store.save_v1(&report));
    corrupt!(store.list_capabilities_v1());
    corrupt!(store.open_capability_v1(&cap.id, None));
    corrupt!(store.create_capability_v1(&report.source.id, &report.fragments[0].id, "Other"));
    corrupt!(store.revise_capability_v1(&cap.id, &cap.latest_revision_id, "Other", "Other"));
    corrupt!(store.review_capability_v1(&cap.id, &cap.latest_revision_id));
    corrupt!(store.preview_composition(&fixture_request()));
    corrupt!(store.apply_composition(&preview, true));
    corrupt!(store.compile_capability(
        &cap.id,
        &cap.latest_revision_id,
        rangoon_compile::Profile::AgentsMdV1
    ));
    assert_eq!(fs::read(store.path()).unwrap(), before);
}

#[test]
fn workflow_metadata_change_invalidates_composition_preview() {
    let dir = TestDir::new();
    let report = source(&dir);
    let cap = legacy(&dir, &report);
    install_workflow(&dir, &cap.id, &cap.latest_revision_id);
    let store = dir.store();
    let preview = store.preview_composition(&fixture_request()).unwrap();
    dir.db()
        .execute(
            "UPDATE workflow_revisions SET saved_at_ms=saved_at_ms+1",
            [],
        )
        .unwrap();
    Records::load_complete(&dir.db()).unwrap();
    let before = fs::read(store.path()).unwrap();
    assert_eq!(
        store.apply_composition(&preview, true).unwrap_err(),
        StoreError::WorkspaceChanged
    );
    assert_eq!(fs::read(store.path()).unwrap(), before);
}
