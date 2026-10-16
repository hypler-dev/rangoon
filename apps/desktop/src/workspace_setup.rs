//! Main-window setup/unlock protocol. No plaintext conversion or factory mount.
//! Every Flight, including its finalizer, stays inside a blocking worker.
use super::{View, WorkspaceClose, allowed, empty, failure::WorkerCleanup};
use crate::workspace_source_access::ControlKind;
use rangoon_desktop::workspace_session::{Action as NativeAction, Preview};
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use tauri::{
    AppHandle, Manager, WebviewWindow,
    ipc::{InvokeBody, Request},
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogResult};

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Action {
    CreateEmpty,
    Unlock,
}
impl Action {
    fn kind(self) -> ControlKind {
        match self {
            Self::CreateEmpty => ControlKind::CreateEmpty,
            Self::Unlock => ControlKind::Unlock,
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Preparation {
    schema_version: String,
    attempt_id: String,
    epoch: String,
    action: Action,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Cycle {
    schema_version: String,
    attempt_id: String,
    epoch: String,
}
fn identity(id: &str, epoch: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && epoch.len() <= 20
        && epoch
            .parse::<u64>()
            .is_ok_and(|n| n > 0 && n.to_string() == epoch)
}
impl Preparation {
    pub(crate) fn parse(body: &InvokeBody) -> Option<Self> {
        let InvokeBody::Raw(raw) = body else {
            return None;
        };
        if raw.len() > 512 {
            return None;
        }
        let value: Self = serde_json::from_slice(raw).ok()?;
        (value.schema_version == "rangoon.workspace-control-prepare.v1"
            && identity(&value.attempt_id, &value.epoch))
        .then_some(value)
    }
}
impl Cycle {
    pub(crate) fn parse(body: &InvokeBody) -> Option<Self> {
        let InvokeBody::Raw(raw) = body else {
            return None;
        };
        if raw.len() > 512 {
            return None;
        }
        let value: Self = serde_json::from_slice(raw).ok()?;
        (value.schema_version == "rangoon.workspace-control-cycle.v1"
            && identity(&value.attempt_id, &value.epoch))
        .then_some(value)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Response {
    schema_version: &'static str,
    close: View,
    phase: &'static str,
    action: Option<Action>,
    outcome: &'static str,
    diagnostic: Option<&'static str>,
    reload: bool,
}
impl Response {
    pub(crate) fn observe(close: &WorkspaceClose) -> Self {
        Self::result(close, "observed", None, false)
    }
    fn result(
        close: &WorkspaceClose,
        outcome: &'static str,
        diagnostic: Option<&'static str>,
        reload: bool,
    ) -> Self {
        let (close, kind, phase) = close.inspect_control();
        let action = match kind {
            Some(ControlKind::CreateEmpty) => Some(Action::CreateEmpty),
            Some(ControlKind::Unlock) => Some(Action::Unlock),
            _ => None,
        };
        Self {
            schema_version: "rangoon.workspace-control-state.v1",
            close,
            phase,
            action,
            outcome,
            diagnostic,
            reload,
        }
    }
    fn refused(close: &WorkspaceClose) -> Self {
        Self::result(
            close,
            "refused",
            Some("workspace_control_unavailable"),
            false,
        )
    }
}

/// Call only on a blocking worker. Retaining New custody performs no Owner
/// preparation and no vault lookup; exact C0 cleanup is checked under State.
pub(crate) fn prepare(
    close: &WorkspaceClose,
    request: Preparation,
    cleanup: impl FnOnce(bool) -> bool + Send + 'static,
) -> Response {
    if close
        .start_control_for(
            &request.attempt_id,
            &request.epoch,
            request.action.kind(),
            cleanup,
        )
        .and_then(|flight| flight.retain())
        .is_err()
    {
        return Response::refused(close);
    }
    Response::result(close, "prepared", None, false)
}

/// Native preview and actual callback custody never leave this worker. The
/// consumption callback is transport bookkeeping, never human consent.
pub(crate) fn confirm(
    close: &WorkspaceClose,
    request: Cycle,
    cleanup: impl FnOnce(bool) -> bool + Send + 'static,
    consent: impl FnOnce(&Preview) -> bool,
    consumed: impl FnOnce() -> bool,
) -> Response {
    let Ok(mut flight) = close.take_control_ready(&request.attempt_id, &request.epoch, cleanup)
    else {
        return Response::refused(close);
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let preview = flight.prepare().ok()?;
        if !matches!(
            preview.kind,
            NativeAction::CreateEmpty | NativeAction::Unlock
        ) || preview.source_id.is_some()
            || !consent(&preview)
        {
            return None;
        }
        flight.confirm(&preview.id).ok()?;
        consumed().then_some(())
    }));
    if !matches!(result, Ok(Some(()))) {
        drop(flight);
        return Response::result(
            close,
            "closed",
            Some("workspace_control_not_completed"),
            false,
        );
    }
    if flight.resume().is_err() {
        return Response::result(
            close,
            "closed",
            Some("workspace_control_not_completed"),
            false,
        );
    }
    // A later IPC-response loss is delivery-unknown. Only a fresh document may
    // bootstrap the newly published client identifier; this response omits it.
    Response::result(close, "opened", None, true)
}

pub(crate) fn cancel(
    close: &WorkspaceClose,
    request: Cycle,
    cleanup: impl FnOnce(bool) -> bool,
) -> Response {
    let view = close.cancel_control(&request.attempt_id, &request.epoch, cleanup);
    let refused = view.diagnostic.is_some();
    drop(view);
    if refused {
        Response::refused(close)
    } else {
        Response::result(close, "cancelled", None, false)
    }
}

fn native_consent(app: &AppHandle, cleanup: &WorkerCleanup, preview: &Preview) -> bool {
    if !cleanup.alive() || preview.source_id.is_some() {
        return false;
    }
    let Some(parent) = app.get_webview_window("main") else {
        return false;
    };
    let (title, question, affirmative) = match preview.kind {
        NativeAction::CreateEmpty => (
            "Set up encrypted workspace?",
            "Create an empty encrypted Rangoon workspace and save its key in this user's operating-system credential store? Existing plaintext data will not be imported or deleted.",
            "Set up workspace",
        ),
        NativeAction::Unlock => (
            "Unlock encrypted workspace?",
            "Read the existing Rangoon workspace key from this user's operating-system credential store and unlock the encrypted workspace? This does not authorize external calls or execution.",
            "Unlock workspace",
        ),
        _ => return false,
    };
    let result = app
        .dialog()
        .message(question)
        .title(title)
        .parent(&parent)
        .buttons(MessageDialogButtons::OkCancelCustom(
            affirmative.into(),
            "Cancel".into(),
        ))
        .blocking_show_with_result();
    cleanup.alive() && matches!(result, MessageDialogResult::Custom(label) if label == affirmative)
}

#[tauri::command]
pub(crate) fn get_workspace_control_state(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<Response, ()> {
    allowed(window.label())?;
    if !empty(request.body()) {
        return Err(());
    }
    Ok(Response::observe(app.state::<WorkspaceClose>().inner()))
}

#[tauri::command]
pub(crate) async fn prepare_workspace_control(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<Response, ()> {
    allowed(window.label())?;
    let request = Preparation::parse(request.body()).ok_or(())?;
    let close = app.state::<WorkspaceClose>().inner().clone();
    let Some(cleanup) = app
        .try_state::<WorkerCleanup>()
        .map(|s| s.inner().clone())
        .filter(WorkerCleanup::alive)
    else {
        return Ok(Response::refused(&close));
    };
    tauri::async_runtime::spawn_blocking(move || {
        prepare(&close, request, move |drained| cleanup.cleanup(drained))
    })
    .await
    .map_err(|_| ())
}

#[tauri::command]
pub(crate) async fn confirm_workspace_control(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<Response, ()> {
    allowed(window.label())?;
    let request = Cycle::parse(request.body()).ok_or(())?;
    let close = app.state::<WorkspaceClose>().inner().clone();
    let Some(cleanup) = app
        .try_state::<WorkerCleanup>()
        .map(|s| s.inner().clone())
        .filter(WorkerCleanup::alive)
    else {
        return Ok(Response::refused(&close));
    };
    let (ready_sender, ready_receiver) = tokio::sync::oneshot::channel();
    let (consume_sender, consume_receiver) = mpsc::channel();
    let finalizer = cleanup.clone();
    let worker = tauri::async_runtime::spawn_blocking(move || {
        confirm(
            &close,
            request,
            move |drained| finalizer.cleanup(drained),
            |preview| native_consent(&app, &cleanup, preview),
            || ready_sender.send(()).is_ok() && cleanup.consume(consume_receiver),
        )
    });
    if ready_receiver.await.is_ok() {
        let _ = consume_sender.send(());
    }
    drop(consume_sender);
    worker.await.map_err(|_| ())
}

#[tauri::command]
pub(crate) async fn cancel_workspace_control(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<Response, ()> {
    allowed(window.label())?;
    let request = Cycle::parse(request.body()).ok_or(())?;
    let close = app.state::<WorkspaceClose>().inner().clone();
    let Some(cleanup) = app
        .try_state::<WorkerCleanup>()
        .map(|s| s.inner().clone())
        .filter(WorkerCleanup::alive)
    else {
        return Ok(Response::refused(&close));
    };
    tauri::async_runtime::spawn_blocking(move || {
        cancel(&close, request, move |drained| cleanup.cleanup(drained))
    })
    .await
    .map_err(|_| ())
}
