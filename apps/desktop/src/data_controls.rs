//! Explicit owner-managed backup, restore and deletion. No renderer file paths.
use super::{
    AdmissionError, AppHandle, PublicError, StoreError, workspace_source_access::SourceAccess,
};
use rangoon_desktop::workspace_lifecycle::GenerationWitness;
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
pub struct BackupSession(Mutex<Option<RetainedBackup>>);
struct RetainedBackup {
    backup: Arc<Backup>,
    plan: RestorePlan,
    generation: Option<GenerationWitness>,
}
impl BackupSession {
    #[cfg(test)]
    pub(super) fn retained_for_close_test(&self) -> bool {
        self.0.lock().unwrap().is_some()
    }
    #[cfg(test)]
    fn stage(&self, backup: Backup, plan: RestorePlan) -> Result<(), StoreError> {
        self.stage_inner(backup, plan, None)
    }
    fn stage_inner(
        &self,
        backup: Backup,
        plan: RestorePlan,
        generation: Option<GenerationWitness>,
    ) -> Result<(), StoreError> {
        let mut guard = self.0.lock().map_err(|_| StoreError::Unavailable)?;
        if generation
            .as_ref()
            .is_some_and(|generation| !generation.is_current().unwrap_or(false))
        {
            *guard = None;
            return Err(StoreError::Unavailable);
        }
        *guard = Some(RetainedBackup {
            backup: Arc::new(backup),
            plan,
            generation,
        });
        Ok(())
    }
    pub(super) fn stage_routed(
        &self,
        backup: Backup,
        plan: RestorePlan,
        generation: GenerationWitness,
    ) -> Result<(), StoreError> {
        if !generation.is_current().unwrap_or(false) {
            return Err(StoreError::Unavailable);
        }
        self.stage_inner(backup, plan, Some(generation))
    }
    #[cfg(test)]
    fn claim(&self, id: &str, expected: &str) -> Result<(Arc<Backup>, RestorePlan), StoreError> {
        self.claim_inner(id, expected, false)
    }
    pub(super) fn claim_routed(
        &self,
        id: &str,
        expected: &str,
    ) -> Result<(Arc<Backup>, RestorePlan), StoreError> {
        self.claim_inner(id, expected, true)
    }
    fn claim_inner(
        &self,
        id: &str,
        expected: &str,
        routed: bool,
    ) -> Result<(Arc<Backup>, RestorePlan), StoreError> {
        let mut guard = self.0.lock().map_err(|_| StoreError::Unavailable)?;
        let retained = guard.as_ref().ok_or(StoreError::BackupInvalid)?;
        if retained.backup.id() != id || retained.plan.expected_state_id() != expected {
            return Err(StoreError::WorkspaceChanged);
        }
        if routed
            && !retained
                .generation
                .as_ref()
                .is_some_and(|generation| generation.is_current().unwrap_or(false))
        {
            *guard = None;
            return Err(StoreError::Unavailable);
        }
        let retained = guard.take().ok_or(StoreError::BackupInvalid)?;
        Ok((retained.backup, retained.plan))
    }
    pub(super) fn purge(&self) -> bool {
        match self.0.lock() {
            Ok(mut guard) => {
                *guard = None;
                true
            }
            Err(error) => {
                *error.into_inner() = None;
                false
            }
        }
    }
}

/// Retain the exact dependency and removal preview; confirmation cannot replace it.
#[derive(Default)]
pub struct DeletionSession(Mutex<Option<RetainedDeletion>>);
struct RetainedDeletion {
    plan: DeletionPlan,
    generation: Option<GenerationWitness>,
}
impl DeletionSession {
    #[cfg(test)]
    pub(super) fn retained_for_close_test(&self) -> bool {
        self.0.lock().unwrap().is_some()
    }
    #[cfg(test)]
    fn stage(&self, plan: DeletionPlan) -> Result<(), StoreError> {
        self.stage_inner(plan, None)
    }
    fn stage_inner(
        &self,
        plan: DeletionPlan,
        generation: Option<GenerationWitness>,
    ) -> Result<(), StoreError> {
        let mut guard = self.0.lock().map_err(|_| StoreError::Unavailable)?;
        if generation
            .as_ref()
            .is_some_and(|generation| !generation.is_current().unwrap_or(false))
        {
            *guard = None;
            return Err(StoreError::Unavailable);
        }
        *guard = Some(RetainedDeletion { plan, generation });
        Ok(())
    }
    pub(super) fn stage_routed(
        &self,
        plan: DeletionPlan,
        generation: GenerationWitness,
    ) -> Result<(), StoreError> {
        if !generation.is_current().unwrap_or(false) {
            return Err(StoreError::Unavailable);
        }
        self.stage_inner(plan, Some(generation))
    }
    #[cfg(test)]
    fn claim(
        &self,
        kind: RecordKind,
        id: &str,
        expected: &str,
    ) -> Result<DeletionPlan, StoreError> {
        self.claim_inner(kind, id, expected, false)
    }
    pub(super) fn claim_routed(
        &self,
        kind: RecordKind,
        id: &str,
        expected: &str,
    ) -> Result<DeletionPlan, StoreError> {
        self.claim_inner(kind, id, expected, true)
    }
    fn claim_inner(
        &self,
        kind: RecordKind,
        id: &str,
        expected: &str,
        routed: bool,
    ) -> Result<DeletionPlan, StoreError> {
        let mut guard = self.0.lock().map_err(|_| StoreError::Unavailable)?;
        let retained = guard.as_ref().ok_or(StoreError::WorkspaceChanged)?;
        if retained.plan.kind() != kind
            || retained.plan.id() != id
            || retained.plan.expected_state_id() != expected
        {
            return Err(StoreError::WorkspaceChanged);
        }
        if routed
            && !retained
                .generation
                .as_ref()
                .is_some_and(|generation| generation.is_current().unwrap_or(false))
        {
            *guard = None;
            return Err(StoreError::Unavailable);
        }
        Ok(guard.take().ok_or(StoreError::WorkspaceChanged)?.plan)
    }
    pub(super) fn purge(&self) -> bool {
        match self.0.lock() {
            Ok(mut guard) => {
                *guard = None;
                true
            }
            Err(error) => {
                *error.into_inner() = None;
                false
            }
        }
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
    pub(super) fn store(self) -> RecordKind {
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
        encrypted_recovery_supported: bool,
    },
    Exported {
        backup_id: String,
        byte_length: usize,
    },
    RecoveryKeyCreated,
    EncryptedExported {
        format: &'static str,
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
pub(super) fn failure(error: StoreError) -> DataResult {
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
pub(super) fn admission(error: AdmissionError) -> DataResult {
    match error {
        AdmissionError::Busy => DataResult::Failed {
            error: PublicError {
                code: "workspace_busy",
                message: "Finish the current local workspace operation first.",
            },
        },
        AdmissionError::Unavailable => failure(StoreError::Unavailable),
    }
}
fn pick(app: &AppHandle, exporting: bool) -> Result<Option<std::path::PathBuf>, PublicError> {
    let window = app.get_webview_window("main").ok_or(PublicError {
        code: "workspace_unavailable",
        message: "The native backup picker is unavailable.",
    })?;
    let dialog = app
        .dialog()
        .file()
        .set_parent(&window)
        .add_filter("Rangoon backup", &["rangoon-backup"]);
    let selected = if exporting {
        dialog
            .set_title("Save an unencrypted Rangoon backup")
            .set_file_name("Rangoon.rangoon-backup")
            .blocking_save_file()
    } else {
        dialog
            .set_title("Choose a Rangoon backup to review")
            .blocking_pick_file()
    };
    selected
        .map(|value| {
            value.into_path().map_err(|_| PublicError {
                code: "workspace_unavailable",
                message: "The native backup selection is unavailable.",
            })
        })
        .transpose()
}

fn pick_recovery(
    app: &AppHandle,
    key: bool,
    saving: bool,
) -> Result<Option<std::path::PathBuf>, PublicError> {
    let window = app.get_webview_window("main").ok_or(PublicError {
        code: "workspace_unavailable",
        message: "The native recovery picker is unavailable.",
    })?;
    let (label, extension, name, title) = match (key, saving) {
        (true, true) => (
            "Rangoon recovery key",
            "rangoon-key",
            "Rangoon.rangoon-key",
            "Save recovery key separately from your backup",
        ),
        (true, false) => (
            "Rangoon recovery key",
            "rangoon-key",
            "Rangoon.rangoon-key",
            "Choose the recovery key for this backup",
        ),
        (false, true) => (
            "Encrypted Rangoon backup",
            "rangoon-encrypted",
            "Rangoon.rangoon-encrypted",
            "Save encrypted workspace backup",
        ),
        (false, false) => (
            "Encrypted Rangoon backup",
            "rangoon-encrypted",
            "Rangoon.rangoon-encrypted",
            "Choose encrypted workspace backup",
        ),
    };
    let dialog = app
        .dialog()
        .file()
        .set_parent(&window)
        .add_filter(label, &[extension])
        .set_title(title);
    let selected = if saving {
        dialog.set_file_name(name).blocking_save_file()
    } else {
        dialog.blocking_pick_file()
    };
    selected
        .map(|value| {
            value
                .into_path()
                .map_err(|_| crate::encrypted_recovery::invalid())
        })
        .transpose()
}

#[tauri::command]
pub async fn create_workspace_recovery_key(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
) -> DataResult {
    let operation = match app
        .state::<SourceAccess>()
        .for_request(&workspace)
        .begin_file()
    {
        Ok(op) => op,
        Err(error) => return admission(error),
    };
    match tauri::async_runtime::spawn_blocking(move || {
        operation.create_recovery_key(|| pick_recovery(&app, true, true))
    })
    .await
    {
        Ok(completed) => completed.deliver(),
        Err(_) => DataResult::Failed {
            error: PublicError {
                code: "workspace_recovery_output_uncertain",
                message: "Key creation did not return a confirmed result. A secret key file may remain; inspect your selected location before retrying.",
            },
        },
    }
}
#[tauri::command]
pub async fn export_encrypted_workspace_backup(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
) -> DataResult {
    let operation = match app
        .state::<SourceAccess>()
        .for_request(&workspace)
        .begin_file()
    {
        Ok(op) => op,
        Err(error) => return admission(error),
    };
    match tauri::async_runtime::spawn_blocking(move || {
        operation.export_encrypted_backup(
            || pick_recovery(&app, true, false),
            || pick_recovery(&app, false, true),
            crate::encrypted_recovery::write_cipher,
            crate::encrypted_recovery::read_cipher,
        )
    })
    .await
    {
        Ok(completed) => completed.deliver(),
        Err(_) => DataResult::Failed {
            error: PublicError {
                code: "workspace_recovery_output_uncertain",
                message: "Backup did not return a confirmed result. A file may remain; inspect your selected output and use a new name before retrying.",
            },
        },
    }
}
#[tauri::command]
pub async fn prepare_encrypted_workspace_restore(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
) -> DataResult {
    let access = app.state::<SourceAccess>().for_request(&workspace);
    let operation = match access.begin_file() {
        Ok(op) => op,
        Err(error) => return admission(error),
    };
    match tauri::async_runtime::spawn_blocking(move || {
        operation.prepare_encrypted_restore(
            || pick_recovery(&app, false, false),
            || pick_recovery(&app, true, false),
            crate::encrypted_recovery::read_cipher,
        )
    })
    .await
    {
        Ok(completed) => completed.deliver(),
        Err(_) => {
            access.purge_data_controls();
            failure(StoreError::Unavailable)
        }
    }
}

#[tauri::command]
pub async fn get_workspace_data(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
) -> DataResult {
    let access = app.state::<SourceAccess>().for_request(&workspace);
    access.workflow_data().await
}
#[tauri::command]
pub async fn export_workspace_backup(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
) -> DataResult {
    let operation = match app
        .state::<SourceAccess>()
        .for_request(&workspace)
        .begin_file()
    {
        Ok(operation) => operation,
        Err(error) => return admission(error),
    };
    match tauri::async_runtime::spawn_blocking(move || {
        operation.export_backup(|| pick(&app, true), write_selected_backup)
    })
    .await
    {
        Ok(completed) => completed.deliver(),
        Err(_) => DataResult::Failed {
            error: PublicError {
                code: "workspace_export_uncertain",
                message: "Export did not return a confirmed result. A file may remain; inspect it and choose a new name before retrying.",
            },
        },
    }
}
#[tauri::command]
pub async fn prepare_workspace_restore(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
) -> DataResult {
    let access = app.state::<SourceAccess>().for_request(&workspace);
    let operation = match access.begin_file() {
        Ok(operation) => operation,
        Err(error) => return admission(error),
    };
    match tauri::async_runtime::spawn_blocking(move || {
        operation.prepare_restore(|| pick(&app, false), read_selected_backup)
    })
    .await
    {
        Ok(completed) => completed.deliver(),
        Err(_) => {
            access.purge_data_controls();
            failure(StoreError::Unavailable)
        }
    }
}
#[tauri::command]
pub async fn restore_workspace_backup(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
    backup_id: String,
    expected_state_id: String,
) -> DataResult {
    app.state::<SourceAccess>()
        .for_request(&workspace)
        .restore_workspace_backup(backup_id, expected_state_id)
        .await
}
#[tauri::command]
pub async fn inspect_workspace_deletion(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
    kind: NativeRecordKind,
    id: String,
) -> DataResult {
    app.state::<SourceAccess>()
        .for_request(&workspace)
        .inspect_workspace_deletion(kind, id)
        .await
}
#[tauri::command]
pub async fn delete_workspace_record(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
    kind: NativeRecordKind,
    id: String,
    expected_state_id: String,
) -> DataResult {
    app.state::<SourceAccess>()
        .for_request(&workspace)
        .delete_workspace_record(kind, id, expected_state_id)
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
