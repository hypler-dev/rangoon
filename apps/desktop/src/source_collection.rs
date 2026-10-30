//! Native-owned bulk selection. No renderer paths, writes, or analysis selection.
use crate::{AdmissionError, workspace_source_access::SourceAccess};
use rangoon_host::{SourceCollectionResult, inspect_selected_sources};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub(crate) async fn select_and_inspect_sources(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
) -> SourceCollectionResult {
    let operation = match app
        .state::<SourceAccess>()
        .for_request(&workspace)
        .begin_selection()
    {
        Ok(operation) => operation,
        Err(AdmissionError::Busy) => {
            return SourceCollectionResult::failed(
                "source_collection_busy",
                "Finish the current workspace operation first.",
            );
        }
        Err(AdmissionError::Unavailable) => {
            return SourceCollectionResult::failed(
                "source_collection_unavailable",
                "Unlock or reopen the workspace before choosing files.",
            );
        }
    };
    tauri::async_runtime::spawn_blocking(move || {
        let result = if !operation.is_current() {
            SourceCollectionResult::stale()
        } else if let Some(window) = app.get_webview_window("main") {
            let selection = app
                .dialog()
                .file()
                .set_parent(&window)
                .set_title("Choose up to 32 agent Markdown files")
                .add_filter("Markdown", &["md"])
                .blocking_pick_files();
            if !operation.is_current() {
                SourceCollectionResult::stale()
            } else {
                match selection {
                    None => SourceCollectionResult::Cancelled,
                    Some(selection) => match selection
                        .into_iter()
                        .map(|file| file.into_path())
                        .collect::<Result<Vec<_>, _>>()
                    {
                        Ok(paths) => inspect_selected_sources(&paths, || operation.is_current()),
                        Err(_) => SourceCollectionResult::failed(
                            "source_collection_unavailable",
                            "Choose regular Markdown files available on this computer.",
                        ),
                    },
                }
            }
        } else {
            SourceCollectionResult::failed(
                "source_collection_unavailable",
                "The file workbench is unavailable.",
            )
        };
        operation.complete_read_only(result)
    })
    .await
    .map(|completed| completed.deliver(SourceCollectionResult::stale))
    .unwrap_or_else(|_| {
        SourceCollectionResult::failed(
            "source_collection_failed",
            "File inspection did not complete. Previous results remain available.",
        )
    })
}

/// Fresh native folder selection; renderer supplies no path or row reference.
#[tauri::command]
pub(crate) async fn select_and_list_sources(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
) -> rangoon_host::FolderCollectionResult {
    use rangoon_host::FolderCollectionResult;
    #[cfg(not(unix))]
    {
        let _ = (workspace, app);
        FolderCollectionResult::failed(
            "folder_inventory_unsupported",
            "Folder discovery is unavailable on this platform. Choose individual files instead.",
        )
    }
    #[cfg(unix)]
    {
        let operation = match app
            .state::<SourceAccess>()
            .for_request(&workspace)
            .begin_selection()
        {
            Ok(operation) => operation,
            Err(AdmissionError::Busy) => {
                return FolderCollectionResult::failed(
                    "folder_inventory_busy",
                    "Finish the current workspace operation first.",
                );
            }
            Err(AdmissionError::Unavailable) => {
                return FolderCollectionResult::failed(
                    "folder_inventory_unavailable",
                    "Unlock or reopen the workspace before choosing a folder.",
                );
            }
        };
        tauri::async_runtime::spawn_blocking(move || {
            let result = if !operation.is_current() {
                FolderCollectionResult::stale()
            } else if let Some(window) = app.get_webview_window("main") {
                let selection = app
                    .dialog()
                    .file()
                    .set_parent(&window)
                    .set_title("Choose folder to list Markdown file metadata")
                    .blocking_pick_folder();
                if !operation.is_current() {
                    FolderCollectionResult::stale()
                } else {
                    match selection {
                        None => FolderCollectionResult::Cancelled,
                        Some(file) => match file.into_path() {
                            Ok(path) => {
                                rangoon_host::list_selected_root(&path, || operation.is_current())
                            }
                            Err(_) => FolderCollectionResult::failed(
                                "folder_inventory_unavailable",
                                "Choose a folder available on this computer.",
                            ),
                        },
                    }
                }
            } else {
                FolderCollectionResult::failed(
                    "folder_inventory_unavailable",
                    "The file workbench is unavailable.",
                )
            };
            operation.complete_read_only(result)
        })
        .await
        .map(|completed| completed.deliver(FolderCollectionResult::stale))
        .unwrap_or_else(|_| {
            FolderCollectionResult::failed(
                "folder_inventory_failed",
                "Folder listing did not finish. Previous results remain available.",
            )
        })
    }
}
