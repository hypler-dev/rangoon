//! Host-retained composition previews. Selection handles confer no authority.
use super::{AdmissionError, AppHandle, PublicError, StoreError, begin_operation, workspace};
use rangoon_compose::application::{self, ApplicationPreview};
#[cfg(test)]
use rangoon_store::Workspace;
use rangoon_store::{CompositionPreview, CompositionReceipt};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{Manager, ipc::InvokeBody};

const SESSION_SCHEMA: &str = "rangoon.composition-session.v0";
const COMMIT_SCHEMA: &str = "rangoon.composition-confirmation.v0";
const MAX_CONFIRMATION_BYTES: usize = 4096;

struct IssuedPreview {
    id: String,
    prepared: Arc<CompositionPreview>,
    attempted: bool,
}

#[derive(Default)]
pub struct CompositionSession(Mutex<Option<IssuedPreview>>);

impl CompositionSession {
    fn stage(&self, prepared: CompositionPreview) -> Result<CompositionResult, StoreError> {
        self.stage_with(prepared, |bytes| {
            getrandom::fill(bytes).map_err(|_| StoreError::Unavailable)
        })
    }

    fn stage_with(
        &self,
        prepared: CompositionPreview,
        random: impl FnOnce(&mut [u8]) -> Result<(), StoreError>,
    ) -> Result<CompositionResult, StoreError> {
        let mut bytes = [0_u8; 32];
        random(&mut bytes)?;
        let id = format!(
            "preview:{}",
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        let result = CompositionResult::Ready {
            schema_version: SESSION_SCHEMA,
            preview_id: id.clone(),
            expected_state_id: prepared.expected_state_id.clone(),
            preview: Box::new(prepared.preview.clone()),
        };
        *self.0.lock().map_err(|_| StoreError::Unavailable)? = Some(IssuedPreview {
            id,
            prepared: Arc::new(prepared),
            attempted: false,
        });
        Ok(result)
    }

    fn select(
        &self,
        confirmation: &Confirmation,
        claim: bool,
    ) -> Result<Arc<CompositionPreview>, StoreError> {
        if !confirmation.acknowledged {
            return Err(StoreError::CompositionAcknowledgmentRequired);
        }
        let mut guard = self.0.lock().map_err(|_| StoreError::Unavailable)?;
        let issued = guard.as_mut().ok_or(StoreError::WorkspaceChanged)?;
        if issued.attempted
            || issued.id != confirmation.preview_id
            || issued.prepared.expected_state_id != confirmation.expected_state_id
        {
            return Err(StoreError::WorkspaceChanged);
        }
        // Claim before entering storage. A lost response or failed transaction
        // cannot reuse this confirmation even if the database state is unchanged.
        issued.attempted |= claim;
        Ok(Arc::clone(&issued.prepared))
    }

    #[cfg(test)]
    fn selected(&self, confirmation: &Confirmation) -> Result<Arc<CompositionPreview>, StoreError> {
        self.select(confirmation, false)
    }

    #[cfg(test)]
    fn apply(
        &self,
        store: &Workspace,
        confirmation: &Confirmation,
    ) -> Result<CompositionReceipt, StoreError> {
        self.apply_with(confirmation, |prepared| {
            store.apply_composition(prepared, confirmation.acknowledged)
        })
    }

    fn apply_with(
        &self,
        confirmation: &Confirmation,
        action: impl FnOnce(&CompositionPreview) -> Result<CompositionReceipt, StoreError>,
    ) -> Result<CompositionReceipt, StoreError> {
        let prepared = self.select(confirmation, true)?;
        let receipt = action(&prepared)?;
        // The native busy guard serializes preview/commit. Match the handle too,
        // so consuming one result can never erase a different prepared preview.
        if let Ok(mut guard) = self.0.lock()
            && guard
                .as_ref()
                .is_some_and(|issued| issued.id == confirmation.preview_id)
        {
            *guard = None;
        }
        Ok(receipt)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Confirmation {
    schema_version: String,
    preview_id: String,
    expected_state_id: String,
    acknowledged: bool,
}

fn raw_bytes(body: &InvokeBody, limit: usize) -> Result<&[u8], StoreError> {
    match body {
        InvokeBody::Raw(bytes) if bytes.len() <= limit => Ok(bytes),
        _ => Err(StoreError::CompositionInvalid),
    }
}

fn decode_preview(body: &InvokeBody) -> Result<application::Request, StoreError> {
    application::decode_request_json(raw_bytes(body, application::MAX_REQUEST_BYTES)?)
        .map_err(|_| StoreError::CompositionInvalid)
}

fn decode_confirmation(body: &InvokeBody) -> Result<Confirmation, StoreError> {
    let confirmation: Confirmation =
        serde_json::from_slice(raw_bytes(body, MAX_CONFIRMATION_BYTES)?)
            .map_err(|_| StoreError::CompositionInvalid)?;
    let valid_handle = confirmation
        .preview_id
        .strip_prefix("preview:")
        .is_some_and(|id| {
            id.len() == 64
                && id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        });
    if confirmation.schema_version != COMMIT_SCHEMA || !valid_handle {
        return Err(StoreError::CompositionInvalid);
    }
    Ok(confirmation)
}

#[derive(Serialize)]
#[serde(
    tag = "outcome",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum CompositionResult {
    Ready {
        schema_version: &'static str,
        preview_id: String,
        expected_state_id: String,
        preview: Box<ApplicationPreview>,
    },
    Committed {
        receipt: CompositionReceipt,
    },
    Failed {
        error: PublicError,
    },
}

fn failure(error: StoreError) -> CompositionResult {
    let (code, message) = if error == StoreError::Unavailable {
        (
            "workspace_unavailable",
            "The composition did not return a confirmed result. Refresh saved skills and prepare a new preview before saving again.",
        )
    } else {
        error.public()
    };
    CompositionResult::Failed {
        error: PublicError { code, message },
    }
}

fn busy() -> CompositionResult {
    CompositionResult::Failed {
        error: PublicError {
            code: "workspace_busy",
            message: "Finish the current local workspace operation first. Your composition draft remains available.",
        },
    }
}

/// Raw UTF-8 JSON lets us enforce byte limits before domain deserialization.
/// Tauri has already received the transport buffer; this is not a transport cap.
#[tauri::command]
pub async fn preview_composition(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
) -> Result<CompositionResult, ()> {
    let pending = match begin_operation(&app) {
        Ok(lease) => lease,
        Err(AdmissionError::Busy) => {
            return Ok(busy());
        }
        Err(AdmissionError::Unavailable) => {
            return Ok(failure(StoreError::Unavailable));
        }
    };
    let draft = match decode_preview(request.body()) {
        Ok(draft) => draft,
        Err(error) => return Ok(failure(error)),
    };
    Ok(tauri::async_runtime::spawn_blocking(move || {
        let _pending = pending;
        match workspace(&app)
            .and_then(|store| store.preview_composition(&draft))
            .and_then(|prepared| app.state::<CompositionSession>().stage(prepared))
        {
            Ok(result) => result,
            Err(error) => failure(error),
        }
    })
    .await
    .unwrap_or_else(|_| failure(StoreError::Unavailable)))
}

#[tauri::command]
pub async fn commit_composition(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
) -> Result<CompositionResult, ()> {
    let pending = match begin_operation(&app) {
        Ok(lease) => lease,
        Err(AdmissionError::Busy) => {
            return Ok(busy());
        }
        Err(AdmissionError::Unavailable) => {
            return Ok(failure(StoreError::Unavailable));
        }
    };
    let confirmation = match decode_confirmation(request.body()) {
        Ok(confirmation) => confirmation,
        Err(error) => return Ok(failure(error)),
    };
    Ok(tauri::async_runtime::spawn_blocking(move || {
        let _pending = pending;
        match app
            .state::<CompositionSession>()
            .apply_with(&confirmation, |prepared| {
                workspace(&app)?.apply_composition(prepared, confirmation.acknowledged)
            }) {
            Ok(receipt) => CompositionResult::Committed { receipt },
            Err(error) => failure(error),
        }
    })
    .await
    .unwrap_or_else(|_| failure(StoreError::Unavailable)))
}

#[cfg(test)]
#[path = "composition_tests.rs"]
mod tests;
