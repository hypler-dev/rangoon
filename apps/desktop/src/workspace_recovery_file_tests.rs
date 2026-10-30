use super::*;
#[cfg(all(unix, feature = "encrypted-workspace"))]
use crate::encrypted_recovery as recovery;

fn code(result: DataResult) -> &'static str {
    match result {
        DataResult::Failed { error } => error.code,
        _ => panic!("expected failure"),
    }
}
#[test]
fn unsupported_backend_refuses_before_every_picker_or_file_callback() {
    let root = TestRoot::new("recovery-unsupported");
    let access = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));
    assert_eq!(
        code(
            access
                .begin_file()
                .unwrap()
                .create_recovery_key(|| panic!("no picker"))
                .deliver()
        ),
        "encrypted_recovery_unavailable"
    );
    assert_eq!(
        code(
            access
                .begin_file()
                .unwrap()
                .export_encrypted_backup(
                    || panic!("no key"),
                    || panic!("no output"),
                    |_, _| panic!("no write"),
                    |_| panic!("no read")
                )
                .deliver()
        ),
        "encrypted_recovery_unavailable"
    );
    assert_eq!(
        code(
            access
                .begin_file()
                .unwrap()
                .prepare_encrypted_restore(
                    || panic!("no backup"),
                    || panic!("no key"),
                    |_| panic!("no read")
                )
                .deliver()
        ),
        "encrypted_recovery_unavailable"
    );
    let result =
        serde_json::to_value(tauri::async_runtime::block_on(access.workflow_data())).unwrap();
    assert_eq!(result["encryptedRecoverySupported"], false);
}

#[cfg(all(unix, feature = "encrypted-workspace"))]
mod managed {
    use super::*;
    #[test]
    #[cfg(all(target_os = "macos", feature = "native-encrypted-qualification"))]
    fn qualification_reader_uses_existing_registered_request_not_host_fixture_bypass() {
        let root = TestRoot::new("recovery-reader-binding");
        let vault = ThreadVault::default();
        let (access, _) = encrypted_access(&root, &vault);
        let unbound = access.for_request(&crate::workspace_requests::WorkspaceRequest::Refused);
        assert_eq!(
            unbound.qualification_recovery_snapshot().unwrap_err().code,
            "workspace_unavailable"
        );
        let initial = access.0.gate.request_drain().unwrap();
        access.0.clients.resume(&access.0.gate, &initial).unwrap();
        let expected = snapshot(&access);
        let backup = Backup::decode(&expected).unwrap();
        let (id, length) = unbound.qualification_recovery_snapshot().unwrap();
        assert_eq!(id, backup.id());
        assert_eq!(length, expected.len());
        access.0.clients.revoke().unwrap();
        let opens = vault.0.lock().unwrap().opens;
        assert_eq!(
            unbound.qualification_recovery_snapshot().unwrap_err().code,
            "workspace_unavailable"
        );
        assert_eq!(vault.0.lock().unwrap().opens, opens);
    }
    #[test]
    fn source_capture_revoked_before_encode_creates_no_cipher_or_output() {
        let root = TestRoot::new("recovery-capture-drain");
        let vault = ThreadVault::default();
        let (access, _) = encrypted_access(&root, &vault);
        let key = root.0.join("key.rangoon-key");
        recovery::create_key(&key).unwrap();
        let output = root.0.join("copy.rangoon-encrypted");
        let gate = access.0.gate.clone();
        let holder = Arc::new(Mutex::new(None));
        let observed = Arc::clone(&holder);
        RECOVERY_CAPTURE_CHECKPOINT.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move || {
                *observed.lock().unwrap() = Some(gate.request_drain().unwrap());
            }))
        });
        let before = recovery::encode_calls();
        let result = access
            .begin_file()
            .unwrap()
            .export_encrypted_backup(
                || Ok(Some(key)),
                || Ok(Some(output.clone())),
                |_, _| panic!("revoked capture must not write"),
                |_| panic!("revoked capture must not read"),
            )
            .deliver();
        assert_eq!(code(result), "workspace_unavailable");
        assert_eq!(recovery::encode_calls(), before);
        assert!(!output.exists());
        assert!(
            holder
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .is_quiescent()
                .unwrap()
        );
    }
    #[test]
    #[cfg(all(target_os = "macos", feature = "native-encrypted-qualification"))]
    fn qualification_snapshot_refuses_capture_revoked_before_decode_or_emit() {
        let root = TestRoot::new("recovery-snapshot-drain");
        let vault = ThreadVault::default();
        let (access, _) = encrypted_access(&root, &vault);
        let initial = access.0.gate.request_drain().unwrap();
        access.0.clients.resume(&access.0.gate, &initial).unwrap();
        let gate = access.0.gate.clone();
        let holder = Arc::new(Mutex::new(None));
        let observed = Arc::clone(&holder);
        RECOVERY_CHECKPOINT.with(|cell| {
            *cell.borrow_mut() = Some(Box::new(move || {
                *observed.lock().unwrap() = Some(gate.request_drain().unwrap());
            }))
        });
        let error = access.qualification_recovery_snapshot().unwrap_err();
        assert_eq!(error.code, "workspace_unavailable");
        assert!(
            holder
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .is_quiescent()
                .unwrap()
        );
        assert!(!access.0.backup.retained_for_close_test());
    }
    #[test]
    fn source_drift_before_write_has_no_output_and_post_write_drift_is_uncertain() {
        for after_write in [false, true] {
            let root = TestRoot::new("recovery-source-drift");
            let vault = ThreadVault::default();
            let (access, _) = encrypted_access(&root, &vault);
            let key = root.0.join("key.rangoon-key");
            recovery::create_key(&key).unwrap();
            let path = root.0.join("backup.rangoon-encrypted");
            let changed = access.clone();
            let mutate = move || {
                changed
                    .0
                    .backend
                    .lock()
                    .unwrap()
                    .save(&report("changed.md", b"# External drift\nChanged source\n"))
                    .unwrap_or_else(|_| panic!("synthetic mutation failed"));
            };
            let mut mutate = Some(mutate);
            if !after_write {
                RECOVERY_CHECKPOINT
                    .with(|cell| *cell.borrow_mut() = Some(Box::new(mutate.take().unwrap())));
            }
            let result = access
                .begin_file()
                .unwrap()
                .export_encrypted_backup(
                    || Ok(Some(key)),
                    || Ok(Some(path.clone())),
                    |path, bytes| {
                        recovery::write_cipher(path, bytes)?;
                        if let Some(change) = mutate.take() {
                            change();
                        }
                        Ok(())
                    },
                    recovery::read_cipher,
                )
                .deliver();
            assert_eq!(
                code(result),
                if after_write {
                    "workspace_recovery_output_uncertain"
                } else {
                    "workspace_recovery_source_changed"
                }
            );
            assert_eq!(path.exists(), after_write);
            assert!(!access.0.backup.retained_for_close_test());
            assert!(!access.failure_latched());
        }
    }
    #[test]
    fn partial_write_failure_is_uncertain_and_key_close_discloses_secret_file() {
        let root = TestRoot::new("recovery-partial-write");
        let vault = ThreadVault::default();
        let (access, _) = encrypted_access(&root, &vault);
        let key = root.0.join("key.rangoon-key");
        let completed = access
            .begin_file()
            .unwrap()
            .create_recovery_key(|| Ok(Some(key.clone())));
        let drain = access.0.gate.request_drain().unwrap();
        assert!(!drain.is_quiescent().unwrap());
        let result = serde_json::to_value(completed.deliver()).unwrap();
        assert_eq!(result["error"]["code"], "workspace_recovery_key_created");
        assert!(
            result["error"]["message"]
                .as_str()
                .unwrap()
                .contains("secret recovery key file")
        );
        assert!(key.exists());
        assert!(drain.is_quiescent().unwrap());
        access.0.gate.resume(&drain).unwrap();
        let path = root.0.join("partial.rangoon-encrypted");
        let result = access
            .begin_file()
            .unwrap()
            .export_encrypted_backup(
                || Ok(Some(key)),
                || Ok(Some(path.clone())),
                |path, bytes| {
                    std::fs::write(path, &bytes[..100]).unwrap();
                    Err(PublicError {
                        code: "fixture_io",
                        message: "Synthetic partial write",
                    })
                },
                |_| panic!("failed write must not read"),
            )
            .deliver();
        assert_eq!(code(result), "workspace_recovery_output_uncertain");
        assert_eq!(fs::metadata(path).unwrap().len(), 100);
        assert!(!access.0.backup.retained_for_close_test());
        assert!(!access.failure_latched());
    }
    #[test]
    fn close_during_selected_output_read_refuses_before_decryption_and_drains() {
        let root = TestRoot::new("recovery-reread-drain");
        let vault = ThreadVault::default();
        let (access, _) = encrypted_access(&root, &vault);
        let key = root.0.join("key.rangoon-key");
        recovery::create_key(&key).unwrap();
        let path = root.0.join("copy.rangoon-encrypted");
        let gate = access.0.gate.clone();
        let mut drain = None;
        let before = recovery::decode_calls();
        let completed = access.begin_file().unwrap().export_encrypted_backup(
            || Ok(Some(key)),
            || Ok(Some(path.clone())),
            recovery::write_cipher,
            |path| {
                let bytes = recovery::read_cipher(path)?;
                drain = Some(gate.request_drain().unwrap());
                Ok(bytes)
            },
        );
        assert_eq!(recovery::decode_calls(), before);
        let drain = drain.unwrap();
        assert!(!drain.is_quiescent().unwrap());
        assert_eq!(
            code(completed.deliver()),
            "workspace_recovery_output_uncertain"
        );
        assert!(drain.is_quiescent().unwrap());
        assert!(path.exists());
    }
    fn export(access: &SourceAccess, key: &Path, path: &Path) -> DataResult {
        access
            .begin_file()
            .unwrap()
            .export_encrypted_backup(
                || Ok(Some(key.into())),
                || Ok(Some(path.into())),
                recovery::write_cipher,
                recovery::read_cipher,
            )
            .deliver()
    }
    fn prepare(access: &SourceAccess, key: &Path, path: &Path) -> DataResult {
        access
            .begin_file()
            .unwrap()
            .prepare_encrypted_restore(
                || Ok(Some(path.into())),
                || Ok(Some(key.into())),
                recovery::read_cipher,
            )
            .deliver()
    }
    fn snapshot(access: &SourceAccess) -> zeroize::Zeroizing<Vec<u8>> {
        let operation = access.begin_file().unwrap();
        zeroize::Zeroizing::new(
            operation
                .0
                .call(|backend| backend.capture_recovery_snapshot())
                .unwrap_or_else(|_| panic!("synthetic snapshot failed")),
        )
    }
    #[test]
    fn native_managed_backup_recovers_full_records_into_fresh_owner_and_survives_reopen() {
        use rangoon_desktop::workspace_session::Owner;
        let root = TestRoot::new("recovery-routed-source");
        let vault = ThreadVault::default();
        let (access, identity) = encrypted_access(&root, &vault);
        let source = report(
            "AGENTS.md",
            b"# Rules\nSynthetic routed recovery\n# Tests\nKeep exact history\n",
        );
        select(&access, &source);
        assert!(matches!(
            tauri::async_runtime::block_on(access.save_analysis(source.source.id.clone())),
            WorkspaceResult::Saved { .. }
        ));
        // Add full capability/composition/workflow records via the existing native Owner.
        let op = access.begin_file().unwrap();
        op.0.call(|backend| {
            let first=backend.create_capability(&source.source.id,&source.fragments[0].id,"Recovery skill")?.capability;
            let revised=backend.revise_capability(&first.id,&first.latest_revision_id,"Retained revision","Synthetic retained revision")?.capability;
            backend.review_capability(&revised.id,&revised.latest_revision_id)?;
            use rangoon_workflow::records::{SaveIntent,prepare_revision,workflow_id_from_nonce};
            let mut parent=None;
            for title in ["Recovery workflow","Retained workflow"]{
                let def=serde_json::to_vec(&serde_json::json!({"schemaVersion":"rangoon.workflow-definition.v1","title":title,"nodes":[],"controlEdges":[],"dataEdges":[]})).unwrap();
                let revision=prepare_revision(&workflow_id_from_nonce(&[111;32]),parent.as_deref(),SaveIntent::Draft,&def,br#"{"positions":[]}"#).unwrap();
                let plan=backend.inspect_workflow_save(&revision)?; backend.save_workflow(&revision,&plan.expected_state_id)?;parent=Some(revision.id().to_owned());
            }
            Ok(())
        }).unwrap_or_else(|_| panic!("synthetic authoring failed"));
        drop(op);
        let expected = snapshot(&access);
        let db = root
            .0
            .join("encrypted-workspaces")
            .join(&identity)
            .join("workspace.sqlite3");
        let source_cipher = fs::read(&db).unwrap();
        let key = root.0.join("independent.rangoon-key");
        assert!(matches!(
            access
                .begin_file()
                .unwrap()
                .create_recovery_key(|| Ok(Some(key.clone())))
                .deliver(),
            DataResult::RecoveryKeyCreated
        ));
        let path = root.0.join("portable.rangoon-encrypted");
        let receipt = serde_json::to_value(export(&access, &key, &path)).unwrap();
        assert_eq!(receipt["outcome"], "encrypted_exported");
        assert_eq!(receipt["format"], recovery::FORMAT);
        assert_eq!(receipt.as_object().unwrap().len(), 4);
        assert_eq!(fs::read(&db).unwrap(), source_cipher);
        // Original database key is gone; only independently selected recovery key remains.
        drop(access);
        vault.0.lock().unwrap().slots.clear();
        let target_root = TestRoot::new("recovery-fresh-target");
        let target_vault = ThreadVault::default();
        let (target, target_identity) = encrypted_access(&target_root, &target_vault);
        assert_ne!(identity, target_identity);
        let target_db = target_root
            .0
            .join("encrypted-workspaces")
            .join(&target_identity)
            .join("workspace.sqlite3");
        let before = fs::read(&target_db).unwrap();
        let plan = match prepare(&target, &key, &path) {
            DataResult::RestoreReady { plan } => plan,
            _ => panic!("expected preview"),
        };
        assert_eq!(fs::read(&target_db).unwrap(), before);
        assert!(matches!(
            tauri::async_runtime::block_on(target.restore_workspace_backup(
                plan.backup_id().into(),
                plan.expected_state_id().into()
            )),
            DataResult::Restored { .. }
        ));
        assert_eq!(*snapshot(&target), *expected);
        assert_ne!(fs::read(&target_db).unwrap(), source_cipher);
        assert_eq!(
            code(tauri::async_runtime::block_on(
                target.restore_workspace_backup(
                    plan.backup_id().into(),
                    plan.expected_state_id().into()
                )
            )),
            "backup_invalid"
        );
        drop(target);
        let mut owner = Owner::acquire(target_root.0.clone()).unwrap();
        let preview = owner.prepare_unlock().unwrap();
        owner.confirm(&preview.id, &target_vault).unwrap();
        assert_eq!(
            owner.export_workflow_backup(&target_vault).unwrap(),
            *expected
        );
    }

    #[test]
    fn bad_key_tamper_cancel_and_collision_leave_healthy_target_unchanged() {
        let root = TestRoot::new("recovery-input-errors");
        let vault = ThreadVault::default();
        let (access, identity) = encrypted_access(&root, &vault);
        let key = root.0.join("key.rangoon-key");
        recovery::create_key(&key).unwrap();
        let wrong = root.0.join("wrong.rangoon-key");
        recovery::create_key(&wrong).unwrap();
        let path = root.0.join("backup.rangoon-encrypted");
        assert!(matches!(
            export(&access, &key, &path),
            DataResult::EncryptedExported { .. }
        ));
        let db = root
            .0
            .join("encrypted-workspaces")
            .join(identity)
            .join("workspace.sqlite3");
        let before = fs::read(&db).unwrap();
        assert_eq!(
            code(prepare(&access, &wrong, &path)),
            "workspace_recovery_invalid"
        );
        assert!(!access.failure_latched());
        let cipher = fs::read(&path).unwrap();
        let mut altered = cipher.clone();
        altered[100] ^= 1;
        fs::write(&path, altered).unwrap();
        assert_eq!(
            code(prepare(&access, &key, &path)),
            "workspace_recovery_invalid"
        );
        assert!(!access.failure_latched());
        fs::write(&path, &cipher).unwrap();
        assert_eq!(
            code(export(&access, &key, &path)),
            "workspace_recovery_output_uncertain"
        );
        assert_eq!(fs::read(&path).unwrap(), cipher);
        assert!(matches!(
            access
                .begin_file()
                .unwrap()
                .prepare_encrypted_restore(|| Ok(None), || panic!("no key"), |_| panic!("no read"))
                .deliver(),
            DataResult::Cancelled
        ));
        assert!(matches!(
            access
                .begin_file()
                .unwrap()
                .export_encrypted_backup(
                    || Ok(None),
                    || panic!("no output"),
                    |_, _| panic!("no write"),
                    |_| panic!("no read")
                )
                .deliver(),
            DataResult::Cancelled
        ));
        assert_eq!(fs::read(db).unwrap(), before);
    }

    #[test]
    fn close_during_real_file_write_retains_lease_until_failure_and_never_reports_cancel() {
        let root = TestRoot::new("recovery-drain");
        let vault = ThreadVault::default();
        let (access, _) = encrypted_access(&root, &vault);
        let key = root.0.join("key.rangoon-key");
        recovery::create_key(&key).unwrap();
        let path = root.0.join("copy.rangoon-encrypted");
        let gate = access.0.gate.clone();
        let mut drain = None;
        let completed = access.begin_file().unwrap().export_encrypted_backup(
            || Ok(Some(key)),
            || Ok(Some(path.clone())),
            |path, bytes| {
                recovery::write_cipher(path, bytes)?;
                drain = Some(gate.request_drain().unwrap());
                Ok(())
            },
            recovery::read_cipher,
        );
        assert!(path.exists());
        let drain = drain.unwrap();
        assert!(!drain.is_quiescent().unwrap());
        assert_eq!(
            code(completed.deliver()),
            "workspace_recovery_output_uncertain"
        );
        assert!(drain.is_quiescent().unwrap());
    }
}
