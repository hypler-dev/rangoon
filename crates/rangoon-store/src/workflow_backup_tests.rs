use super::*;
use crate::composition_records_tests::{TestDir, legacy, source};
use rangoon_workflow::records::{SaveIntent, prepare_revision, workflow_id_from_nonce};
use workflow_records::SavedRevision;

fn encode(base: &Records, workflows: &WorkflowRows) -> Result<Vec<u8>, StoreError> {
    let mut complete = base.clone();
    complete.workflows = workflows.clone();
    super::encode(&complete)
}
fn canonical_database(base: &Records, workflows: &WorkflowRows) -> Result<Connection, StoreError> {
    let mut complete = base.clone();
    complete.workflows = workflows.clone();
    super::canonical_database(&complete)
}

fn fixture(base: &Records) -> WorkflowRows {
    let mut rows = WorkflowRows::default();
    let owner = workflow_id_from_nonce(&[7; 32]);
    let mut parent = None;
    for time in [7, 3] {
        let definition = serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":format!("Draft 😀 {time}"),"nodes":[],"controlEdges":[],"dataEdges":[]});
        let record = prepare_revision(
            &owner,
            parent.as_deref(),
            SaveIntent::Draft,
            &serde_json::to_vec(&definition).unwrap(),
            br#"{"positions":[]}"#,
        )
        .unwrap();
        parent = Some(record.id().to_owned());
        rows.owners.insert(owner.clone(), record.id().into());
        rows.revisions.insert(
            record.id().into(),
            SavedRevision {
                record,
                saved_at_ms: time,
            },
        );
    }
    rows.state_id_v4(base).unwrap();
    rows
}

fn empty4() -> Records {
    let mut base = Records::empty();
    base.version = 4;
    base
}

fn parts(bytes: &[u8]) -> (Manifest, Vec<u8>) {
    let start = MAGIC.len() + 4;
    let size = u32::from_be_bytes(bytes[MAGIC.len()..start].try_into().unwrap()) as usize;
    (
        serde_json::from_slice(&bytes[start..start + size]).unwrap(),
        bytes[start + size..bytes.len() - 64].to_vec(),
    )
}

fn archive(metadata: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&(metadata.len() as u32).to_be_bytes());
    bytes.extend_from_slice(metadata);
    bytes.extend_from_slice(payload);
    bytes.extend_from_slice(byte_digest(&bytes).as_bytes());
    bytes
}

fn rejected(bytes: &[u8]) {
    assert_eq!(
        WorkspaceBackup::decode(bytes).unwrap_err(),
        StoreError::BackupInvalid
    );
}

#[test]
fn empty_v3_matches_independent_exact_archive_and_state_golden() {
    let base = empty4();
    let rows = WorkflowRows::default();
    let bytes = encode(&base, &rows).unwrap();
    let manifest = br#"{"schemaVersion":"rangoon.backup.v3","sources":[],"owners":[],"revisions":[],"recipes":[],"applications":[],"workflows":[],"workflowRevisions":[]}"#;
    assert_eq!(manifest.len(), 146);
    assert_eq!(bytes, archive(manifest, b""));
    assert_eq!(bytes.len(), 232);
    assert_eq!(
        &bytes[bytes.len() - 64..],
        b"099fbfe3bcd82913ca13f642ce4d618e3be0a0e2155fa629a5928013d3e1914c"
    );
    let decoded = WorkspaceBackup::decode(&bytes).unwrap();
    assert_eq!(
        decoded.id(),
        "backup:a43e96d40ce24f0902421e5c2cacad0586525d69b0f4583121df7108847ce6f4"
    );
    assert_eq!(decoded.byte_length(), 232);
    assert_eq!(decoded.records.version, 4);
    assert_eq!(
        decoded
            .records
            .workflows
            .state_id_v4(&decoded.records)
            .unwrap(),
        "workspace:6e2f1390532757e98127f7e9faa935f4a3e3b1daf67804e1e4203a5ad872c253"
    );
}

#[test]
fn complete_source_capability_and_workflow_bytes_round_trip_without_external_writes() {
    let dir = TestDir::new();
    let report = source(&dir);
    legacy(&dir, &report);
    let before = fs::read(dir.store().path()).unwrap();
    let mut base = Records::load(&dir.db()).unwrap();
    base.version = 4;
    let rows = fixture(&base);
    let bytes = encode(&base, &rows).unwrap();
    let decoded = WorkspaceBackup::decode(&bytes).unwrap();
    assert_eq!(
        decoded
            .records
            .workflows
            .state_id_v4(&decoded.records)
            .unwrap(),
        rows.state_id_v4(&base).unwrap()
    );
    assert_eq!(
        decoded.records.sources[&report.source.id]
            .report
            .source
            .content,
        report.source.content
    );
    for (id, saved) in &rows.revisions {
        assert_eq!(
            decoded.records.workflows.revisions[id]
                .record
                .serialized_bytes()
                .unwrap(),
            saved.record.serialized_bytes().unwrap()
        );
        assert_eq!(
            decoded.records.workflows.revisions[id].saved_at_ms,
            saved.saved_at_ms
        );
    }
    assert_eq!(
        encode(&decoded.records, &decoded.records.workflows).unwrap(),
        bytes
    );
    assert_eq!(fs::read(dir.store().path()).unwrap(), before);
    let canonical = canonical_database(&base, &rows).unwrap();
    assert_eq!(
        canonical
            .pragma_query_value::<i64, _>(None, "user_version", |r| r.get(0))
            .unwrap(),
        4
    );
}

#[test]
fn legacy_v1_v2_decode_and_old_export_bytes_are_unchanged() {
    let dir = TestDir::new();
    let report = source(&dir);
    let v1 = dir.store().export_backup().unwrap();
    let legacy_decoded = WorkspaceBackup::decode(&v1).unwrap();
    assert_eq!(legacy_decoded.records.version, 1);
    assert!(legacy_decoded.records.workflows.owners.is_empty());
    legacy(&dir, &report);
    let base = Records::load(&dir.db()).unwrap();
    let old = composition_backup::encode(&base).unwrap();
    assert_eq!(encode(&base, &WorkflowRows::default()).unwrap(), old);
    let v2 = WorkspaceBackup::decode(&old).unwrap();
    assert_eq!(
        v2.records.state_id().unwrap(),
        CompositionBackup::decode(&old)
            .unwrap()
            .records
            .state_id()
            .unwrap()
    );
    assert!(v2.records.workflows.revisions.is_empty());
    assert!(encode(&base, &fixture(&base)).is_err());
    for version in [0, 5, i64::MAX] {
        let mut unknown = base.clone();
        unknown.version = version;
        assert_eq!(
            encode(&unknown, &WorkflowRows::default()).unwrap_err(),
            StoreError::UnsupportedSchema
        );
    }
    rejected(b"RANGOON-BACKUP-V4\n");
}

#[test]
fn framing_lengths_checksums_and_manifest_canonicality_fail_closed() {
    let bytes = encode(&empty4(), &fixture(&empty4())).unwrap();
    for cut in [0, 1, MAGIC.len(), MAGIC.len() + 3, bytes.len() - 1] {
        rejected(&bytes[..cut]);
    }
    let mut changed = bytes.clone();
    changed[0] ^= 1;
    rejected(&changed);
    let mut changed = bytes.clone();
    let end = changed.len() - 1;
    changed[end] ^= 1;
    rejected(&changed);
    let mut changed = bytes.clone();
    changed[MAGIC.len()..MAGIC.len() + 4].copy_from_slice(&u32::MAX.to_be_bytes());
    let end = changed.len() - 64;
    let digest = byte_digest(&changed[..end]);
    changed[end..].copy_from_slice(digest.as_bytes());
    rejected(&changed);
    let (manifest, payload) = parts(&bytes);
    let metadata = manifest.bytes().unwrap();
    let mut whitespace = metadata.clone();
    whitespace.push(b' ');
    rejected(&archive(&whitespace, &payload));
    let duplicate = String::from_utf8(metadata.clone()).unwrap().replacen(
        "{",
        "{\"schemaVersion\":\"rangoon.backup.v3\",",
        1,
    );
    rejected(&archive(duplicate.as_bytes(), &payload));
    let unknown = String::from_utf8(metadata)
        .unwrap()
        .replacen("{", "{\"extra\":0,", 1);
    rejected(&archive(unknown.as_bytes(), &payload));
    rejected(&archive(
        &manifest.bytes().unwrap(),
        &payload[..payload.len() - 1],
    ));
    let mut extra = payload;
    extra.push(0);
    rejected(&archive(&manifest.bytes().unwrap(), &extra));
}

#[test]
fn descriptor_identity_order_limits_and_payload_digests_are_verified() {
    let bytes = encode(&empty4(), &fixture(&empty4())).unwrap();
    for case in 0..10 {
        let (mut manifest, mut payload) = parts(&bytes);
        match case {
            0 => manifest.schema_version = "rangoon.backup.v2".into(),
            1 => manifest.workflow_revisions.reverse(),
            2 => manifest.workflow_revisions[1].id = manifest.workflow_revisions[0].id.clone(),
            3 => manifest.workflow_revisions[0].workflow_id = workflow_id_from_nonce(&[8; 32]),
            4 => manifest.workflow_revisions[0].byte_length = 163841,
            5 => manifest.workflow_revisions[0].saved_at_ms = -1,
            6 => manifest.workflow_revisions[0].sha256 = "A".repeat(64),
            7 => {
                manifest.workflows[0].latest_revision_id =
                    format!("workflow-revision:{}", "0".repeat(64))
            }
            8 => {
                manifest.workflows.clear();
            }
            _ => payload[0] ^= 1,
        }
        rejected(&archive(&manifest.bytes().unwrap(), &payload));
    }
}

#[test]
fn rehashed_noncanonical_workflow_payload_still_fails_record_validation() {
    let base = empty4();
    let rows = fixture(&base);
    let bytes = encode(&base, &rows).unwrap();
    let (mut manifest, mut payload) = parts(&bytes);
    let size = manifest.workflow_revisions[0].byte_length as usize;
    payload.insert(size, b' ');
    manifest.workflow_revisions[0].byte_length += 1;
    manifest.workflow_revisions[0].sha256 = byte_digest(&payload[..size + 1]);
    rejected(&archive(&manifest.bytes().unwrap(), &payload));
}

fn histories(owners: usize, revisions: usize) -> WorkflowRows {
    let mut rows = WorkflowRows::default();
    for owner in 0..owners {
        let id = workflow_id_from_nonce(&[owner as u8; 32]);
        let mut parent = None;
        for index in 0..revisions {
            let definition = serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":format!("Revision {index}"),"nodes":[],"controlEdges":[],"dataEdges":[]});
            let record = prepare_revision(
                &id,
                parent.as_deref(),
                SaveIntent::Draft,
                &serde_json::to_vec(&definition).unwrap(),
                br#"{"positions":[]}"#,
            )
            .unwrap();
            parent = Some(record.id().to_owned());
            rows.owners.insert(id.clone(), record.id().into());
            rows.revisions.insert(
                record.id().into(),
                SavedRevision {
                    record,
                    saved_at_ms: 0,
                },
            );
        }
    }
    rows
}

#[test]
fn exact_descriptor_counts_and_histories_round_trip_and_one_over_rejects() {
    let base = empty4();
    for rows in [histories(128, 1), histories(32, 32)] {
        let bytes = encode(&base, &rows).unwrap();
        let decoded = WorkspaceBackup::decode(&bytes).unwrap();
        assert_eq!(decoded.records.workflows.owners.len(), rows.owners.len());
        assert_eq!(
            decoded.records.workflows.revisions.len(),
            rows.revisions.len()
        );
        assert_eq!(
            encode(&decoded.records, &decoded.records.workflows).unwrap(),
            bytes
        );
    }
    for rows in [histories(129, 1), histories(33, 32), histories(1, 33)] {
        assert!(encode(&base, &rows).is_err());
    }
    let bytes = encode(&base, &fixture(&base)).unwrap();
    let (manifest, payload) = parts(&bytes);
    for (field, count) in [("workflows", 129), ("workflowRevisions", 1025)] {
        let mut value = serde_json::to_value(&manifest).unwrap();
        let item = value[field][0].clone();
        value[field] = vec![item; count].into();
        let metadata = serde_json::to_vec(&value).unwrap();
        assert!(serde_json::from_slice::<Manifest>(&metadata).is_err());
        rejected(&archive(&metadata, &payload));
    }
}

#[test]
fn composed_capability_history_and_recipe_payload_survive_flat_v3() {
    use crate::composition_records_tests::{
        fixture_request, resolved_source, retain_outputs, schema3,
    };
    let dir = TestDir::new();
    let report = source(&dir);
    let db = dir.db();
    schema3(&db);
    retain_outputs(
        &db,
        &fixture_request(),
        &resolved_source(&report),
        &[],
        &[0],
    );
    let mut base = Records::load(&db).unwrap();
    assert!(!base.recipes.is_empty());
    assert!(!base.applications.is_empty());
    let v2 = composition_backup::encode(&base).unwrap();
    let old = CompositionBackup::decode(&v2).unwrap();
    assert_eq!(composition_backup::encode(&old.records).unwrap(), v2);
    base.version = 4;
    let rows = fixture(&base);
    let bytes = encode(&base, &rows).unwrap();
    let decoded = WorkspaceBackup::decode(&bytes).unwrap();
    assert_eq!(
        decoded
            .records
            .workflows
            .state_id_v4(&decoded.records)
            .unwrap(),
        rows.state_id_v4(&base).unwrap()
    );
    assert_eq!(
        encode(&decoded.records, &decoded.records.workflows).unwrap(),
        bytes
    );
}

#[test]
fn validated_workflow_pin_is_required_even_with_rehashed_archive_metadata() {
    let dir = TestDir::new();
    let report = source(&dir);
    legacy(&dir, &report);
    let mut base = Records::load(&dir.db()).unwrap();
    base.version = 4;
    let (cap, owner) = base.owners.first_key_value().unwrap();
    let definition = serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":"Pinned",
        "nodes":[
            {"id":"start","title":"Start","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"text","dataType":"text"}]},
            {"id":"work","title":"Work","operation":{"kind":"capability","capabilityId":cap,"revisionId":owner.latest_revision_id},"inputs":[{"name":"text","dataType":"text"}],"outputs":[{"name":"text","dataType":"text"}]},
            {"id":"finish","title":"Finish","operation":{"kind":"output"},"inputs":[{"name":"text","dataType":"text"}],"outputs":[]}],
        "controlEdges":[{"fromNode":"start","outlet":"next","toNode":"work"},{"fromNode":"work","outlet":"next","toNode":"finish"}],
        "dataEdges":[{"fromNode":"start","fromPort":"text","toNode":"work","toPort":"text"},{"fromNode":"work","fromPort":"text","toNode":"finish","toPort":"text"}]});
    let record = prepare_revision(
        &workflow_id_from_nonce(&[9; 32]),
        None,
        SaveIntent::Validated,
        &serde_json::to_vec(&definition).unwrap(),
        br#"{"positions":[]}"#,
    )
    .unwrap();
    let mut rows = WorkflowRows::default();
    rows.owners
        .insert(record.workflow_id().into(), record.id().into());
    rows.revisions.insert(
        record.id().into(),
        SavedRevision {
            record,
            saved_at_ms: 0,
        },
    );
    let bytes = encode(&base, &rows).unwrap();
    WorkspaceBackup::decode(&bytes).unwrap();
    let (mut manifest, payload) = parts(&bytes);
    let base_length = manifest.base().preflight().unwrap();
    let source_length: usize = manifest
        .sources
        .iter()
        .map(|s| s.byte_length as usize)
        .sum();
    manifest.owners.clear();
    manifest.revisions.clear();
    let mut trimmed = payload[..source_length].to_vec();
    trimmed.extend_from_slice(&payload[base_length..]);
    rejected(&archive(&manifest.bytes().unwrap(), &trimmed));
}

#[test]
fn sqlite_allocation_limit_maps_to_closed_full_error() {
    let db = Connection::open_in_memory().unwrap();
    db.pragma_update(None, "page_size", 4096).unwrap();
    db.pragma_update(None, "max_page_count", 1).unwrap();
    let failure = db
        .execute_batch("CREATE TABLE allocation_fixture (value BLOB NOT NULL)")
        .unwrap_err();
    assert_eq!(StoreError::from(failure), StoreError::Full);
    assert_eq!(
        StoreError::from(rusqlite::Error::InvalidQuery),
        StoreError::Unavailable
    );
}

#[test]
fn archive_manifest_and_payload_limits_precede_reconstruction() {
    let rows = histories(32, 32);
    let mut manifest = Manifest::from_records(&empty4(), &rows).unwrap();
    for revision in &mut manifest.workflow_revisions {
        revision.byte_length = 65536;
    }
    assert_eq!(manifest.preflight().unwrap(), 64 * 1024 * 1024);
    manifest.workflow_revisions[0].byte_length += 1;
    assert!(manifest.preflight().is_err());
    rejected(&archive(&vec![b' '; MAX_MANIFEST + 1], b""));
    let mut oversized = vec![0; MAX_BACKUP_BYTES + 1];
    oversized[..MAGIC.len()].copy_from_slice(MAGIC);
    rejected(&oversized);
}

#[test]
fn exact_64_mib_workflow_payload_rejects_sqlite_allocation_overhead() {
    let mut rows = WorkflowRows::default();
    for owner in 0..32 {
        let id = workflow_id_from_nonce(&[owner; 32]);
        let mut parent = None;
        for _ in 0..32 {
            let mut definition = serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":"X","nodes":[],"controlEdges":[],"dataEdges":[]});
            let small = prepare_revision(
                &id,
                parent.as_deref(),
                SaveIntent::Draft,
                &serde_json::to_vec(&definition).unwrap(),
                br#"{"positions":[]}"#,
            )
            .unwrap();
            definition["title"] = "x"
                .repeat(65536 - small.serialized_bytes().unwrap().len() + 1)
                .into();
            let record = prepare_revision(
                &id,
                parent.as_deref(),
                SaveIntent::Draft,
                &serde_json::to_vec(&definition).unwrap(),
                br#"{"positions":[]}"#,
            )
            .unwrap();
            assert_eq!(record.serialized_bytes().unwrap().len(), 65536);
            parent = Some(record.id().to_owned());
            rows.owners.insert(id.clone(), record.id().into());
            rows.revisions.insert(
                record.id().into(),
                SavedRevision {
                    record,
                    saved_at_ms: 0,
                },
            );
        }
    }
    let base = empty4();
    let manifest = Manifest::from_records(&base, &rows).unwrap();
    assert_eq!(manifest.preflight().unwrap(), 64 * 1024 * 1024);
    assert_eq!(encode(&base, &rows).unwrap_err(), StoreError::Full);
    let mut payload = Vec::with_capacity(64 * 1024 * 1024);
    for saved in rows.revisions.values() {
        payload.extend_from_slice(&saved.record.serialized_bytes().unwrap());
    }
    let bytes = archive(&manifest.bytes().unwrap(), &payload);
    assert!(bytes.len() < MAX_BACKUP_BYTES);
    assert_eq!(
        WorkspaceBackup::decode(&bytes).unwrap_err(),
        StoreError::Full
    );
}
