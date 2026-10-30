//! Disposable strict encrypted-destination transfer tests.
use super::*;
use crate::composition_records_tests::TestDir;

#[test]
#[cfg(unix)]
fn source_fingerprint_matches_root_to_file_identity_and_is_unavailable_after_cancel() {
    use std::os::unix::fs::MetadataExt;
    let source = TestDir::new();
    crate::composition_records_tests::source(&source);
    let mut capture = source.store().capture_existing_snapshot().unwrap();
    let paths: Vec<_> = source.0.ancestors().collect();
    let mut expected = b"rangoon.legacy-identity.v1\0".to_vec();
    expected.push(u8::try_from(paths.len()).unwrap());
    let database = source.store().path();
    for path in paths
        .into_iter()
        .rev()
        .chain(std::iter::once(database.as_path()))
    {
        let metadata = fs::symlink_metadata(path).unwrap();
        expected.extend_from_slice(&metadata.dev().to_be_bytes());
        expected.extend_from_slice(&metadata.ino().to_be_bytes());
    }
    assert_eq!(
        capture.identity_fingerprint().unwrap(),
        rangoon_domain::byte_digest(&expected)
    );
    capture.cancel();
    assert!(capture.identity_fingerprint().is_err());
}

#[test]
fn plaintext_target_cannot_prepare_strict_transfer_or_create_data() {
    let target = TestDir::new();
    let source = TestDir::new();
    let archive =
        WorkspaceBackup::decode(&source.store().export_workflow_backup().unwrap()).unwrap();
    assert!(matches!(
        target.store().prepare_empty_workflow_restore(&archive, 0),
        Err(StoreError::EncryptionUnavailable)
    ));
    assert!(!target.0.exists());
}

#[cfg(all(unix, feature = "encrypted-sqlite"))]
fn fixture(schema: u32, populated: bool) -> TestDir {
    use crate::composition_records_tests::{fixture_request, legacy, source};
    let root = TestDir::new();
    if populated {
        let saved = source(&root);
        if schema >= 2 {
            let capability = legacy(&root, &saved);
            root.store()
                .review_capability(&capability.id, &capability.latest_revision_id)
                .unwrap();
            let revised = root
                .store()
                .revise_capability(
                    &capability.id,
                    &capability.latest_revision_id,
                    "Retained history",
                    "Reviewed historical content.\r\n",
                )
                .unwrap()
                .capability;
            root.store()
                .review_capability(&revised.id, &revised.latest_revision_id)
                .unwrap();
        }
        if schema >= 3 {
            let preview = root
                .store()
                .preview_composition(&fixture_request())
                .unwrap();
            root.store().apply_composition(&preview, true).unwrap();
        }
        if schema == 4 {
            use rangoon_workflow::records::{SaveIntent, prepare_revision, workflow_id_from_nonce};
            let record = prepare_revision(&workflow_id_from_nonce(&[87; 32]), None, SaveIntent::Draft, br#"{"schemaVersion":"rangoon.workflow-definition.v1","title":"Adoption draft","nodes":[],"controlEdges":[],"dataEdges":[]}"#, br#"{"positions":[]}"#).unwrap();
            let plan = root.store().inspect_workflow_save(&record).unwrap();
            root.store()
                .save_workflow(&record, &plan.expected_state_id)
                .unwrap();
            let next = prepare_revision(&workflow_id_from_nonce(&[87;32]), Some(record.id()), SaveIntent::Draft, br#"{"schemaVersion":"rangoon.workflow-definition.v1","title":"Retained workflow history","nodes":[],"controlEdges":[],"dataEdges":[]}"#, br#"{"positions":[]}"#).unwrap();
            let plan = root.store().inspect_workflow_save(&next).unwrap();
            root.store()
                .save_workflow(&next, &plan.expected_state_id)
                .unwrap();
        }
    } else {
        fs::create_dir(&root.0).unwrap();
        let database = root.db();
        database
            .execute_batch("PRAGMA page_size=4096; VACUUM;")
            .unwrap();
        if schema > 0 {
            workflow_mutations::ensure_schema(&database, i64::from(schema)).unwrap();
        }
    }
    root
}

#[cfg(feature = "encrypted-sqlite")]
fn encrypted(root: &TestDir) -> Workspace {
    Workspace::encrypted(root.0.clone(), WorkspaceKey::from_bytes([89; 32])).unwrap()
}

#[test]
#[cfg(all(unix, feature = "encrypted-sqlite"))]
fn exact_transfer_preserves_all_empty_and_populated_schemas_and_records() {
    for (schema, populated) in (0..=4)
        .map(|s| (s, false))
        .chain((1..=4).map(|s| (s, true)))
    {
        let source = fixture(schema, populated);
        let before = fs::read(source.store().path()).unwrap();
        let capture = source.store().capture_existing_snapshot().unwrap();
        let backup = WorkspaceBackup::decode(capture.archive().unwrap()).unwrap();
        let target = TestDir::new();
        let store = encrypted(&target);
        store.initialize_empty_encrypted().unwrap();
        let plan = store
            .prepare_empty_workflow_restore(&backup, schema)
            .unwrap();
        assert_eq!(format!("{plan:?}"), "EmptyWorkflowRestorePlan(<redacted>)");
        store.restore_empty_workflow_backup(&backup, &plan).unwrap();
        assert_eq!(
            store.export_workflow_backup().unwrap(),
            capture.archive().unwrap()
        );
        let database = store.connect_complete_read_only(true).unwrap().unwrap();
        let actual: u32 = database
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(actual, schema.max(1));
        drop(database);
        assert_eq!(
            encrypted(&target).export_workflow_backup().unwrap(),
            capture.archive().unwrap()
        );
        assert_eq!(fs::read(source.store().path()).unwrap(), before);
        for capability in store.list_capabilities_v1().unwrap() {
            assert_eq!(
                serde_json::to_value(store.open_capability_v1(&capability.id, None).unwrap())
                    .unwrap(),
                serde_json::to_value(
                    source
                        .store()
                        .open_capability_v1(&capability.id, None)
                        .unwrap()
                )
                .unwrap()
            );
            for profile in [
                rangoon_compile::Profile::AgentsMdV1,
                rangoon_compile::Profile::ClaudeMdV1,
            ] {
                let actual = store
                    .compile_capability(&capability.id, &capability.latest_revision_id, profile)
                    .unwrap();
                let expected = source
                    .store()
                    .compile_capability(&capability.id, &capability.latest_revision_id, profile)
                    .unwrap();
                assert_eq!(
                    serde_json::to_value(&actual).unwrap(),
                    serde_json::to_value(&expected).unwrap()
                );
                assert_eq!(
                    rangoon_compile::encode_bundle(&actual.compilation),
                    rangoon_compile::encode_bundle(&expected.compilation)
                );
            }
        }
        assert!(
            !fs::read(store.path())
                .unwrap()
                .starts_with(b"SQLite format 3")
        );
    }
}

#[test]
#[cfg(all(unix, feature = "encrypted-sqlite"))]
fn strict_transfer_keeps_shared_application_with_missing_output() {
    use crate::composition_records_tests::{
        fixture_request, resolved_source, retain_outputs, schema3, source,
    };
    let origin = TestDir::new();
    let saved = source(&origin);
    let database = origin.db();
    schema3(&database);
    retain_outputs(
        &database,
        &fixture_request(),
        &resolved_source(&saved),
        &[],
        &[0],
    );
    drop(database);
    let capture = origin.store().capture_existing_snapshot().unwrap();
    assert_eq!(capture.inventory().unwrap().recipes, 1);
    assert_eq!(capture.inventory().unwrap().applications, 1);
    assert_eq!(capture.inventory().unwrap().capabilities, 1);
    let backup = WorkspaceBackup::decode(capture.archive().unwrap()).unwrap();
    let target = TestDir::new();
    let store = encrypted(&target);
    store.initialize_empty_encrypted().unwrap();
    let plan = store.prepare_empty_workflow_restore(&backup, 3).unwrap();
    store.restore_empty_workflow_backup(&backup, &plan).unwrap();
    assert_eq!(
        store.export_workflow_backup().unwrap(),
        capture.archive().unwrap()
    );
}

#[test]
#[cfg(all(unix, feature = "encrypted-sqlite"))]
fn source_schema_must_represent_complete_archive_without_silent_downconversion() {
    let origin = fixture(4, true);
    let capture = origin.store().capture_existing_snapshot().unwrap();
    let backup = WorkspaceBackup::decode(capture.archive().unwrap()).unwrap();
    let target = TestDir::new();
    let store = encrypted(&target);
    store.initialize_empty_encrypted().unwrap();
    let before = fs::read(store.path()).unwrap();
    for schema in [0, 1, 2, 3, 5, u32::MAX] {
        assert!(
            store
                .prepare_empty_workflow_restore(&backup, schema)
                .is_err()
        );
        assert_eq!(fs::read(store.path()).unwrap(), before);
    }
}

#[test]
#[cfg(all(unix, feature = "encrypted-sqlite"))]
fn strict_transaction_full_rolls_back_and_kills_refuse_uncertain_destination() {
    for (phase, mode) in [
        ("before_workflow_restore_migration", "full"),
        ("after_workflow_restore_migration", "full"),
        ("before_workflow_restore_migration", "crash"),
        ("after_workflow_restore_migration", "crash"),
        ("after_workflow_restore_rows", "crash"),
    ] {
        let source = fixture(4, true);
        let large = format!(
            "# Large synthetic source\n{}",
            format!("{}\n", "x".repeat(2000)).repeat(90)
        );
        source
            .store()
            .save_v1(&analyze("SKILL.md", large.as_bytes()).unwrap())
            .unwrap();
        let capture = source.store().capture_existing_snapshot().unwrap();
        let target = TestDir::new();
        let store = encrypted(&target);
        store.initialize_empty_encrypted().unwrap();
        let before = store.export_workflow_backup().unwrap();
        let input = target.0.join("synthetic-archive");
        fs::write(&input, capture.archive().unwrap()).unwrap();
        crate::composition_recovery_tests::fault_child_for(
            &target,
            &input,
            phase,
            mode,
            "unused",
            "workflow_recovery::adoption_tests::strict_transaction_child",
        );
        if mode == "full" || phase == "before_workflow_restore_migration" {
            assert_eq!(encrypted(&target).export_workflow_backup().unwrap(), before);
            assert_eq!(
                encrypted(&target).workflow_data().unwrap().records,
                WorkflowRecoveryCounts::default()
            );
        } else {
            // A killed spilled transaction leaves an uncertain encrypted journal.
            // Existing readers refuse it; this packet must not recover/repair it.
            assert!(matches!(
                encrypted(&target).export_workflow_backup(),
                Err(StoreError::EncryptedStoreInvalid)
            ));
            assert!(encrypted(&target).workflow_data().is_err());
            let backup = WorkspaceBackup::decode(capture.archive().unwrap()).unwrap();
            assert!(
                encrypted(&target)
                    .prepare_empty_workflow_restore(&backup, 4)
                    .is_err()
            );
        }
    }
}

#[test]
#[ignore = "Disposable encrypted store child driven only by its parent"]
#[cfg(all(unix, feature = "encrypted-sqlite"))]
fn strict_transaction_child() {
    let root = PathBuf::from(std::env::var_os("RANGOON_RECOVERY_TEST_WORKSPACE").unwrap());
    assert!(root.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    assert!(
        root.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("rangoon-composition-records-")
    );
    let store = Workspace::encrypted(root.clone(), WorkspaceKey::from_bytes([89; 32])).unwrap();
    let backup =
        WorkspaceBackup::decode(&fs::read(root.join("synthetic-archive")).unwrap()).unwrap();
    let plan = store.prepare_empty_workflow_restore(&backup, 4).unwrap();
    let result = store.restore_empty_workflow_backup(&backup, &plan);
    assert_eq!(
        std::env::var("RANGOON_RECOVERY_TEST_MODE").as_deref(),
        Ok("full")
    );
    assert!(
        matches!(result, Err(StoreError::Full)),
        "actual SQLite-full result required"
    );
}

#[test]
#[cfg(all(unix, feature = "encrypted-sqlite"))]
fn strict_transfer_refuses_absent_nonempty_stale_foreign_and_changed_backup_targets() {
    let source = fixture(2, true);
    let capture = source.store().capture_existing_snapshot().unwrap();
    let backup = WorkspaceBackup::decode(capture.archive().unwrap()).unwrap();
    let absent = TestDir::new();
    assert!(
        encrypted(&absent)
            .prepare_empty_workflow_restore(&backup, 2)
            .is_err()
    );
    assert!(!absent.0.exists());
    let target = TestDir::new();
    let store = encrypted(&target);
    store.initialize_empty_encrypted().unwrap();
    let plan = store.prepare_empty_workflow_restore(&backup, 2).unwrap();
    let foreign = TestDir::new();
    let other = encrypted(&foreign);
    other.initialize_empty_encrypted().unwrap();
    assert!(other.restore_empty_workflow_backup(&backup, &plan).is_err());
    let empty = WorkspaceBackup::decode(
        &Workspace::new(TestDir::new().0.clone())
            .export_workflow_backup()
            .unwrap(),
    )
    .unwrap();
    assert!(store.restore_empty_workflow_backup(&empty, &plan).is_err());
    store
        .save(&analyze("AGENTS.md", b"# Target changed\n").unwrap())
        .unwrap();
    let before = store.export_workflow_backup().unwrap();
    assert!(store.prepare_empty_workflow_restore(&backup, 2).is_err());
    assert!(store.restore_empty_workflow_backup(&backup, &plan).is_err());
    assert_eq!(store.export_workflow_backup().unwrap(), before);
}

#[test]
#[cfg(all(unix, feature = "encrypted-sqlite"))]
fn additive_noop_restore_keeps_schema_while_strict_transfer_preserves_empty_higher_schema() {
    let source = fixture(4, false);
    let backup = WorkspaceBackup::decode(
        source
            .store()
            .capture_existing_snapshot()
            .unwrap()
            .archive()
            .unwrap(),
    )
    .unwrap();
    let target = TestDir::new();
    let store = encrypted(&target);
    store.initialize_empty_encrypted().unwrap();
    let before = store.export_workflow_backup().unwrap();
    let plan = store.prepare_workflow_restore(&backup).unwrap();
    store.restore_workflow_backup(&backup, &plan).unwrap();
    assert_eq!(store.export_workflow_backup().unwrap(), before);
    let strict = store.prepare_empty_workflow_restore(&backup, 4).unwrap();
    store
        .restore_empty_workflow_backup(&backup, &strict)
        .unwrap();
    assert_ne!(store.export_workflow_backup().unwrap(), before);
}
