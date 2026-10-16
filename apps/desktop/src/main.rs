#![forbid(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rangoon_desktop::workspace_lifecycle::{Error as GateError, Gate, Lane, Lease};

mod workflows;
use workflows::{
    begin_workflow_draft, clear_workflow_draft, commit_workflow_save, inspect_workflow_save,
    list_workflows, open_workflow,
};
mod capabilities;
mod compilation;
use compilation::compile_capability;
mod composition;
use composition::{commit_composition, preview_composition};
mod data_controls;
use data_controls::{
    delete_workspace_record, export_workspace_backup, get_workspace_data,
    inspect_workspace_deletion, prepare_workspace_restore, restore_workspace_backup,
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
#[cfg(test)]
use rangoon_store::Workspace;
use rangoon_store::{SnapshotMetadata, StoreError};
use serde::Serialize;
mod workspace_close;
mod workspace_requests;
use workspace_requests::get_workspace_request_state;
mod workspace_source_access;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::DialogExt;
use workspace_close::setup::{
    cancel_workspace_control, confirm_workspace_control, get_workspace_control_state,
    prepare_workspace_control,
};
use workspace_close::{
    WorkspaceClose, acknowledge_workspace_purge, begin_workspace_close, get_workspace_close_state,
};
use workspace_source_access::SourceAccess;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AdmissionError {
    Busy,
    Unavailable,
}
impl AdmissionError {
    fn from_gate(error: GateError) -> Self {
        match error {
            GateError::Busy => Self::Busy,
            GateError::Closed
            | GateError::Stale
            | GateError::Unavailable
            | GateError::Exhausted => Self::Unavailable,
        }
    }
}
fn admit(gate: &Gate, lane: Lane) -> Result<Lease, AdmissionError> {
    gate.begin(lane).map_err(AdmissionError::from_gate)
}

/// The renderer supplies no path, content or options. Every read starts with a
/// fresh native picker. The host retains the current report for explicit saving.
#[tauri::command]
async fn select_and_analyze(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
) -> SelectionResult {
    let operation = match app
        .state::<SourceAccess>()
        .for_request(&workspace)
        .begin_selection()
    {
        Ok(operation) => operation,
        Err(AdmissionError::Busy) => {
            return SelectionResult::failed(
                "selection_busy",
                "Finish or cancel the current file selection first.",
            );
        }
        Err(AdmissionError::Unavailable) => {
            return SelectionResult::failed(
                "session_unavailable",
                "Reopen Rangoon to restore the source workbench.",
            );
        }
    };
    tauri::async_runtime::spawn_blocking(move || {
        if !operation.is_current() {
            return operation.complete(SelectionResult::failed(
                "analysis_cleared",
                "This analysis was cleared. Choose or open a source again.",
            ));
        }
        let Some(window) = app.get_webview_window("main") else {
            return operation.complete(SelectionResult::failed(
                "selection_failed",
                "The source workbench is unavailable.",
            ));
        };
        let selected = app
            .dialog()
            .file()
            .set_parent(&window)
            .set_title("Choose a Markdown file to analyze")
            .add_filter("Markdown", &["md"])
            .blocking_pick_file();
        // Closing admission while the dialog was open must prevent the next
        // file-read phase, not merely discard its later output.
        if !operation.is_current() {
            return operation.complete(SelectionResult::failed(
                "analysis_cleared",
                "This analysis was cleared. Choose or open a source again.",
            ));
        }
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
        operation.complete(result)
    })
    .await
    .map(|completed| completed.deliver())
    .unwrap_or_else(|_| {
        SelectionResult::failed(
            "selection_failed",
            "File selection did not complete. Try again.",
        )
    })
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
/// One shared native state factory for the Builder and integration fixtures.
fn admission_state() -> (Gate, model_flight::ModelFlight) {
    let gate = Gate::new();
    let model = model_flight::ModelFlight::with_gate(gate.clone());
    (gate, model)
}

/// Admission precedes scheduling; the actual worker retains custody even if
/// its IPC waiter drops. No gate mutex spans the callback.
#[cfg(test)]
fn dispatch_read<T: Send + 'static>(
    gate: &Gate,
    action: impl FnOnce() -> T + Send + 'static,
) -> Result<tauri::async_runtime::JoinHandle<T>, AdmissionError> {
    let pending = admit(gate, Lane::Read)?;
    Ok(tauri::async_runtime::spawn_blocking(move || {
        let _pending = pending;
        action()
    }))
}
/// Uses the same private source state in Builder setup and synthetic fixtures.
fn source_state(gate: &Gate, root: Result<std::path::PathBuf, StoreError>) -> SourceAccess {
    SourceAccess::legacy(gate.clone(), root)
}
#[tauri::command]
async fn list_snapshots(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
) -> WorkspaceResult {
    let sources = app.state::<SourceAccess>().for_request(&workspace);
    sources.list_snapshots().await
}
#[tauri::command]
async fn save_analysis(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
    source_id: String,
) -> WorkspaceResult {
    let sources = app.state::<SourceAccess>().for_request(&workspace);
    sources.save_analysis(source_id).await
}
#[tauri::command]
async fn open_snapshot(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
    source_id: String,
) -> SelectionResult {
    let sources = app.state::<SourceAccess>().for_request(&workspace);
    sources.open_snapshot(source_id).await
}
/// Composition inspection leaves the Import selection unchanged.
#[tauri::command]
async fn read_composition_source(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
    source_id: String,
) -> SelectionResult {
    let sources = app.state::<SourceAccess>().for_request(&workspace);
    sources.read_composition_source(source_id).await
}
#[tauri::command]
fn clear_analysis(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
) -> WorkspaceResult {
    app.state::<SourceAccess>()
        .for_request(&workspace)
        .clear_analysis()
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
    let (gate, model_flight) = admission_state();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(gate.clone())
        .manage(NativeModel::default())
        .manage(NativeCloud::default())
        .manage(model_flight)
        .manage(cloud_custody::Custody::with_gate(gate.clone()))
        .on_window_event(|window, event| {
            models::window_event(window, event);
            cloud_models::window_event(window, event);
            cloud_credentials::window_event(window, event);
        })
        .invoke_handler(tauri::generate_handler![
            get_workspace_close_state,
            get_workspace_control_state,
            prepare_workspace_control,
            confirm_workspace_control,
            cancel_workspace_control,
            get_workspace_request_state,
            begin_workspace_close,
            acknowledge_workspace_purge,
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
            list_workflows,
            open_workflow,
            begin_workflow_draft,
            inspect_workflow_save,
            commit_workflow_save,
            clear_workflow_draft,
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
            let root = app
                .path()
                .app_local_data_dir()
                .map(|path| path.join("source-workspace"))
                .map_err(|_| StoreError::Unavailable);
            app.manage(source_state(app.state::<Gate>().inner(), root));
            app.manage(WorkspaceClose::new(
                app.state::<SourceAccess>().inner().clone(),
            ));
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("analyze.html".into()))
                .title("Rangoon")
                .inner_size(1440.0, 960.0)
                .min_inner_size(320.0, 480.0)
                .on_navigation(bundled_navigation)
                .build()?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("could not start Rangoon desktop");
    // The lifetime anchor belongs to the event loop, outside managed state.
    // SourceAccess retains only a weak host notifier and cannot keep it alive.
    let mut failure_anchor = workspace_close::failure::install(app.handle());
    app.run(move |_, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            failure_anchor.take();
        }
    });
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
        let gate = Gate::new();
        {
            let _guard = admit(&gate, Lane::Exclusive).unwrap();
            assert!(matches!(
                admit(&gate, Lane::Exclusive),
                Err(AdmissionError::Busy)
            ));
        }
        assert!(admit(&gate, Lane::Exclusive).is_ok());
    }

    #[test]
    fn admission_reports_capacity_separately_from_closed_gate() {
        let gate = Gate::new();
        let owner = admit(&gate, Lane::Exclusive).unwrap();
        assert!(matches!(
            admit(&gate, Lane::Exclusive),
            Err(AdmissionError::Busy)
        ));
        let drain = gate.request_drain().unwrap();
        for lane in [Lane::Exclusive, Lane::Read, Lane::Model] {
            assert!(matches!(
                admit(&gate, lane),
                Err(AdmissionError::Unavailable)
            ));
        }
        drop(owner);
        assert!(drain.is_quiescent().unwrap());
        gate.resume(&drain).unwrap();
        assert!(admit(&gate, Lane::Read).is_ok());
    }

    #[test]
    fn only_gate_busy_becomes_public_busy() {
        assert_eq!(
            AdmissionError::from_gate(GateError::Busy),
            AdmissionError::Busy
        );
        for error in [
            GateError::Closed,
            GateError::Stale,
            GateError::Unavailable,
            GateError::Exhausted,
        ] {
            assert_eq!(
                AdmissionError::from_gate(error),
                AdmissionError::Unavailable
            );
        }
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
        let sources = source_state(&Gate::new(), Ok(root.clone()));
        let selection = sources.begin_selection().unwrap();
        assert!(matches!(
            selection
                .complete(SelectionResult::Analyzed {
                    report: Box::new(imported.clone())
                })
                .deliver(),
            SelectionResult::Analyzed { .. }
        ));
        store.save_v1(&saved).unwrap();
        let before = store.list().unwrap();
        match tauri::async_runtime::block_on(
            sources.read_composition_source(saved.source.id.clone()),
        ) {
            SelectionResult::Analyzed { report } => assert_eq!(*report, saved),
            _ => panic!("expected saved source analysis"),
        }
        assert_eq!(store.list().unwrap(), before);
        assert!(matches!(
            tauri::async_runtime::block_on(sources.read_composition_source("../../outside".into())),
            SelectionResult::Failed { .. }
        ));
        // The unsaved imported source remains the exact selected save input.
        assert!(matches!(
            tauri::async_runtime::block_on(sources.save_analysis(imported.source.id.clone())),
            WorkspaceResult::Saved { .. }
        ));
        assert_eq!(store.open(&imported.source.id).unwrap(), imported);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
#[path = "native_admission_tests.rs"]
mod native_admission_tests;
