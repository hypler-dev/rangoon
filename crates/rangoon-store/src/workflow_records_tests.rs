//! Synthetic SQLite fixtures only; these tests do not enable schema 4.
use super::*;
use crate::composition_records_tests::{TestDir, legacy, source};
use rangoon_workflow::records::{prepare_revision, workflow_id_from_nonce};
use rusqlite::params;
use serde_json::{Value, json};

fn database() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    for (_, sql) in SCHEMAS {
        db.execute_batch(sql).unwrap();
    }
    db
}

fn definition() -> Value {
    json!({"schemaVersion":"rangoon.workflow-definition.v1","title":"Draft 😀",
        "nodes":[],"controlEdges":[],"dataEdges":[]})
}

fn record(owner: u8, parent: Option<&str>, title: &str) -> WorkflowRevision {
    let mut value = definition();
    value["title"] = title.into();
    prepare_revision(
        &workflow_id_from_nonce(&[owner; 32]),
        parent,
        SaveIntent::Draft,
        &serde_json::to_vec(&value).unwrap(),
        br#"{"positions":[]}"#,
    )
    .unwrap()
}

fn insert(db: &Connection, record: &WorkflowRevision, time: i64) {
    db.execute(
        "INSERT INTO workflow_revisions VALUES (?1,?2,?3,?4)",
        params![
            record.id(),
            record.workflow_id(),
            record.serialized_bytes().unwrap(),
            time,
        ],
    )
    .unwrap();
}

fn head(db: &Connection, record: &WorkflowRevision) {
    db.execute(
        "INSERT OR REPLACE INTO workflows VALUES (?1,?2)",
        params![record.workflow_id(), record.id()],
    )
    .unwrap();
}

fn assert_corrupt(db: &Connection) {
    assert_eq!(
        WorkflowRows::load(db, &Records::empty()).unwrap_err(),
        StoreError::Corrupt
    );
}

#[test]
fn empty_tables_and_incomplete_draft_history_preserve_bytes_and_parent_order() {
    let db = database();
    assert!(
        WorkflowRows::load(&db, &Records::empty())
            .unwrap()
            .owners
            .is_empty()
    );
    let root = record(1, None, "Original 😀\r\n");
    let child = record(1, Some(root.id()), "Unfinished");
    insert(&db, &root, 8_640_000_000_000_000);
    insert(&db, &child, 0);
    head(&db, &child);
    let before = db.total_changes();
    let rows = WorkflowRows::load(&db, &Records::empty()).unwrap();
    assert_eq!(before, db.total_changes());
    assert_eq!(
        rows.history(root.workflow_id()).unwrap(),
        [root.id(), child.id()]
    );
    assert!(rows.history(&workflow_id_from_nonce(&[2; 32])).is_none());
    assert_eq!(rows.revisions[root.id()].record, root);
    assert_eq!(rows.revisions[child.id()].record, child);
    assert!(
        !rows.revisions[child.id()]
            .record
            .inspection()
            .report()
            .structurally_valid
    );
    assert_eq!(rows.revisions[root.id()].saved_at_ms, 8_640_000_000_000_000);
}

#[test]
fn stored_record_must_be_canonical_and_match_both_row_ids() {
    let root = record(1, None, "Draft");
    let canonical = root.serialized_bytes().unwrap();
    for mutation in 0..7 {
        let db = database();
        insert(&db, &root, 1);
        head(&db, &root);
        match mutation {
            0 => {
                let mut bytes = canonical.clone();
                bytes.push(b' ');
                db.execute("UPDATE workflow_revisions SET record_json=?1", [bytes])
                    .unwrap();
            }
            1 => {
                let mut value: Value = serde_json::from_slice(&canonical).unwrap();
                value["definition"]["title"] = "Tampered".into();
                db.execute(
                    "UPDATE workflow_revisions SET record_json=?1",
                    [serde_json::to_vec(&value).unwrap()],
                )
                .unwrap();
            }
            2 => {
                db.execute(
                    "UPDATE workflow_revisions SET workflow_id=?1",
                    [workflow_id_from_nonce(&[2; 32])],
                )
                .unwrap();
            }
            3 => {
                db.execute(
                    "UPDATE workflow_revisions SET revision_id=?1",
                    [format!("workflow-revision:{}", "0".repeat(64))],
                )
                .unwrap();
            }
            4 => {
                db.execute("UPDATE workflow_revisions SET record_json=?1", [vec![0xff]])
                    .unwrap();
            }
            5 => {
                db.execute(
                    "UPDATE workflow_revisions SET record_json=?1",
                    [canonical[..canonical.len() - 1].to_vec()],
                )
                .unwrap();
            }
            _ => {
                let text = String::from_utf8(canonical.clone()).unwrap().replacen(
                    "{",
                    "{\"schemaVersion\":\"rangoon.workflow-revision.v1\",",
                    1,
                );
                db.execute(
                    "UPDATE workflow_revisions SET record_json=?1",
                    [text.into_bytes()],
                )
                .unwrap();
            }
        }
        assert_corrupt(&db);
    }
}

#[test]
fn sqlite_storage_types_lengths_and_time_bounds_are_checked() {
    let root = record(1, None, "Draft");
    for sql in [
        "UPDATE workflows SET workflow_id=zeroblob(73)",
        "UPDATE workflows SET latest_revision_id=zeroblob(82)",
        "UPDATE workflows SET workflow_id='workflow:invalid'",
        "UPDATE workflow_revisions SET revision_id=zeroblob(82)",
        "UPDATE workflow_revisions SET workflow_id=zeroblob(73)",
        "UPDATE workflow_revisions SET record_json='{}'",
        "UPDATE workflow_revisions SET record_json=zeroblob(163841)",
        "UPDATE workflow_revisions SET saved_at_ms=-1",
        "UPDATE workflow_revisions SET saved_at_ms=8640000000000001",
        "UPDATE workflow_revisions SET saved_at_ms=1.5",
        "UPDATE workflow_revisions SET saved_at_ms=zeroblob(8)",
    ] {
        let db = database();
        insert(&db, &root, 1);
        head(&db, &root);
        db.execute_batch(sql).unwrap();
        assert_corrupt(&db);
    }
}

#[test]
fn empty_owners_orphan_roots_missing_cross_owner_parents_and_forks_fail() {
    let root = record(1, None, "Root");
    let child = record(1, Some(root.id()), "Child");
    let fork = record(1, Some(root.id()), "Fork");
    let cross = record(2, Some(root.id()), "Cross owner");
    for case in 0..7 {
        let db = database();
        match case {
            0 => head(&db, &root),
            1 => insert(&db, &root, 1),
            2 => {
                insert(&db, &child, 1);
                head(&db, &child);
            }
            3 => {
                insert(&db, &root, 1);
                insert(&db, &cross, 1);
                head(&db, &root);
                head(&db, &cross);
            }
            4 => {
                insert(&db, &root, 1);
                insert(&db, &child, 1);
                insert(&db, &fork, 1);
                head(&db, &child);
            }
            5 => {
                insert(&db, &root, 1);
                insert(&db, &child, 1);
                head(&db, &root);
            }
            _ => {
                insert(&db, &root, 1);
                let another = record(1, None, "Second root");
                insert(&db, &another, 1);
                head(&db, &another);
            }
        }
        assert_corrupt(&db);
    }
}

fn chain(db: &Connection, owner: u8, length: usize) {
    let mut parent = None;
    for index in 0..length {
        let revision = record(owner, parent.as_deref(), &format!("Revision {index}"));
        insert(db, &revision, index as i64);
        head(db, &revision);
        parent = Some(revision.id().to_owned());
    }
}

#[test]
fn exact_workflow_and_history_quotas_pass_one_over_fails() {
    let db = database();
    for owner in 0..128 {
        chain(&db, owner, 1);
    }
    assert_eq!(
        WorkflowRows::load(&db, &Records::empty())
            .unwrap()
            .owners
            .len(),
        128
    );
    chain(&db, 128, 1);
    assert_corrupt(&db);
    let db = database();
    chain(&db, 0, 32);
    let rows = WorkflowRows::load(&db, &Records::empty()).unwrap();
    let id = workflow_id_from_nonce(&[0; 32]);
    assert_eq!(rows.history(&id).unwrap().len(), 32);
    let extra = record(0, Some(&rows.owners[&id]), "Thirty third");
    insert(&db, &extra, 33);
    head(&db, &extra);
    assert_corrupt(&db);
}

#[test]
fn exact_total_revision_quota_passes_one_over_fails() {
    let db = database();
    for owner in 0..32 {
        chain(&db, owner, 32);
    }
    assert_eq!(
        WorkflowRows::load(&db, &Records::empty())
            .unwrap()
            .revisions
            .len(),
        1024
    );
    chain(&db, 32, 1);
    assert_corrupt(&db);
}

fn with_references(
    owner: u8,
    parent: Option<&str>,
    pins: &[(&str, &str)],
    intent: SaveIntent,
) -> WorkflowRevision {
    let mut value = definition();
    let mut nodes = vec![
        json!({"id":"start","title":"Start","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"text","dataType":"text"}]}),
    ];
    let mut edges = Vec::new();
    let mut data = Vec::new();
    let mut previous = "start".to_owned();
    for (index, (capability, revision)) in pins.iter().enumerate() {
        let id = format!("cap{index}");
        nodes.push(json!({"id":id,"title":"Capability","operation":{"kind":"capability","capabilityId":capability,"revisionId":revision},"inputs":[{"name":"text","dataType":"text"}],"outputs":[{"name":"text","dataType":"text"}]}));
        edges.push(json!({"fromNode":previous,"outlet":"next","toNode":id}));
        data.push(json!({"fromNode":previous,"fromPort":"text","toNode":id,"toPort":"text"}));
        previous = id;
    }
    nodes.push(json!({"id":"finish","title":"Finish","operation":{"kind":"output"},"inputs":[{"name":"text","dataType":"text"}],"outputs":[]}));
    edges.push(json!({"fromNode":previous,"outlet":"next","toNode":"finish"}));
    data.push(json!({"fromNode":previous,"fromPort":"text","toNode":"finish","toPort":"text"}));
    value["nodes"] = nodes.into();
    value["controlEdges"] = edges.into();
    value["dataEdges"] = data.into();
    prepare_revision(
        &workflow_id_from_nonce(&[owner; 32]),
        parent,
        intent,
        &serde_json::to_vec(&value).unwrap(),
        br#"{"positions":[{"nodeIndex":1,"x":-100000,"y":100000}]}"#,
    )
    .unwrap()
}

#[test]
fn draft_dependency_observations_use_exact_owner_revision_and_node_index() {
    let dir = TestDir::new();
    let source = source(&dir);
    legacy(&dir, &source);
    let base = Records::load(&dir.db()).unwrap();
    let (cap, owner) = base.owners.first_key_value().unwrap();
    let revision = &owner.latest_revision_id;
    let missing_cap = format!("capability:{}", "0".repeat(64));
    let missing_rev = format!("revision:{}", "0".repeat(64));
    let record = with_references(
        1,
        None,
        &[
            (cap, revision),
            (cap, "bad"),
            (&missing_cap, &missing_rev),
            (cap, &missing_rev),
        ],
        SaveIntent::Draft,
    );
    let expected = vec![
        (1, ReferenceStatus::Resolved),
        (2, ReferenceStatus::InvalidReference),
        (3, ReferenceStatus::MissingCapability),
        (4, ReferenceStatus::MissingRevision),
    ];
    assert_eq!(references(&record, &base), expected);
    let db = database();
    insert(&db, &record, 0);
    head(&db, &record);
    let rows = WorkflowRows::load(&db, &base).unwrap();
    assert_eq!(rows.revisions[record.id()].record, record);
    let mut wrong_owner = base.clone();
    wrong_owner
        .revisions
        .get_mut(revision)
        .unwrap()
        .capability_id = missing_cap;
    assert_eq!(
        references(&record, &wrong_owner)[0].1,
        ReferenceStatus::MissingRevision
    );
}

#[test]
fn duplicate_node_ids_preserve_distinct_array_reference_observations() {
    let dir = TestDir::new();
    let source = source(&dir);
    legacy(&dir, &source);
    let base = Records::load(&dir.db()).unwrap();
    let (cap, owner) = base.owners.first_key_value().unwrap();
    let original = with_references(
        1,
        None,
        &[(cap, &owner.latest_revision_id), (cap, "unfinished")],
        SaveIntent::Draft,
    );
    let mut definition: Value =
        serde_json::from_slice(&original.inspection().canonical_definition_bytes().unwrap())
            .unwrap();
    definition["nodes"][2]["id"] = definition["nodes"][1]["id"].clone();
    let draft = prepare_revision(
        original.workflow_id(),
        None,
        SaveIntent::Draft,
        &serde_json::to_vec(&definition).unwrap(),
        br#"{"positions":[{"nodeIndex":1,"x":12,"y":-24},{"nodeIndex":2,"x":48,"y":96}]}"#,
    )
    .unwrap();
    assert!(!draft.inspection().report().structurally_valid);
    let db = database();
    insert(&db, &draft, 0);
    head(&db, &draft);
    let rows = WorkflowRows::load(&db, &base).unwrap();
    let loaded = &rows.revisions[draft.id()].record;
    assert_eq!(
        references(loaded, &base),
        vec![
            (1, ReferenceStatus::Resolved),
            (2, ReferenceStatus::InvalidReference),
        ]
    );
    assert_eq!(
        loaded.serialized_bytes().unwrap(),
        draft.serialized_bytes().unwrap()
    );
    assert_eq!(loaded.layout().positions()[1].node_index, 2);
}

#[test]
fn every_validated_history_pin_is_required_even_below_a_draft_head() {
    let dir = TestDir::new();
    let source = source(&dir);
    legacy(&dir, &source);
    let base = Records::load(&dir.db()).unwrap();
    let (cap, owner) = base.owners.first_key_value().unwrap();
    let root = with_references(
        1,
        None,
        &[(cap, &owner.latest_revision_id)],
        SaveIntent::Validated,
    );
    let child = record(1, Some(root.id()), "Incomplete draft head");
    let db = database();
    insert(&db, &root, 0);
    insert(&db, &child, 1);
    head(&db, &child);
    let rows = WorkflowRows::load(&db, &base).unwrap();
    assert_eq!(rows.revisions[root.id()].record.layout(), root.layout());
    assert_corrupt(&db);
    let mut missing_revision = base.clone();
    missing_revision.revisions.clear();
    assert_eq!(
        WorkflowRows::load(&db, &missing_revision).unwrap_err(),
        StoreError::Corrupt
    );
}

#[test]
fn nonmatching_extra_table_schema_is_rejected_and_legacy_gate_stays_closed() {
    let db = database();
    db.execute_batch(
        "DROP TABLE workflows; CREATE TABLE workflows (workflow_id TEXT, latest_revision_id TEXT)",
    )
    .unwrap();
    assert_corrupt(&db);
    let dir = TestDir::new();
    source(&dir);
    let db = dir.db();
    crate::composition_records_tests::schema3(&db);
    db.pragma_update(None, "user_version", 4).unwrap();
    assert_eq!(
        crate::verify_schema(&db).unwrap_err(),
        StoreError::UnsupportedSchema
    );
    for (_, sql) in SCHEMAS {
        db.execute_batch(sql).unwrap();
    }
    // Version rejection now precedes schema enumeration, including implicit indexes.
    let before = std::fs::read(dir.store().path()).unwrap();
    assert_eq!(
        crate::verify_schema(&db).unwrap_err(),
        StoreError::UnsupportedSchema
    );
    // Source listing now validates complete schema-4 state explicitly.
    assert_eq!(dir.store().list().unwrap().len(), 1);
    assert_eq!(std::fs::read(dir.store().path()).unwrap(), before);
}

fn loaded_chain(owner: u8, length: usize) -> WorkflowRows {
    let db = database();
    chain(&db, owner, length);
    WorkflowRows::load(&db, &Records::empty()).unwrap()
}

#[test]
fn union_keeps_exact_or_older_prefix_and_imports_complete_absent_owners() {
    let local = loaded_chain(1, 3);
    let older = loaded_chain(1, 2);
    let base = Records::empty();
    let original = local.state_id_v4(&base).unwrap();
    for incoming in [&local, &older] {
        let (result, kept) = local.union(incoming, &base).unwrap();
        assert_eq!(kept, 1);
        assert_eq!(result.state_id_v4(&base).unwrap(), original);
        assert_eq!(
            result
                .history(&workflow_id_from_nonce(&[1; 32]))
                .unwrap()
                .len(),
            3
        );
    }
    let added = loaded_chain(2, 2);
    let (result, kept) = local.union(&added, &base).unwrap();
    assert_eq!(kept, 0);
    assert_eq!(result.revisions.len(), 5);
    assert_eq!(
        result.history(&workflow_id_from_nonce(&[2; 32])),
        added.history(&workflow_id_from_nonce(&[2; 32]))
    );
    assert_ne!(result.state_id_v4(&base).unwrap(), original);
    assert_eq!(local.state_id_v4(&base).unwrap(), original);
}

#[test]
fn union_rejects_newer_divergent_metadata_conflicts_and_corrupt_input_without_mutation() {
    let local = loaded_chain(1, 2);
    let base = Records::empty();
    let original = local.state_id_v4(&base).unwrap();
    let newer = loaded_chain(1, 3);
    let mut retimed = local.clone();
    retimed.revisions.values_mut().next().unwrap().saved_at_ms += 1;
    let db = database();
    let different = record(1, None, "Divergent root");
    insert(&db, &different, 0);
    head(&db, &different);
    let divergent = WorkflowRows::load(&db, &base).unwrap();
    let mut wrong_key = local.clone();
    let (id, saved) = wrong_key.revisions.pop_first().unwrap();
    wrong_key.revisions.insert(format!("{id}0"), saved);
    for incoming in [&newer, &retimed, &divergent, &wrong_key] {
        assert_eq!(
            local.union(incoming, &base).unwrap_err(),
            UnionError::Invalid
        );
        assert_eq!(local.state_id_v4(&base).unwrap(), original);
    }
}

#[test]
fn union_enforces_final_counts_and_validated_pins_against_final_capability_union() {
    let db = database();
    for owner in 0..128 {
        chain(&db, owner, 1);
    }
    let full = WorkflowRows::load(&db, &Records::empty()).unwrap();
    assert_eq!(
        full.union(&loaded_chain(128, 1), &Records::empty())
            .unwrap_err(),
        UnionError::Full
    );
    let db = database();
    for owner in 0..32 {
        chain(&db, owner, 32);
    }
    let full = WorkflowRows::load(&db, &Records::empty()).unwrap();
    assert_eq!(
        full.union(&loaded_chain(32, 1), &Records::empty())
            .unwrap_err(),
        UnionError::Full
    );
    let dir = TestDir::new();
    let source = source(&dir);
    legacy(&dir, &source);
    let base = Records::load(&dir.db()).unwrap();
    let (cap, owner) = base.owners.first_key_value().unwrap();
    let record = with_references(
        1,
        None,
        &[(cap, &owner.latest_revision_id)],
        SaveIntent::Validated,
    );
    let db = database();
    insert(&db, &record, 0);
    head(&db, &record);
    let incoming = WorkflowRows::load(&db, &base).unwrap();
    let empty = WorkflowRows::default();
    assert_eq!(
        empty.union(&incoming, &Records::empty()).unwrap_err(),
        UnionError::DependencyMissing
    );
    let (result, _) = empty.union(&incoming, &base).unwrap();
    assert_eq!(result.revisions[record.id()].record, record);
    // The same check also applies to kept local histories after capability union.
    assert_eq!(
        incoming.union(&empty, &Records::empty()).unwrap_err(),
        UnionError::DependencyMissing
    );
}

#[test]
fn schema4_state_matches_independent_empty_golden_and_binds_saved_metadata() {
    let base = Records::empty();
    let empty = WorkflowRows::default();
    assert_eq!(
        empty.state_id_v4(&base).unwrap(),
        "workspace:6e2f1390532757e98127f7e9faa935f4a3e3b1daf67804e1e4203a5ad872c253"
    );
    assert_ne!(empty.state_id_v4(&base).unwrap(), base.state_id().unwrap());
    let local = loaded_chain(1, 2);
    let before = local.state_id_v4(&base).unwrap();
    let mut retimed = local.clone();
    retimed.revisions.values_mut().next().unwrap().saved_at_ms += 1;
    assert_ne!(retimed.state_id_v4(&base).unwrap(), before);
    assert_ne!(loaded_chain(1, 1).state_id_v4(&base).unwrap(), before);
    assert_ne!(loaded_chain(2, 2).state_id_v4(&base).unwrap(), before);
    let dir = TestDir::new();
    let source = source(&dir);
    let source_base = Records::load(&dir.db()).unwrap();
    assert_ne!(local.state_id_v4(&source_base).unwrap(), before);
    legacy(&dir, &source);
    let capability_base = Records::load(&dir.db()).unwrap();
    assert_ne!(
        local.state_id_v4(&capability_base).unwrap(),
        local.state_id_v4(&source_base).unwrap()
    );
    let mut retimed_source = source_base.clone();
    retimed_source
        .sources
        .values_mut()
        .next()
        .unwrap()
        .metadata
        .saved_at_ms += 1;
    assert_ne!(
        local.state_id_v4(&retimed_source).unwrap(),
        local.state_id_v4(&source_base).unwrap()
    );
}

#[test]
fn schema4_nonempty_state_matches_python_json_sha256_golden() {
    let definition = json!({"schemaVersion":"rangoon.workflow-definition.v1","title":"Draft \"A\" 😀",
        "nodes":[
            {"id":"start","title":"Input","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"text","dataType":"text"}]},
            {"id":"finish","title":"Output","operation":{"kind":"output"},"inputs":[{"name":"text","dataType":"text"}],"outputs":[]}],
        "controlEdges":[{"fromNode":"start","outlet":"next","toNode":"finish"}],
        "dataEdges":[{"fromNode":"start","fromPort":"text","toNode":"finish","toPort":"text"}]});
    let record = prepare_revision(
        &workflow_id_from_nonce(&std::array::from_fn(|index| index as u8)),
        None,
        SaveIntent::Draft,
        &serde_json::to_vec(&definition).unwrap(),
        br#"{"positions":[{"nodeIndex":0,"x":-100000,"y":0},{"nodeIndex":1,"x":100000,"y":42}]}"#,
    )
    .unwrap();
    assert_eq!(
        record.id(),
        "workflow-revision:c54954b55718aa53d3752c9d6d760082146782253397ca61b043d2198efa3650"
    );
    let db = database();
    insert(&db, &record, 7);
    head(&db, &record);
    let base = Records::empty();
    let rows = WorkflowRows::load(&db, &base).unwrap();
    assert_eq!(
        rows.state_id_v4(&base).unwrap(),
        "workspace:ea600a31e64e8f19a640256967d055b027d262e37a690f2dc16005845ad12cea"
    );
}
