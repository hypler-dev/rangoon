use super::*;

use crate::data_controls::{DataResult, NativeRecordKind};
use crate::instruction_bundles::{self, BundleResult};
use rangoon_compile::Profile;
use std::fs;
#[cfg(feature = "encrypted-workspace")]
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::ipc::InvokeBody;

fn data_code(result: DataResult) -> &'static str {
    match result {
        DataResult::Failed { error } => error.code,
        _ => panic!("expected failed data result"),
    }
}

fn host_error() -> PublicError {
    PublicError {
        code: "fixture_file_error",
        message: "synthetic file callback error",
    }
}

fn export_request(
    detail: &CapabilityDetail,
    candidate_id: &str,
) -> instruction_bundles::ExportRequest {
    let value = serde_json::json!({
        "schemaVersion": "rangoon.instruction-bundle-export-request.v1",
        "capabilityId": detail.id,
        "revisionId": detail.revision.id,
        "profile": "agents_md_v1",
        "expectedCandidateId": candidate_id,
    });
    instruction_bundles::decode_export(&InvokeBody::Raw(serde_json::to_vec(&value).unwrap()))
        .unwrap()
}

#[test]
fn instruction_export_and_inspection_use_exact_reviewed_saved_revision() {
    let root = TestRoot::new("data-access-instruction");
    let source = report("instruction.md", "# Rules\r\nExact 🦀 bytes\r\n".as_bytes());
    let workspace = Workspace::new(root.0.clone());
    workspace.save_v1(&source).unwrap();
    let detail = workspace
        .create_capability_v1(&source.source.id, &source.fragments[0].id, "Rules")
        .unwrap()
        .capability;
    let detail = workspace
        .review_capability_v1(&detail.id, &detail.latest_revision_id)
        .unwrap()
        .capability;
    let compilation = workspace
        .compile_capability(&detail.id, &detail.latest_revision_id, Profile::AgentsMdV1)
        .unwrap();
    let candidate = compilation.compilation.candidate.as_ref().unwrap();
    let request = export_request(&detail, &candidate.candidate_id);
    let access = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));
    let path = root.0.join("exact.rangoon-instructions");
    let result = access
        .begin_file()
        .unwrap()
        .export_instruction(
            request,
            || Ok(Some(path.clone())),
            |path, bytes| fs::write(path, bytes).map_err(|_| host_error()),
        )
        .deliver();
    match result {
        BundleResult::Exported { candidate_id, .. } => {
            assert_eq!(candidate_id, candidate.candidate_id)
        }
        _ => panic!("expected instruction export"),
    }
    let bytes = fs::read(&path).unwrap();
    assert!(!bytes.is_empty());
    match access
        .begin_file()
        .unwrap()
        .inspect_instruction(
            || Ok(Some(path.clone())),
            |path| fs::read(path).map_err(|_| host_error()),
        )
        .deliver()
    {
        BundleResult::Inspected { inspection, .. } => {
            assert_eq!(
                inspection.compilation.artifact.content,
                source.source.content
            )
        }
        _ => panic!("expected instruction inspection"),
    }
}

#[test]
fn legacy_backup_restore_and_deletion_keep_exact_selected_bytes() {
    let source_root = TestRoot::new("data-access-source");
    let source = report(
        "data-access.md",
        "\u{feff}# Backup\r\nExact 🦀 bytes\r\n".as_bytes(),
    );
    let store = Workspace::new(source_root.0.clone());
    store.save_v1(&source).unwrap();
    let access = SourceAccess::legacy(Gate::new(), Ok(source_root.0.clone()));
    let backup_path = source_root.0.join("selected.rangoon-backup");

    match access
        .begin_file()
        .unwrap()
        .export_backup(
            || Ok(Some(backup_path.clone())),
            |path, bytes| fs::write(path, bytes).map_err(|_| host_error()),
        )
        .deliver()
    {
        DataResult::Exported { byte_length, .. } => assert!(byte_length > 0),
        _ => panic!("expected legacy backup export"),
    }
    let backup = fs::read(&backup_path).unwrap();
    assert!(!backup.is_empty());

    let target_root = TestRoot::new("data-access-target");
    let target = SourceAccess::legacy(Gate::new(), Ok(target_root.0.clone()));
    let plan = match target
        .begin_file()
        .unwrap()
        .prepare_restore(
            || Ok(Some(backup_path.clone())),
            |path| fs::read(path).map_err(|_| host_error()),
        )
        .deliver()
    {
        DataResult::RestoreReady { plan } => plan,
        _ => panic!("expected restore preview"),
    };
    assert_eq!(
        data_code(tauri::async_runtime::block_on(
            target.restore_workspace_backup(plan.backup_id().into(), "wrong-state".into())
        )),
        "workspace_changed"
    );
    assert!(matches!(
        tauri::async_runtime::block_on(
            target.restore_workspace_backup(
                plan.backup_id().into(),
                plan.expected_state_id().into(),
            )
        ),
        DataResult::Restored { .. }
    ));
    match tauri::async_runtime::block_on(target.open_snapshot(source.source.id.clone())) {
        SelectionResult::Analyzed { report } => assert_eq!(*report, source),
        _ => panic!("expected restored saved source"),
    }

    let deletion = match tauri::async_runtime::block_on(
        target.inspect_workspace_deletion(NativeRecordKind::Source, source.source.id.clone()),
    ) {
        DataResult::DeletionReady { plan } => plan,
        _ => panic!("expected source deletion preview"),
    };
    assert!(matches!(
        tauri::async_runtime::block_on(target.delete_workspace_record(
            NativeRecordKind::Source,
            source.source.id.clone(),
            deletion.expected_state_id().into(),
        )),
        DataResult::Deleted { .. }
    ));
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn managed_refuses_plaintext_export_before_picker_then_restores_and_deletes() {
    let source_root = TestRoot::new("data-access-managed-source");
    let source = report("managed-backup.md", b"# Managed backup\r\nExact bytes\r\n");
    let source_store = Workspace::new(source_root.0.clone());
    source_store.save_v1(&source).unwrap();
    let backup_path = source_root.0.join("managed.rangoon-backup");
    fs::write(&backup_path, source_store.export_workflow_backup().unwrap()).unwrap();

    let target_root = TestRoot::new("data-access-managed-target");
    let vault = ThreadVault::default();
    let (access, identity) = encrypted_access(&target_root, &vault);
    let database = target_root
        .0
        .join("encrypted-workspaces")
        .join(identity)
        .join("workspace.sqlite3");
    let before_export = fs::read(&database).unwrap();
    let opens_before_export = vault.0.lock().unwrap().opens;
    let picker_calls = AtomicUsize::new(0);
    assert_eq!(
        data_code(
            access
                .begin_file()
                .unwrap()
                .export_backup(
                    || {
                        picker_calls.fetch_add(1, Ordering::SeqCst);
                        Ok(Some(target_root.0.join("forbidden.rangoon-backup")))
                    },
                    |_, _| panic!("managed export must not write"),
                )
                .deliver(),
        ),
        "encrypted_recovery_unavailable"
    );
    assert_eq!(picker_calls.load(Ordering::SeqCst), 0);
    assert_eq!(vault.0.lock().unwrap().opens, opens_before_export);
    assert_eq!(fs::read(&database).unwrap(), before_export);

    let plan = match access
        .begin_file()
        .unwrap()
        .prepare_restore(
            || Ok(Some(backup_path.clone())),
            |path| fs::read(path).map_err(|_| host_error()),
        )
        .deliver()
    {
        DataResult::RestoreReady { plan } => plan,
        _ => panic!("expected managed restore preview"),
    };
    assert!(matches!(
        tauri::async_runtime::block_on(
            access.restore_workspace_backup(
                plan.backup_id().into(),
                plan.expected_state_id().into(),
            )
        ),
        DataResult::Restored { .. }
    ));
    match tauri::async_runtime::block_on(access.open_snapshot(source.source.id.clone())) {
        SelectionResult::Analyzed { report } => assert_eq!(*report, source),
        _ => panic!("expected exact source restored into encrypted workspace"),
    }
    let ciphertext = fs::read(&database).unwrap();
    assert!(!ciphertext.starts_with(b"SQLite format 3\0"));
    assert!(
        !ciphertext
            .windows(source.source.content.len())
            .any(|window| window == source.source.content.as_bytes())
    );
    let deletion = match tauri::async_runtime::block_on(
        access.inspect_workspace_deletion(NativeRecordKind::Source, source.source.id.clone()),
    ) {
        DataResult::DeletionReady { plan } => plan,
        _ => panic!("expected managed deletion preview"),
    };
    assert!(matches!(
        tauri::async_runtime::block_on(access.delete_workspace_record(
            NativeRecordKind::Source,
            source.source.id.clone(),
            deletion.expected_state_id().into(),
        )),
        DataResult::Deleted { .. }
    ));
}

#[test]
fn cancelled_or_stale_file_callbacks_do_not_retain_restore_plan() {
    let root = TestRoot::new("data-access-cancel");
    let access = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));
    assert!(matches!(
        access
            .begin_file()
            .unwrap()
            .prepare_restore(|| Ok(None), |_| panic!("cancel must not read"))
            .deliver(),
        DataResult::Cancelled
    ));
    assert_eq!(
        data_code(tauri::async_runtime::block_on(
            access.restore_workspace_backup("backup:missing".into(), "workspace:missing".into())
        )),
        "backup_invalid"
    );
}

#[test]
fn gate_close_before_file_callback_or_restore_confirmation_does_no_io_or_replay() {
    let root = TestRoot::new("data-access-stale");
    let gate = Gate::new();
    let access = SourceAccess::legacy(gate.clone(), Ok(root.0.clone()));
    let operation = access.begin_file().unwrap();
    let drain = gate.request_drain().unwrap();
    assert_eq!(
        data_code(
            operation
                .prepare_restore(
                    || panic!("closed gate must not open picker"),
                    |_| panic!("closed gate must not read"),
                )
                .deliver(),
        ),
        "workspace_unavailable"
    );
    assert!(drain.is_quiescent().unwrap());
    gate.resume(&drain).unwrap();

    let source = report("stale.md", b"# Stale\nExact bytes\n");
    let source_store = Workspace::new(root.0.clone());
    source_store.save_v1(&source).unwrap();
    let backup_path = root.0.join("stale.rangoon-backup");
    fs::write(&backup_path, source_store.export_workflow_backup().unwrap()).unwrap();
    let plan = match access
        .begin_file()
        .unwrap()
        .prepare_restore(
            || Ok(Some(backup_path.clone())),
            |path| fs::read(path).map_err(|_| host_error()),
        )
        .deliver()
    {
        DataResult::RestoreReady { plan } => plan,
        _ => panic!("expected retained restore plan"),
    };
    let before = fs::read(root.0.join("workspace.sqlite3")).unwrap();
    let drain = gate.request_drain().unwrap();
    assert!(drain.is_quiescent().unwrap());
    gate.resume(&drain).unwrap();
    assert_eq!(
        data_code(
            tauri::async_runtime::block_on(access.restore_workspace_backup(
                plan.backup_id().into(),
                plan.expected_state_id().into(),
            ))
        ),
        "workspace_unavailable"
    );
    assert_eq!(fs::read(root.0.join("workspace.sqlite3")).unwrap(), before);
}

#[test]
fn file_completions_hold_drain_and_report_completed_outputs_without_metadata() {
    let root = TestRoot::new("data-access-late-export");
    let store = Workspace::new(root.0.clone());
    let source = report(
        "late-secret.md",
        b"# Late export\nDo not expose this content\n",
    );
    store.save_v1(&source).unwrap();
    let skill = store
        .create_capability_v1(&source.source.id, &source.fragments[0].id, "Secret skill")
        .unwrap()
        .capability;
    let skill = store
        .review_capability_v1(&skill.id, &skill.latest_revision_id)
        .unwrap()
        .capability;
    let compiled = store
        .compile_capability(&skill.id, &skill.revision.id, Profile::AgentsMdV1)
        .unwrap();
    let request = export_request(
        &skill,
        &compiled.compilation.candidate.unwrap().candidate_id,
    );
    let gate = Gate::new();
    let access = SourceAccess::legacy(gate.clone(), Ok(root.0.clone()));
    let path = root.0.join("late.rangoon-instructions");
    let mut drain = None;
    let completed = access.begin_file().unwrap().export_instruction(
        request,
        || Ok(Some(path.clone())),
        |path, bytes| {
            rangoon_host::write_selected_instruction_bundle(path, bytes)?;
            drain = Some(gate.request_drain().unwrap());
            Ok(())
        },
    );
    let drain = drain.unwrap();
    assert!(path.exists());
    assert!(!drain.is_quiescent().unwrap());
    let result = serde_json::to_value(completed.deliver()).unwrap();
    assert_eq!(
        result["error"]["code"],
        "instruction_bundle_export_completed"
    );
    assert_eq!(result["outputState"], "uncertain");
    for field in ["bundleId", "candidateId", "sha256", "byteLength", "path"] {
        assert!(result.get(field).is_none());
    }
    assert!(!result.to_string().contains("Secret skill"));
    assert!(drain.is_quiescent().unwrap());
    gate.resume(&drain).unwrap();

    let path = root.0.join("late.rangoon-backup");
    let mut drain = None;
    let completed = access.begin_file().unwrap().export_backup(
        || Ok(Some(path.clone())),
        |path, bytes| {
            rangoon_host::write_selected_backup(path, bytes)?;
            drain = Some(gate.request_drain().unwrap());
            Ok(())
        },
    );
    let drain = drain.unwrap();
    assert!(!drain.is_quiescent().unwrap());
    assert_eq!(data_code(completed.deliver()), "workspace_export_completed");
    assert!(path.exists());
    assert!(drain.is_quiescent().unwrap());
}

#[test]
fn uncertain_file_failure_stays_uncertain_after_closure_and_panics_release_custody() {
    let root = TestRoot::new("data-access-uncertain-file");
    let store = Workspace::new(root.0.clone());
    let source = report("uncertain.md", b"# Uncertain\nExact content\n");
    store.save_v1(&source).unwrap();
    let skill = store
        .create_capability_v1(&source.source.id, &source.fragments[0].id, "Uncertain")
        .unwrap()
        .capability;
    let skill = store
        .review_capability_v1(&skill.id, &skill.revision.id)
        .unwrap()
        .capability;
    let compile = store
        .compile_capability(&skill.id, &skill.revision.id, Profile::AgentsMdV1)
        .unwrap();
    let request = export_request(&skill, &compile.compilation.candidate.unwrap().candidate_id);
    let gate = Gate::new();
    let access = SourceAccess::legacy(gate.clone(), Ok(root.0.clone()));
    let mut drain = None;
    let completed = access.begin_file().unwrap().export_instruction(
        request,
        || Ok(Some(root.0.join("partial"))),
        |_, _| {
            drain = Some(gate.request_drain().unwrap());
            Err(PublicError {
                code: "instruction_bundle_write_failed",
                message: "The selected write has an uncertain outcome.",
            })
        },
    );
    let drain = drain.unwrap();
    let result = serde_json::to_value(completed.deliver()).unwrap();
    assert_eq!(result["error"]["code"], "instruction_bundle_write_failed");
    assert_eq!(result["outputState"], "uncertain");
    assert!(drain.is_quiescent().unwrap());
    gate.resume(&drain).unwrap();
    let operation = access.begin_file().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        operation.export_backup(
            || Ok(Some(root.0.join("panic"))),
            |_, _| panic!("synthetic file callback panic"),
        );
    }));
    assert!(result.is_err());
    let drain = gate.request_drain().unwrap();
    assert!(drain.is_quiescent().unwrap());
}

#[test]
fn unobserved_restore_preview_purges_plan_and_closure_during_read_retains_no_bytes() {
    let root = TestRoot::new("data-access-unobserved");
    let store = Workspace::new(root.0.clone());
    store
        .save_v1(&report("unobserved.md", b"# Content\nExact\n"))
        .unwrap();
    let bytes = store.export_workflow_backup().unwrap();
    let backup = Backup::decode(&bytes).unwrap();
    let expected = store.prepare_workflow_restore(&backup).unwrap();
    let gate = Gate::new();
    let access = SourceAccess::legacy(gate.clone(), Ok(root.0.clone()));
    let completed = access
        .begin_file()
        .unwrap()
        .prepare_restore(|| Ok(Some(root.0.join("chosen"))), |_| Ok(bytes.clone()));
    let drain = gate.request_drain().unwrap();
    assert!(!drain.is_quiescent().unwrap());
    drop(completed);
    assert!(drain.is_quiescent().unwrap());
    gate.resume(&drain).unwrap();
    assert_eq!(
        data_code(tauri::async_runtime::block_on(
            access
                .restore_workspace_backup(backup.id().into(), expected.expected_state_id().into())
        )),
        "backup_invalid"
    );
    let mut drain = None;
    let completed = access.begin_file().unwrap().prepare_restore(
        || Ok(Some(root.0.join("chosen"))),
        |_| {
            drain = Some(gate.request_drain().unwrap());
            Ok(bytes)
        },
    );
    let drain = drain.unwrap();
    assert_eq!(data_code(completed.deliver()), "workspace_unavailable");
    assert!(drain.is_quiescent().unwrap());
}

#[test]
fn deletion_wrong_state_retains_plan_but_new_generation_and_replay_never_delete() {
    let root = TestRoot::new("data-access-deletion-handles");
    let store = Workspace::new(root.0.clone());
    let source = report("delete.md", b"# Delete\nExact\n");
    store.save_v1(&source).unwrap();
    let gate = Gate::new();
    let access = SourceAccess::legacy(gate.clone(), Ok(root.0.clone()));
    let plan = match tauri::async_runtime::block_on(
        access.inspect_workspace_deletion(NativeRecordKind::Source, source.source.id.clone()),
    ) {
        DataResult::DeletionReady { plan } => plan,
        _ => panic!("expected deletion plan"),
    };
    assert_eq!(
        data_code(tauri::async_runtime::block_on(
            access.delete_workspace_record(
                NativeRecordKind::Source,
                source.source.id.clone(),
                "wrong-state".into()
            )
        )),
        "workspace_changed"
    );
    assert!(store.open(&source.source.id).is_ok());
    let ticket = access.begin_capability(Lane::Exclusive).unwrap();
    let completed = ticket.data(DataOperation::Delete {
        kind: NativeRecordKind::Source,
        id: source.source.id.clone(),
        expected: plan.expected_state_id().into(),
    });
    let drain = gate.request_drain().unwrap();
    assert!(!drain.is_quiescent().unwrap());
    assert_eq!(data_code(completed.deliver()), "workspace_write_completed");
    assert!(drain.is_quiescent().unwrap());
    gate.resume(&drain).unwrap();
    assert_eq!(
        data_code(tauri::async_runtime::block_on(
            access.delete_workspace_record(
                NativeRecordKind::Source,
                source.source.id.clone(),
                plan.expected_state_id().into()
            )
        )),
        "workspace_changed"
    );
    assert!(store.open(&source.source.id).is_err());
    store.save_v1(&source).unwrap();
    let plan = match tauri::async_runtime::block_on(
        access.inspect_workspace_deletion(NativeRecordKind::Source, source.source.id.clone()),
    ) {
        DataResult::DeletionReady { plan } => plan,
        _ => panic!("expected deletion plan"),
    };
    let drain = gate.request_drain().unwrap();
    assert!(drain.is_quiescent().unwrap());
    gate.resume(&drain).unwrap();
    let before = fs::read(root.0.join("workspace.sqlite3")).unwrap();
    assert_eq!(
        data_code(tauri::async_runtime::block_on(
            access.delete_workspace_record(
                NativeRecordKind::Source,
                source.source.id.clone(),
                plan.expected_state_id().into()
            )
        )),
        "workspace_unavailable"
    );
    assert_eq!(fs::read(root.0.join("workspace.sqlite3")).unwrap(), before);
}

#[test]
fn picker_callback_keeps_lease_after_ipc_waiter_drop_and_wrappers_use_typed_access() {
    let root = TestRoot::new("data-access-picker-drain");
    let gate = Gate::new();
    let access = SourceAccess::legacy(gate.clone(), Ok(root.0.clone()));
    let operation = access.begin_file().unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let waiter = tauri::async_runtime::spawn_blocking(move || {
        let completed = operation.prepare_restore(
            || {
                started_tx.send(()).unwrap();
                release_rx.recv_timeout(WAIT).unwrap();
                Ok(None)
            },
            |_| panic!("cancelled picker must not read"),
        );
        done_tx.send(completed.deliver()).unwrap();
    });
    started_rx.recv_timeout(WAIT).unwrap();
    drop(waiter);
    let drain = gate.request_drain().unwrap();
    assert!(!drain.is_quiescent().unwrap());
    release_tx.send(()).unwrap();
    assert_eq!(
        data_code(done_rx.recv_timeout(WAIT).unwrap()),
        "workspace_unavailable"
    );
    assert!(drain.is_quiescent().unwrap());
    let bundles = include_str!("instruction_bundles.rs");
    let data = include_str!("data_controls.rs");
    let main = include_str!("main.rs");
    assert!(!bundles.contains("workspace(&app)"));
    assert!(!data.contains("workspace(&app)"));
    assert!(!data.contains("begin_operation("));
    assert!(bundles.contains(".begin_file()"));
    assert!(data.contains(".begin_file()"));
    assert!(!main.contains(".manage(BackupSession::default())"));
    assert!(!main.contains(".manage(DeletionSession::default())"));
}
