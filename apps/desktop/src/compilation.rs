//! Read-only compilation of exact host-resolved saved revisions.
use super::{AdmissionError, AppHandle, PublicError, StoreError, begin_operation, workspace};
use rangoon_compile::{CompilationReport, Profile};
use rangoon_domain::capability;
use rangoon_store::Workspace;
use serde::{Deserialize, Serialize};
use tauri::ipc::InvokeBody;

const REQUEST_SCHEMA: &str = "rangoon.compile-request.v1";
const RESPONSE_SCHEMA: &str = "rangoon.compilation-inspection.v1";
const MAX_REQUEST_BYTES: usize = 4096;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    capability_id: String,
    revision_id: String,
    profile: Profile,
}

fn decode_request(body: &InvokeBody) -> Result<Request, StoreError> {
    let bytes = match body {
        InvokeBody::Raw(bytes) if bytes.len() <= MAX_REQUEST_BYTES => bytes,
        _ => return Err(StoreError::CompilationInvalid),
    };
    let request: Request =
        serde_json::from_slice(bytes).map_err(|_| StoreError::CompilationInvalid)?;
    if request.schema_version != REQUEST_SCHEMA
        || !capability::valid_id(&request.capability_id, "capability:")
        || !capability::valid_id(&request.revision_id, "revision:")
    {
        return Err(StoreError::CompilationInvalid);
    }
    Ok(request)
}

#[derive(Serialize)]
#[serde(
    tag = "outcome",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum CompilationResult {
    Compiled {
        schema_version: &'static str,
        observed_current_head: String,
        compilation: Box<CompilationReport>,
        candidate_manifest_json: Option<String>,
    },
    Failed {
        error: PublicError,
    },
}

fn failure(error: StoreError) -> CompilationResult {
    let (code, message) = error.public();
    CompilationResult::Failed {
        error: PublicError { code, message },
    }
}

fn inspect(store: &Workspace, request: &Request) -> Result<CompilationResult, StoreError> {
    let result = store.compile_capability(
        &request.capability_id,
        &request.revision_id,
        request.profile,
    )?;
    let candidate_manifest_json = result
        .compilation
        .canonical_manifest_bytes()
        .map_err(|_| StoreError::CompilationInvalid)?
        .map(String::from_utf8)
        .transpose()
        .map_err(|_| StoreError::CompilationInvalid)?;
    Ok(CompilationResult::Compiled {
        schema_version: RESPONSE_SCHEMA,
        observed_current_head: result.observed_current_head,
        compilation: Box::new(result.compilation),
        candidate_manifest_json,
    })
}

/// Bound raw JSON before typed decoding; Tauri already received the buffer.
/// This operation does not touch source-analysis or composition session state.
#[tauri::command]
pub async fn compile_capability(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
) -> Result<CompilationResult, ()> {
    let pending = match begin_operation(&app) {
        Ok(lease) => lease,
        Err(AdmissionError::Busy) => {
            return Ok(CompilationResult::Failed {
                error: PublicError {
                    code: "workspace_busy",
                    message: "Finish the current local workspace operation before compiling.",
                },
            });
        }
        Err(AdmissionError::Unavailable) => {
            return Ok(failure(StoreError::Unavailable));
        }
    };
    let request = match decode_request(request.body()) {
        Ok(request) => request,
        Err(error) => return Ok(failure(error)),
    };
    Ok(tauri::async_runtime::spawn_blocking(move || {
        let _pending = pending;
        match workspace(&app).and_then(|store| inspect(&store, &request)) {
            Ok(result) => result,
            Err(error) => failure(error),
        }
    })
    .await
    .unwrap_or_else(|_| failure(StoreError::Unavailable)))
}

#[cfg(test)]
#[path = "compilation_tests.rs"]
mod tests;
