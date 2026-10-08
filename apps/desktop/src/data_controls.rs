//! Explicit owner-managed backup, restore and deletion. No renderer file paths.
use super::{
    AdmissionError, AppHandle, Gate, PublicError, StoreError, begin_operation, dispatch_read,
    workspace,
};
use rangoon_host::{read_selected_backup, write_selected_backup};
use rangoon_store::{
    WorkflowDeletionPlan as DeletionPlan, WorkflowRecordKind as RecordKind,
    WorkflowRestorePlan as RestorePlan, WorkflowWorkspaceData as WorkspaceData,
    WorkspaceBackup as Backup,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

#[derive(Default)]
pub struct BackupSession(Mutex<Option<(Arc<Backup>, RestorePlan)>>);
impl BackupSession {
    fn stage(&self, backup: Backup, plan: RestorePlan) -> Result<(), StoreError> {
        *self.0.lock().map_err(|_| StoreError::Unavailable)? = Some((Arc::new(backup), plan));
        Ok(())
    }
    fn claim(&self, id: &str, expected: &str) -> Result<(Arc<Backup>, RestorePlan), StoreError> {
        let mut guard = self.0.lock().map_err(|_| StoreError::Unavailable)?;
        let (backup, plan) = guard.as_ref().ok_or(StoreError::BackupInvalid)?;
        if backup.id() != id || plan.expected_state_id() != expected {
            return Err(StoreError::WorkspaceChanged);
        }
        // Claim before storage. Failed or uncertain results cannot replay this
        // attempt, and completing it cannot clear a newer prepared candidate.
        guard.take().ok_or(StoreError::BackupInvalid)
    }
}

/// Retain the exact dependency and removal preview; confirmation cannot replace it.
#[derive(Default)]
pub struct DeletionSession(Mutex<Option<DeletionPlan>>);
impl DeletionSession {
    fn stage(&self, plan: DeletionPlan) -> Result<(), StoreError> {
        *self.0.lock().map_err(|_| StoreError::Unavailable)? = Some(plan);
        Ok(())
    }
    fn claim(
        &self,
        kind: RecordKind,
        id: &str,
        expected: &str,
    ) -> Result<DeletionPlan, StoreError> {
        let mut guard = self.0.lock().map_err(|_| StoreError::Unavailable)?;
        let plan = guard.as_ref().ok_or(StoreError::WorkspaceChanged)?;
        if plan.kind() != kind || plan.id() != id || plan.expected_state_id() != expected {
            return Err(StoreError::WorkspaceChanged);
        }
        guard.take().ok_or(StoreError::WorkspaceChanged)
    }
}

/// Renderer selectors do not deserialize a store-issued plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NativeRecordKind {
    Source,
    Capability,
    Workflow,
}
impl NativeRecordKind {
    fn store(self) -> RecordKind {
        match self {
            Self::Source => RecordKind::Source,
            Self::Capability => RecordKind::Capability,
            Self::Workflow => RecordKind::Workflow,
        }
    }
}

#[derive(Serialize)]
#[serde(
    tag = "outcome",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum DataResult {
    Loaded {
        workspace: WorkspaceData,
    },
    Exported {
        backup_id: String,
        byte_length: usize,
    },
    Cancelled,
    RestoreReady {
        plan: RestorePlan,
    },
    Restored {
        plan: RestorePlan,
    },
    DeletionReady {
        plan: DeletionPlan,
    },
    Deleted {
        kind: NativeRecordKind,
        id: String,
    },
    Failed {
        error: PublicError,
    },
}
fn failure(error: StoreError) -> DataResult {
    // A transport/storage failure is not proof that a write rolled back. Unlike
    // source Save, these confirmations are never automatically retried.
    let (code, message) = if error == StoreError::Unavailable {
        (
            "workspace_unavailable",
            "The workspace operation did not return a confirmed result. Refresh saved data before preparing another operation.",
        )
    } else {
        error.public()
    };
    DataResult::Failed {
        error: PublicError { code, message },
    }
}
async fn guarded<F>(app: AppHandle, action: F) -> DataResult
where
    F: FnOnce(AppHandle) -> DataResult + Send + 'static,
{
    let pending = match begin_operation(&app) {
        Ok(lease) => lease,
        Err(AdmissionError::Busy) => {
            return DataResult::Failed {
                error: PublicError {
                    code: "workspace_busy",
                    message: "Finish the current local workspace operation first.",
                },
            };
        }
        Err(AdmissionError::Unavailable) => {
            return failure(StoreError::Unavailable);
        }
    };
    tauri::async_runtime::spawn_blocking(move || {
        let _pending = pending;
        action(app)
    })
    .await
    .unwrap_or_else(|_| failure(StoreError::Unavailable))
}

#[tauri::command]
pub async fn get_workspace_data(app: AppHandle) -> DataResult {
    let gate = app.state::<Gate>().inner().clone();
    let worker = match dispatch_read(&gate, move || {
        match workspace(&app).and_then(|w| w.workflow_data()) {
            Ok(workspace) => DataResult::Loaded { workspace },
            Err(error) => failure(error),
        }
    }) {
        Ok(worker) => worker,
        Err(AdmissionError::Busy) => {
            return DataResult::Failed {
                error: PublicError {
                    code: "workspace_busy",
                    message: "Finish the current local workspace operation first.",
                },
            };
        }
        Err(AdmissionError::Unavailable) => return failure(StoreError::Unavailable),
    };
    worker
        .await
        .unwrap_or_else(|_| failure(StoreError::Unavailable))
}

#[tauri::command]
pub async fn export_workspace_backup(app: AppHandle) -> DataResult {
    guarded(app, |app| {
        let Some(window) = app.get_webview_window("main") else {
            return failure(StoreError::Unavailable);
        };
        let Some(selected) = app
            .dialog()
            .file()
            .set_parent(&window)
            .set_title("Save an unencrypted Rangoon backup")
            .set_file_name("Rangoon.rangoon-backup")
            .add_filter("Rangoon backup", &["rangoon-backup"])
            .blocking_save_file()
        else {
            return DataResult::Cancelled;
        };
        let Ok(path) = selected.into_path() else {
            return failure(StoreError::Unavailable);
        };
        let bytes = match workspace(&app).and_then(|w| w.export_workflow_backup()) {
            Ok(bytes) => bytes,
            Err(error) => return failure(error),
        };
        if let Err(error) = write_selected_backup(&path, &bytes) {
            return DataResult::Failed { error };
        }
        DataResult::Exported {
            backup_id: format!("backup:{}", rangoon_domain::byte_digest(&bytes)),
            byte_length: bytes.len(),
        }
    })
    .await
}

#[tauri::command]
pub async fn prepare_workspace_restore(app: AppHandle) -> DataResult {
    guarded(app, |app| {
        let Some(window) = app.get_webview_window("main") else {
            return failure(StoreError::Unavailable);
        };
        let Some(selected) = app
            .dialog()
            .file()
            .set_parent(&window)
            .set_title("Choose a Rangoon backup to review")
            .add_filter("Rangoon backup", &["rangoon-backup"])
            .blocking_pick_file()
        else {
            return DataResult::Cancelled;
        };
        let Ok(path) = selected.into_path() else {
            return failure(StoreError::Unavailable);
        };
        let bytes = match read_selected_backup(&path) {
            Ok(bytes) => bytes,
            Err(error) => return DataResult::Failed { error },
        };
        let backup = match Backup::decode(&bytes) {
            Ok(backup) => backup,
            Err(error) => return failure(error),
        };
        drop(bytes);
        let plan = match workspace(&app).and_then(|w| w.prepare_workflow_restore(&backup)) {
            Ok(plan) => plan,
            Err(error) => return failure(error),
        };
        if let Err(error) = app.state::<BackupSession>().stage(backup, plan.clone()) {
            return failure(error);
        }
        DataResult::RestoreReady { plan }
    })
    .await
}

#[tauri::command]
pub async fn restore_workspace_backup(
    app: AppHandle,
    backup_id: String,
    expected_state_id: String,
) -> DataResult {
    guarded(app, move |app| {
        let (backup, plan) = match app
            .state::<BackupSession>()
            .claim(&backup_id, &expected_state_id)
        {
            Ok(backup) => backup,
            Err(error) => return failure(error),
        };
        match workspace(&app).and_then(|w| w.restore_workflow_backup(&backup, &plan)) {
            Ok(_) => DataResult::Restored { plan },
            Err(error) => failure(error),
        }
    })
    .await
}

#[tauri::command]
pub async fn inspect_workspace_deletion(
    app: AppHandle,
    kind: NativeRecordKind,
    id: String,
) -> DataResult {
    guarded(app, move |app| {
        match workspace(&app).and_then(|w| w.inspect_workflow_deletion(kind.store(), &id)) {
            Ok(plan) => {
                if let Err(error) = app.state::<DeletionSession>().stage(plan.clone()) {
                    return failure(error);
                }
                DataResult::DeletionReady { plan }
            }
            Err(error) => failure(error),
        }
    })
    .await
}

#[tauri::command]
pub async fn delete_workspace_record(
    app: AppHandle,
    kind: NativeRecordKind,
    id: String,
    expected_state_id: String,
) -> DataResult {
    guarded(app, move |app| {
        let plan = match app
            .state::<DeletionSession>()
            .claim(kind.store(), &id, &expected_state_id)
        {
            Ok(plan) => plan,
            Err(error) => return failure(error),
        };
        match workspace(&app).and_then(|w| w.delete_workflow_record(&plan)) {
            Ok(_) => DataResult::Deleted { kind, id },
            Err(error) => failure(error),
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use rangoon_store::Workspace;
    use rangoon_workflow::records::{SaveIntent, prepare_revision, workflow_id_from_nonce};

    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let mut bytes = [0; 32];
            getrandom::fill(&mut bytes).unwrap();
            Self(
                std::fs::canonicalize(std::env::temp_dir())
                    .unwrap()
                    .join(format!(
                        "rangoon-native-controls-{}",
                        rangoon_domain::byte_digest(&bytes)
                    )),
            )
        }
        fn store(&self) -> Workspace {
            Workspace::new(self.0.clone())
        }
        fn save(&self, seed: u8, parent: Option<&str>) -> String {
            self.save_with_reference(seed, parent, None)
        }
        fn save_with_reference(
            &self,
            seed: u8,
            parent: Option<&str>,
            capability: Option<&str>,
        ) -> String {
            let nodes = capability
                .map(|id| {
                    serde_json::json!([{
                        "id":"skill", "title":"Skill", "operation":{
                            "kind":"capability", "capabilityId":id, "revisionId":"missing"
                        }, "inputs":[], "outputs":[]
                    }])
                })
                .unwrap_or_else(|| serde_json::json!([]));
            let definition = serde_json::to_vec(&serde_json::json!({
                "schemaVersion":"rangoon.workflow-definition.v1", "title":"Unfinished workflow",
                "nodes":nodes, "controlEdges":[], "dataEdges":[]
            }))
            .unwrap();
            let record = prepare_revision(
                &workflow_id_from_nonce(&[seed; 32]),
                parent,
                SaveIntent::Draft,
                &definition,
                br#"{"positions":[]}"#,
            )
            .unwrap();
            let store = self.store();
            let plan = store.inspect_workflow_save(&record).unwrap();
            store
                .save_workflow(&record, &plan.expected_state_id)
                .unwrap();
            record.id().into()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn closed_native_kind_maps_without_deserializing_store_plans() {
        for (wire, native, store) in [
            ("source", NativeRecordKind::Source, RecordKind::Source),
            (
                "capability",
                NativeRecordKind::Capability,
                RecordKind::Capability,
            ),
            ("workflow", NativeRecordKind::Workflow, RecordKind::Workflow),
        ] {
            assert_eq!(
                serde_json::from_str::<NativeRecordKind>(&format!("\"{wire}\"")).unwrap(),
                native
            );
            assert_eq!(native.store(), store);
            assert_eq!(
                serde_json::to_string(&native).unwrap(),
                format!("\"{wire}\"")
            );
        }
        for bad in [r#""Workflow""#, r#""recipe""#, "null", "{}", "1"] {
            assert!(serde_json::from_str::<NativeRecordKind>(bad).is_err());
        }
    }

    #[test]
    fn backup_session_claim_binds_exact_selection_and_is_single_attempt() {
        let fixture = Fixture::new();
        let store = fixture.store();
        let bytes = store.export_workflow_backup().unwrap();
        let backup = Backup::decode(&bytes).unwrap();
        let plan = store.prepare_workflow_restore(&backup).unwrap();
        let session = BackupSession::default();
        assert!(
            session
                .claim(plan.backup_id(), plan.expected_state_id())
                .is_err()
        );
        session.stage(backup, plan.clone()).unwrap();
        assert!(
            session
                .claim("wrong backup", plan.expected_state_id())
                .is_err()
        );
        assert!(session.claim(plan.backup_id(), "wrong state").is_err());
        let (retained, retained_plan) = session
            .claim(plan.backup_id(), plan.expected_state_id())
            .unwrap();
        assert_eq!(retained_plan, plan);
        assert_eq!(retained.byte_length(), bytes.len());
        assert_eq!(retained.id(), plan.backup_id());
        // Even a lost callback cannot reuse the issued attempt.
        assert!(
            session
                .claim(plan.backup_id(), plan.expected_state_id())
                .is_err()
        );
        assert!(!fixture.0.exists());
    }

    #[test]
    fn native_retained_restore_preserves_v1_v2_and_complete_v3_history() {
        let source = Fixture::new();
        let store = source.store();
        let legacy = [
            store.export_backup().unwrap(),
            store.export_composition_backup().unwrap(),
        ];
        for bytes in legacy {
            let destination = Fixture::new();
            let backup = Backup::decode(&bytes).unwrap();
            let plan = destination
                .store()
                .prepare_workflow_restore(&backup)
                .unwrap();
            let session = BackupSession::default();
            session.stage(backup, plan.clone()).unwrap();
            let (retained, issued) = session
                .claim(plan.backup_id(), plan.expected_state_id())
                .unwrap();
            let data = destination
                .store()
                .restore_workflow_backup(&retained, &issued)
                .unwrap();
            assert_eq!(
                serde_json::to_value(data).unwrap()["schemaVersion"],
                "rangoon.workspace-data.v2"
            );
            assert!(!destination.0.exists());
        }
        let root = source.save(1, None);
        source.save(1, Some(&root));
        let bytes = store.export_workflow_backup().unwrap();
        assert!(bytes.starts_with(b"RANGOON-BACKUP-V3\n"));
        let destination = Fixture::new();
        let backup = Backup::decode(&bytes).unwrap();
        let plan = destination
            .store()
            .prepare_workflow_restore(&backup)
            .unwrap();
        assert!(!destination.0.exists());
        let session = BackupSession::default();
        session.stage(backup, plan.clone()).unwrap();
        let (retained, issued) = session
            .claim(plan.backup_id(), plan.expected_state_id())
            .unwrap();
        let receipt = destination
            .store()
            .restore_workflow_backup(&retained, &issued)
            .unwrap();
        assert_eq!(receipt.records.workflows, 1);
        assert_eq!(receipt.records.workflow_revisions, 2);
        assert_eq!(
            destination
                .store()
                .open_workflow(&workflow_id_from_nonce(&[1; 32]), None)
                .unwrap()
                .history
                .len(),
            2
        );
        assert_eq!(destination.store().export_workflow_backup().unwrap(), bytes);
        let result = serde_json::to_value(DataResult::Restored { plan: issued }).unwrap();
        assert_eq!(result["plan"]["schemaVersion"], "rangoon.restore-plan.v2");
        assert_eq!(result["plan"]["add"]["workflowRevisions"], 2);
        assert_eq!(result["plan"]["add"]["sources"], 0);
        assert!(
            session
                .claim(plan.backup_id(), plan.expected_state_id())
                .is_err()
        );
    }

    #[test]
    fn replaced_deletion_plan_keeps_other_workflows_and_inputs() {
        let fixture = Fixture::new();
        fixture.save(1, None);
        let second = fixture.save(2, None);
        fixture.save(2, Some(&second));
        let store = fixture.store();
        let one = workflow_id_from_nonce(&[1; 32]);
        let two = workflow_id_from_nonce(&[2; 32]);
        let prior = store
            .inspect_workflow_deletion(RecordKind::Workflow, &one)
            .unwrap();
        let replacement = store
            .inspect_workflow_deletion(RecordKind::Workflow, &two)
            .unwrap();
        let session = DeletionSession::default();
        session.stage(prior.clone()).unwrap();
        assert!(
            session
                .claim(RecordKind::Source, &one, prior.expected_state_id())
                .is_err()
        );
        assert!(
            session
                .claim(RecordKind::Workflow, "wrong", prior.expected_state_id())
                .is_err()
        );
        assert!(session.claim(RecordKind::Workflow, &one, "stale").is_err());
        session.stage(replacement.clone()).unwrap();
        assert!(
            session
                .claim(RecordKind::Workflow, &one, prior.expected_state_id())
                .is_err()
        );
        let issued = session
            .claim(RecordKind::Workflow, &two, replacement.expected_state_id())
            .unwrap();
        assert_eq!(issued, replacement);
        let impact = serde_json::to_value(&issued).unwrap();
        assert_eq!(impact["schemaVersion"], "rangoon.deletion-plan.v2");
        assert_eq!(impact["remove"]["workflows"], 1);
        assert_eq!(impact["remove"]["workflowRevisions"], 2);
        assert_eq!(impact["remove"]["sources"], 0);
        let data = store.delete_workflow_record(&issued).unwrap();
        assert_eq!(data.records.workflows, 1);
        assert_eq!(data.records.workflow_revisions, 1);
        assert_eq!(store.open_workflow(&one, None).unwrap().head.id, one);
        assert_eq!(
            store.open_workflow(&two, None).unwrap_err(),
            StoreError::WorkflowNotFound
        );
        assert!(
            session
                .claim(RecordKind::Workflow, &two, replacement.expected_state_id())
                .is_err()
        );
        // Completion of the first claim cannot erase a newly staged candidate.
        let fresh = store
            .inspect_workflow_deletion(RecordKind::Workflow, &one)
            .unwrap();
        session.stage(fresh.clone()).unwrap();
        assert_eq!(
            session
                .claim(RecordKind::Workflow, &one, fresh.expected_state_id())
                .unwrap(),
            fresh
        );
    }

    #[test]
    fn stale_store_refusals_consume_native_attempt_and_preserve_complete_data() {
        let source = Fixture::new();
        source.save(1, None);
        let target = Fixture::new();
        let store = target.store();
        let backup = Backup::decode(&source.store().export_workflow_backup().unwrap()).unwrap();
        let plan = store.prepare_workflow_restore(&backup).unwrap();
        let restore = BackupSession::default();
        restore.stage(backup, plan.clone()).unwrap();
        target.save(2, None);
        let before = std::fs::read(target.0.join("workspace.sqlite3")).unwrap();
        let (backup, issued) = restore
            .claim(plan.backup_id(), plan.expected_state_id())
            .unwrap();
        assert_eq!(
            store.restore_workflow_backup(&backup, &issued).unwrap_err(),
            StoreError::WorkspaceChanged
        );
        assert!(
            restore
                .claim(plan.backup_id(), plan.expected_state_id())
                .is_err()
        );
        assert_eq!(
            std::fs::read(target.0.join("workspace.sqlite3")).unwrap(),
            before
        );
        let id = workflow_id_from_nonce(&[2; 32]);
        let plan = store
            .inspect_workflow_deletion(RecordKind::Workflow, &id)
            .unwrap();
        let deletion = DeletionSession::default();
        deletion.stage(plan.clone()).unwrap();
        target.save(3, None);
        let before = std::fs::read(target.0.join("workspace.sqlite3")).unwrap();
        let issued = deletion
            .claim(RecordKind::Workflow, &id, plan.expected_state_id())
            .unwrap();
        assert_eq!(
            store.delete_workflow_record(&issued).unwrap_err(),
            StoreError::WorkspaceChanged
        );
        assert!(
            deletion
                .claim(RecordKind::Workflow, &id, plan.expected_state_id())
                .is_err()
        );
        assert_eq!(
            std::fs::read(target.0.join("workspace.sqlite3")).unwrap(),
            before
        );
        assert_eq!(store.workflow_data().unwrap().records.workflows, 2);
    }

    #[test]
    fn historical_draft_blocks_skill_delete_and_workflow_delete_preserves_inputs() {
        let fixture = Fixture::new();
        let store = fixture.store();
        let source = rangoon_import::analyze("AGENTS.md", b"# Rules\r\nKeep inputs.\r\n").unwrap();
        store.save_v1(&source).unwrap();
        let skill = store
            .create_capability_v1(&source.source.id, &source.fragments[0].id, "Rules")
            .unwrap()
            .capability;
        let root = fixture.save_with_reference(1, None, Some(&skill.id));
        fixture.save(1, Some(&root));
        let plan = store
            .inspect_workflow_deletion(RecordKind::Capability, &skill.id)
            .unwrap();
        let impact = serde_json::to_value(&plan).unwrap();
        assert_eq!(impact["workflowDependencies"].as_array().unwrap().len(), 1);
        assert_eq!(impact["workflowDependencies"][0]["unresolvedReferences"], 0);
        let session = DeletionSession::default();
        session.stage(plan.clone()).unwrap();
        let issued = session
            .claim(RecordKind::Capability, &skill.id, plan.expected_state_id())
            .unwrap();
        let before = std::fs::read(fixture.0.join("workspace.sqlite3")).unwrap();
        assert_eq!(
            store.delete_workflow_record(&issued).unwrap_err(),
            StoreError::RecordInUse
        );
        assert_eq!(
            std::fs::read(fixture.0.join("workspace.sqlite3")).unwrap(),
            before
        );
        assert!(
            session
                .claim(RecordKind::Capability, &skill.id, plan.expected_state_id())
                .is_err()
        );
        let id = workflow_id_from_nonce(&[1; 32]);
        let plan = store
            .inspect_workflow_deletion(RecordKind::Workflow, &id)
            .unwrap();
        session.stage(plan.clone()).unwrap();
        let issued = session
            .claim(RecordKind::Workflow, &id, plan.expected_state_id())
            .unwrap();
        let data = store.delete_workflow_record(&issued).unwrap();
        assert_eq!(
            (
                data.records.sources,
                data.records.capabilities,
                data.records.workflows
            ),
            (1, 1, 0)
        );
        assert_eq!(store.open(&source.source.id).unwrap(), source);
        assert_eq!(store.open_capability_v1(&skill.id, None).unwrap(), skill);
        assert!(
            serde_json::to_value(
                store
                    .inspect_workflow_deletion(RecordKind::Capability, &skill.id)
                    .unwrap()
            )
            .unwrap()["workflowDependencies"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn backup_caps_and_local_permissions_match() {
        assert_eq!(
            rangoon_host::MAX_BACKUP_FILE_BYTES,
            rangoon_store::MAX_BACKUP_BYTES
        );
        let permissions = include_str!("../capabilities/source-analysis.json");
        let manifest = include_str!("../build.rs");
        for command in [
            "get_workspace_data",
            "export_workspace_backup",
            "prepare_workspace_restore",
            "restore_workspace_backup",
            "inspect_workspace_deletion",
            "delete_workspace_record",
        ] {
            assert!(manifest.contains(&format!("\"{command}\"")));
            assert!(permissions.contains(&format!("\"allow-{}\"", command.replace('_', "-"))));
        }
        assert!(permissions.contains("\"local\": true"));
        assert!(!permissions.contains("\"remote\""));
    }
}
