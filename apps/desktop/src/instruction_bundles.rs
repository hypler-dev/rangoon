//! Native picker-only portable bundles; external evidence never enters the workspace.
use super::{AdmissionError, AppHandle, PublicError, StoreError, begin_operation, workspace};
use rangoon_compile::{BundleInspection, Profile, encode_bundle, inspect_bundle};
use rangoon_domain::{Authority, capability};
use rangoon_host::{read_selected_instruction_bundle, write_selected_instruction_bundle};
use rangoon_store::Workspace;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::path::{Path, PathBuf};
use tauri::{Manager, ipc::InvokeBody};
use tauri_plugin_dialog::DialogExt;

const MAX_REQUEST_BYTES: usize = 4096;
const EXPORT_REQUEST: &str = "rangoon.instruction-bundle-export-request.v1";
const INSPECT_REQUEST: &str = "rangoon.instruction-bundle-inspect-request.v1";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportRequest {
    schema_version: String,
    capability_id: String,
    revision_id: String,
    profile: Profile,
    expected_candidate_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InspectRequest {
    schema_version: String,
}

fn public(code: &'static str, message: &'static str) -> PublicError {
    PublicError { code, message }
}
fn invalid_request() -> PublicError {
    public(
        "instruction_bundle_request_invalid",
        "This instruction-bundle request is invalid. No file was created.",
    )
}
fn unavailable() -> PublicError {
    public(
        "instruction_bundle_unavailable",
        "The native instruction-bundle picker is unavailable. No file was created.",
    )
}
fn candidate_changed() -> PublicError {
    public(
        "instruction_candidate_changed",
        "The saved candidate is missing or changed. Refresh and compile again. No file was created.",
    )
}
fn decode<T: DeserializeOwned>(body: &InvokeBody) -> Result<T, PublicError> {
    match body {
        InvokeBody::Raw(bytes) if bytes.len() <= MAX_REQUEST_BYTES => {
            serde_json::from_slice(bytes).map_err(|_| invalid_request())
        }
        _ => Err(invalid_request()),
    }
}
fn decode_export(body: &InvokeBody) -> Result<ExportRequest, PublicError> {
    let request: ExportRequest = decode(body)?;
    if request.schema_version != EXPORT_REQUEST
        || !capability::valid_id(&request.capability_id, "capability:")
        || !capability::valid_id(&request.revision_id, "revision:")
        || !capability::valid_id(&request.expected_candidate_id, "candidate:")
    {
        return Err(invalid_request());
    }
    Ok(request)
}
fn decode_inspect(body: &InvokeBody) -> Result<(), PublicError> {
    let request: InspectRequest = decode(body)?;
    if request.schema_version != INSPECT_REQUEST {
        return Err(invalid_request());
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputState {
    NotCreated,
    Uncertain,
}
#[derive(Serialize)]
#[serde(
    tag = "outcome",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum BundleResult {
    Exported {
        schema_version: &'static str,
        bundle_id: String,
        candidate_id: String,
        sha256: String,
        byte_length: u64,
        authority: Authority,
    },
    Inspected {
        schema_version: &'static str,
        inspection: Box<BundleInspection>,
        candidate_manifest_json: String,
    },
    Cancelled,
    Failed {
        error: PublicError,
        output_state: OutputState,
    },
}
fn failure(error: PublicError) -> BundleResult {
    BundleResult::Failed {
        error,
        output_state: OutputState::NotCreated,
    }
}
fn writer_failure(error: PublicError) -> BundleResult {
    let output_state = if error.code == "instruction_bundle_write_failed" {
        OutputState::Uncertain
    } else {
        OutputState::NotCreated
    };
    BundleResult::Failed {
        error,
        output_state,
    }
}
fn store_failure(error: StoreError) -> BundleResult {
    if matches!(error, StoreError::CapabilityNotFound | StoreError::NotFound) {
        return failure(candidate_changed());
    }
    let (code, message) = error.public();
    failure(public(code, message))
}
fn bundle_failure(error: rangoon_compile::BundleError) -> BundleResult {
    failure(public(
        error.code(),
        "The instruction bundle is invalid or inconsistent. No workspace records changed.",
    ))
}

/// Picker precedes workspace resolution; the write sees only the newly compiled object.
fn export_selected(
    request: &ExportRequest,
    pick: impl FnOnce() -> Result<Option<PathBuf>, PublicError>,
    resolve: impl FnOnce() -> Result<Workspace, StoreError>,
    write: impl FnOnce(&Path, &[u8]) -> Result<(), PublicError>,
) -> BundleResult {
    let path = match pick() {
        Ok(Some(path)) => path,
        Ok(None) => return BundleResult::Cancelled,
        Err(error) => return failure(error),
    };
    let compiled = match resolve().and_then(|store| {
        store.compile_capability(
            &request.capability_id,
            &request.revision_id,
            request.profile,
        )
    }) {
        Ok(result) => result.compilation,
        Err(error) => return store_failure(error),
    };
    if compiled
        .candidate
        .as_ref()
        .map(|candidate| candidate.candidate_id.as_str())
        != Some(request.expected_candidate_id.as_str())
    {
        return failure(candidate_changed());
    }
    let bundle = match encode_bundle(&compiled) {
        Ok(bundle) => bundle,
        Err(error) => return bundle_failure(error),
    };
    if let Err(error) = write(&path, &bundle.bytes) {
        return writer_failure(error);
    }
    BundleResult::Exported {
        schema_version: "rangoon.instruction-bundle-export.v1",
        bundle_id: bundle.inspection.bundle_id,
        candidate_id: request.expected_candidate_id.clone(),
        sha256: bundle.inspection.sha256,
        byte_length: bundle.inspection.byte_length,
        authority: Authority::None,
    }
}

fn inspect_selected(pick: impl FnOnce() -> Result<Option<PathBuf>, PublicError>) -> BundleResult {
    let path = match pick() {
        Ok(Some(path)) => path,
        Ok(None) => return BundleResult::Cancelled,
        Err(error) => return failure(error),
    };
    let bytes = match read_selected_instruction_bundle(&path) {
        Ok(bytes) => bytes,
        Err(error) => return failure(error),
    };
    let inspection = match inspect_bundle(&bytes) {
        Ok(inspection) => inspection,
        Err(error) => return bundle_failure(error),
    };
    let manifest = match inspection.compilation.canonical_manifest_bytes() {
        Ok(Some(bytes)) => match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => return bundle_failure(rangoon_compile::BundleError::SerializationFailed),
        },
        _ => return bundle_failure(rangoon_compile::BundleError::SerializationFailed),
    };
    BundleResult::Inspected {
        schema_version: "rangoon.instruction-bundle-file.v1",
        inspection: Box::new(inspection),
        candidate_manifest_json: manifest,
    }
}

fn pick(app: &AppHandle, exporting: bool) -> Result<Option<PathBuf>, PublicError> {
    let window = app.get_webview_window("main").ok_or_else(unavailable)?;
    let dialog = app
        .dialog()
        .file()
        .set_parent(&window)
        .add_filter("Rangoon instructions", &["rangoon-instructions"]);
    let selected = if exporting {
        dialog
            .set_title("Export a plaintext Rangoon instruction bundle")
            .set_file_name("Rangoon.rangoon-instructions")
            .blocking_save_file()
    } else {
        dialog
            .set_title("Inspect an external Rangoon instruction bundle")
            .blocking_pick_file()
    };
    selected
        .map(|value| value.into_path().map_err(|_| unavailable()))
        .transpose()
}
fn busy() -> BundleResult {
    failure(public(
        "workspace_busy",
        "Finish the current local operation before using instruction bundles.",
    ))
}
fn join_failure(exporting: bool) -> BundleResult {
    BundleResult::Failed {
        error: public(
            "instruction_bundle_unavailable",
            if exporting {
                "Export did not return a confirmed result. A file may remain. Choose a new name before retrying."
            } else {
                "Inspection did not return a confirmed result. No workspace records changed."
            },
        ),
        output_state: if exporting {
            OutputState::Uncertain
        } else {
            OutputState::NotCreated
        },
    }
}

#[tauri::command]
pub async fn export_instruction_bundle(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
) -> Result<BundleResult, ()> {
    let pending = match begin_operation(&app) {
        Ok(lease) => lease,
        Err(AdmissionError::Busy) => {
            return Ok(busy());
        }
        Err(AdmissionError::Unavailable) => {
            return Ok(store_failure(StoreError::Unavailable));
        }
    };
    let request = match decode_export(request.body()) {
        Ok(request) => request,
        Err(error) => return Ok(failure(error)),
    };
    Ok(tauri::async_runtime::spawn_blocking(move || {
        let _pending = pending;
        export_selected(
            &request,
            || pick(&app, true),
            || workspace(&app),
            write_selected_instruction_bundle,
        )
    })
    .await
    .unwrap_or_else(|_| join_failure(true)))
}

#[tauri::command]
pub async fn inspect_instruction_bundle(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
) -> Result<BundleResult, ()> {
    let pending = match begin_operation(&app) {
        Ok(lease) => lease,
        Err(AdmissionError::Busy) => {
            return Ok(busy());
        }
        Err(AdmissionError::Unavailable) => {
            return Ok(store_failure(StoreError::Unavailable));
        }
    };
    if let Err(error) = decode_inspect(request.body()) {
        return Ok(failure(error));
    }
    Ok(tauri::async_runtime::spawn_blocking(move || {
        let _pending = pending;
        inspect_selected(|| pick(&app, false))
    })
    .await
    .unwrap_or_else(|_| join_failure(false)))
}

#[cfg(test)]
#[path = "instruction_bundles_tests.rs"]
mod tests;
