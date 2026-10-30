//! Disposable existing-only capture and drift tests. No normal workspace/key.
use super::*;
use crate::composition_records_tests::TestDir;
#[cfg(unix)]
use crate::composition_records_tests::{fixture_request, legacy, source};

#[cfg(unix)]
fn populated(version: u32) -> TestDir {
    let directory = TestDir::new();
    let saved = source(&directory);
    if version >= 2 {
        let capability = legacy(&directory, &saved);
        directory
            .store()
            .review_capability(&capability.id, &capability.latest_revision_id)
            .unwrap();
    }
    if version >= 3 {
        let preview = directory
            .store()
            .preview_composition(&fixture_request())
            .unwrap();
        directory.store().apply_composition(&preview, true).unwrap();
    }
    if version == 4 {
        append_workflow(&directory, None);
    }
    directory
}

#[cfg(unix)]
fn append_workflow(
    directory: &TestDir,
    parent: Option<&str>,
) -> rangoon_workflow::records::WorkflowRevision {
    use rangoon_workflow::records::{SaveIntent, prepare_revision, workflow_id_from_nonce};
    let definition = serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":if parent.is_some() {"Next draft"} else {"First draft"},"nodes":[],"controlEdges":[],"dataEdges":[]});
    let record = prepare_revision(
        &workflow_id_from_nonce(&[91; 32]),
        parent,
        SaveIntent::Draft,
        &serde_json::to_vec(&definition).unwrap(),
        br#"{"positions":[]}"#,
    )
    .unwrap();
    let plan = directory.store().inspect_workflow_save(&record).unwrap();
    directory
        .store()
        .save_workflow(&record, &plan.expected_state_id)
        .unwrap();
    record
}

#[test]
#[cfg(unix)]
fn complete_existing_capture_preserves_all_versions_and_inventory_without_writes() {
    for version in 1..=4 {
        let directory = populated(version);
        let before = fs::read(directory.store().path()).unwrap();
        let mut capture = directory.store().capture_existing_snapshot().unwrap();
        assert_eq!(capture.schema_version().unwrap(), version);
        let counts = capture.inventory().unwrap();
        assert_eq!(counts.sources, 1);
        assert_eq!(counts.reviews, u32::from(version >= 2));
        assert_eq!(
            counts.capabilities,
            if version >= 3 {
                3
            } else {
                u32::from(version >= 2)
            }
        );
        assert_eq!(
            counts.revisions,
            if version >= 3 {
                3
            } else {
                u32::from(version >= 2)
            }
        );
        assert_eq!(counts.recipes, u32::from(version >= 3));
        assert_eq!(counts.applications, u32::from(version >= 3));
        assert_eq!(counts.derivations, if version >= 3 { 2 } else { 0 });
        assert_eq!(counts.workflows, u32::from(version == 4));
        assert_eq!(counts.workflow_revisions, u32::from(version == 4));
        assert!(capture.archive().unwrap().starts_with(if version == 4 {
            b"RANGOON-BACKUP-V3\n"
        } else {
            b"RANGOON-BACKUP-V2\n"
        }));
        assert_eq!(
            capture.digest().unwrap(),
            byte_digest(capture.archive().unwrap())
        );
        let decoded = WorkspaceBackup::decode(capture.archive().unwrap()).unwrap();
        assert_eq!(workflow_recovery::counts(&decoded.records), *counts);
        assert_eq!(
            encode(&decoded.records).unwrap(),
            capture.archive().unwrap()
        );
        directory
            .store()
            .revalidate_existing_capture(&mut capture)
            .unwrap();
        assert_eq!(fs::read(directory.store().path()).unwrap(), before);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }
}

#[test]
#[cfg(unix)]
fn missing_zero_byte_and_valid_initialized_empty_are_distinct() {
    let directory = TestDir::new();
    assert!(directory.store().capture_existing_snapshot().is_err());
    assert!(!directory.0.exists());
    // Preserve the older export's intentionally empty behavior.
    assert!(directory.store().export_workflow_backup().is_ok());
    fs::create_dir(&directory.0).unwrap();
    assert!(directory.store().capture_existing_snapshot().is_err());
    fs::write(directory.store().path(), []).unwrap();
    assert!(directory.store().capture_existing_snapshot().is_err());
    assert_eq!(fs::metadata(directory.store().path()).unwrap().len(), 0);
    let database = directory.db();
    database
        .execute_batch("PRAGMA page_size=4096; VACUUM;")
        .unwrap();
    drop(database);
    let before = fs::read(directory.store().path()).unwrap();
    let capture = directory.store().capture_existing_snapshot().unwrap();
    assert_eq!(capture.schema_version().unwrap(), 0);
    assert_eq!(
        capture.inventory().unwrap(),
        &WorkflowRecoveryCounts::default()
    );
    assert!(
        capture
            .archive()
            .unwrap()
            .starts_with(b"RANGOON-BACKUP-V2\n")
    );
    assert_eq!(fs::read(directory.store().path()).unwrap(), before);
}

#[test]
#[cfg(unix)]
fn same_size_same_modified_time_logical_change_invalidates_capture() {
    let directory = populated(2);
    let mut capture = directory.store().capture_existing_snapshot().unwrap();
    let counts = capture.inventory().unwrap().clone();
    let before = fs::metadata(directory.store().path()).unwrap();
    let digest = capture.digest().unwrap().to_owned();
    directory
        .db()
        .execute("UPDATE snapshots SET saved_at_ms=saved_at_ms+1", [])
        .unwrap();
    fs::File::open(directory.store().path())
        .unwrap()
        .set_modified(before.modified().unwrap())
        .unwrap();
    assert_eq!(
        fs::metadata(directory.store().path()).unwrap().len(),
        before.len()
    );
    let changed = directory.store().capture_existing_snapshot().unwrap();
    assert_eq!(changed.inventory().unwrap(), &counts);
    assert_ne!(changed.digest().unwrap(), digest);
    assert_eq!(
        directory.store().revalidate_existing_capture(&mut capture),
        Err(StoreError::WorkspaceChanged)
    );
    assert!(capture.archive().is_err());
    assert!(capture.inventory().is_err());
}

#[test]
#[cfg(unix)]
fn review_composition_and_workflow_history_drift_cannot_reuse_capture() {
    let directory = TestDir::new();
    let saved = source(&directory);
    let capability = legacy(&directory, &saved);
    let mut capture = directory.store().capture_existing_snapshot().unwrap();
    directory
        .store()
        .review_capability(&capability.id, &capability.latest_revision_id)
        .unwrap();
    assert!(
        directory
            .store()
            .revalidate_existing_capture(&mut capture)
            .is_err()
    );
    assert!(capture.archive().is_err());
    let mut capture = directory.store().capture_existing_snapshot().unwrap();
    let preview = directory
        .store()
        .preview_composition(&fixture_request())
        .unwrap();
    directory.store().apply_composition(&preview, true).unwrap();
    assert!(
        directory
            .store()
            .revalidate_existing_capture(&mut capture)
            .is_err()
    );
    let root = append_workflow(&directory, None);
    let mut capture = directory.store().capture_existing_snapshot().unwrap();
    append_workflow(&directory, Some(root.id()));
    assert!(
        directory
            .store()
            .revalidate_existing_capture(&mut capture)
            .is_err()
    );
    assert!(capture.schema_version().is_err());
}

#[test]
#[cfg(unix)]
fn replacement_disappearance_and_cancel_clear_retained_plaintext() {
    let directory = populated(1);
    let mut capture = directory.store().capture_existing_snapshot().unwrap();
    let original = fs::read(directory.store().path()).unwrap();
    fs::rename(directory.store().path(), directory.0.join("old.sqlite")).unwrap();
    fs::write(directory.store().path(), &original).unwrap();
    assert!(
        directory
            .store()
            .revalidate_existing_capture(&mut capture)
            .is_err()
    );
    assert!(capture.archive().is_err());
    fs::remove_file(directory.0.join("old.sqlite")).unwrap();
    let mut capture = directory.store().capture_existing_snapshot().unwrap();
    fs::remove_file(directory.store().path()).unwrap();
    assert!(
        directory
            .store()
            .revalidate_existing_capture(&mut capture)
            .is_err()
    );
    assert!(capture.archive().is_err());
    fs::write(directory.store().path(), &original).unwrap();
    let mut capture = directory.store().capture_existing_snapshot().unwrap();
    capture.cancel();
    assert!(capture.archive().is_err());
    assert!(
        directory
            .store()
            .revalidate_existing_capture(&mut capture)
            .is_err()
    );
}

#[test]
#[cfg(unix)]
fn another_root_cannot_revalidate_or_reuse_capture() {
    let original = populated(1);
    let other = populated(1);
    let mut capture = original.store().capture_existing_snapshot().unwrap();
    assert!(
        other
            .store()
            .revalidate_existing_capture(&mut capture)
            .is_err()
    );
    assert!(capture.archive().is_err());
    assert!(
        original
            .store()
            .revalidate_existing_capture(&mut capture)
            .is_err()
    );
}

#[test]
#[cfg(unix)]
fn mid_capture_source_substitution_or_removal_returns_no_snapshot() {
    for remove in [false, true] {
        let directory = populated(1);
        let path = directory.store().path();
        CAPTURE_PROBE.with(|probe| {
            *probe.borrow_mut() = Some(Box::new(move || {
                let bytes = fs::read(&path).unwrap();
                fs::remove_file(&path).unwrap();
                if !remove {
                    fs::write(&path, bytes).unwrap();
                }
            }))
        });
        assert!(directory.store().capture_existing_snapshot().is_err());
    }
}

#[test]
#[cfg(unix)]
fn sidecars_wal_busy_corrupt_and_over_capacity_are_refused_without_repair() {
    for suffix in ["-journal", "-wal", "-shm"] {
        let directory = populated(1);
        let path = directory.0.join(format!("workspace.sqlite3{suffix}"));
        fs::write(&path, b"untrusted sidecar").unwrap();
        let before = fs::read(directory.store().path()).unwrap();
        assert!(directory.store().capture_existing_snapshot().is_err());
        assert_eq!(fs::read(directory.store().path()).unwrap(), before);
        assert_eq!(fs::read(path).unwrap(), b"untrusted sidecar");
    }
    let directory = populated(1);
    let database = directory.db();
    database.execute_batch("BEGIN EXCLUSIVE").unwrap();
    assert!(directory.store().capture_existing_snapshot().is_err());
    database.execute_batch("ROLLBACK").unwrap();
    database.pragma_update(None, "journal_mode", "WAL").unwrap();
    let before = fs::read(directory.store().path()).unwrap();
    assert!(directory.store().capture_existing_snapshot().is_err());
    assert_eq!(fs::read(directory.store().path()).unwrap(), before);
    drop(database);
    let directory = populated(1);
    fs::write(directory.store().path(), b"not SQLite").unwrap();
    assert!(directory.store().capture_existing_snapshot().is_err());
    assert_eq!(fs::read(directory.store().path()).unwrap(), b"not SQLite");
    fs::OpenOptions::new()
        .write(true)
        .open(directory.store().path())
        .unwrap()
        .set_len(MAX_DATABASE_BYTES + 1)
        .unwrap();
    assert!(matches!(
        directory.store().capture_existing_snapshot(),
        Err(StoreError::Full)
    ));
}

#[test]
#[cfg(unix)]
fn links_and_denied_directory_do_not_become_empty_or_valid_capture() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let directory = populated(1);
    let alias = TestDir::new();
    symlink(&directory.0, &alias.0).unwrap();
    assert!(alias.store().capture_existing_snapshot().is_err());
    fs::remove_file(&alias.0).unwrap();
    let before = fs::read(directory.store().path()).unwrap();
    let outside = TestDir::new();
    fs::create_dir(&outside.0).unwrap();
    let linked = outside.0.join("linked.sqlite");
    fs::hard_link(directory.store().path(), &linked).unwrap();
    assert!(directory.store().capture_existing_snapshot().is_err());
    fs::remove_file(linked).unwrap();
    let permissions = fs::metadata(&directory.0).unwrap().permissions();
    fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o000)).unwrap();
    let denied = directory.store().capture_existing_snapshot();
    fs::set_permissions(&directory.0, permissions).unwrap();
    // A root test runner does not establish an access-denial case.
    use std::os::unix::fs::MetadataExt;
    if fs::metadata(&directory.0).unwrap().uid() != 0 {
        assert!(denied.is_err());
    }
    assert_eq!(fs::read(directory.store().path()).unwrap(), before);
}

#[test]
#[cfg(unix)]
fn capture_and_backup_debug_are_fixed_redactions() {
    let directory = TestDir::new();
    let report = analyze("AGENTS.md", b"# E1C5A_PRIVATE_MARKER\nNever print me.\n").unwrap();
    directory.store().save(&report).unwrap();
    let capture = directory.store().capture_existing_snapshot().unwrap();
    let backup = WorkspaceBackup::decode(capture.archive().unwrap()).unwrap();
    assert_eq!(
        format!("{capture:?}"),
        "ExistingWorkspaceCapture(<redacted>)"
    );
    assert_eq!(format!("{backup:?}"), "WorkspaceBackup(<redacted>)");
    assert!(!format!("{capture:?}{backup:?}").contains("E1C5A_PRIVATE_MARKER"));
}

#[test]
#[cfg(unix)]
fn unsupported_schema_and_excess_records_do_not_produce_partial_capture() {
    let directory = populated(1);
    directory
        .db()
        .pragma_update(None, "user_version", 99)
        .unwrap();
    let before = fs::read(directory.store().path()).unwrap();
    assert!(matches!(
        directory.store().capture_existing_snapshot(),
        Err(StoreError::UnsupportedSchema)
    ));
    assert_eq!(fs::read(directory.store().path()).unwrap(), before);
    let directory = populated(1);
    let database = directory.db();
    for index in 0..MAX_SNAPSHOTS {
        let report = analyze("AGENTS.md", format!("# Synthetic {index}\n").as_bytes()).unwrap();
        database
            .execute(
                "INSERT INTO snapshots VALUES (?1,?2,?3,?4,1)",
                params![
                    report.source.id,
                    report.source.display_name,
                    report.source.sha256,
                    report.source.content.as_bytes()
                ],
            )
            .unwrap();
    }
    drop(database);
    let before = fs::read(directory.store().path()).unwrap();
    assert!(directory.store().capture_existing_snapshot().is_err());
    assert_eq!(fs::read(directory.store().path()).unwrap(), before);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
#[cfg(not(unix))]
fn existing_capture_refuses_without_supported_file_identity() {
    assert!(matches!(
        TestDir::new().store().capture_existing_snapshot(),
        Err(StoreError::Unavailable)
    ));
}
