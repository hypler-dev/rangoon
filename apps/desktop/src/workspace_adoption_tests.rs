//! Disposable transfer, process-restart and crash-journal qualification. No OS vault.
use super::*;

#[test]
fn unsupported_backend_does_not_create_an_attempt() {
    if cfg!(feature = "encrypted-workspace") {
        return;
    }
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "rangoon-adoption-absent-{}",
        WorkspaceId::generate().unwrap().as_hex()
    ));
    assert!(matches!(
        Conversion::prepare(root.clone()),
        Err(Error::BackendUnavailable | Error::UnsupportedPlatform)
    ));
    assert!(!root.exists());
}

#[cfg(all(unix, feature = "encrypted-workspace"))]
mod fixture {
    use super::*;
    use rangoon_desktop::workspace_keys::{Error as KeyError, Store};
    use std::{
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };
    pub(super) struct Root {
        pub base: PathBuf,
        pub anchor: PathBuf,
    }
    impl Root {
        pub fn new(schema: u32, populated: bool) -> Self {
            let base = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "rangoon-adoption-{}",
                WorkspaceId::generate().unwrap().as_hex()
            ));
            fs::create_dir(&base).unwrap();
            fs::write(base.join("test-owner"), b"rangoon.e1c5b.disposable.v1").unwrap();
            let anchor = base.join("anchor");
            fs::create_dir(&anchor).unwrap();
            fs::create_dir(base.join("keys")).unwrap();
            let root = Self { base, anchor };
            root.source(schema, populated);
            root
        }
        pub fn vault(&self) -> TestVault {
            TestVault(self.base.join("keys"))
        }
        pub fn source(&self, schema: u32, populated: bool) {
            let store = Workspace::new(self.anchor.join("source-workspace"));
            if schema == 0 {
                // Independent one-page empty SQLite fixture; the real existing-only
                // reader must validate it before any adoption can proceed.
                fs::create_dir(self.anchor.join("source-workspace")).unwrap();
                let mut bytes = vec![0; 4096];
                bytes[..16].copy_from_slice(b"SQLite format 3\0");
                bytes[16..18].copy_from_slice(&4096u16.to_be_bytes());
                bytes[18] = 1;
                bytes[19] = 1;
                bytes[21] = 64;
                bytes[22] = 32;
                bytes[23] = 32;
                for (offset, value) in [
                    (24, 1u32),
                    (28, 1),
                    (44, 4),
                    (56, 1),
                    (92, 1),
                    (96, 3040000),
                ] {
                    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
                }
                bytes[100] = 13;
                bytes[105..107].copy_from_slice(&4096u16.to_be_bytes());
                fs::write(
                    self.anchor.join("source-workspace/workspace.sqlite3"),
                    bytes,
                )
                .unwrap();
            } else {
                let report = rangoon_import::analyze(
                    "AGENTS.md",
                    b"\xef\xbb\xbf# Rules\r\nKeep originals.\r\n# Tests\r\nCheck edits.\r\n",
                )
                .unwrap();
                store.save(&report).unwrap();
                if schema >= 2 {
                    let cap = store
                        .create_capability(
                            &report.source.id,
                            &report.fragments[0].id,
                            "Transfer skill",
                        )
                        .unwrap()
                        .capability;
                    store
                        .review_capability(&cap.id, &cap.latest_revision_id)
                        .unwrap();
                    let revised = store
                        .revise_capability(
                            &cap.id,
                            &cap.latest_revision_id,
                            "Retained history",
                            "Reviewed historical content.\r\n",
                        )
                        .unwrap()
                        .capability;
                    store
                        .review_capability(&revised.id, &revised.latest_revision_id)
                        .unwrap();
                }
                if schema >= 3 {
                    let value: serde_json::Value = serde_json::from_str(include_str!(
                        "../../../fixtures/composition/decompose-v0.json"
                    ))
                    .unwrap();
                    let request = rangoon_compose::application::Request {
                        schema_version: rangoon_compose::application::REQUEST_SCHEMA.into(),
                        draft: serde_json::from_value(value["draft"].clone()).unwrap(),
                        targets: vec![
                            rangoon_compose::application::Target::New {},
                            rangoon_compose::application::Target::New {},
                        ],
                    };
                    let preview = store.preview_composition(&request).unwrap();
                    store.apply_composition(&preview, true).unwrap();
                }
                if schema == 4 {
                    use rangoon_workflow::records::{
                        SaveIntent, prepare_revision, workflow_id_from_nonce,
                    };
                    let record=prepare_revision(&workflow_id_from_nonce(&[97;32]),None,SaveIntent::Draft,br#"{"schemaVersion":"rangoon.workflow-definition.v1","title":"Transfer workflow","nodes":[],"controlEdges":[],"dataEdges":[]}"#,br#"{"positions":[]}"#).unwrap();
                    let plan = store.inspect_workflow_save(&record).unwrap();
                    store
                        .save_workflow(&record, &plan.expected_state_id)
                        .unwrap();
                    let next = prepare_revision(&workflow_id_from_nonce(&[97;32]), Some(record.id()), SaveIntent::Draft, br#"{"schemaVersion":"rangoon.workflow-definition.v1","title":"Retained workflow history","nodes":[],"controlEdges":[],"dataEdges":[]}"#, br#"{"positions":[]}"#).unwrap();
                    let plan = store.inspect_workflow_save(&next).unwrap();
                    store.save_workflow(&next, &plan.expected_state_id).unwrap();
                }
                if !populated {
                    let data = store.workflow_data().unwrap();
                    for workflow in data.workflows {
                        let plan = store
                            .inspect_workflow_deletion(
                                rangoon_store::WorkflowRecordKind::Workflow,
                                &workflow.id,
                            )
                            .unwrap();
                        store.delete_workflow_record(&plan).unwrap();
                    }
                    let data = store.workflow_data().unwrap();
                    for cap in data.capabilities {
                        let plan = store
                            .inspect_workflow_deletion(
                                rangoon_store::WorkflowRecordKind::Capability,
                                &cap.id,
                            )
                            .unwrap();
                        store.delete_workflow_record(&plan).unwrap();
                    }
                    let data = store.workflow_data().unwrap();
                    for source in data.sources {
                        let plan = store
                            .inspect_workflow_deletion(
                                rangoon_store::WorkflowRecordKind::Source,
                                &source.source_id,
                            )
                            .unwrap();
                        store.delete_workflow_record(&plan).unwrap();
                    }
                }
            }
            let capture = store.capture_existing_snapshot().unwrap();
            assert_eq!(capture.schema_version().unwrap(), schema);
        }
        pub fn copied(&self) {
            let conversion = Conversion::prepare(self.anchor.clone()).unwrap();
            let consent = conversion.synthetic_consent();
            conversion.copy(consent, &self.vault()).unwrap();
        }
        pub fn retain_expected_outputs(&self) {
            let store = Workspace::new(self.anchor.join("source-workspace"));
            if store
                .capture_existing_snapshot()
                .unwrap()
                .schema_version()
                .unwrap()
                == 0
            {
                fs::write(self.base.join("expected-outputs.json"), b"[]").unwrap();
                return;
            }
            let mut values = Vec::new();
            for source in store.list().unwrap() {
                values.push(serde_json::to_value(store.open(&source.source_id).unwrap()).unwrap());
            }
            for capability in store.list_capabilities_v1().unwrap() {
                values.push(
                    serde_json::to_value(store.open_capability_v1(&capability.id, None).unwrap())
                        .unwrap(),
                );
                for profile in [
                    rangoon_compile::Profile::AgentsMdV1,
                    rangoon_compile::Profile::ClaudeMdV1,
                ] {
                    let compiled = store
                        .compile_capability(&capability.id, &capability.latest_revision_id, profile)
                        .unwrap();
                    values.push(serde_json::to_value(&compiled).unwrap());
                    if let Ok(bundle) = rangoon_compile::encode_bundle(&compiled.compilation) {
                        values.push(serde_json::json!({"bundle":bundle.bytes}));
                    }
                }
            }
            fs::write(
                self.base.join("expected-outputs.json"),
                serde_json::to_vec(&values).unwrap(),
            )
            .unwrap();
        }
        pub fn spawn(&self, mode: &str, stage: Option<(&str, &str)>) -> Child {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "workspace_adoption::tests::process_child",
                    "--ignored",
                    "--nocapture",
                ])
                .env("RANGOON_E1C5B_TEST_ROOT", &self.base)
                .env("RANGOON_E1C5B_TEST_MODE", mode)
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            if let Some((phase, boundary)) = stage {
                command
                    .env("RANGOON_E1C5B_TEST_PHASE", phase)
                    .env("RANGOON_E1C5B_TEST_BOUNDARY", boundary);
            }
            command.spawn().unwrap()
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }
    #[derive(Clone)]
    pub(super) struct TestVault(pub PathBuf);
    pub(super) struct Slot {
        id: WorkspaceId,
        path: PathBuf,
    }
    impl Vault for TestVault {
        type Slot = Slot;
        fn open(&self, id: &WorkspaceId) -> Result<Slot, KeyError> {
            point("vault", "before_open").map_err(|_| KeyError::Unavailable)?;
            Ok(Slot {
                id: id.clone(),
                path: self.0.join(id.as_hex()),
            })
        }
    }
    impl Store for Slot {
        fn identity(&self) -> &WorkspaceId {
            &self.id
        }
        fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, KeyError> {
            point("vault", "before_read").map_err(|_| KeyError::Unavailable)?;
            match fs::read(&self.path) {
                Ok(bytes) if bytes.len() <= 512 => Ok(Some(Zeroizing::new(bytes))),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                _ => Err(KeyError::Unavailable),
            }
        }
        fn write(&self, bytes: &[u8]) -> Result<(), KeyError> {
            point("vault", "before_write").map_err(|_| KeyError::WriteUncertain)?;
            let mut file = options(true)
                .create_new(true)
                .open(&self.path)
                .map_err(|_| KeyError::WriteUncertain)?;
            file.write_all(bytes)
                .and_then(|_| file.sync_all())
                .map_err(|_| KeyError::WriteUncertain)?;
            point("vault", "after_write").map_err(|_| KeyError::WriteUncertain)
        }
    }
    pub(super) struct ChildGuard(pub Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    pub(super) fn kill_at(root: &Root, mode: &str, phase: &str, boundary: &str) {
        let mut child = ChildGuard(root.spawn(mode, Some((phase, boundary))));
        let start = Instant::now();
        while !root.base.join("ready").exists() {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "child exited before checkpoint"
            );
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "child checkpoint timeout"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(child.0.try_wait().unwrap().is_none());
        child.0.kill().unwrap();
        assert!(!child.0.wait().unwrap().success());
    }
    pub(super) fn intent() -> Record {
        Record {
            schema_version: records::SCHEMA.into(),
            phase: Phase::Intent,
            attempt_id: "1".repeat(64),
            creator_process: "2".repeat(64),
            previous_digest: None,
            source_identity: "3".repeat(64),
            source_archive: "4".repeat(64),
            source_schema: 1,
            target_schema: 1,
            inventory: Inventory::from(&rangoon_store::WorkflowRecoveryCounts::default()),
            destination: None,
            verifier_process: None,
        }
    }
    pub(super) fn append_phase(journal: &mut Journal, phase: Phase) {
        let record = if phase == Phase::Intent {
            intent()
        } else {
            journal
                .next(
                    phase,
                    Some("5".repeat(64)),
                    if matches!(phase, Phase::Reopened | Phase::Selected) {
                        Some("6".repeat(64))
                    } else {
                        None
                    },
                )
                .unwrap()
        };
        journal.append(record).unwrap();
    }
}
#[cfg(all(unix, feature = "encrypted-workspace"))]
use crate::{
    workspace_close::{WorkspaceClose, setup::Consent},
    workspace_source_access::{ControlError, ControlKind, SourceAccess},
};
#[cfg(all(unix, feature = "encrypted-workspace"))]
use fixture::*;
#[cfg(all(unix, feature = "encrypted-workspace"))]
use rangoon_desktop::workspace_lifecycle::Gate;
#[cfg(all(unix, feature = "encrypted-workspace"))]
use serde_json::json;

#[cfg(all(unix, feature = "encrypted-workspace"))]
fn control_identity(close: &WorkspaceClose) -> (String, String) {
    let view = serde_json::to_value(close.inspect(|_| true)).unwrap();
    (
        view["attemptId"].as_str().unwrap().to_owned(),
        view["epoch"].as_str().unwrap().to_owned(),
    )
}

#[cfg(all(unix, feature = "encrypted-workspace"))]
fn acknowledgement(id: &str, epoch: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.workspace-purge-ack.v1",
        "attemptId": id,
        "epoch": epoch,
    }))
    .unwrap()
}

#[cfg(all(unix, feature = "encrypted-workspace"))]
fn startup_close(root: &Root) -> WorkspaceClose {
    let access = SourceAccess::managed_deferred(
        Gate::new(),
        root.anchor.join("managed-workspace"),
        root.vault(),
    );
    let close = WorkspaceClose::new(access);
    let started = serde_json::to_value(close.begin(|_| true)).unwrap();
    let id = started["attemptId"].as_str().unwrap().to_owned();
    let epoch = started["epoch"].as_str().unwrap().to_owned();
    let locked =
        serde_json::to_value(close.acknowledge(&acknowledgement(&id, &epoch), |_| true)).unwrap();
    assert_eq!(locked["state"], "locked");
    close
}

#[cfg(all(unix, feature = "encrypted-workspace"))]
fn acknowledge_startup_flight(close: &WorkspaceClose) {
    let (id, epoch) = control_identity(close);
    let view =
        serde_json::to_value(close.acknowledge(&acknowledgement(&id, &epoch), |_| true)).unwrap();
    assert_eq!(view["state"], "closing");
}

#[cfg(all(unix, feature = "encrypted-workspace"))]
fn startup_action_request(
    view: &serde_json::Value,
    action: &str,
) -> crate::workspace_startup_host::ActionRequest {
    crate::workspace_startup_host::ActionRequest::parse(
        &serde_json::to_vec(&json!({
            "schemaVersion": "rangoon.workspace-startup-action.v1",
            "startupId": view["startupId"].as_str().unwrap(),
            "attemptId": view["attemptId"].as_str().unwrap(),
            "epoch": view["epoch"].as_str().unwrap(),
            "action": action,
        }))
        .unwrap(),
    )
    .unwrap()
}

#[cfg(all(unix, feature = "encrypted-workspace"))]
fn assert_no_copy_effects(root: &Root) {
    assert!(!root.anchor.join(DIRECTORY).exists());
    assert!(!root.anchor.join("managed-workspace").exists());
    assert_eq!(fs::read_dir(root.vault().0).unwrap().count(), 0);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn recovery_vault_scopes_existing_identity_and_never_delegates_write() {
    use rangoon_desktop::{
        workspace_keys::{Error as KeyError, Store, WorkspaceId},
        workspace_session::Vault,
    };
    use std::sync::{Arc, Mutex};

    let root = Root::new(1, true);
    let expected = WorkspaceId::generate().unwrap();
    let wrong = WorkspaceId::generate().unwrap();
    let counts = Arc::new(Mutex::new([0_usize; 2]));
    PROBE.with(|probe| {
        let counts = Arc::clone(&counts);
        *probe.borrow_mut() = Some(Box::new(move |stage, boundary| {
            let index = match (stage, boundary) {
                ("vault", "before_open") => Some(0),
                ("vault", "before_write") => Some(1),
                _ => None,
            };
            if let Some(index) = index {
                counts.lock().unwrap()[index] += 1;
            }
            Ok(())
        }));
    });
    let vault = root.vault();
    let scoped = RecoveryVault {
        expected: &expected.as_hex(),
        vault: &vault,
    };

    assert!(matches!(scoped.open(&wrong), Err(KeyError::Changed)));
    assert_eq!(*counts.lock().unwrap(), [0, 0]);
    let slot = scoped.open(&expected).unwrap();
    assert_eq!(*counts.lock().unwrap(), [1, 0]);
    assert!(matches!(
        slot.write(b"never write"),
        Err(KeyError::Unavailable)
    ));
    assert_eq!(*counts.lock().unwrap(), [1, 0]);
    PROBE.with(|probe| *probe.borrow_mut() = None);
}

#[cfg(all(unix, feature = "encrypted-workspace"))]
fn interrupted_copy(root: &Root, boundary: &'static str) {
    let conversion = Conversion::prepare(root.anchor.clone()).unwrap();
    let consent = conversion.synthetic_consent();
    PROBE.with(|probe| {
        *probe.borrow_mut() = Some(Box::new(move |stage, current| {
            if stage == "destination" && current == boundary {
                return Err(Error::WriteUncertain);
            }
            Ok(())
        }));
    });
    assert_eq!(
        conversion.copy(consent, &root.vault()),
        Err(Error::WriteUncertain)
    );
    PROBE.with(|probe| *probe.borrow_mut() = None);
    assert!(root.anchor.join(DIRECTORY).join("target.json").is_file());
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn killed_key_write_before_database_recovers_only_after_explicit_native_completion() {
    let root = Root::new(4, true);
    root.retain_expected_outputs();
    let source = Workspace::new(root.anchor.join("source-workspace"));
    let mut capture = source.capture_existing_snapshot().unwrap();
    let original = fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap();

    // The existing Store checkpoint follows a real synced envelope write and
    // precedes its return to create_key. No record is forged or removed.
    kill_at(&root, "copy", "vault", "after_write");
    let journal = Journal::open(&root.anchor).unwrap();
    let target = &journal.records.last().unwrap().record;
    assert_eq!(target.phase, Phase::Target);
    let destination = target.destination.as_ref().unwrap().clone();
    let attempt_id = target.attempt_id.clone();
    let database = root
        .anchor
        .join("managed-workspace/encrypted-workspaces")
        .join(&destination)
        .join("workspace.sqlite3");
    assert!(!database.exists());
    let binding_path = root
        .anchor
        .join("managed-workspace/encrypted-workspace.binding");
    let pending_path = root
        .anchor
        .join("managed-workspace/encrypted-workspace.operation.pending");
    let binding_before = fs::read(&binding_path).unwrap();
    let pending_before = fs::read(&pending_path).unwrap();
    let immutable_paths = [
        root.anchor.join(DIRECTORY).join("intent.json"),
        root.anchor.join(DIRECTORY).join("target.json"),
    ];
    let immutable: Vec<_> = immutable_paths
        .iter()
        .map(|path| fs::read(path).unwrap())
        .collect();
    drop(journal);
    let key_path = root.vault().0.join(&destination);
    let key = Zeroizing::new(fs::read(&key_path).unwrap());
    assert_eq!(fs::read_dir(root.vault().0).unwrap().count(), 1);
    let run_child = |mode: &str| {
        let mut child = ChildGuard(root.spawn(mode, None));
        let started = std::time::Instant::now();
        let status = loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                break status;
            }
            if started.elapsed() >= std::time::Duration::from_secs(20) {
                let _ = child.0.kill();
                let _ = child.0.wait();
                panic!("{mode} child timeout");
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        let mut diagnostics = String::new();
        if let Some(stderr) = child.0.stderr.take() {
            stderr.take(8192).read_to_string(&mut diagnostics).unwrap();
        }
        assert!(status.success(), "{mode} failed: {diagnostics}");
    };

    // Cancelled native consent cannot repair the interrupted target or replace
    // its existing key. Both paths are real fresh host processes.
    for mode in [
        "recover-host-no-database-cancel-inspect",
        "recover-host-no-database-cancel-initialize",
    ] {
        run_child(mode);
        source.revalidate_existing_capture(&mut capture).unwrap();
        assert_eq!(
            fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap(),
            original
        );
        for (path, previous) in immutable_paths.iter().zip(&immutable) {
            assert_eq!(fs::read(path).unwrap(), *previous);
        }
        assert_eq!(fs::read(&binding_path).unwrap(), binding_before);
        assert_eq!(fs::read(&pending_path).unwrap(), pending_before);
        let current_key = Zeroizing::new(fs::read(&key_path).unwrap());
        assert!(
            current_key.as_slice() == key.as_slice(),
            "disposable key changed"
        );
        assert_eq!(fs::read_dir(root.vault().0).unwrap().count(), 1);
        assert!(!database.exists());
        assert!(
            !root
                .anchor
                .join("managed-workspace/encrypted-workspace.operation.terminal")
                .exists()
        );
        for phase in [Phase::Copied, Phase::Reopened, Phase::Selected] {
            assert!(!root.anchor.join(DIRECTORY).join(phase.name()).exists());
        }
    }

    run_child("recover-host-no-database");
    source.revalidate_existing_capture(&mut capture).unwrap();
    assert_eq!(
        fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap(),
        original
    );
    for (path, previous) in immutable_paths.iter().zip(&immutable) {
        assert_eq!(fs::read(path).unwrap(), *previous);
    }
    let binding_after = fs::read(&binding_path).unwrap();
    assert_eq!(binding_after.len(), binding_before.len());
    assert_eq!(&binding_after[..29], &binding_before[..29]);
    assert_eq!(binding_before[29], 0);
    assert_eq!(binding_after[29], 1);
    assert_eq!(&binding_after[30..], &binding_before[30..]);
    // Initialize identity is checked at Unlock, before Restore starts its own attempt.

    let current_key = Zeroizing::new(fs::read(&key_path).unwrap());
    assert!(
        current_key.as_slice() == key.as_slice(),
        "disposable key changed"
    );
    assert_eq!(fs::read_dir(root.vault().0).unwrap().count(), 1);
    assert!(database.is_file());
    let journal = Journal::open(&root.anchor).unwrap();
    let copied = &journal.records.last().unwrap().record;
    assert_eq!(copied.phase, Phase::Copied);
    assert_eq!(copied.destination.as_deref(), Some(destination.as_str()));
    assert_eq!(copied.attempt_id, attempt_id);
    drop(journal);

    run_child("verify");
    assert!(root.anchor.join(DIRECTORY).join("reopened.json").is_file());
    assert!(root.anchor.join(DIRECTORY).join("selected.json").is_file());
    assert_eq!(fs::read_dir(root.vault().0).unwrap().count(), 1);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn host_recovery_restores_empty_target_after_interrupted_confirm() {
    let root = Root::new(1, true);
    let original = fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap();
    interrupted_copy(&root, "after_confirm");

    let output = root
        .spawn("recover-host-target-empty", None)
        .wait_with_output()
        .unwrap();
    assert!(
        output.status.success(),
        "empty target recovery failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap(),
        original
    );
    assert!(root.anchor.join(DIRECTORY).join("copied.json").is_file());
    assert_eq!(fs::read_dir(root.vault().0.clone()).unwrap().count(), 1);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn host_recovery_records_matching_target_after_interrupted_restore() {
    let root = Root::new(1, true);
    let original = fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap();
    interrupted_copy(&root, "after_restore");

    let output = root
        .spawn("recover-host-target-full", None)
        .wait_with_output()
        .unwrap();
    assert!(
        output.status.success(),
        "matching target recovery failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap(),
        original
    );
    assert!(root.anchor.join(DIRECTORY).join("copied.json").is_file());
    assert_eq!(fs::read_dir(root.vault().0.clone()).unwrap().count(), 1);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn host_recovery_reconciles_pending_terminal_before_recording_matching_target() {
    let root = Root::new(1, true);
    interrupted_copy(&root, "after_restore");
    let terminal = root
        .anchor
        .join("managed-workspace/encrypted-workspace.operation.terminal");
    assert!(terminal.is_file());
    fs::remove_file(&terminal).unwrap();

    let output = root
        .spawn("recover-host-target-pending", None)
        .wait_with_output()
        .unwrap();
    assert!(
        output.status.success(),
        "pending target recovery failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(root.anchor.join(DIRECTORY).join("copied.json").is_file());
    assert_eq!(fs::read_dir(root.vault().0.clone()).unwrap().count(), 1);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn third_process_host_recovery_completes_reopened_selection_without_rewriting_history() {
    let root = Root::new(1, true);
    root.copied();
    kill_at(&root, "verify", "verification", "before_selection");
    let reopened_path = root.anchor.join(DIRECTORY).join("reopened.json");
    let reopened = fs::read(&reopened_path).unwrap();
    assert!(!root.anchor.join(DIRECTORY).join("selected.json").exists());

    let output = root
        .spawn("recover-host-reopened", None)
        .wait_with_output()
        .unwrap();
    assert!(
        output.status.success(),
        "reopened selection recovery failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(&reopened_path).unwrap(), reopened);
    let reopened_record = Record::decode(&reopened).unwrap();
    let selected =
        Record::decode(&fs::read(root.anchor.join(DIRECTORY).join("selected.json")).unwrap())
            .unwrap();
    assert_eq!(selected.verifier_process, reopened_record.verifier_process);
    assert_eq!(inspect(&root.anchor), Status::SelectionRecorded);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn host_recovery_cancel_before_inspect_reoffers_rotated_token_without_vault_access() {
    let root = Root::new(1, true);
    interrupted_copy(&root, "after_confirm");

    let output = root
        .spawn("recover-host-cancel-inspect", None)
        .wait_with_output()
        .unwrap();
    assert!(
        output.status.success(),
        "inspect cancellation recovery failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(root.anchor.join(DIRECTORY).join("target.json").is_file());
    assert!(!root.anchor.join(DIRECTORY).join("copied.json").exists());
    assert_eq!(fs::read_dir(root.vault().0.clone()).unwrap().count(), 1);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn host_recovery_cancel_at_unlock_reoffers_rotated_token_and_retries() {
    let root = Root::new(1, true);
    interrupted_copy(&root, "after_restore");
    fs::remove_file(
        root.anchor
            .join("managed-workspace/encrypted-workspace.operation.terminal"),
    )
    .unwrap();

    let output = root
        .spawn("recover-host-cancel-unlock", None)
        .wait_with_output()
        .unwrap();
    assert!(
        output.status.success(),
        "unlock cancellation recovery failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(root.anchor.join(DIRECTORY).join("copied.json").is_file());
    assert_eq!(fs::read_dir(root.vault().0.clone()).unwrap().count(), 1);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn host_recovery_record_copied_destination_drift_refuses_before_journal_append() {
    let root = Root::new(1, true);
    let original = fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap();
    interrupted_copy(&root, "after_restore");

    let output = root
        .spawn("recover-host-record-copied-drift", None)
        .wait_with_output()
        .unwrap();
    assert!(
        output.status.success(),
        "record-copied destination drift recovery failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap(),
        original
    );
    assert!(root.anchor.join(DIRECTORY).join("target.json").is_file());
    assert!(!root.anchor.join(DIRECTORY).join("copied.json").exists());
    assert!(!root.anchor.join(DIRECTORY).join("selected.json").exists());
    assert_eq!(fs::read_dir(root.vault().0.clone()).unwrap().count(), 1);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn host_recovery_missing_existing_key_refuses_without_regeneration() {
    let root = Root::new(1, true);
    interrupted_copy(&root, "after_restore");
    for entry in fs::read_dir(root.vault().0.clone()).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }

    let output = root
        .spawn("recover-host-missing-key", None)
        .wait_with_output()
        .unwrap();
    assert!(
        output.status.success(),
        "missing recovery key refusal failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(root.anchor.join(DIRECTORY).join("target.json").is_file());
    assert!(!root.anchor.join(DIRECTORY).join("copied.json").exists());
    assert_eq!(fs::read_dir(root.vault().0.clone()).unwrap().count(), 0);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn host_recovery_wrong_target_identity_refuses_before_vault_open() {
    let root = Root::new(1, true);
    interrupted_copy(&root, "after_restore");
    let target = root.anchor.join(DIRECTORY).join("target.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&target).unwrap()).unwrap();
    value["destination"] = serde_json::Value::String("e".repeat(64));
    fs::write(&target, serde_json::to_vec(&value).unwrap()).unwrap();

    let output = root
        .spawn("recover-host-wrong-target", None)
        .wait_with_output()
        .unwrap();
    assert!(
        output.status.success(),
        "wrong recovery target refusal failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!root.anchor.join(DIRECTORY).join("copied.json").exists());
    assert_eq!(fs::read_dir(root.vault().0.clone()).unwrap().count(), 1);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn host_recovery_c1_revocation_during_confirmation_blocks_followup_effects() {
    let root = Root::new(1, true);
    let original = fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap();
    interrupted_copy(&root, "after_confirm");

    let output = root
        .spawn("recover-host-c1-revoked", None)
        .wait_with_output()
        .unwrap();
    assert!(
        output.status.success(),
        "recovery C1 revocation refusal failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap(),
        original
    );
    assert!(root.anchor.join(DIRECTORY).join("target.json").is_file());
    assert!(!root.anchor.join(DIRECTORY).join("copied.json").exists());
    assert!(!root.anchor.join(DIRECTORY).join("selected.json").exists());
    assert_eq!(fs::read_dir(root.vault().0.clone()).unwrap().count(), 1);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn preparation_cancel_bad_consent_and_drift_have_no_destination_or_key_effects() {
    let root = Root::new(1, true);
    let conversion = Conversion::prepare(root.anchor.clone()).unwrap();
    assert_eq!(format!("{conversion:?}"), "Conversion(<redacted>)");
    drop(conversion);
    assert_eq!(inspect(&root.anchor), Status::NoAttempt);
    assert!(!root.anchor.join("managed-workspace").exists());
    assert_eq!(fs::read_dir(&root.vault().0).unwrap().count(), 0);
    let conversion = Conversion::prepare(root.anchor.clone()).unwrap();
    assert_eq!(
        conversion.copy(
            CopyConsent {
                token: "f".repeat(64)
            },
            &root.vault()
        ),
        Err(Error::ConsentRequired)
    );
    assert_eq!(inspect(&root.anchor), Status::NoAttempt);
    let conversion = Conversion::prepare(root.anchor.clone()).unwrap();
    let consent = conversion.synthetic_consent();
    Workspace::new(root.anchor.join("source-workspace"))
        .save(&rangoon_import::analyze("SKILL.md", b"# Changed\n").unwrap())
        .unwrap();
    assert_eq!(conversion.copy(consent, &root.vault()), Err(Error::Changed));
    assert_eq!(inspect(&root.anchor), Status::NoAttempt);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn unacknowledged_startup_control_cannot_mint_native_permit_or_capture() {
    let root = Root::new(1, true);
    let close = startup_close(&root);
    let flight = close.start_control(ControlKind::Startup, |_| true).unwrap();

    assert!(matches!(
        flight.startup_permit(),
        Err(ControlError::Unavailable)
    ));
    assert_no_copy_effects(&root);
    drop(flight);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn every_schema_transfers_and_selects_only_after_real_process_restart() {
    for (schema, populated) in (0..=4)
        .map(|s| (s, false))
        .chain((1..=4).map(|s| (s, true)))
    {
        let root = Root::new(schema, populated);
        root.retain_expected_outputs();
        let original = fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap();
        root.copied();
        assert_eq!(inspect(&root.anchor), Status::Interrupted);
        assert!(matches!(
            Verification::prepare(root.anchor.clone()),
            Err(Error::SameProcess)
        ));
        let output = root.spawn("verify", None).wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "verification child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(inspect(&root.anchor), Status::SelectionRecorded);
        assert_eq!(
            fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap(),
            original
        );
        assert_eq!(fs::read_dir(root.vault().0).unwrap().count(), 1);
        assert!(Conversion::prepare(root.anchor.clone()).is_err());
    }
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn journal_failures_at_all_phase_boundaries_preserve_prior_files_and_refuse_resume() {
    let boundaries = [
        "before_create",
        "after_create",
        "before_write",
        "after_write",
        "before_file_sync",
        "after_file_sync",
        "before_directory_sync",
        "after_directory_sync",
        "before_readback",
        "after_readback",
    ];
    for phase in Phase::ALL {
        for boundary in boundaries {
            let root = Root::new(1, true);
            let mut journal = Journal::create(&root.anchor).unwrap();
            for previous in Phase::ALL.into_iter().take(phase.index()) {
                append_phase(&mut journal, previous);
            }
            let originals: Vec<_> = journal
                .records
                .iter()
                .map(|r| (r.record.phase.name(), r.bytes.clone()))
                .collect();
            let selected = phase.name().to_owned();
            let label = boundary.to_owned();
            PROBE.with(|p| {
                *p.borrow_mut() = Some(Box::new(move |stage, point| {
                    if stage == selected && point == label {
                        Err(Error::WriteUncertain)
                    } else {
                        Ok(())
                    }
                }))
            });
            let record = if phase == Phase::Intent {
                intent()
            } else {
                journal
                    .next(
                        phase,
                        Some("5".repeat(64)),
                        if matches!(phase, Phase::Reopened | Phase::Selected) {
                            Some("6".repeat(64))
                        } else {
                            None
                        },
                    )
                    .unwrap()
            };
            assert!(journal.append(record).is_err());
            PROBE.with(|p| *p.borrow_mut() = None);
            drop(journal);
            for (name, bytes) in originals {
                assert_eq!(
                    fs::read(root.anchor.join(DIRECTORY).join(name)).unwrap(),
                    bytes
                );
            }
            // The disk reader distinguishes complete surviving metadata from torn records;
            // it never supplies an operational client or historical acknowledgment.
            let observed = inspect(&root.anchor);
            assert!(
                matches!(
                    observed,
                    Status::Interrupted | Status::RecoveryRequired | Status::LegacyDiverged
                ),
                "journal phase {} boundary {boundary}: {observed:?}",
                phase.name()
            );
        }
    }
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn journal_rejects_unknown_linked_replaced_and_conflicting_files() {
    use std::os::unix::fs::symlink;
    let root = Root::new(1, true);
    let mut journal = Journal::create(&root.anchor).unwrap();
    append_phase(&mut journal, Phase::Intent);
    drop(journal);
    let path = root.anchor.join(DIRECTORY);
    fs::write(path.join("unknown"), b"x").unwrap();
    assert_eq!(inspect(&root.anchor), Status::RecoveryRequired);
    fs::remove_file(path.join("unknown")).unwrap();
    fs::hard_link(path.join("intent.json"), root.base.join("alias")).unwrap();
    assert_eq!(inspect(&root.anchor), Status::RecoveryRequired);
    fs::remove_file(root.base.join("alias")).unwrap();
    fs::rename(path.join("intent.json"), root.base.join("original")).unwrap();
    symlink(root.base.join("original"), path.join("intent.json")).unwrap();
    assert_eq!(inspect(&root.anchor), Status::RecoveryRequired);
    fs::remove_file(path.join("intent.json")).unwrap();
    let journal = Journal::open(&root.anchor).unwrap();
    assert!(matches!(journal.read(Phase::Intent), Err(Error::Changed)));
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn initial_journal_faults_and_busy_owner_do_not_reuse_or_replace_attempts() {
    for boundary in [
        "before_root_create",
        "after_root_create",
        "before_parent_sync",
        "after_parent_sync",
        "before_lock_create",
        "after_lock_create",
        "before_lock_sync",
        "after_lock_sync",
        "before_root_sync",
        "after_root_sync",
    ] {
        let root = Root::new(1, true);
        let label = boundary.to_owned();
        PROBE.with(|p| {
            *p.borrow_mut() = Some(Box::new(move |stage, b| {
                if stage == "journal" && b == label {
                    Err(Error::WriteUncertain)
                } else {
                    Ok(())
                }
            }))
        });
        assert!(Journal::create(&root.anchor).is_err());
        PROBE.with(|p| *p.borrow_mut() = None);
        assert!(matches!(
            inspect(&root.anchor),
            Status::NoAttempt | Status::Interrupted | Status::RecoveryRequired
        ));
        assert!(!root.anchor.join("managed-workspace").exists());
        assert_eq!(fs::read_dir(root.vault().0).unwrap().count(), 0);
    }
    let root = Root::new(1, true);
    let journal = Journal::create(&root.anchor).unwrap();
    assert!(Journal::open(&root.anchor).is_err());
    assert!(Journal::create(&root.anchor).is_err());
    drop(journal);
    assert_eq!(inspect(&root.anchor), Status::Interrupted);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn competing_source_commits_before_effects_and_during_copy_invalidate_attempt() {
    for boundary in [
        "before_create",
        "before_confirm",
        "before_restore",
        "after_restore",
    ] {
        let root = Root::new(2, true);
        let source = root.anchor.join("source-workspace");
        let label = boundary.to_owned();
        PROBE.with(|p| {
            *p.borrow_mut() = Some(Box::new(move |stage, b| {
                if stage == "destination" && b == label {
                    Workspace::new(source.clone())
                        .save(
                            &rangoon_import::analyze("SKILL.md", b"# Competing writer\n").unwrap(),
                        )
                        .unwrap();
                }
                Ok(())
            }))
        });
        let conversion = Conversion::prepare(root.anchor.clone()).unwrap();
        let consent = conversion.synthetic_consent();
        assert!(conversion.copy(consent, &root.vault()).is_err());
        PROBE.with(|p| *p.borrow_mut() = None);
        assert!(!root.anchor.join(DIRECTORY).join("copied.json").exists());
        assert!(!root.anchor.join(DIRECTORY).join("selected.json").exists());
        let writes = fs::read_dir(root.vault().0).unwrap().count();
        assert_eq!(
            writes,
            usize::from(matches!(boundary, "before_restore" | "after_restore"))
        );
    }
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn killed_destination_provision_and_restore_boundaries_require_inspection() {
    for (stage, boundary) in [
        ("destination", "before_create"),
        ("destination", "after_create"),
        ("vault", "before_write"),
        ("vault", "after_write"),
        ("destination", "after_confirm"),
        ("destination", "before_restore"),
        ("destination", "after_restore"),
        ("destination", "after_close"),
    ] {
        let root = Root::new(3, true);
        kill_at(&root, "copy", stage, boundary);
        assert!(matches!(
            inspect(&root.anchor),
            Status::Interrupted | Status::RecoveryRequired
        ));
        assert!(Conversion::prepare(root.anchor.clone()).is_err());
        assert!(fs::read_dir(root.vault().0).unwrap().count() <= 1);
    }
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn source_commit_during_distinct_process_verification_prevents_selection() {
    for boundary in ["after_unlock", "before_selection"] {
        let root = Root::new(2, true);
        root.copied();
        let output = root
            .spawn("verify-drift", Some(("verification", boundary)))
            .wait_with_output()
            .unwrap();
        assert!(
            output.status.success(),
            "verification drift refusal failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!root.anchor.join(DIRECTORY).join("selected.json").exists());
    }
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn killed_copy_processes_leave_inspection_only_and_no_second_key() {
    for phase in ["intent.json", "target.json", "copied.json"] {
        for boundary in [
            "before_create",
            "after_write",
            "after_file_sync",
            "after_directory_sync",
            "after_readback",
        ] {
            let root = Root::new(1, true);
            let before = fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap();
            kill_at(&root, "copy", phase, boundary);
            assert!(matches!(
                inspect(&root.anchor),
                Status::Interrupted | Status::RecoveryRequired
            ));
            assert!(Conversion::prepare(root.anchor.clone()).is_err());
            assert_eq!(
                fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap(),
                before
            );
            assert!(fs::read_dir(root.vault().0).unwrap().count() <= 1);
        }
    }
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn killed_verification_and_selection_processes_report_only_surviving_metadata() {
    for phase in ["reopened.json", "selected.json"] {
        for boundary in [
            "before_create",
            "after_write",
            "after_file_sync",
            "after_directory_sync",
            "after_readback",
        ] {
            let root = Root::new(2, true);
            root.copied();
            kill_at(&root, "verify", phase, boundary);
            assert!(matches!(
                inspect(&root.anchor),
                Status::Interrupted | Status::RecoveryRequired | Status::SelectionRecorded
            ));
            assert_eq!(fs::read_dir(root.vault().0).unwrap().count(), 1);
        }
    }
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn missing_key_wrong_target_and_source_drift_prevent_selection() {
    for mode in ["missing-key", "source-drift", "wrong-target"] {
        let root = Root::new(1, true);
        root.copied();
        match mode {
            "missing-key" => {
                for entry in fs::read_dir(root.vault().0).unwrap() {
                    fs::remove_file(entry.unwrap().path()).unwrap();
                }
            }
            "source-drift" => {
                Workspace::new(root.anchor.join("source-workspace"))
                    .save(&rangoon_import::analyze("SKILL.md", b"# Later\n").unwrap())
                    .unwrap();
            }
            _ => {
                let path = root.anchor.join(DIRECTORY).join("target.json");
                let mut value: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                value["destination"] = serde_json::Value::String("e".repeat(64));
                fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
            }
        }
        let output = root
            .spawn("expect-refusal", None)
            .wait_with_output()
            .unwrap();
        assert!(
            output.status.success(),
            "refusal child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!root.anchor.join(DIRECTORY).join("selected.json").exists());
    }
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn verifier_cancel_wrong_consent_and_busy_owner_never_advance_selection() {
    let root = Root::new(2, true);
    root.copied();
    for mode in ["verify-cancel", "verify-wrong-consent"] {
        let output = root.spawn(mode, None).wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "verifier refusal failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(inspect(&root.anchor), Status::Interrupted);
        assert_eq!(fs::read_dir(root.vault().0.clone()).unwrap().count(), 1);
    }
    let owner = Owner::acquire(root.anchor.join("managed-workspace")).unwrap();
    assert!(
        root.spawn("expect-refusal", None)
            .wait_with_output()
            .unwrap()
            .status
            .success()
    );
    drop(owner);
    assert!(!root.anchor.join(DIRECTORY).join("reopened.json").exists());
    assert!(
        root.spawn("verify", None)
            .wait_with_output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn replaced_removed_or_mixed_source_never_creates_destination_or_key() {
    for mode in ["replace", "remove", "mixed"] {
        let root = Root::new(1, true);
        let conversion = Conversion::prepare(root.anchor.clone()).unwrap();
        let consent = conversion.synthetic_consent();
        let database = root.anchor.join("source-workspace/workspace.sqlite3");
        match mode {
            "replace" => {
                fs::rename(&database, root.base.join("retained-original")).unwrap();
                fs::copy(root.base.join("retained-original"), &database).unwrap();
            }
            "remove" => fs::remove_file(database).unwrap(),
            _ => fs::create_dir(root.anchor.join("managed-workspace")).unwrap(),
        }
        assert!(conversion.copy(consent, &root.vault()).is_err());
        assert!(!root.anchor.join(DIRECTORY).exists());
        assert_eq!(fs::read_dir(root.vault().0).unwrap().count(), 0);
    }
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn later_encrypted_authoring_is_valid_but_legacy_divergence_is_separate() {
    let root = Root::new(1, true);
    root.copied();
    assert!(
        root.spawn("verify", None)
            .wait_with_output()
            .unwrap()
            .status
            .success()
    );
    let mut owner = Owner::acquire(root.anchor.join("managed-workspace")).unwrap();
    let preview = owner.prepare_unlock().unwrap();
    owner.confirm(&preview.id, &root.vault()).unwrap();
    owner
        .save(
            &root.vault(),
            &rangoon_import::analyze("SKILL.md", b"# Managed addition\n").unwrap(),
        )
        .unwrap();
    owner.lock();
    drop(owner);
    assert_eq!(inspect(&root.anchor), Status::SelectionRecorded);
    Workspace::new(root.anchor.join("source-workspace"))
        .save(&rangoon_import::analyze("SKILL.md", b"# Legacy diverged\n").unwrap())
        .unwrap();
    assert_eq!(inspect(&root.anchor), Status::LegacyDiverged);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn native_confirmation_cancel_or_unavailable_leaves_no_journal_destination_or_key() {
    for outcome in [Consent::Cancelled, Consent::Unavailable] {
        let root = Root::new(1, true);
        let close = startup_close(&root);
        let flight = close.start_control(ControlKind::Startup, |_| true).unwrap();
        acknowledge_startup_flight(&close);
        let permit = flight.startup_permit().unwrap();
        let conversion = Conversion::prepare_native(root.anchor.clone(), &permit).unwrap();

        match outcome {
            Consent::Cancelled => assert!(
                conversion
                    .confirm_native(&permit, || Consent::Cancelled)
                    .unwrap()
                    .is_none()
            ),
            Consent::Unavailable => assert!(matches!(
                conversion.confirm_native(&permit, || Consent::Unavailable),
                Err(Error::Unavailable)
            )),
            Consent::Approved => unreachable!(),
        }
        assert_no_copy_effects(&root);
        drop(flight);
    }
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn stale_native_permit_refuses_before_prepare_without_io() {
    let root = Root::new(1, true);
    let close = startup_close(&root);
    let flight = close.start_control(ControlKind::Startup, |_| true).unwrap();
    acknowledge_startup_flight(&close);
    let permit = flight.startup_permit().unwrap();
    let (id, epoch) = control_identity(&close);
    close.cancel_control(&id, &epoch, |_| true);

    assert!(matches!(
        Conversion::prepare_native(root.anchor.clone(), &permit),
        Err(Error::Unavailable)
    ));
    assert_no_copy_effects(&root);
    drop(flight);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn native_confirmation_cancelled_during_callback_cannot_mint_copy_consent() {
    let root = Root::new(1, true);
    let close = startup_close(&root);
    let flight = close.start_control(ControlKind::Startup, |_| true).unwrap();
    acknowledge_startup_flight(&close);
    let permit = flight.startup_permit().unwrap();
    let conversion = Conversion::prepare_native(root.anchor.clone(), &permit).unwrap();
    let (id, epoch) = control_identity(&close);
    let cancelling_close = close.clone();

    assert!(matches!(
        conversion.confirm_native(&permit, move || {
            cancelling_close.cancel_control(&id, &epoch, |_| true);
            Consent::Approved
        }),
        Err(Error::Unavailable)
    ));
    assert_no_copy_effects(&root);
    drop(flight);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn native_confirmation_source_drift_after_callback_refuses_before_copy_effects() {
    let root = Root::new(1, true);
    let close = startup_close(&root);
    let flight = close.start_control(ControlKind::Startup, |_| true).unwrap();
    acknowledge_startup_flight(&close);
    let permit = flight.startup_permit().unwrap();
    let conversion = Conversion::prepare_native(root.anchor.clone(), &permit).unwrap();
    let source = root.anchor.join("source-workspace");

    assert!(matches!(
        conversion.confirm_native(&permit, move || {
            Workspace::new(source)
                .save(&rangoon_import::analyze("SKILL.md", b"# Changed after sheet\n").unwrap())
                .unwrap();
            Consent::Approved
        }),
        Err(Error::Changed)
    ));
    assert_no_copy_effects(&root);
    drop(flight);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn native_conversion_rejects_new_startup_flight_after_original_cancelled() {
    let root = Root::new(1, true);
    let close = startup_close(&root);
    let old_flight = close.start_control(ControlKind::Startup, |_| true).unwrap();
    acknowledge_startup_flight(&close);
    let old_permit = old_flight.startup_permit().unwrap();
    let conversion = Conversion::prepare_native(root.anchor.clone(), &old_permit).unwrap();
    let (conversion, consent) = conversion
        .confirm_native(&old_permit, || Consent::Approved)
        .unwrap()
        .unwrap();
    let (old_id, old_epoch) = control_identity(&close);
    let cancelled =
        serde_json::to_value(close.cancel_control(&old_id, &old_epoch, |_| true)).unwrap();
    let next_id = cancelled["attemptId"].as_str().unwrap().to_owned();
    let next_epoch = cancelled["epoch"].as_str().unwrap().to_owned();
    drop(old_flight);
    assert_eq!(
        serde_json::to_value(close.acknowledge(&acknowledgement(&next_id, &next_epoch), |_| true))
            .unwrap()["state"],
        "locked"
    );

    let new_flight = close.start_control(ControlKind::Startup, |_| true).unwrap();
    acknowledge_startup_flight(&close);
    let new_permit = new_flight.startup_permit().unwrap();
    assert!(matches!(
        conversion.copy_native(consent, &root.vault(), &new_permit),
        Err(Error::Unavailable)
    ));
    assert_no_copy_effects(&root);
    drop(new_flight);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn native_conversion_rejects_matching_serial_from_different_close_coordinator() {
    let root = Root::new(1, true);
    let first_close = startup_close(&root);
    let first_flight = first_close
        .start_control(ControlKind::Startup, |_| true)
        .unwrap();
    acknowledge_startup_flight(&first_close);
    let first_permit = first_flight.startup_permit().unwrap();
    let conversion = Conversion::prepare_native(root.anchor.clone(), &first_permit).unwrap();

    let second_close = startup_close(&root);
    let second_flight = second_close
        .start_control(ControlKind::Startup, |_| true)
        .unwrap();
    acknowledge_startup_flight(&second_close);
    let second_permit = second_flight.startup_permit().unwrap();
    assert!(matches!(
        conversion.confirm_native(&second_permit, || panic!("foreign permit reached callback")),
        Err(Error::Unavailable)
    ));
    assert_no_copy_effects(&root);
    drop(second_flight);
    drop(first_flight);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn native_verification_replay_refuses_before_confirmation_or_selection() {
    for mode in [
        "verify-native-replay-confirm",
        "verify-native-replay-select",
    ] {
        let root = Root::new(2, true);
        root.copied();
        let original = fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap();
        let output = root.spawn(mode, None).wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "native verification replay refusal failed for {mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap(),
            original
        );
        assert!(!root.anchor.join(DIRECTORY).join("reopened.json").exists());
        assert!(!root.anchor.join(DIRECTORY).join("selected.json").exists());
        assert_eq!(fs::read_dir(root.vault().0.clone()).unwrap().count(), 1);
    }
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn approved_native_copy_uses_real_permit_preserves_original_and_verifies_after_worker_drop() {
    let root = Root::new(3, true);
    let original = fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap();
    let close = startup_close(&root);
    let flight = close.start_control(ControlKind::Startup, |_| true).unwrap();
    acknowledge_startup_flight(&close);
    let permit = flight.startup_permit().unwrap();
    let conversion = Conversion::prepare_native(root.anchor.clone(), &permit).unwrap();
    let (conversion, consent) = conversion
        .confirm_native(&permit, || Consent::Approved)
        .unwrap()
        .unwrap();
    conversion
        .copy_native(consent, &root.vault(), &permit)
        .unwrap();

    assert_eq!(
        fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap(),
        original
    );
    assert!(root.anchor.join(DIRECTORY).join("copied.json").exists());
    assert_eq!(fs::read_dir(root.vault().0.clone()).unwrap().count(), 1);
    drop(flight);
    assert_eq!(
        serde_json::to_value(close.inspect(|_| true)).unwrap()["state"],
        "locked"
    );
    drop(close);

    let output = root
        .spawn("verify-native", None)
        .wait_with_output()
        .unwrap();
    assert!(
        output.status.success(),
        "native copy parity verification failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(inspect(&root.anchor), Status::SelectionRecorded);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn host_verification_cancel_then_approved_after_distinct_restart_keeps_source_and_key_custody() {
    let root = Root::new(1, true);
    root.copied();
    let original = fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap();

    let output = root
        .spawn("verify-host-cancel-approve", None)
        .wait_with_output()
        .unwrap();
    assert!(
        output.status.success(),
        "host verification cancel/approve flow failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap(),
        original
    );
    assert_eq!(inspect(&root.anchor), Status::SelectionRecorded);
    assert!(root.anchor.join(DIRECTORY).join("reopened.json").is_file());
    assert!(root.anchor.join(DIRECTORY).join("selected.json").is_file());
    assert_eq!(fs::read_dir(root.vault().0.clone()).unwrap().count(), 1);
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn native_copy_cancellation_checkpoints_preserve_original_and_known_partial_state() {
    for (boundary, expected_keys) in [("before_confirm", 0), ("after_restore", 1)] {
        let root = Root::new(1, true);
        let original = fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap();
        let close = startup_close(&root);
        let flight = close.start_control(ControlKind::Startup, |_| true).unwrap();
        acknowledge_startup_flight(&close);
        let permit = flight.startup_permit().unwrap();
        let conversion = Conversion::prepare_native(root.anchor.clone(), &permit).unwrap();
        let (conversion, consent) = conversion
            .confirm_native(&permit, || Consent::Approved)
            .unwrap()
            .unwrap();
        let (id, epoch) = control_identity(&close);
        let cancelling_close = close.clone();
        let expected_boundary = boundary.to_owned();
        PROBE.with(|probe| {
            *probe.borrow_mut() = Some(Box::new(move |stage, current_boundary| {
                if stage == "destination" && current_boundary == expected_boundary {
                    cancelling_close.cancel_control(&id, &epoch, |_| true);
                }
                Ok(())
            }))
        });

        assert!(matches!(
            conversion.copy_native(consent, &root.vault(), &permit),
            Err(Error::Unavailable)
        ));
        PROBE.with(|probe| *probe.borrow_mut() = None);
        assert_eq!(
            fs::read(root.anchor.join("source-workspace/workspace.sqlite3")).unwrap(),
            original
        );
        assert!(root.anchor.join(DIRECTORY).join("target.json").exists());
        assert!(!root.anchor.join(DIRECTORY).join("copied.json").exists());
        assert_eq!(
            fs::read_dir(root.vault().0.clone()).unwrap().count(),
            expected_keys
        );
        drop(flight);
    }
}

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn verification_control_disappearance_refuses_without_recreation_or_selection() {
    let root = Root::new(1, true);
    root.copied();
    let output = root
        .spawn("verify-control-disappears", None)
        .wait_with_output()
        .unwrap();
    assert!(
        output.status.success(),
        "existing-control disappearance refusal failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !root
            .anchor
            .join("managed-workspace/encrypted-workspace.lock")
            .exists()
    );
    assert!(!root.anchor.join(DIRECTORY).join("reopened.json").exists());
    assert!(!root.anchor.join(DIRECTORY).join("selected.json").exists());
    assert_eq!(fs::read_dir(root.vault().0.clone()).unwrap().count(), 1);
}

#[test]
#[ignore = "Disposable process child; only its exact parent fixtures supply context"]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn process_child() {
    let base = PathBuf::from(std::env::var_os("RANGOON_E1C5B_TEST_ROOT").expect("test root"));
    let temp = std::env::temp_dir().canonicalize().unwrap();
    assert!(base.starts_with(&temp) && base == base.canonicalize().unwrap());
    assert!(
        base.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("rangoon-adoption-")
    );
    assert_eq!(
        fs::read(base.join("test-owner")).unwrap(),
        b"rangoon.e1c5b.disposable.v1"
    );
    let anchor = base.join("anchor");
    let vault = TestVault(base.join("keys"));
    if let (Ok(stage), Ok(boundary)) = (
        std::env::var("RANGOON_E1C5B_TEST_PHASE"),
        std::env::var("RANGOON_E1C5B_TEST_BOUNDARY"),
    ) {
        let marker = base.join("ready");
        PROBE.with(|p| {
            *p.borrow_mut() = Some(Box::new(move |s, b| {
                if s == stage && b == boundary {
                    if std::env::var("RANGOON_E1C5B_TEST_MODE").as_deref() == Ok("verify-drift") {
                        Workspace::new(marker.parent().unwrap().join("anchor/source-workspace"))
                            .save(
                                &rangoon_import::analyze(
                                    "SKILL.md",
                                    b"# Concurrent verification writer\n",
                                )
                                .unwrap(),
                            )
                            .unwrap();
                        return Ok(());
                    }
                    fs::write(&marker, b"ready").unwrap();
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                }
                Ok(())
            }))
        });
    }
    match std::env::var("RANGOON_E1C5B_TEST_MODE").unwrap().as_str() {
        "copy" => {
            let conversion = Conversion::prepare(anchor).unwrap();
            let consent = conversion.synthetic_consent();
            conversion.copy(consent, &vault).unwrap();
        }
        "verify" => {
            let verification = Verification::prepare(anchor.clone()).unwrap();
            let consent = verification.synthetic_consent();
            verification.select(consent, &vault).unwrap();
            if base.join("expected-outputs.json").exists() {
                let mut owner = Owner::acquire(anchor.join("managed-workspace")).unwrap();
                let preview = owner.prepare_unlock().unwrap();
                owner.confirm(&preview.id, &vault).unwrap();
                let mut values = Vec::new();
                for source in owner.list(&vault).unwrap() {
                    values.push(
                        serde_json::to_value(owner.open(&vault, &source.source_id).unwrap())
                            .unwrap(),
                    );
                }
                for capability in owner.list_capabilities(&vault).unwrap() {
                    values.push(
                        serde_json::to_value(
                            owner.open_capability(&vault, &capability.id, None).unwrap(),
                        )
                        .unwrap(),
                    );
                    for profile in [
                        rangoon_compile::Profile::AgentsMdV1,
                        rangoon_compile::Profile::ClaudeMdV1,
                    ] {
                        let compiled = owner
                            .compile_capability(
                                &vault,
                                &capability.id,
                                &capability.latest_revision_id,
                                profile,
                            )
                            .unwrap();
                        values.push(serde_json::to_value(&compiled).unwrap());
                        if let Ok(bundle) = rangoon_compile::encode_bundle(&compiled.compilation) {
                            values.push(serde_json::json!({"bundle":bundle.bytes}));
                        }
                    }
                }
                let expected: Vec<serde_json::Value> =
                    serde_json::from_slice(&fs::read(base.join("expected-outputs.json")).unwrap())
                        .unwrap();
                assert!(
                    values == expected,
                    "exact source/history/compiler/export parity required after restart"
                );
                owner.lock();
            }
        }
        "verify-native" => {
            let access = SourceAccess::managed_deferred(
                Gate::new(),
                anchor.join("managed-workspace"),
                vault.clone(),
            );
            let close = WorkspaceClose::new(access);
            let started = serde_json::to_value(close.begin(|_| true)).unwrap();
            let id = started["attemptId"].as_str().unwrap().to_owned();
            let epoch = started["epoch"].as_str().unwrap().to_owned();
            assert_eq!(
                serde_json::to_value(close.acknowledge(&acknowledgement(&id, &epoch), |_| true))
                    .unwrap()["state"],
                "locked"
            );
            let flight = close.start_control(ControlKind::Startup, |_| true).unwrap();
            acknowledge_startup_flight(&close);
            let permit = flight.startup_permit().unwrap();
            let verification = Verification::prepare_native(anchor, &permit).unwrap();
            let (verification, consent) = verification
                .confirm_native(&permit, || Consent::Approved)
                .unwrap()
                .unwrap();
            verification
                .select_native(consent, &vault, &permit)
                .unwrap();
            drop(flight);
        }
        "verify-host-cancel-approve" => {
            use std::sync::{Arc, Mutex};

            let gate = Gate::new();
            let (access, close, host) = crate::workspace_native_bootstrap::startup_state(
                &gate,
                anchor.clone(),
                vault.clone(),
            );
            let initial = serde_json::to_value(close.begin(|_| true)).unwrap();
            assert_eq!(initial["state"], "closing");
            assert_eq!(
                serde_json::to_value(close.acknowledge(
                    &acknowledgement(
                        initial["attemptId"].as_str().unwrap(),
                        initial["epoch"].as_str().unwrap(),
                    ),
                    |_| true
                ))
                .unwrap()["state"],
                "locked"
            );
            let first = serde_json::to_value(host.status(
                |_| true,
                |current| {
                    let view = serde_json::to_value(current.inspect(|_| true)).unwrap();
                    assert_eq!(view["state"], "closing");
                    let acknowledged = serde_json::to_value(current.acknowledge(
                        &acknowledgement(
                            view["attemptId"].as_str().unwrap(),
                            view["epoch"].as_str().unwrap(),
                        ),
                        |_| true,
                    ))
                    .unwrap();
                    assert_eq!(acknowledged["state"], "closing");
                    true
                },
            ))
            .unwrap();
            assert_eq!(first["state"], "verify_required");
            assert_eq!(first["retainedOriginal"], "present_unencrypted");
            assert!(serde_json::to_value(access.request_view()).unwrap()["clientId"].is_null());

            let counts = Arc::new(Mutex::new([0_usize; 3]));
            PROBE.with(|probe| {
                let counts = Arc::clone(&counts);
                *probe.borrow_mut() = Some(Box::new(move |stage, boundary| {
                    let index = match (stage, boundary) {
                        ("vault", "before_open") => Some(0),
                        ("vault", "before_read") => Some(1),
                        ("vault", "before_write") => Some(2),
                        _ => None,
                    };
                    if let Some(index) = index {
                        counts.lock().unwrap()[index] += 1;
                    }
                    Ok(())
                }));
            });

            let cancelled = serde_json::to_value(host.action(
                startup_action_request(&first, "verify"),
                &vault,
                |_| Consent::Cancelled,
            ))
            .unwrap();
            assert_eq!(cancelled["state"], "verify_required");
            assert_ne!(cancelled["startupId"], first["startupId"]);
            assert!(!anchor.join(DIRECTORY).join("reopened.json").exists());
            assert!(!anchor.join(DIRECTORY).join("selected.json").exists());
            assert_eq!(*counts.lock().unwrap(), [0, 0, 0]);

            let stale = serde_json::to_value(host.action(
                startup_action_request(&first, "verify"),
                &vault,
                |_| panic!("stale verify reached native confirmation"),
            ))
            .unwrap();
            assert_eq!(stale["state"], "verify_required");
            assert_eq!(stale["startupId"], cancelled["startupId"]);
            assert_eq!(*counts.lock().unwrap(), [0, 0, 0]);

            let handoff = serde_json::to_value(host.action(
                startup_action_request(&cancelled, "verify"),
                &vault,
                |_| Consent::Approved,
            ))
            .unwrap();
            assert_eq!(handoff["state"], "handoff");
            assert!(handoff["startupId"].is_null());
            assert!(handoff["attemptId"].is_null());
            assert!(handoff["epoch"].is_null());
            assert!(anchor.join(DIRECTORY).join("reopened.json").is_file());
            assert!(anchor.join(DIRECTORY).join("selected.json").is_file());
            let final_counts = *counts.lock().unwrap();
            assert!(final_counts[0] > 0);
            assert!(final_counts[1] > 0);
            assert_eq!(final_counts[2], 0);
            assert!(serde_json::to_value(access.request_view()).unwrap()["clientId"].is_null());
            PROBE.with(|probe| *probe.borrow_mut() = None);
        }
        mode @ ("recover-host-target-empty"
        | "recover-host-target-full"
        | "recover-host-target-pending"
        | "recover-host-no-database"
        | "recover-host-no-database-cancel-inspect"
        | "recover-host-no-database-cancel-initialize"
        | "recover-host-reopened"
        | "recover-host-cancel-inspect"
        | "recover-host-cancel-unlock"
        | "recover-host-record-copied-drift"
        | "recover-host-missing-key"
        | "recover-host-wrong-target"
        | "recover-host-c1-revoked") => {
            use std::sync::{Arc, Mutex};

            let gate = Gate::new();
            let (access, close, host) = crate::workspace_native_bootstrap::startup_state(
                &gate,
                anchor.clone(),
                vault.clone(),
            );
            let c1_identity = Arc::new(Mutex::new(None));
            let initial = serde_json::to_value(close.begin(|_| true)).unwrap();
            assert_eq!(initial["state"], "closing");
            assert_eq!(
                serde_json::to_value(close.acknowledge(
                    &acknowledgement(
                        initial["attemptId"].as_str().unwrap(),
                        initial["epoch"].as_str().unwrap(),
                    ),
                    |_| true,
                ))
                .unwrap()["state"],
                "locked"
            );
            let target_destination = Journal::open(&anchor)
                .unwrap()
                .records
                .last()
                .unwrap()
                .record
                .destination
                .clone()
                .unwrap();
            let pending_before_initialize = (mode == "recover-host-no-database").then(|| {
                fs::read(anchor.join("managed-workspace/encrypted-workspace.operation.pending"))
                    .unwrap()
            });
            let acknowledged = Arc::clone(&c1_identity);
            let recovery = serde_json::to_value(host.status(
                |_| true,
                |current| {
                    let view = serde_json::to_value(current.inspect(|_| true)).unwrap();
                    *acknowledged.lock().unwrap() = Some((
                        view["attemptId"].as_str().unwrap().to_owned(),
                        view["epoch"].as_str().unwrap().to_owned(),
                    ));
                    let _ = current.acknowledge(
                        &acknowledgement(
                            view["attemptId"].as_str().unwrap(),
                            view["epoch"].as_str().unwrap(),
                        ),
                        |_| true,
                    );
                    true
                },
            ))
            .unwrap();
            assert_eq!(recovery["state"], "recovery_required");
            assert_eq!(recovery["retainedOriginal"], "not_verified");
            assert!(serde_json::to_value(access.request_view()).unwrap()["clientId"].is_null());

            let counts = Arc::new(Mutex::new([0_usize; 3]));
            PROBE.with(|probe| {
                let counts = Arc::clone(&counts);
                *probe.borrow_mut() = Some(Box::new(move |stage, boundary| {
                    let index = match (stage, boundary) {
                        ("vault", "before_open") => Some(0),
                        ("vault", "before_read") => Some(1),
                        ("vault", "before_write") => Some(2),
                        _ => None,
                    };
                    if let Some(index) = index {
                        counts.lock().unwrap()[index] += 1;
                    }
                    Ok(())
                }));
            });
            if mode == "recover-host-no-database-cancel-inspect" {
                let mut steps = Vec::new();
                let cancelled = serde_json::to_value(host.action(
                    startup_action_request(&recovery, "recover"),
                    &vault,
                    |confirmation| match confirmation {
                        crate::workspace_startup_host::StartupConfirmation::Recovery {
                            step,
                            ..
                        } => {
                            steps.push(step);
                            Consent::Cancelled
                        }
                        _ => Consent::Unavailable,
                    },
                ))
                .unwrap();
                assert_eq!(steps, vec![RecoveryStep::Inspect]);
                assert_eq!(cancelled["state"], "recovery_required");
                assert_ne!(cancelled["startupId"], recovery["startupId"]);
                assert_eq!(*counts.lock().unwrap(), [0, 0, 0]);
                assert!(
                    !anchor
                        .join("managed-workspace/encrypted-workspaces")
                        .join(&target_destination)
                        .join("workspace.sqlite3")
                        .exists()
                );
                assert!(serde_json::to_value(access.request_view()).unwrap()["clientId"].is_null());
                PROBE.with(|probe| *probe.borrow_mut() = None);
                return;
            }
            if mode == "recover-host-no-database-cancel-initialize" {
                let mut steps = Vec::new();
                let cancelled = serde_json::to_value(host.action(
                    startup_action_request(&recovery, "recover"),
                    &vault,
                    |confirmation| match confirmation {
                        crate::workspace_startup_host::StartupConfirmation::Recovery {
                            step,
                            ..
                        } => {
                            steps.push(step);
                            if step == RecoveryStep::InitializeEmpty {
                                Consent::Cancelled
                            } else {
                                Consent::Approved
                            }
                        }
                        _ => Consent::Unavailable,
                    },
                ))
                .unwrap();
                assert_eq!(
                    steps,
                    vec![RecoveryStep::Inspect, RecoveryStep::InitializeEmpty]
                );
                assert_eq!(cancelled["state"], "recovery_required");
                assert_ne!(cancelled["startupId"], recovery["startupId"]);
                let counts = *counts.lock().unwrap();
                assert!(counts[0] > 0);
                assert!(counts[1] > 0);
                assert_eq!(counts[2], 0);
                assert!(
                    !anchor
                        .join("managed-workspace/encrypted-workspaces")
                        .join(&target_destination)
                        .join("workspace.sqlite3")
                        .exists()
                );
                assert!(serde_json::to_value(access.request_view()).unwrap()["clientId"].is_null());
                PROBE.with(|probe| *probe.borrow_mut() = None);
                return;
            }
            if mode == "recover-host-record-copied-drift" {
                let binding = anchor.join("managed-workspace/encrypted-workspace.binding");
                let mut steps = Vec::new();
                let refused = serde_json::to_value(host.action(
                    startup_action_request(&recovery, "recover"),
                    &vault,
                    |confirmation| match confirmation {
                        crate::workspace_startup_host::StartupConfirmation::Recovery {
                            step,
                            ..
                        } => {
                            steps.push(step);
                            if step == RecoveryStep::RecordCopied {
                                let mut bytes = fs::read(&binding).unwrap();
                                bytes[0] ^= 1;
                                fs::write(&binding, bytes).unwrap();
                            }
                            Consent::Approved
                        }
                        _ => Consent::Unavailable,
                    },
                ))
                .unwrap();
                assert_eq!(
                    steps,
                    vec![RecoveryStep::Inspect, RecoveryStep::RecordCopied]
                );
                assert_eq!(refused["state"], "unavailable");
                assert!(!anchor.join(DIRECTORY).join("copied.json").exists());
                assert!(!anchor.join(DIRECTORY).join("selected.json").exists());
                assert_eq!(counts.lock().unwrap()[2], 0);
                assert!(serde_json::to_value(access.request_view()).unwrap()["clientId"].is_null());
                PROBE.with(|probe| *probe.borrow_mut() = None);
                return;
            }
            if mode == "recover-host-missing-key" {
                let mut steps = Vec::new();
                let refused = serde_json::to_value(host.action(
                    startup_action_request(&recovery, "recover"),
                    &vault,
                    |confirmation| match confirmation {
                        crate::workspace_startup_host::StartupConfirmation::Recovery {
                            step,
                            ..
                        } => {
                            steps.push(step);
                            Consent::Approved
                        }
                        _ => Consent::Unavailable,
                    },
                ))
                .unwrap();
                assert_eq!(steps, vec![RecoveryStep::Inspect]);
                assert_eq!(refused["state"], "unavailable");
                let counts = *counts.lock().unwrap();
                assert!(counts[0] > 0);
                assert!(counts[1] > 0);
                assert_eq!(counts[2], 0);
                assert!(!anchor.join(DIRECTORY).join("copied.json").exists());
                assert!(serde_json::to_value(access.request_view()).unwrap()["clientId"].is_null());
                PROBE.with(|probe| *probe.borrow_mut() = None);
                return;
            }
            if mode == "recover-host-wrong-target" {
                let refused = serde_json::to_value(host.action(
                    startup_action_request(&recovery, "recover"),
                    &vault,
                    |_| panic!("wrong target reached recovery confirmation"),
                ))
                .unwrap();
                assert_eq!(refused["state"], "unavailable");
                assert_eq!(*counts.lock().unwrap(), [0, 0, 0]);
                assert!(!anchor.join(DIRECTORY).join("copied.json").exists());
                assert!(serde_json::to_value(access.request_view()).unwrap()["clientId"].is_null());
                PROBE.with(|probe| *probe.borrow_mut() = None);
                return;
            }
            if mode == "recover-host-c1-revoked" {
                let (id, epoch) = c1_identity.lock().unwrap().clone().unwrap();
                let cancelling_close = close.clone();
                let mut steps = Vec::new();
                let refused = serde_json::to_value(host.action(
                    startup_action_request(&recovery, "recover"),
                    &vault,
                    |confirmation| match confirmation {
                        crate::workspace_startup_host::StartupConfirmation::Recovery {
                            step,
                            ..
                        } => {
                            steps.push(step);
                            assert_eq!(step, RecoveryStep::Inspect);
                            cancelling_close.cancel_control(&id, &epoch, |_| true);
                            Consent::Approved
                        }
                        _ => Consent::Unavailable,
                    },
                ))
                .unwrap();
                assert_eq!(steps, vec![RecoveryStep::Inspect]);
                assert_eq!(refused["state"], "unavailable");
                assert_eq!(*counts.lock().unwrap(), [0, 0, 0]);
                assert!(!anchor.join(DIRECTORY).join("copied.json").exists());
                assert!(!anchor.join(DIRECTORY).join("selected.json").exists());
                assert!(serde_json::to_value(access.request_view()).unwrap()["clientId"].is_null());
                PROBE.with(|probe| *probe.borrow_mut() = None);
                return;
            }
            if mode == "recover-host-cancel-inspect" {
                let mut steps = Vec::new();
                let cancelled = serde_json::to_value(host.action(
                    startup_action_request(&recovery, "recover"),
                    &vault,
                    |confirmation| match confirmation {
                        crate::workspace_startup_host::StartupConfirmation::Recovery {
                            step,
                            ..
                        } => {
                            steps.push(step);
                            Consent::Cancelled
                        }
                        _ => Consent::Unavailable,
                    },
                ))
                .unwrap();
                assert_eq!(steps, vec![RecoveryStep::Inspect]);
                assert_eq!(cancelled["state"], "recovery_required");
                assert_ne!(cancelled["startupId"], recovery["startupId"]);
                assert_eq!(*counts.lock().unwrap(), [0, 0, 0]);

                let stale = serde_json::to_value(host.action(
                    startup_action_request(&recovery, "recover"),
                    &vault,
                    |_| panic!("stale recovery reached confirmation"),
                ))
                .unwrap();
                assert_eq!(stale["state"], "recovery_required");
                assert_eq!(stale["startupId"], cancelled["startupId"]);
                assert_eq!(*counts.lock().unwrap(), [0, 0, 0]);
                assert!(anchor.join(DIRECTORY).join("target.json").is_file());
                assert!(!anchor.join(DIRECTORY).join("copied.json").exists());
                assert!(serde_json::to_value(access.request_view()).unwrap()["clientId"].is_null());
                PROBE.with(|probe| *probe.borrow_mut() = None);
                return;
            }
            if mode == "recover-host-cancel-unlock" {
                let mut steps = Vec::new();
                let cancelled = serde_json::to_value(host.action(
                    startup_action_request(&recovery, "recover"),
                    &vault,
                    |confirmation| match confirmation {
                        crate::workspace_startup_host::StartupConfirmation::Recovery {
                            step,
                            ..
                        } => {
                            steps.push(step);
                            if step == RecoveryStep::Unlock {
                                Consent::Cancelled
                            } else {
                                Consent::Approved
                            }
                        }
                        _ => Consent::Unavailable,
                    },
                ))
                .unwrap();
                assert_eq!(
                    steps,
                    vec![
                        RecoveryStep::Inspect,
                        RecoveryStep::Reconcile,
                        RecoveryStep::Unlock,
                    ]
                );
                assert_eq!(cancelled["state"], "recovery_required");
                assert_ne!(cancelled["startupId"], recovery["startupId"]);
                assert!(!anchor.join(DIRECTORY).join("copied.json").exists());

                let stale = serde_json::to_value(host.action(
                    startup_action_request(&recovery, "recover"),
                    &vault,
                    |_| panic!("stale recovery reached confirmation"),
                ))
                .unwrap();
                assert_eq!(stale["state"], "recovery_required");
                assert_eq!(stale["startupId"], cancelled["startupId"]);

                let mut retry_steps = Vec::new();
                let retried = serde_json::to_value(host.action(
                    startup_action_request(&cancelled, "recover"),
                    &vault,
                    |confirmation| match confirmation {
                        crate::workspace_startup_host::StartupConfirmation::Recovery {
                            step,
                            ..
                        } => {
                            retry_steps.push(step);
                            Consent::Approved
                        }
                        _ => Consent::Unavailable,
                    },
                ))
                .unwrap();
                assert_eq!(
                    retry_steps,
                    vec![RecoveryStep::Inspect, RecoveryStep::RecordCopied]
                );
                assert_eq!(retried["state"], "restart_required");
                assert_eq!(
                    serde_json::to_value(host.status(|_| true, |_| true)).unwrap()["state"],
                    "restart_required"
                );
                let counts = *counts.lock().unwrap();
                assert!(counts[0] > 0);
                assert!(counts[1] > 0);
                assert_eq!(counts[2], 0);
                assert!(serde_json::to_value(access.request_view()).unwrap()["clientId"].is_null());
                PROBE.with(|probe| *probe.borrow_mut() = None);
                return;
            }
            let mut steps = Vec::new();
            let recovered = serde_json::to_value(host.action(
                startup_action_request(&recovery, "recover"),
                &vault,
                |confirmation| match confirmation {
                    crate::workspace_startup_host::StartupConfirmation::Recovery {
                        step, ..
                    } => {
                        steps.push(step);
                        if mode == "recover-host-no-database" && step == RecoveryStep::Unlock {
                            let pending = pending_before_initialize.as_ref().unwrap();
                            assert!(
                                fs::read(anchor.join(
                                    "managed-workspace/encrypted-workspace.operation.pending"
                                ))
                                .unwrap()
                                    == *pending
                            );
                            let terminal =
                                fs::read(anchor.join(
                                    "managed-workspace/encrypted-workspace.operation.terminal",
                                ))
                                .unwrap();
                            let prefix = b"rangoon.workspace-operation.v1\0";
                            assert!(terminal.starts_with(prefix));
                            assert_eq!(pending[prefix.len()], 0);
                            assert_eq!(terminal[prefix.len()], 1);
                            // Same operation, sequence, workspace and exact Owner attempt.
                            assert_eq!(
                                &terminal[prefix.len() + 1..prefix.len() + 138],
                                &pending[prefix.len() + 1..prefix.len() + 138]
                            );
                        }
                        Consent::Approved
                    }
                    _ => Consent::Unavailable,
                },
            ))
            .unwrap();
            let (expected, terminal_state) = match mode {
                "recover-host-target-empty" => (
                    vec![RecoveryStep::Inspect, RecoveryStep::Restore],
                    "restart_required",
                ),
                "recover-host-target-full" => (
                    vec![RecoveryStep::Inspect, RecoveryStep::RecordCopied],
                    "restart_required",
                ),
                "recover-host-target-pending" => (
                    vec![
                        RecoveryStep::Inspect,
                        RecoveryStep::Reconcile,
                        RecoveryStep::Unlock,
                        RecoveryStep::RecordCopied,
                    ],
                    "restart_required",
                ),
                "recover-host-no-database" => (
                    vec![
                        RecoveryStep::Inspect,
                        RecoveryStep::InitializeEmpty,
                        RecoveryStep::Unlock,
                        RecoveryStep::Restore,
                    ],
                    "restart_required",
                ),
                "recover-host-reopened" => (vec![RecoveryStep::CompleteSelection], "handoff"),
                _ => unreachable!(),
            };
            assert_eq!(recovered["state"], terminal_state);
            assert_eq!(steps, expected);
            if terminal_state == "restart_required" {
                assert_eq!(
                    serde_json::to_value(host.status(|_| true, |_| true)).unwrap()["state"],
                    "restart_required"
                );
            } else {
                assert!(recovered["startupId"].is_null());
                assert!(recovered["attemptId"].is_null());
                assert!(recovered["epoch"].is_null());
            }
            let counts = *counts.lock().unwrap();
            assert!(counts[0] > 0);
            assert!(counts[1] > 0);
            assert_eq!(counts[2], 0);
            assert!(serde_json::to_value(access.request_view()).unwrap()["clientId"].is_null());
            PROBE.with(|probe| *probe.borrow_mut() = None);
        }
        "verify-native-replay-confirm" => {
            let access = SourceAccess::managed_deferred(
                Gate::new(),
                anchor.join("managed-workspace"),
                vault.clone(),
            );
            let close = WorkspaceClose::new(access);
            let started = serde_json::to_value(close.begin(|_| true)).unwrap();
            let id = started["attemptId"].as_str().unwrap().to_owned();
            let epoch = started["epoch"].as_str().unwrap().to_owned();
            assert_eq!(
                serde_json::to_value(close.acknowledge(&acknowledgement(&id, &epoch), |_| true))
                    .unwrap()["state"],
                "locked"
            );
            let old_flight = close.start_control(ControlKind::Startup, |_| true).unwrap();
            acknowledge_startup_flight(&close);
            let old_permit = old_flight.startup_permit().unwrap();
            let verification = Verification::prepare_native(anchor, &old_permit).unwrap();
            let (old_id, old_epoch) = control_identity(&close);
            let cancelled =
                serde_json::to_value(close.cancel_control(&old_id, &old_epoch, |_| true)).unwrap();
            let next_id = cancelled["attemptId"].as_str().unwrap().to_owned();
            let next_epoch = cancelled["epoch"].as_str().unwrap().to_owned();
            drop(old_flight);
            assert_eq!(
                serde_json::to_value(
                    close.acknowledge(&acknowledgement(&next_id, &next_epoch), |_| true)
                )
                .unwrap()["state"],
                "locked"
            );
            let fresh_flight = close.start_control(ControlKind::Startup, |_| true).unwrap();
            acknowledge_startup_flight(&close);
            let fresh_permit = fresh_flight.startup_permit().unwrap();
            assert!(matches!(
                verification
                    .confirm_native(&fresh_permit, || panic!("replayed verify reached callback")),
                Err(Error::Unavailable)
            ));
            drop(fresh_flight);
        }
        "verify-native-replay-select" => {
            let access = SourceAccess::managed_deferred(
                Gate::new(),
                anchor.join("managed-workspace"),
                vault.clone(),
            );
            let close = WorkspaceClose::new(access);
            let started = serde_json::to_value(close.begin(|_| true)).unwrap();
            let id = started["attemptId"].as_str().unwrap().to_owned();
            let epoch = started["epoch"].as_str().unwrap().to_owned();
            assert_eq!(
                serde_json::to_value(close.acknowledge(&acknowledgement(&id, &epoch), |_| true))
                    .unwrap()["state"],
                "locked"
            );
            let old_flight = close.start_control(ControlKind::Startup, |_| true).unwrap();
            acknowledge_startup_flight(&close);
            let old_permit = old_flight.startup_permit().unwrap();
            let verification = Verification::prepare_native(anchor, &old_permit).unwrap();
            let (verification, consent) = verification
                .confirm_native(&old_permit, || Consent::Approved)
                .unwrap()
                .unwrap();
            let (old_id, old_epoch) = control_identity(&close);
            let cancelled =
                serde_json::to_value(close.cancel_control(&old_id, &old_epoch, |_| true)).unwrap();
            let next_id = cancelled["attemptId"].as_str().unwrap().to_owned();
            let next_epoch = cancelled["epoch"].as_str().unwrap().to_owned();
            drop(old_flight);
            assert_eq!(
                serde_json::to_value(
                    close.acknowledge(&acknowledgement(&next_id, &next_epoch), |_| true)
                )
                .unwrap()["state"],
                "locked"
            );
            let fresh_flight = close.start_control(ControlKind::Startup, |_| true).unwrap();
            acknowledge_startup_flight(&close);
            let fresh_permit = fresh_flight.startup_permit().unwrap();
            assert!(matches!(
                verification.select_native(consent, &vault, &fresh_permit),
                Err(Error::Unavailable)
            ));
            drop(fresh_flight);
        }
        "verify-drift" => {
            let verification = Verification::prepare(anchor).unwrap();
            let consent = verification.synthetic_consent();
            assert!(verification.select(consent, &vault).is_err());
        }
        "verify-control-disappears" => {
            let control = anchor.join("managed-workspace/encrypted-workspace.lock");
            let mut checkpoints = 0;
            assert!(
                Verification::prepare_checked(anchor, &mut || {
                    checkpoints += 1;
                    if checkpoints == 2 {
                        fs::remove_file(&control).unwrap();
                    }
                    Ok(())
                })
                .is_err()
            );
            assert_eq!(checkpoints, 2);
            assert!(!control.exists());
            assert_eq!(fs::read_dir(vault.0).unwrap().count(), 1);
        }
        "verify-cancel" => {
            let verification = Verification::prepare(anchor).unwrap();
            drop(verification);
        }
        "verify-wrong-consent" => {
            let verification = Verification::prepare(anchor).unwrap();
            assert!(matches!(
                verification.select(
                    VerifyConsent {
                        token: "f".repeat(64)
                    },
                    &vault
                ),
                Err(Error::ConsentRequired)
            ));
        }
        "expect-refusal" => {
            if let Ok(verification) = Verification::prepare(anchor) {
                let consent = verification.synthetic_consent();
                assert!(verification.select(consent, &vault).is_err());
            }
        }
        _ => panic!("unsupported disposable test mode"),
    }
}
