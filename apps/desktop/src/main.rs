#![forbid(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

mod capabilities;
mod compilation;
use compilation::compile_capability;
mod composition;
use composition::{CompositionSession, commit_composition, preview_composition};
mod data_controls;
use data_controls::{
    BackupSession, DeletionSession, delete_workspace_record, export_workspace_backup,
    get_workspace_data, inspect_workspace_deletion, prepare_workspace_restore,
    restore_workspace_backup,
};
mod instruction_bundles;
use instruction_bundles::{export_instruction_bundle, inspect_instruction_bundle};
mod cloud_credentials;
mod cloud_custody;
mod cloud_model_consent;
mod cloud_models;
mod cloud_store;
use cloud_credentials::*;
use cloud_models::{
    NativeCloud, cancel_cloud_model, cancel_cloud_model_review, check_cloud_model,
    clear_cloud_model, configure_cloud_model, confirm_cloud_model_review, get_cloud_model_profile,
    get_cloud_model_review, prepare_cloud_model, send_cloud_model,
};
mod model_consent;
mod model_flight;
mod models;
mod session;
use capabilities::{
    create_capability, list_capabilities, open_capability, review_capability, revise_capability,
};
use models::*;

use rangoon_engine::{EngineStatus, GovernancePort, LnsatPlaceholder};
use rangoon_host::{PublicError, SelectionResult, analyze_selected_path};
use rangoon_store::{SnapshotMetadata, StoreError, Workspace};
use serde::Serialize;
use session::Session;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::DialogExt;

#[derive(Default)]
struct PickerState(Arc<AtomicBool>);

struct PendingPicker(Arc<AtomicBool>);
impl Drop for PendingPicker {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// The renderer supplies no path, content or options. Every read starts with a
/// fresh native picker. The host retains the current report for explicit saving.
#[tauri::command]
async fn select_and_analyze(app: AppHandle) -> SelectionResult {
    let Some(generation) = app.state::<Session>().generation() else {
        return SelectionResult::failed(
            "session_unavailable",
            "Reopen Rangoon to restore the source workbench.",
        );
    };
    let busy = Arc::clone(&app.state::<PickerState>().0);
    if busy.swap(true, Ordering::AcqRel) {
        return SelectionResult::failed(
            "selection_busy",
            "Finish or cancel the current file selection first.",
        );
    }
    let pending = PendingPicker(busy);
    tauri::async_runtime::spawn_blocking(move || {
        let _pending = pending;
        let Some(window) = app.get_webview_window("main") else {
            return SelectionResult::failed(
                "selection_failed",
                "The source workbench is unavailable.",
            );
        };
        let selected = app
            .dialog()
            .file()
            .set_parent(&window)
            .set_title("Choose a Markdown file to analyze")
            .add_filter("Markdown", &["md"])
            .blocking_pick_file();
        let result = match selected {
            None => SelectionResult::Cancelled,
            Some(selected) => match selected.into_path() {
                Ok(path) => analyze_selected_path(&path),
                Err(_) => SelectionResult::failed(
                    "selection_failed",
                    "Choose a regular file available on this computer.",
                ),
            },
        };
        accept_result(&app, generation, result)
    })
    .await
    .unwrap_or_else(|_| {
        SelectionResult::failed(
            "selection_failed",
            "File selection did not complete. Try again.",
        )
    })
}

fn accept_result(app: &AppHandle, generation: u64, result: SelectionResult) -> SelectionResult {
    if let SelectionResult::Analyzed { ref report } = result
        && !app.state::<Session>().accept(generation, report)
    {
        return SelectionResult::failed(
            "analysis_cleared",
            "This analysis was cleared. Choose or open a source again.",
        );
    }
    result
}

#[derive(Serialize)]
#[serde(
    tag = "outcome",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum WorkspaceResult {
    Listed {
        snapshots: Vec<SnapshotMetadata>,
    },
    Saved {
        snapshot: SnapshotMetadata,
        already_saved: bool,
    },
    Cleared,
    Failed {
        error: PublicError,
    },
}
fn workspace_error(error: StoreError) -> WorkspaceResult {
    let (code, message) = error.public();
    WorkspaceResult::Failed {
        error: PublicError { code, message },
    }
}
fn workspace(app: &AppHandle) -> Result<Workspace, StoreError> {
    app.path()
        .app_local_data_dir()
        .map(|path| Workspace::new(path.join("source-workspace")))
        .map_err(|_| StoreError::Unavailable)
}
fn begin_operation(app: &AppHandle) -> Option<PendingPicker> {
    let busy = Arc::clone(&app.state::<PickerState>().0);
    if busy.swap(true, Ordering::AcqRel) {
        None
    } else {
        Some(PendingPicker(busy))
    }
}
#[tauri::command]
async fn list_snapshots(app: AppHandle) -> WorkspaceResult {
    tauri::async_runtime::spawn_blocking(move || {
        match workspace(&app).and_then(|store| store.list()) {
            Ok(snapshots) => WorkspaceResult::Listed { snapshots },
            Err(error) => workspace_error(error),
        }
    })
    .await
    .unwrap_or_else(|_| workspace_error(StoreError::Unavailable))
}
#[tauri::command]
async fn save_analysis(app: AppHandle, source_id: String) -> WorkspaceResult {
    if rangoon_store::validate_id(&source_id).is_err() {
        return workspace_error(StoreError::InvalidId);
    }
    let Some(pending) = begin_operation(&app) else {
        return WorkspaceResult::Failed {
            error: PublicError {
                code: "analysis_busy",
                message: "Finish the current source operation first.",
            },
        };
    };
    let Some(report) = app.state::<Session>().snapshot(&source_id) else {
        return WorkspaceResult::Failed {
            error: PublicError {
                code: "analysis_changed",
                message: "The current analysis changed. Choose or open the source before saving.",
            },
        };
    };
    // An explicit save owns this copy. Clear never cancels a disk commit.
    tauri::async_runtime::spawn_blocking(move || {
        let _pending = pending;
        match workspace(&app).and_then(|store| store.save_v1(&report)) {
            Ok(receipt) => WorkspaceResult::Saved {
                snapshot: receipt.snapshot,
                already_saved: receipt.already_saved,
            },
            Err(error) => workspace_error(error),
        }
    })
    .await
    .unwrap_or_else(|_| workspace_error(StoreError::Unavailable))
}
#[tauri::command]
async fn open_snapshot(app: AppHandle, source_id: String) -> SelectionResult {
    let Some(pending) = begin_operation(&app) else {
        return SelectionResult::failed(
            "analysis_busy",
            "Finish the current source operation first.",
        );
    };
    let Some(generation) = app.state::<Session>().generation() else {
        return SelectionResult::failed(
            "session_unavailable",
            "Reopen Rangoon to restore the source workbench.",
        );
    };
    tauri::async_runtime::spawn_blocking(move || {
        let _pending = pending;
        match workspace(&app).and_then(|store| store.open(&source_id)) {
            Ok(report) => accept_result(
                &app,
                generation,
                SelectionResult::Analyzed {
                    report: Box::new(report),
                },
            ),
            Err(error) => {
                let (code, message) = error.public();
                SelectionResult::failed(code, message)
            }
        }
    })
    .await
    .unwrap_or_else(|_| {
        SelectionResult::failed(
            "workspace_unavailable",
            "The saved snapshot could not be opened. Retry.",
        )
    })
}

/// Read a saved composition input without replacing the Import workbench's
/// selected report. The renderer supplies only a content-addressed source ID.
#[tauri::command]
async fn read_composition_source(app: AppHandle, source_id: String) -> SelectionResult {
    if rangoon_store::validate_id(&source_id).is_err() {
        let (code, message) = StoreError::InvalidId.public();
        return SelectionResult::failed(code, message);
    }
    let Some(pending) = begin_operation(&app) else {
        return SelectionResult::failed(
            "analysis_busy",
            "Finish the current source operation first.",
        );
    };
    tauri::async_runtime::spawn_blocking(move || {
        let _pending = pending;
        match workspace(&app) {
            Ok(store) => read_saved_composition_source(&store, &source_id),
            Err(error) => {
                let (code, message) = error.public();
                SelectionResult::failed(code, message)
            }
        }
    })
    .await
    .unwrap_or_else(|_| {
        SelectionResult::failed(
            "workspace_unavailable",
            "The saved composition source could not be opened. Retry.",
        )
    })
}

fn read_saved_composition_source(store: &Workspace, source_id: &str) -> SelectionResult {
    match store.open(source_id) {
        Ok(report) => SelectionResult::Analyzed {
            report: Box::new(report),
        },
        Err(error) => {
            let (code, message) = error.public();
            SelectionResult::failed(code, message)
        }
    }
}

#[tauri::command]
fn clear_analysis(app: AppHandle) -> WorkspaceResult {
    if app.state::<Session>().clear() {
        WorkspaceResult::Cleared
    } else {
        workspace_error(StoreError::Unavailable)
    }
}

/// Describes this build's integration only. Never discovers or contacts LNSAT.
#[tauri::command]
fn get_engine_status() -> EngineStatus {
    LnsatPlaceholder.status()
}

fn bundled_navigation(url: &tauri::Url) -> bool {
    matches!(
        (url.scheme(), url.host_str()),
        ("tauri", Some("localhost")) | ("http", Some("tauri.localhost"))
    )
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(PickerState::default())
        .manage(Session::default())
        .manage(BackupSession::default())
        .manage(DeletionSession::default())
        .manage(CompositionSession::default())
        .manage(NativeModel::default())
        .manage(NativeCloud::default())
        .manage(model_flight::ModelFlight::default())
        .manage(cloud_custody::Custody::default())
        .on_window_event(|window, event| {
            models::window_event(window, event);
            cloud_models::window_event(window, event);
            cloud_credentials::window_event(window, event);
        })
        .invoke_handler(tauri::generate_handler![
            select_and_analyze,
            list_snapshots,
            save_analysis,
            open_snapshot,
            read_composition_source,
            clear_analysis,
            get_engine_status,
            list_capabilities,
            open_capability,
            create_capability,
            revise_capability,
            review_capability,
            compile_capability,
            export_instruction_bundle,
            inspect_instruction_bundle,
            get_workspace_data,
            export_workspace_backup,
            prepare_workspace_restore,
            restore_workspace_backup,
            inspect_workspace_deletion,
            delete_workspace_record,
            preview_composition,
            commit_composition,
            inspect_cloud_credential,
            edit_cloud_credential,
            remove_cloud_credential,
            get_cloud_credential_edit,
            submit_cloud_credential,
            cancel_cloud_credential_edit,
            get_cloud_model_profile,
            configure_cloud_model,
            clear_cloud_model,
            prepare_cloud_model,
            check_cloud_model,
            send_cloud_model,
            cancel_cloud_model,
            get_cloud_model_review,
            confirm_cloud_model_review,
            cancel_cloud_model_review,
            get_local_model_profile,
            configure_local_model,
            clear_local_model,
            prepare_local_model,
            check_local_model,
            send_local_model,
            cancel_local_model,
            get_local_model_review,
            confirm_local_model_review,
            cancel_local_model_review,
        ])
        .setup(|app| {
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("analyze.html".into()))
                .title("Rangoon")
                .inner_size(1440.0, 960.0)
                .min_inner_size(320.0, 480.0)
                .on_navigation(bundled_navigation)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("could not start Rangoon desktop");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_stays_on_bundled_origins() {
        for allowed in [
            "tauri://localhost/analyze.html",
            "http://tauri.localhost/index.html#command",
        ] {
            assert!(bundled_navigation(&allowed.parse().unwrap()));
        }
        for blocked in [
            "https://example.com",
            "file:///etc/passwd",
            "http://localhost:4377",
            "http://tauri.localhost.evil/analyze.html",
        ] {
            assert!(!bundled_navigation(&blocked.parse().unwrap()));
        }
    }
    #[test]
    fn picker_guard_releases_busy_state() {
        let busy = Arc::new(AtomicBool::new(true));
        {
            let _guard = PendingPicker(Arc::clone(&busy));
        }
        assert!(!busy.load(Ordering::Acquire));
    }

    #[test]
    fn composition_source_read_preserves_import_selection_and_exact_bytes() {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rangoon-composition-source-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = Workspace::new(root.clone());
        let imported = rangoon_import::analyze("current.md", b"# Current\nUnsaved.").unwrap();
        let saved =
            rangoon_import::analyze("saved.md", "\u{feff}# Saved\r\nExact 🦀\r\n".as_bytes())
                .unwrap();
        let session = Session::default();
        assert!(session.accept(session.generation().unwrap(), &imported));
        store.save_v1(&saved).unwrap();
        let before = store.list().unwrap();
        match read_saved_composition_source(&store, &saved.source.id) {
            SelectionResult::Analyzed { report } => assert_eq!(*report, saved),
            _ => panic!("expected saved source analysis"),
        }
        assert_eq!(
            session.snapshot(&imported.source.id),
            Some(imported.clone())
        );
        assert!(session.snapshot(&saved.source.id).is_none());
        assert_eq!(store.list().unwrap(), before);
        assert!(matches!(
            read_saved_composition_source(&store, "../../outside"),
            SelectionResult::Failed { .. }
        ));
        assert_eq!(session.snapshot(&imported.source.id), Some(imported));
        std::fs::remove_dir_all(root).unwrap();
    }
}
