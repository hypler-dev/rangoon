//! Explicit owner-managed backup, restore and deletion. No renderer file paths.
use super::{AppHandle, PublicError, StoreError, begin_operation, workspace};
use rangoon_host::{read_selected_backup, write_selected_backup};
use rangoon_store::{Backup, DeletionPlan, RecordKind, RestorePlan, WorkspaceData};
use serde::Serialize;
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
    fn selected(&self, id: &str, expected: &str) -> Result<Arc<Backup>, StoreError> {
        let guard = self.0.lock().map_err(|_| StoreError::Unavailable)?;
        let (backup, plan) = guard.as_ref().ok_or(StoreError::BackupInvalid)?;
        if backup.id() != id || plan.expected_state_id != expected {
            return Err(StoreError::WorkspaceChanged);
        }
        Ok(Arc::clone(backup))
    }
    fn consumed(&self) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = None;
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
        kind: RecordKind,
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
    let Some(pending) = begin_operation(&app) else {
        return DataResult::Failed {
            error: PublicError {
                code: "workspace_busy",
                message: "Finish the current local workspace operation first.",
            },
        };
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
    tauri::async_runtime::spawn_blocking(move || match workspace(&app).and_then(|w| w.data()) {
        Ok(workspace) => DataResult::Loaded { workspace },
        Err(error) => failure(error),
    })
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
        let bytes = match workspace(&app).and_then(|w| w.export_backup()) {
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
        let plan = match workspace(&app).and_then(|w| w.prepare_restore(&backup)) {
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
        let backup = match app
            .state::<BackupSession>()
            .selected(&backup_id, &expected_state_id)
        {
            Ok(backup) => backup,
            Err(error) => return failure(error),
        };
        match workspace(&app).and_then(|w| w.restore_backup(&backup, &expected_state_id)) {
            Ok(plan) => {
                app.state::<BackupSession>().consumed();
                DataResult::Restored { plan }
            }
            Err(error) => failure(error),
        }
    })
    .await
}

#[tauri::command]
pub async fn inspect_workspace_deletion(
    app: AppHandle,
    kind: RecordKind,
    id: String,
) -> DataResult {
    tauri::async_runtime::spawn_blocking(move || {
        match workspace(&app).and_then(|w| w.inspect_deletion(kind, &id)) {
            Ok(plan) => DataResult::DeletionReady { plan },
            Err(error) => failure(error),
        }
    })
    .await
    .unwrap_or_else(|_| failure(StoreError::Unavailable))
}

#[tauri::command]
pub async fn delete_workspace_record(
    app: AppHandle,
    kind: RecordKind,
    id: String,
    expected_state_id: String,
) -> DataResult {
    guarded(app, move |app| {
        match workspace(&app).and_then(|w| w.delete_record(kind, &id, &expected_state_id)) {
            Ok(()) => DataResult::Deleted { kind, id },
            Err(error) => failure(error),
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backup_session_binds_one_selected_backup_to_its_preview() {
        let path = std::env::temp_dir().join(format!(
            "rangoon-unused-backup-session-{}",
            std::process::id()
        ));
        let workspace = rangoon_store::Workspace::new(path);
        let bytes = workspace.export_backup().unwrap();
        let backup = Backup::decode(&bytes).unwrap();
        let plan = workspace.prepare_restore(&backup).unwrap();
        let session = BackupSession::default();
        assert!(
            session
                .selected(&plan.backup_id, &plan.expected_state_id)
                .is_err()
        );
        session.stage(backup, plan.clone()).unwrap();
        assert!(
            session
                .selected("wrong backup", &plan.expected_state_id)
                .is_err()
        );
        assert!(session.selected(&plan.backup_id, "wrong state").is_err());
        assert_eq!(
            session
                .selected(&plan.backup_id, &plan.expected_state_id)
                .unwrap()
                .byte_length(),
            bytes.len()
        );
        session.consumed();
        assert!(
            session
                .selected(&plan.backup_id, &plan.expected_state_id)
                .is_err()
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
