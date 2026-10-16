//! Native request identity. This is neither authentication nor execution authority.
//! Registry observation never issues an identity or opens workspace admission.
use crate::{AdmissionError, SourceAccess};
use rangoon_desktop::workspace_lifecycle::{Drain, Gate, GenerationWitness, Lane, Lease};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{
    AppHandle, Manager, Runtime, WebviewWindow,
    ipc::{CommandArg, CommandItem, InvokeBody, InvokeError, Request},
};

pub(super) const HEADER: &str = "x-rangoon-workspace-client";

#[derive(Clone)]
pub(crate) enum WorkspaceRequest {
    Legacy,
    Managed(Arc<GenerationWitness>),
    Refused,
}
impl WorkspaceRequest {
    pub(crate) fn admit(&self, gate: &Gate, lane: Lane) -> Result<Lease, AdmissionError> {
        match self {
            Self::Legacy => return crate::admit(gate, lane),
            Self::Managed(witness) => gate.begin_for(witness, lane),
            Self::Refused => return Err(AdmissionError::Unavailable),
        }
        .map_err(AdmissionError::from_gate)
    }
}
impl<'de, R: Runtime> CommandArg<'de, R> for WorkspaceRequest {
    fn from_command(command: CommandItem<'de, R>) -> Result<Self, InvokeError> {
        let view = command.message.webview_ref();
        let Some(access) = view.try_state::<SourceAccess>() else {
            return Ok(Self::Refused);
        };
        Ok(access.request_context(view.label(), header(command.message.headers())))
    }
}

struct OpenClient {
    id: String,
    witness: Arc<GenerationWitness>,
}
pub(super) struct Registry {
    encrypted: bool,
    open: Mutex<Option<OpenClient>>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct View {
    schema_version: &'static str,
    storage: &'static str,
    state: &'static str,
    client_id: Option<String>,
}
fn header(headers: &tauri::http::HeaderMap) -> Option<&str> {
    let mut values = headers.get_all(HEADER).iter();
    let first = values.next()?;
    if values.next().is_some() {
        return None;
    }
    first.to_str().ok()
}
fn identifier(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl Registry {
    pub(super) fn new(encrypted: bool) -> Self {
        Self {
            encrypted,
            open: Mutex::new(None),
        }
    }
    pub(super) fn view(&self) -> View {
        let client_id = self.open.lock().ok().and_then(|slot| {
            slot.as_ref()
                .filter(|client| client.witness.is_current() == Ok(true))
                .map(|client| client.id.clone())
        });
        View {
            schema_version: "rangoon.workspace-request-state.v1",
            storage: if self.encrypted {
                "managed_encrypted"
            } else {
                "legacy_plaintext"
            },
            state: if !self.encrypted {
                "legacy"
            } else if client_id.is_some() {
                "open"
            } else {
                "unavailable"
            },
            client_id,
        }
    }
    pub(super) fn request(&self, label: &str, header: Option<&str>) -> WorkspaceRequest {
        if label != "main" {
            return WorkspaceRequest::Refused;
        }
        if !self.encrypted {
            return WorkspaceRequest::Legacy;
        }
        let Some(header) = header.filter(|value| identifier(value)) else {
            return WorkspaceRequest::Refused;
        };
        let Ok(slot) = self.open.lock() else {
            return WorkspaceRequest::Refused;
        };
        match slot.as_ref() {
            Some(client) if client.id == header => {
                WorkspaceRequest::Managed(Arc::clone(&client.witness))
            }
            _ => WorkspaceRequest::Refused,
        }
    }
    pub(super) fn revoke(&self) -> Result<(), ()> {
        match self.open.lock() {
            Ok(mut slot) => {
                *slot = None;
                Ok(())
            }
            Err(error) => {
                *error.into_inner() = None;
                Err(())
            }
        }
    }
    /// Caller holds Source.closing. No new ordinary admission can interleave.
    pub(super) fn resume(&self, gate: &Gate, drain: &Drain) -> Result<(), ()> {
        self.resume_with(gate, drain, |bytes| getrandom::fill(bytes).map_err(|_| ()))
    }
    fn resume_with(
        &self,
        gate: &Gate,
        drain: &Drain,
        entropy: impl FnOnce(&mut [u8]) -> Result<(), ()>,
    ) -> Result<(), ()> {
        let mut slot = self.open.lock().map_err(|_| ())?;
        *slot = None;
        if !self.encrypted {
            return Err(());
        }
        let mut bytes = [0; 32];
        entropy(&mut bytes)?;
        let id = bytes.iter().map(|b| format!("{b:02x}")).collect();
        gate.resume(drain).map_err(|_| ())?;
        let witness = gate.observe_open().map_err(|_| ())?;
        *slot = Some(OpenClient {
            id,
            witness: Arc::new(witness),
        });
        Ok(())
    }
}

#[tauri::command]
pub(super) fn get_workspace_request_state(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<View, ()> {
    if window.label() != "main"
        || !matches!(request.body(), InvokeBody::Json(value) if value.as_object().is_some_and(|object| object.is_empty()))
    {
        return Err(());
    }
    Ok(app.state::<SourceAccess>().request_view())
}

#[cfg(test)]
#[path = "workspace_request_tests.rs"]
mod tests;
