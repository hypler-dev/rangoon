//! One native-picker-selected, read-only O1 window. Nothing is persisted or executed.
use super::{AdmissionError, AppHandle, PublicError, SourceAccess};
use rangoon_host::read_selected_operations_window;
use rangoon_ops::{
    OperatorViewDocument, build_operator_view, decode_window, inspect_approvals,
    inspect_delegations, inspect_lifecycle, project_window, relate_window,
};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

use crate::workspace_source_access::{ReadOnlySelectionCompletion, SelectionOperation};

const RESULT_SCHEMA: &str = "rangoon.ops.local-inspection.v1";
const MAX_IPC_BYTES: usize = 4_194_304;

fn error(code: &'static str, message: &'static str) -> PublicError {
    PublicError { code, message }
}

#[derive(Serialize)]
#[serde(
    tag = "outcome",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub(super) enum OperationsResult {
    Inspected {
        schema_version: &'static str,
        view: Box<OperatorViewDocument>,
    },
    Cancelled {
        schema_version: &'static str,
    },
    Rejected {
        schema_version: &'static str,
        error: PublicError,
    },
    Failed {
        schema_version: &'static str,
        error: PublicError,
    },
}

impl OperationsResult {
    fn cancelled() -> Self {
        Self::Cancelled {
            schema_version: RESULT_SCHEMA,
        }
    }

    fn rejected(code: &'static str, message: &'static str) -> Self {
        Self::Rejected {
            schema_version: RESULT_SCHEMA,
            error: error(code, message),
        }
    }

    fn failed(code: &'static str, message: &'static str) -> Self {
        Self::Failed {
            schema_version: RESULT_SCHEMA,
            error: error(code, message),
        }
    }
}

fn inspect_bytes(bytes: &[u8]) -> OperationsResult {
    let Ok(decoded) = decode_window(bytes) else {
        return OperationsResult::rejected(
            "operations_window_invalid",
            "This file is not a valid Rangoon operations window.",
        );
    };
    let Ok(projection) = project_window(&decoded) else {
        return stage_failed("operations_projection_failed");
    };
    let Ok(relationships) = relate_window(&decoded) else {
        return stage_failed("operations_relationships_failed");
    };
    let Ok(delegations) = inspect_delegations(&decoded) else {
        return stage_failed("operations_delegations_failed");
    };
    let Ok(approvals) = inspect_approvals(&decoded) else {
        return stage_failed("operations_approvals_failed");
    };
    let Ok(lifecycle) = inspect_lifecycle(&decoded) else {
        return stage_failed("operations_lifecycle_failed");
    };
    let Ok(view) = build_operator_view(
        &decoded,
        &projection,
        &relationships,
        &delegations,
        &approvals,
        &lifecycle,
    ) else {
        return stage_failed("operations_view_failed");
    };
    let result = OperationsResult::Inspected {
        schema_version: RESULT_SCHEMA,
        view: Box::new(view.into_document()),
    };
    match serde_json::to_vec(&result) {
        Ok(encoded) if encoded.len() <= MAX_IPC_BYTES => result,
        _ => stage_failed("operations_response_limit"),
    }
}

fn stage_failed(code: &'static str) -> OperationsResult {
    OperationsResult::failed(
        code,
        "This operations window could not be inspected. No partial view was loaded.",
    )
}

fn inspect_selected(path: &Path) -> OperationsResult {
    match read_selected_operations_window(path) {
        Ok(bytes) => inspect_bytes(&bytes),
        Err(error) if error.code == "operations_window_too_large" => OperationsResult::Rejected {
            schema_version: RESULT_SCHEMA,
            error,
        },
        Err(error) => OperationsResult::Failed {
            schema_version: RESULT_SCHEMA,
            error,
        },
    }
}

pub(crate) fn inspect_selection(
    operation: SelectionOperation,
    pick: impl FnOnce() -> Result<Option<PathBuf>, PublicError>,
    inspect: impl FnOnce(&Path) -> OperationsResult,
) -> ReadOnlySelectionCompletion<OperationsResult> {
    if !operation.is_current() {
        return operation.complete_read_only(stale());
    }
    let selected = pick();
    // Close/lock while a dialog is open must stop the subsequent read.
    if !operation.is_current() {
        return operation.complete_read_only(stale());
    }
    let result = match selected {
        Ok(Some(path)) => inspect(&path),
        Ok(None) => OperationsResult::cancelled(),
        Err(error) => OperationsResult::Failed {
            schema_version: RESULT_SCHEMA,
            error,
        },
    };
    // A late read may finish, but its view cannot be delivered after close.
    if !operation.is_current() {
        return operation.complete_read_only(stale());
    }
    operation.complete_read_only(result)
}

#[tauri::command]
pub(super) async fn select_and_inspect_operations(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
) -> OperationsResult {
    let operation = match app
        .state::<SourceAccess>()
        .for_request(&workspace)
        .begin_selection()
    {
        Ok(operation) => operation,
        Err(AdmissionError::Busy) => {
            return OperationsResult::failed(
                "operations_busy",
                "Finish or cancel the current local file selection first.",
            );
        }
        Err(AdmissionError::Unavailable) => {
            return OperationsResult::failed(
                "operations_session_unavailable",
                "Reopen Rangoon to inspect a local operations window.",
            );
        }
    };
    tauri::async_runtime::spawn_blocking(move || {
        inspect_selection(
            operation,
            || {
                let window = app.get_webview_window("main").ok_or_else(|| {
                    error(
                        "operations_picker_unavailable",
                        "The local operations picker is unavailable.",
                    )
                })?;
                app.dialog()
                    .file()
                    .set_parent(&window)
                    .set_title("Choose a Rangoon operations window")
                    .add_filter("JSON", &["json"])
                    .blocking_pick_file()
                    .map(|selected| {
                        selected.into_path().map_err(|_| {
                            error(
                                "operations_picker_failed",
                                "Choose a regular file available on this computer.",
                            )
                        })
                    })
                    .transpose()
            },
            inspect_selected,
        )
    })
    .await
    .map(|completed| completed.deliver(stale))
    .unwrap_or_else(|_| {
        OperationsResult::failed(
            "operations_picker_failed",
            "Local file selection did not complete. Try again.",
        )
    })
}

pub(crate) fn stale() -> OperationsResult {
    OperationsResult::failed(
        "operations_selection_cleared",
        "This operations inspection was cleared. Choose a local window again.",
    )
}

#[cfg(test)]
#[path = "operations_tests.rs"]
mod tests;
