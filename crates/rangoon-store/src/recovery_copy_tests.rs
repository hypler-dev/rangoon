use super::*;
use crate::composition_records_tests::TestDir;
#[cfg(feature = "encrypted-sqlite")]
use crate::composition_records_tests::{fixture_request, legacy, source};

#[test]
fn plaintext_store_cannot_create_or_inspect_recovery_copy() {
    let root = TestDir::new();
    let backup = WorkspaceBackup::decode(&root.store().export_workflow_backup().unwrap()).unwrap();
    assert!(matches!(
        root.store().create_encrypted_recovery_copy(&backup),
        Err(StoreError::EncryptionUnavailable)
    ));
    assert!(matches!(
        root.store().inspect_encrypted_recovery_copy(4096),
        Err(StoreError::EncryptionUnavailable)
    ));
    assert!(!root.0.exists());
}

#[cfg(feature = "encrypted-sqlite")]
fn keyed(root: &TestDir, key: u8) -> Workspace {
    Workspace::encrypted(root.0.clone(), WorkspaceKey::from_bytes([key; 32])).unwrap()
}

#[test]
#[cfg(feature = "encrypted-sqlite")]
fn complete_records_recover_under_three_independent_keys_and_reopen() {
    use rangoon_workflow::records::{SaveIntent, prepare_revision, workflow_id_from_nonce};
    let seed = TestDir::new();
    let saved = source(&seed);
    let capability = legacy(&seed, &saved);
    seed.store()
        .review_capability(&capability.id, &capability.latest_revision_id)
        .unwrap();
    let revised = seed
        .store()
        .revise_capability(
            &capability.id,
            &capability.latest_revision_id,
            "Retained revision",
            "Synthetic retained content",
        )
        .unwrap()
        .capability;
    seed.store()
        .review_capability(&revised.id, &revised.latest_revision_id)
        .unwrap();
    let preview = seed
        .store()
        .preview_composition(&fixture_request())
        .unwrap();
    seed.store().apply_composition(&preview, true).unwrap();
    let mut parent = None;
    for title in ["Recovery draft", "Retained workflow revision"] {
        let definition = serde_json::to_vec(&serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":title,"nodes":[],"controlEdges":[],"dataEdges":[]})).unwrap();
        let revision = prepare_revision(
            &workflow_id_from_nonce(&[42; 32]),
            parent.as_deref(),
            SaveIntent::Draft,
            &definition,
            br#"{"positions":[]}"#,
        )
        .unwrap();
        let plan = seed.store().inspect_workflow_save(&revision).unwrap();
        seed.store()
            .save_workflow(&revision, &plan.expected_state_id)
            .unwrap();
        parent = Some(revision.id().to_owned());
    }
    let expected = seed.store().export_workflow_backup().unwrap();
    let backup = WorkspaceBackup::decode(&expected).unwrap();
    let source_root = TestDir::new();
    keyed(&source_root, 1)
        .create_encrypted_recovery_copy(&backup)
        .unwrap();
    let source_before = fs::read(source_root.0.join("workspace.sqlite3")).unwrap();
    let recovery_root = TestDir::new();
    let ciphertext = keyed(&recovery_root, 2)
        .create_encrypted_recovery_copy(&backup)
        .unwrap();
    assert!(
        !ciphertext
            .windows(16)
            .any(|bytes| bytes == b"SQLite format 3\0")
    );
    assert!(
        !ciphertext
            .windows(saved.source.content.len())
            .any(|b| b == saved.source.content.as_bytes())
    );
    let imported_root = TestDir::new();
    fs::create_dir(&imported_root.0).unwrap();
    fs::write(imported_root.0.join("workspace.sqlite3"), &ciphertext).unwrap();
    let recovered = keyed(&imported_root, 2)
        .inspect_encrypted_recovery_copy(ciphertext.len() as u64)
        .unwrap();
    let target_root = TestDir::new();
    let target = keyed(&target_root, 3);
    target.initialize_empty_encrypted().unwrap();
    let plan = target.prepare_workflow_restore(&recovered).unwrap();
    target.restore_workflow_backup(&recovered, &plan).unwrap();
    drop(target);
    assert_eq!(
        keyed(&target_root, 3).export_workflow_backup().unwrap(),
        expected
    );
    assert_eq!(
        fs::read(source_root.0.join("workspace.sqlite3")).unwrap(),
        source_before
    );
    assert!(keyed(&target_root, 2).workflow_data().is_err());
}

#[test]
#[cfg(feature = "encrypted-sqlite")]
fn wrong_key_tampering_trailing_bytes_pages_and_sidecars_are_rejected_read_only() {
    let source_root = TestDir::new();
    source(&source_root);
    let backup =
        WorkspaceBackup::decode(&source_root.store().export_workflow_backup().unwrap()).unwrap();
    let stage = TestDir::new();
    let ciphertext = keyed(&stage, 2)
        .create_encrypted_recovery_copy(&backup)
        .unwrap();
    assert!(
        keyed(&stage, 9)
            .inspect_encrypted_recovery_copy(ciphertext.len() as u64)
            .is_err()
    );
    assert!(
        keyed(&stage, 2)
            .create_encrypted_recovery_copy(&backup)
            .is_err()
    );
    for mutation in 0..4 {
        let mut altered = ciphertext.clone();
        match mutation {
            0 => altered[100] ^= 1,
            1 => altered.push(0),
            2 => altered.extend_from_slice(&[0; 4096]),
            _ => {
                altered[4096 + 100] ^= 1;
            }
        }
        let root = TestDir::new();
        fs::create_dir(&root.0).unwrap();
        fs::write(root.0.join("workspace.sqlite3"), &altered).unwrap();
        assert!(
            keyed(&root, 2)
                .inspect_encrypted_recovery_copy(altered.len() as u64)
                .is_err(),
            "mutation {mutation}"
        );
        assert_eq!(fs::read(root.0.join("workspace.sqlite3")).unwrap(), altered);
    }
    for suffix in ["-journal", "-wal", "-shm"] {
        let sidecar = stage.0.join(format!("workspace.sqlite3{suffix}"));
        fs::write(&sidecar, b"synthetic").unwrap();
        assert!(
            keyed(&stage, 2)
                .inspect_encrypted_recovery_copy(ciphertext.len() as u64)
                .is_err()
        );
        fs::remove_file(sidecar).unwrap();
    }
    assert_eq!(
        fs::read(stage.0.join("workspace.sqlite3")).unwrap(),
        ciphertext
    );
}
