//! Host-retained composition previews. Selection handles confer no authority.
use super::{AppHandle, PublicError, SourceAccess, StoreError};
use rangoon_compose::application::{self, ApplicationPreview};
use rangoon_desktop::workspace_lifecycle::GenerationWitness;
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
    handle: Option<CompositionHandle>,
    expected_state_id: String,
    generation: Option<GenerationWitness>,
    attempted: bool,
}

// Managed commits accept only the Owner's private retained token. Inspection
// copies are never used as mutation input.
pub(super) enum CompositionHandle {
    Legacy(Arc<CompositionPreview>),
    #[cfg(all(test, feature = "encrypted-workspace"))]
    Managed(String),
}

pub(super) struct RoutedCompositionPreview {
    pub(super) inspection: CompositionPreview,
    pub(super) handle: CompositionHandle,
}
impl RoutedCompositionPreview {
    pub(super) fn legacy(prepared: CompositionPreview) -> Self {
        Self {
            inspection: prepared.clone(),
            handle: CompositionHandle::Legacy(Arc::new(prepared)),
        }
    }
    #[cfg(all(test, feature = "encrypted-workspace"))]
    pub(super) fn managed(
        prepared: rangoon_desktop::workspace_session::CompositionConfirmation,
    ) -> Self {
        Self {
            inspection: prepared.inspection,
            handle: CompositionHandle::Managed(prepared.id),
        }
    }
}

#[derive(Default)]
pub struct CompositionSession(Mutex<Option<IssuedPreview>>);

impl CompositionSession {
    #[cfg(test)]
    pub(super) fn retained_for_close_test(&self) -> bool {
        self.0.lock().unwrap().is_some()
    }
    #[cfg(test)]
    fn stage(&self, prepared: CompositionPreview) -> Result<CompositionResult, StoreError> {
        self.stage_with(prepared, |bytes| {
            getrandom::fill(bytes).map_err(|_| StoreError::Unavailable)
        })
    }

    #[cfg(test)]
    fn stage_with(
        &self,
        prepared: CompositionPreview,
        random: impl FnOnce(&mut [u8]) -> Result<(), StoreError>,
    ) -> Result<CompositionResult, StoreError> {
        self.publish(RoutedCompositionPreview::legacy(prepared), None, random)
    }

    pub(super) fn stage_routed(
        &self,
        prepared: RoutedCompositionPreview,
        generation: GenerationWitness,
    ) -> Result<CompositionResult, StoreError> {
        if !generation.is_current().unwrap_or(false) {
            return Err(StoreError::Unavailable);
        }
        self.publish(prepared, Some(generation), |bytes| {
            getrandom::fill(bytes).map_err(|_| StoreError::Unavailable)
        })
    }

    fn publish(
        &self,
        prepared: RoutedCompositionPreview,
        generation: Option<GenerationWitness>,
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
            expected_state_id: prepared.inspection.expected_state_id.clone(),
            preview: Box::new(prepared.inspection.preview.clone()),
        };
        *self.0.lock().map_err(|_| StoreError::Unavailable)? = Some(IssuedPreview {
            id,
            expected_state_id: prepared.inspection.expected_state_id,
            handle: Some(prepared.handle),
            generation,
            attempted: false,
        });
        Ok(result)
    }

    #[cfg(test)]
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
            || issued.expected_state_id != confirmation.expected_state_id
        {
            return Err(StoreError::WorkspaceChanged);
        }
        // Claim before entering storage. A lost response or failed transaction
        // cannot reuse this confirmation even if the database state is unchanged.
        issued.attempted |= claim;
        match issued.handle.as_ref() {
            Some(CompositionHandle::Legacy(prepared)) => Ok(Arc::clone(prepared)),
            _ => Err(StoreError::WorkspaceChanged),
        }
    }

    pub(super) fn claim(
        &self,
        confirmation: &Confirmation,
    ) -> Result<CompositionHandle, StoreError> {
        if !confirmation.acknowledged {
            return Err(StoreError::CompositionAcknowledgmentRequired);
        }
        let mut guard = self.0.lock().map_err(|_| StoreError::Unavailable)?;
        let issued = guard.as_mut().ok_or(StoreError::WorkspaceChanged)?;
        if issued.attempted
            || issued.id != confirmation.preview_id
            || issued.expected_state_id != confirmation.expected_state_id
        {
            return Err(StoreError::WorkspaceChanged);
        }
        if !issued
            .generation
            .as_ref()
            .is_some_and(|generation| generation.is_current().unwrap_or(false))
        {
            *guard = None;
            return Err(StoreError::Unavailable);
        }
        issued.attempted = true;
        issued.handle.take().ok_or(StoreError::WorkspaceChanged)
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

    #[cfg(test)]
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
pub(super) struct Confirmation {
    schema_version: String,
    pub(super) preview_id: String,
    pub(super) expected_state_id: String,
    pub(super) acknowledged: bool,
}

fn raw_bytes(body: &InvokeBody, limit: usize) -> Result<&[u8], StoreError> {
    match body {
        InvokeBody::Raw(bytes) if bytes.len() <= limit => Ok(bytes),
        _ => Err(StoreError::CompositionInvalid),
    }
}

pub(super) fn decode_preview(body: &InvokeBody) -> Result<application::Request, StoreError> {
    application::decode_request_json(raw_bytes(body, application::MAX_REQUEST_BYTES)?)
        .map_err(|_| StoreError::CompositionInvalid)
}

pub(super) fn decode_confirmation(body: &InvokeBody) -> Result<Confirmation, StoreError> {
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

pub(super) fn failure(error: StoreError) -> CompositionResult {
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

pub(super) fn busy() -> CompositionResult {
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
    let draft = match decode_preview(request.body()) {
        Ok(draft) => draft,
        Err(error) => return Ok(failure(error)),
    };
    Ok(app.state::<SourceAccess>().preview_composition(draft).await)
}

#[tauri::command]
pub async fn commit_composition(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
) -> Result<CompositionResult, ()> {
    let confirmation = match decode_confirmation(request.body()) {
        Ok(confirmation) => confirmation,
        Err(error) => return Ok(failure(error)),
    };
    Ok(app
        .state::<SourceAccess>()
        .commit_composition(confirmation)
        .await)
}

#[cfg(test)]
#[path = "composition_tests.rs"]
mod tests;
