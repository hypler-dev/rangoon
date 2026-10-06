use super::*;
use composition_records::Records;
use composition_records_tests::{
    TestDir, assert_unchanged, fixture_request, legacy, resolved_source, retain_outputs, schema3,
    source, split_request,
};
use rangoon_compose::application::{Target, TargetHead};
use rangoon_domain::{byte_digest, capability_v1::CapabilityDetail};
use std::{
    process::{Child, Command, Stdio},
    thread,
    time::Instant,
};

/// Fault controls exist only in the test binary and are set only in an isolated child.
pub(super) fn fault_checkpoint(db: &Connection, phase: &str) -> Result<(), StoreError> {
    let Ok(selected) = std::env::var("RANGOON_RECOVERY_TEST_PHASE") else {
        return Ok(());
    };
    db.pragma_update(None, "cache_size", 1)?;
    db.pragma_update(None, "cache_spill", true)?;
    if selected != phase {
        return Ok(());
    }
    if std::env::var("RANGOON_RECOVERY_TEST_MODE").as_deref() == Ok("full") {
        let pages: u32 = db.pragma_query_value(None, "page_count", |r| r.get(0))?;
        let actual: u32 =
            db.pragma_query_value(None, &format!("max_page_count={pages}"), |r| r.get(0))?;
        assert_eq!(actual, pages);
        return Ok(());
    }
    let marker = std::env::var_os("RANGOON_RECOVERY_TEST_READY").unwrap();
    fs::write(marker, phase).unwrap();
    loop {
        thread::sleep(Duration::from_millis(100));
    }
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn fault_child(dir: &TestDir, backup: &PathBuf, phase: &str, mode: &str, id: &str) {
    fault_child_for(
        dir,
        backup,
        phase,
        mode,
        id,
        "composition_recovery_tests::recovery_fault_child",
    );
}

pub(super) fn fault_child_for(
    dir: &TestDir,
    backup: &PathBuf,
    phase: &str,
    mode: &str,
    id: &str,
    test_name: &str,
) {
    let marker = dir.0.join(format!("{phase}-ready"));
    let mut child = ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", test_name])
            .env("RANGOON_RECOVERY_TEST_WORKSPACE", &dir.0)
            .env("RANGOON_RECOVERY_TEST_BACKUP", backup)
            .env("RANGOON_RECOVERY_TEST_PHASE", phase)
            .env("RANGOON_RECOVERY_TEST_MODE", mode)
            .env("RANGOON_RECOVERY_TEST_READY", &marker)
            .env("RANGOON_RECOVERY_TEST_DELETE_ID", id)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if mode == "crash" && marker.exists() {
            assert_eq!(fs::read_to_string(&marker).unwrap(), phase);
            // Dropping the guard kills the process before its transaction can commit.
            break;
        }
        if let Some(status) = child.0.try_wait().unwrap() {
            assert_eq!(
                mode, "full",
                "child exited before reaching its crash checkpoint"
            );
            assert!(status.success(), "SQLite-full child failed its assertions");
            break;
        }
        assert!(Instant::now() < deadline, "recovery fault child timed out");
        thread::sleep(Duration::from_millis(20));
    }
}

fn composed(
    dir: &TestDir,
    live: &[usize],
) -> (
    AnalysisReport,
    rangoon_compose::application::ApplicationPreview,
) {
    let saved = source(dir);
    let db = dir.db();
    schema3(&db);
    db.execute("UPDATE snapshots SET saved_at_ms=1", [])
        .unwrap();
    let preview = retain_outputs(&db, &fixture_request(), &resolved_source(&saved), &[], live);
    (saved, preview)
}
fn v2(dir: &TestDir) -> CompositionBackup {
    CompositionBackup::decode(&dir.store().export_composition_backup().unwrap()).unwrap()
}
fn restore(dir: &TestDir, backup: &CompositionBackup) -> CompositionRestorePlan {
    let plan = dir.store().prepare_composition_restore(backup).unwrap();
    assert_eq!(
        dir.store()
            .restore_composition_backup(backup, &plan)
            .unwrap(),
        plan
    );
    plan
}

#[test]
fn v2_round_trip_preserves_exact_provenance_reviews_and_dead_destinations() {
    let original = TestDir::new();
    let (saved, preview) = composed(&original, &[0]);
    let output = &preview.applied_outputs[0];
    original
        .db()
        .execute(
            "INSERT INTO reviews VALUES (?1,'local_operator',2)",
            [&output.revision_id],
        )
        .unwrap();
    let before = fs::read(original.store().path()).unwrap();
    let bytes = original.store().export_composition_backup().unwrap();
    assert!(bytes.starts_with(b"RANGOON-BACKUP-V2\n"));
    let backup = CompositionBackup::decode(&bytes).unwrap();
    assert_eq!(backup.id(), format!("backup:{}", byte_digest(&bytes)));
    assert_eq!(backup.byte_length(), bytes.len());
    assert_unchanged(&original, &before);
    let target = TestDir::new();
    let plan = restore(&target, &backup);
    assert_eq!(
        plan.add,
        RecoveryCounts {
            sources: 1,
            capabilities: 1,
            revisions: 1,
            reviews: 1,
            recipes: 1,
            applications: 1,
            derivations: 1
        }
    );
    assert_eq!(target.store().open(&saved.source.id).unwrap(), saved);
    assert_eq!(
        target
            .store()
            .open_capability_v1(&output.capability_id, None)
            .unwrap(),
        original
            .store()
            .open_capability_v1(&output.capability_id, None)
            .unwrap()
    );
    assert!(
        target
            .store()
            .open_capability_v1(&preview.applied_outputs[1].capability_id, None)
            .is_err()
    );
    assert_eq!(target.store().export_composition_backup().unwrap(), bytes);
    let target_before = fs::read(target.store().path()).unwrap();
    assert_eq!(restore(&target, &backup).add, RecoveryCounts::default());
    assert_unchanged(&target, &target_before);
}

#[test]
fn canonical_archive_does_not_depend_on_sqlite_insertion_order() {
    let a = TestDir::new();
    composed(&a, &[0, 1]);
    let b = TestDir::new();
    composed(&b, &[1, 0]);
    assert_eq!(
        a.store().export_composition_backup().unwrap(),
        b.store().export_composition_backup().unwrap()
    );
}

#[test]
fn additive_restore_preserves_local_times_and_reviews_for_shared_identities() {
    let incoming = TestDir::new();
    composed(&incoming, &[0, 1]);
    let backup = v2(&incoming);
    let local = TestDir::new();
    let (_, preview) = composed(&local, &[0]);
    let db = local.db();
    db.execute("UPDATE snapshots SET saved_at_ms=12", [])
        .unwrap();
    db.execute("UPDATE compositions SET created_at_ms=13", [])
        .unwrap();
    db.execute("UPDATE composition_applications SET created_at_ms=14", [])
        .unwrap();
    db.execute(
        "INSERT INTO reviews VALUES (?1,'local_operator',15)",
        [&preview.applied_outputs[0].revision_id],
    )
    .unwrap();
    drop(db);
    let before = Records::load(&local.db()).unwrap();
    assert_eq!(restore(&local, &backup).add.capabilities, 1);
    let after = Records::load(&local.db()).unwrap();
    assert_eq!(
        after.sources.values().next().unwrap().metadata.saved_at_ms,
        12
    );
    assert_eq!(after.recipes.values().next().unwrap().created_at_ms, 13);
    assert_eq!(
        after.applications.values().next().unwrap().created_at_ms,
        14
    );
    let id = &preview.applied_outputs[0].capability_id;
    assert_eq!(
        after.detail(id, None).unwrap(),
        before.detail(id, None).unwrap()
    );
}

#[test]
fn old_archives_keep_their_meaning_and_add_to_composed_workspaces() {
    let legacy_dir = TestDir::new();
    let saved = source(&legacy_dir);
    let cap = legacy(&legacy_dir, &saved);
    let old_bytes = legacy_dir.store().export_backup().unwrap();
    let old = Backup::decode(&old_bytes).unwrap();
    let projected = CompositionBackup::decode(&old_bytes).unwrap();
    assert_eq!(old.id(), projected.id());
    let target = TestDir::new();
    let (_, existing) = composed(&target, &[0]);
    let preserved = target
        .store()
        .open_capability_v1(&existing.applied_outputs[0].capability_id, None)
        .unwrap();
    let plan = restore(&target, &projected);
    assert_eq!(plan.add.capabilities, 1);
    assert_eq!(plan.kept_sources, 1);
    assert_eq!(plan.add.recipes, 0);
    assert_eq!(
        target.store().open_capability_v1(&cap.id, None).unwrap(),
        CapabilityDetail::from(cap)
    );
    assert_eq!(
        target
            .store()
            .open_capability_v1(&preserved.id, None)
            .unwrap(),
        preserved
    );
    assert_eq!(legacy_dir.store().export_backup().unwrap(), old_bytes);
}

#[test]
fn restore_confirmation_binds_archive_state_and_counts_before_mutation() {
    let a = TestDir::new();
    composed(&a, &[0]);
    let backup = v2(&a);
    let b = TestDir::new();
    composed(&b, &[0, 1]);
    let swapped = v2(&b);
    let target = TestDir::new();
    let plan = target.store().prepare_composition_restore(&backup).unwrap();
    assert!(!target.0.exists());
    assert_eq!(
        target.store().restore_composition_backup(&swapped, &plan),
        Err(StoreError::WorkspaceChanged)
    );
    assert!(!target.0.exists());
    let saved = source(&target);
    let plan = target.store().prepare_composition_restore(&backup).unwrap();
    let before = fs::read(target.store().path()).unwrap();
    let mut wrong = plan.clone();
    wrong.add.revisions += 1;
    assert_eq!(
        target.store().restore_composition_backup(&backup, &wrong),
        Err(StoreError::WorkspaceChanged)
    );
    assert_unchanged(&target, &before);
    legacy(&target, &saved);
    let changed = fs::read(target.store().path()).unwrap();
    assert_eq!(
        target.store().restore_composition_backup(&backup, &plan),
        Err(StoreError::WorkspaceChanged)
    );
    assert_unchanged(&target, &changed);
}

#[test]
fn missing_pinned_history_blocks_additive_restore_but_newer_retained_history_works() {
    let original = TestDir::new();
    let saved = source(&original);
    let cap = legacy(&original, &saved);
    let initial = Backup::decode(&original.store().export_backup().unwrap()).unwrap();
    let input = original
        .store()
        .revise_capability(&cap.id, &cap.revision.id, "Pinned", "Saved input\r\n")
        .unwrap()
        .capability;
    let pinned = Backup::decode(&original.store().export_backup().unwrap()).unwrap();
    let (request, inputs) = split_request(&CapabilityDetail::from(input.clone()), 1);
    let db = original.db();
    schema3(&db);
    let composition = retain_outputs(&db, &request, &inputs, &[], &[0]);
    let backup = v2(&original);
    let missing = TestDir::new();
    let plan = missing.store().prepare_restore(&initial).unwrap();
    missing
        .store()
        .restore_backup(&initial, &plan.expected_state_id)
        .unwrap();
    let before = fs::read(missing.store().path()).unwrap();
    assert!(matches!(
        missing.store().prepare_composition_restore(&backup),
        Err(StoreError::CompositionDependencyMissing)
    ));
    assert_unchanged(&missing, &before);
    let newer = TestDir::new();
    let plan = newer.store().prepare_restore(&pinned).unwrap();
    newer
        .store()
        .restore_backup(&pinned, &plan.expected_state_id)
        .unwrap();
    let latest = newer
        .store()
        .revise_capability(
            &cap.id,
            &input.revision.id,
            "Newer",
            "Keep this newer history",
        )
        .unwrap()
        .capability;
    let imported = restore(&newer, &backup);
    assert_eq!(imported.kept_capabilities, 1);
    assert_eq!(imported.add.capabilities, 1);
    assert_eq!(
        newer.store().open_capability_v1(&cap.id, None).unwrap(),
        CapabilityDetail::from(latest)
    );
    assert!(
        newer
            .store()
            .open_capability_v1(&composition.applied_outputs[0].capability_id, None)
            .is_ok()
    );
}

#[test]
fn deletion_distinguishes_own_history_from_surviving_external_dependencies() {
    let dir = TestDir::new();
    let (saved, root) = composed(&dir, &[0]);
    let cap = &root.applied_outputs[0].capability_id;
    let db = dir.db();
    let original = Records::load(&db).unwrap().detail(cap, None).unwrap();
    let (mut request, inputs) = split_request(&original, 1);
    request.targets[0] = Target::Append {
        capability_id: cap.clone(),
        expected_revision_id: original.revision.id.clone(),
    };
    retain_outputs(
        &db,
        &request,
        &inputs,
        &[TargetHead {
            capability_id: cap.clone(),
            revision_id: original.revision.id,
        }],
        &[0],
    );
    let latest = Records::load(&db).unwrap().detail(cap, None).unwrap();
    let (external_request, external_inputs) = split_request(&latest, 2);
    let other = retain_outputs(&db, &external_request, &external_inputs, &[], &[0]);
    let other_cap = &other.applied_outputs[0].capability_id;
    let blocked = dir
        .store()
        .inspect_composition_deletion(RecordKind::Capability, cap)
        .unwrap();
    assert_eq!(blocked.dependencies.len(), 1);
    assert_eq!(blocked.dependencies[0].id, *other_cap);
    let before = fs::read(dir.store().path()).unwrap();
    assert_eq!(
        dir.store().delete_composition_record(&blocked),
        Err(StoreError::RecordInUse)
    );
    assert_unchanged(&dir, &before);
    let remove_other = dir
        .store()
        .inspect_composition_deletion(RecordKind::Capability, other_cap)
        .unwrap();
    dir.store()
        .delete_composition_record(&remove_other)
        .unwrap();
    assert_eq!(
        dir.store().delete_composition_record(&blocked),
        Err(StoreError::WorkspaceChanged)
    );
    let remove_self = dir
        .store()
        .inspect_composition_deletion(RecordKind::Capability, cap)
        .unwrap();
    assert!(remove_self.dependencies.is_empty());
    assert_eq!(remove_self.remove.revisions, 2);
    assert_eq!(remove_self.remove.recipes, 2);
    dir.store().delete_composition_record(&remove_self).unwrap();
    let state = dir.store().composition_data().unwrap();
    assert_eq!(
        state.records,
        RecoveryCounts {
            sources: 1,
            ..RecoveryCounts::default()
        }
    );
    assert_eq!(dir.store().open(&saved.source.id).unwrap(), saved);
    let remove_source = dir
        .store()
        .inspect_composition_deletion(RecordKind::Source, &saved.source.id)
        .unwrap();
    dir.store()
        .delete_composition_record(&remove_source)
        .unwrap();
    assert_eq!(
        dir.store().composition_data().unwrap().records,
        RecoveryCounts::default()
    );
    assert_eq!(
        dir.db()
            .pragma_query_value::<i64, _>(None, "user_version", |r| r.get(0))
            .unwrap(),
        3
    );
}

#[test]
fn partial_deletion_keeps_shared_application_and_never_restores_missing_output() {
    let dir = TestDir::new();
    let (saved, outputs) = composed(&dir, &[0, 1]);
    let blocked = dir
        .store()
        .inspect_composition_deletion(RecordKind::Source, &saved.source.id)
        .unwrap();
    assert_eq!(blocked.dependencies.len(), 2);
    assert_eq!(
        dir.store().delete_composition_record(&blocked),
        Err(StoreError::RecordInUse)
    );
    let deleted = &outputs.applied_outputs[0].capability_id;
    let plan = dir
        .store()
        .inspect_composition_deletion(RecordKind::Capability, deleted)
        .unwrap();
    assert_eq!(plan.remove.recipes, 0);
    assert_eq!(plan.remove.applications, 0);
    assert_eq!(plan.remove.derivations, 1);
    dir.store().delete_composition_record(&plan).unwrap();
    let target = TestDir::new();
    restore(&target, &v2(&dir));
    assert!(target.store().open_capability_v1(deleted, None).is_err());
    assert_eq!(
        target
            .store()
            .composition_data()
            .unwrap()
            .records
            .capabilities,
        1
    );
}

fn replace_manifest(bytes: &[u8], edit: impl FnOnce(String) -> String) -> Vec<u8> {
    let offset = b"RANGOON-BACKUP-V2\n".len();
    let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
    let start = offset + 4;
    let manifest = edit(String::from_utf8(bytes[start..start + length].to_vec()).unwrap());
    let mut result = bytes[..offset].to_vec();
    result.extend_from_slice(&(manifest.len() as u32).to_be_bytes());
    result.extend_from_slice(manifest.as_bytes());
    result.extend_from_slice(&bytes[start + length..bytes.len() - 64]);
    let digest = byte_digest(&result);
    result.extend_from_slice(digest.as_bytes());
    result
}

#[test]
fn archive_rejects_noncanonical_missing_forged_and_oversized_descriptors() {
    let dir = TestDir::new();
    composed(&dir, &[0]);
    let bytes = dir.store().export_composition_backup().unwrap();
    let edits: [fn(String) -> String; 8] = [
        |s| format!(" {s}"),
        |s| s.replacen("\"owners\":", "\"unknown\":[],\"owners\":", 1),
        |s| s.replacen("\"parentRevisionId\":null,", "", 1),
        |s| s.replacen("\"review\":null", "\"review\":{}", 1),
        |s| s.replacen("\"byteLength\":52", "\"byteLength\":4294967295", 1),
        |s| {
            s.replacen(
                "\"transformationVersion\":\"0.1.0\"",
                "\"transformationVersion\":\"99.0.0\"",
                1,
            )
        },
        |s| s.replacen("\"outputIndex\":0", "\"outputIndex\":1", 1),
        |s| s.replacen("\"kind\":\"new\"", "\"kind\":\"new\",\"extra\":true", 1),
    ];
    for edit in edits {
        let forged = replace_manifest(&bytes, edit);
        assert_ne!(forged, bytes);
        assert!(matches!(
            CompositionBackup::decode(&forged),
            Err(StoreError::BackupInvalid)
        ));
    }
    for end in [0, 17, 21, bytes.len() - 1] {
        assert!(CompositionBackup::decode(&bytes[..end]).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(b'\n');
    assert!(CompositionBackup::decode(&trailing).is_err());
    let mut corrupt = bytes.clone();
    let n = corrupt.len();
    corrupt[n - 65] ^= 1;
    assert!(CompositionBackup::decode(&corrupt).is_err());
    // A recomputed unkeyed trailer cannot conceal inconsistent record identities.
    let recomputed = byte_digest(&corrupt[..n - 64]);
    corrupt[n - 64..].copy_from_slice(recomputed.as_bytes());
    assert!(CompositionBackup::decode(&corrupt).is_err());
}

#[test]
fn versioned_delete_on_legacy_workspace_does_not_migrate() {
    let dir = TestDir::new();
    let saved = source(&dir);
    let plan = dir
        .store()
        .inspect_composition_deletion(RecordKind::Source, &saved.source.id)
        .unwrap();
    let mut forged = plan.clone();
    forged.remove.revisions = 1;
    let before = fs::read(dir.store().path()).unwrap();
    assert_eq!(
        dir.store().delete_composition_record(&forged),
        Err(StoreError::WorkspaceChanged)
    );
    assert_unchanged(&dir, &before);
    dir.store().delete_composition_record(&plan).unwrap();
    assert_eq!(
        dir.db()
            .pragma_query_value::<i64, _>(None, "user_version", |r| r.get(0))
            .unwrap(),
        1
    );
}

#[test]
fn independent_python_vectors_pin_complete_archive_bytes() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/composition/backup-v2.json")).unwrap();
    for vector in fixture["vectors"].as_array().unwrap() {
        let dir = TestDir::new();
        if vector["name"] == "composed_partial_live_output" {
            composed(&dir, &[0]);
        }
        let actual = dir.store().export_composition_backup().unwrap();
        let hex = vector["archiveHex"].as_str().unwrap();
        let expected: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(
            actual.len() as u64,
            vector["archiveByteLength"].as_u64().unwrap()
        );
        let decoded = CompositionBackup::decode(&expected).unwrap();
        assert_eq!(decoded.id(), vector["archiveId"].as_str().unwrap());
        assert_eq!(
            decoded.records.owners.len() as u64,
            vector["expectedRecordCounts"]["owners"].as_u64().unwrap()
        );
        assert_eq!(
            decoded.records.revisions.len() as u64,
            vector["expectedRecordCounts"]["revisions"]
                .as_u64()
                .unwrap()
        );
        if vector["name"] == "empty" {
            assert!(!dir.0.exists());
        }
    }
}

fn canonical_ordinary_archive(count: usize, body: &[u8]) -> Vec<u8> {
    use rangoon_domain::capability::{capability_id, revision_id};
    use std::collections::BTreeMap;
    let mut sources = BTreeMap::new();
    let mut owners = BTreeMap::new();
    let mut revisions = BTreeMap::new();
    for index in 0..count {
        let report = analyze(&format!("source-{index:03}.md"), body).unwrap();
        let fragment = &report.fragments[0];
        let title = format!("Skill {index:03}");
        let id = capability_id(&report.source.id, &fragment.id, &title);
        let revision = revision_id(&id, None, &title, &fragment.text);
        let birth = composition_records::Birth::Source {
            source_id: report.source.id.clone(),
            fragment_id: fragment.id.clone(),
        };
        let summary = rangoon_domain::capability_v1::RevisionSummary {
            id: revision.clone(),
            parent_revision_id: None,
            title,
            sha256: byte_digest(fragment.text.as_bytes()),
            created_at_ms: 1,
            review: None,
            provenance: rangoon_domain::capability_v1::RevisionProvenance::Ordinary {},
        };
        let encoded_id = serde_json::to_string(&id).unwrap();
        owners.insert(
            id,
            format!(
                "{{\"id\":{encoded_id},\"birth\":{},\"latestRevisionId\":{}}}",
                serde_json::to_string(&birth).unwrap(),
                serde_json::to_string(&revision).unwrap()
            ),
        );
        revisions.insert(
            revision,
            (
                format!(
                    "{{\"capabilityId\":{encoded_id},\"revision\":{},\"byteLength\":{}}}",
                    serde_json::to_string(&summary).unwrap(),
                    fragment.text.len()
                ),
                fragment.text.clone(),
            ),
        );
        let metadata = SnapshotMetadata {
            source_id: report.source.id.clone(),
            display_name: report.source.display_name,
            sha256: report.source.sha256,
            byte_length: report.source.byte_length as u32,
            saved_at_ms: 1,
        };
        sources.insert(
            report.source.id,
            (
                serde_json::to_string(&metadata).unwrap(),
                report.source.content,
            ),
        );
    }
    let manifest = format!(
        "{{\"schemaVersion\":\"rangoon.backup.v2\",\"sources\":[{}],\"owners\":[{}],\"revisions\":[{}],\"recipes\":[],\"applications\":[]}}",
        sources
            .values()
            .map(|r| r.0.as_str())
            .collect::<Vec<_>>()
            .join(","),
        owners
            .values()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(","),
        revisions
            .values()
            .map(|r| r.0.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );
    let mut bytes = b"RANGOON-BACKUP-V2\n".to_vec();
    bytes.extend_from_slice(&(manifest.len() as u32).to_be_bytes());
    bytes.extend_from_slice(manifest.as_bytes());
    for (_, content) in sources.values().chain(revisions.values()) {
        bytes.extend_from_slice(content.as_bytes());
    }
    bytes.extend_from_slice(byte_digest(&bytes).as_bytes());
    bytes
}

#[test]
fn v2_raw_limit_still_rejects_sqlite_allocation_overhead() {
    let body = format!("{}\n", "x".repeat(16_383)).repeat(16);
    assert_eq!(body.len(), 256 * 1024);
    let small = canonical_ordinary_archive(2, body.as_bytes());
    assert_eq!(
        CompositionBackup::decode(&small)
            .unwrap()
            .records
            .owners
            .len(),
        2
    );
    let full = canonical_ordinary_archive(128, body.as_bytes());
    assert!(full.len() > 64 * 1024 * 1024 && full.len() <= MAX_BACKUP_BYTES);
    assert!(matches!(
        CompositionBackup::decode(&full),
        Err(StoreError::BackupInvalid)
    ));
}

#[test]
fn interrupted_public_recovery_preserves_schema_rows_reviews_and_heads() {
    let incoming = TestDir::new();
    composed(&incoming, &[0, 1]);
    let bytes = incoming.store().export_composition_backup().unwrap();
    let backup_path = incoming.0.join("recovery.rangoon");
    fs::write(&backup_path, &bytes).unwrap();
    for phase in ["after_migration", "after_restore_rows"] {
        let target = TestDir::new();
        let saved = source(&target);
        let cap = legacy(&target, &saved);
        target
            .store()
            .review_capability(&cap.id, &cap.latest_revision_id)
            .unwrap();
        let before = target.store().export_composition_backup().unwrap();
        fault_child(&target, &backup_path, phase, "crash", "");
        assert_eq!(target.store().export_composition_backup().unwrap(), before);
        assert_eq!(
            target
                .db()
                .pragma_query_value::<i64, _>(None, "user_version", |r| r.get(0))
                .unwrap(),
            2
        );
        assert_eq!(target.store().open(&saved.source.id).unwrap(), saved);
        assert!(
            target
                .store()
                .open_capability(&cap.id, None)
                .unwrap()
                .revision
                .review
                .is_some()
        );
        // The same archive remains usable after recovery, with no abandoned migration.
        restore(&target, &CompositionBackup::decode(&bytes).unwrap());
        assert_eq!(
            target
                .store()
                .composition_data()
                .unwrap()
                .records
                .capabilities,
            3
        );
    }

    let target = TestDir::new();
    let (_, application) = composed(&target, &[0, 1]);
    let before = target.store().export_composition_backup().unwrap();
    let id = &application.applied_outputs[0].capability_id;
    fault_child(&target, &backup_path, "after_delete_rows", "crash", id);
    assert_eq!(target.store().export_composition_backup().unwrap(), before);
    let plan = target
        .store()
        .inspect_composition_deletion(RecordKind::Capability, id)
        .unwrap();
    target.store().delete_composition_record(&plan).unwrap();
    assert_eq!(
        target
            .store()
            .composition_data()
            .unwrap()
            .records
            .capabilities,
        1
    );
}

#[test]
fn sqlite_full_during_public_restore_rolls_back_migration_and_rows() {
    let incoming = TestDir::new();
    let large = analyze(
        "large.md",
        format!(
            "# Larger source\n{}",
            format!("{}\n", "x".repeat(1023)).repeat(200)
        )
        .as_bytes(),
    )
    .unwrap();
    incoming.store().save(&large).unwrap();
    composed(&incoming, &[0, 1]);
    let bytes = incoming.store().export_composition_backup().unwrap();
    let backup_path = incoming.0.join("recovery.rangoon");
    fs::write(&backup_path, &bytes).unwrap();
    for phase in ["before_migration", "after_migration"] {
        let target = TestDir::new();
        let saved = source(&target);
        let cap = legacy(&target, &saved);
        target
            .store()
            .review_capability(&cap.id, &cap.latest_revision_id)
            .unwrap();
        let before = target.store().export_composition_backup().unwrap();
        fault_child(&target, &backup_path, phase, "full", "");
        assert_eq!(target.store().export_composition_backup().unwrap(), before);
        assert_eq!(
            target
                .db()
                .pragma_query_value::<i64, _>(None, "user_version", |r| r.get(0))
                .unwrap(),
            2
        );
        restore(&target, &CompositionBackup::decode(&bytes).unwrap());
        assert_eq!(target.store().open(&large.source.id).unwrap(), large);
    }
}

#[test]
#[ignore = "isolated child used by public recovery crash and SQLite-full tests"]
fn recovery_fault_child() {
    let workspace = Workspace::new(PathBuf::from(
        std::env::var_os("RANGOON_RECOVERY_TEST_WORKSPACE").unwrap(),
    ));
    let id = std::env::var("RANGOON_RECOVERY_TEST_DELETE_ID").unwrap();
    if !id.is_empty() {
        let plan = workspace
            .inspect_composition_deletion(RecordKind::Capability, &id)
            .unwrap();
        workspace.delete_composition_record(&plan).unwrap();
        panic!("delete reached commit instead of crash checkpoint");
    }
    let bytes = fs::read(std::env::var_os("RANGOON_RECOVERY_TEST_BACKUP").unwrap()).unwrap();
    let backup = CompositionBackup::decode(&bytes).unwrap();
    let plan = workspace.prepare_composition_restore(&backup).unwrap();
    let result = workspace.restore_composition_backup(&backup, &plan);
    assert_eq!(
        std::env::var("RANGOON_RECOVERY_TEST_MODE").as_deref(),
        Ok("full")
    );
    // The storage API intentionally maps SQLite errors to a fixed public diagnostic.
    assert_eq!(result, Err(StoreError::Unavailable));
}
