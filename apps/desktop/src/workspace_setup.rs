//! Main-window setup/unlock protocol. No plaintext conversion or factory mount.
//! Every Flight, including its finalizer, stays inside a blocking worker.
use super::{Phase, View, WorkspaceClose, allowed, empty, failure::WorkerCleanup};
use crate::workspace_source_access::ControlKind;
use rangoon_desktop::workspace_session::{Action as NativeAction, Layout, Preview};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, mpsc};
use tauri::{
    AppHandle, Manager, WebviewWindow,
    ipc::{InvokeBody, Request},
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Consent {
    Approved,
    Cancelled,
    Unavailable,
}

pub(crate) fn dialog_consent(
    result: MessageDialogResult,
    affirmative: &str,
    alive: bool,
) -> Consent {
    if !alive {
        return Consent::Unavailable;
    }
    match result {
        MessageDialogResult::Custom(label) if label == affirmative => Consent::Approved,
        MessageDialogResult::Custom(label) if label == "Cancel" => Consent::Cancelled,
        // An unclassified dismissal or unavailable callback is not proof that
        // the user selected this dialog's explicit Cancel button.
        _ => Consent::Unavailable,
    }
}

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
    fn cancelled_after_cleanup(close: &WorkspaceClose, request: &Cycle) -> Option<Self> {
        let state = close.state.lock().ok()?;
        let view = close.snapshot_locked(&state, false, None);
        if state.exiting
            || state.control.is_some()
            || !matches!(view.state, Phase::Locked)
            || view.storage != "managed_encrypted"
            || view.attempt_id.as_deref() != Some(request.attempt_id.as_str())
            || view.epoch.as_deref() != Some(request.epoch.as_str())
            || view.diagnostic.is_some()
        {
            return None;
        }
        Some(Self {
            schema_version: "rangoon.workspace-control-state.v1",
            close: view,
            phase: "locked",
            action: None,
            outcome: "cancelled",
            diagnostic: None,
            reload: false,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtectionResponse {
    schema_version: &'static str,
    close: View,
    layout: &'static str,
    diagnostic: Option<&'static str>,
}
impl ProtectionResponse {
    fn result(close: View, layout: &'static str) -> Self {
        let diagnostic = match layout {
            "absent" | "locked" => None,
            "busy" => Some("workspace_protection_busy"),
            "recovery_required" => Some("workspace_protection_recovery_required"),
            "legacy" => Some("encrypted_protection_unavailable"),
            _ => Some("workspace_protection_unavailable"),
        };
        Self {
            schema_version: "rangoon.workspace-protection-state.v1",
            close,
            layout,
            diagnostic,
        }
    }
    fn name(layout: Layout) -> &'static str {
        match layout {
            Layout::Absent => "absent",
            Layout::Locked => "locked",
            Layout::Busy => "busy",
            Layout::RecoveryRequired => "recovery_required",
            Layout::Unavailable => "unavailable",
        }
    }
    pub(crate) fn observe(close: &WorkspaceClose) -> Self {
        Self::observe_after(close, || {})
    }
    #[cfg(all(test, feature = "encrypted-workspace"))]
    pub(crate) fn observe_after_for_test(
        close: &WorkspaceClose,
        after_read: impl FnOnce(),
    ) -> Self {
        Self::observe_after(close, after_read)
    }
    fn observe_after(close: &WorkspaceClose, after_read: impl FnOnce()) -> Self {
        if !close.source.encrypted_close_available() {
            return Self::result(close.snapshot(None), "legacy");
        }
        let attempt = {
            let state = match close.state.lock() {
                Ok(state) => state,
                Err(error) => {
                    let state = error.into_inner();
                    return Self::result(
                        close.snapshot_locked(&state, true, None),
                        "recovery_required",
                    );
                }
            };
            let view = close.snapshot_locked(&state, false, None);
            if close.source.failure_latched() || state.failed {
                return Self::result(view, "recovery_required");
            }
            if state.control.is_some() {
                return Self::result(view, "busy");
            }
            let Some(attempt) = state.attempt.as_ref() else {
                return Self::result(view, "busy");
            };
            let progress = match attempt.progress.lock() {
                Ok(progress) => progress,
                Err(_) => return Self::result(view, "recovery_required"),
            };
            if !progress.healthy {
                return Self::result(view, "recovery_required");
            }
            if !progress.initial_done
                || !progress.native_done
                || !progress.acknowledged
                || progress.finishing
            {
                return Self::result(view, "busy");
            }
            Arc::clone(attempt)
        };
        // Source and backend guards are released before this State recheck.
        let layout = attempt.ticket.inspect_layout();
        after_read();
        let state = match close.state.lock() {
            Ok(state) => state,
            Err(error) => {
                let state = error.into_inner();
                return Self::result(
                    close.snapshot_locked(&state, true, None),
                    "recovery_required",
                );
            }
        };
        let view = close.snapshot_locked(&state, false, None);
        if close.source.failure_latched() || state.failed {
            return Self::result(view, "recovery_required");
        }
        if state.control.is_some()
            || !state
                .attempt
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &attempt))
        {
            return Self::result(view, "busy");
        }
        let Ok(progress) = attempt.progress.lock() else {
            return Self::result(view, "recovery_required");
        };
        if !progress.healthy {
            return Self::result(view, "recovery_required");
        }
        if !progress.initial_done
            || !progress.native_done
            || !progress.acknowledged
            || progress.finishing
        {
            return Self::result(view, "busy");
        }
        Self::result(view, Self::name(layout))
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
    consent: impl FnOnce(&Preview) -> Consent,
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
        {
            return None;
        }
        match consent(&preview) {
            Consent::Cancelled => Some(false),
            Consent::Unavailable => None,
            Consent::Approved => {
                flight.confirm(&preview.id).ok()?;
                consumed().then_some(true)
            }
        }
    }));
    if !matches!(result, Ok(Some(true))) {
        drop(flight);
        if matches!(result, Ok(Some(false)))
            && let Some(cancelled) = Response::cancelled_after_cleanup(close, &request)
        {
            return cancelled;
        }
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

fn native_consent(app: &AppHandle, cleanup: &WorkerCleanup, preview: &Preview) -> Consent {
    if !cleanup.alive() || preview.source_id.is_some() {
        return Consent::Unavailable;
    }
    let Some(parent) = app.get_webview_window("main") else {
        return Consent::Unavailable;
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
        _ => return Consent::Unavailable,
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
    let consent = dialog_consent(result, affirmative, cleanup.alive());
    #[cfg(all(test, feature = "native-encrypted-qualification", target_os = "macos"))]
    {
        let purpose = match preview.kind {
            NativeAction::CreateEmpty => "create_empty",
            NativeAction::Unlock => "unlock",
            _ => return Consent::Unavailable,
        };
        crate::native_encrypted_qualification::consent_observed(purpose, consent);
    }
    consent
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
pub(crate) async fn get_workspace_protection_state(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ProtectionResponse, ()> {
    allowed(window.label())?;
    if !empty(request.body()) {
        return Err(());
    }
    let close = app.state::<WorkspaceClose>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || ProtectionResponse::observe(&close))
        .await
        .map_err(|_| ())
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

#[cfg(test)]
mod native_dialog_tests {
    use super::{Consent, MessageDialogResult, dialog_consent};

    #[test]
    fn only_live_exact_native_buttons_classify_consent() {
        assert_eq!(
            dialog_consent(
                MessageDialogResult::Custom("Set up workspace".into()),
                "Set up workspace",
                true
            ),
            Consent::Approved
        );
        assert_eq!(
            dialog_consent(
                MessageDialogResult::Custom("Cancel".into()),
                "Set up workspace",
                true
            ),
            Consent::Cancelled
        );
        for result in [
            MessageDialogResult::Cancel,
            MessageDialogResult::Yes,
            MessageDialogResult::No,
            MessageDialogResult::Ok,
            MessageDialogResult::Custom("cancel".into()),
            MessageDialogResult::Custom("Unlock workspace".into()),
        ] {
            assert_eq!(
                dialog_consent(result, "Set up workspace", true),
                Consent::Unavailable
            );
        }
        for label in ["Cancel", "Set up workspace"] {
            assert_eq!(
                dialog_consent(
                    MessageDialogResult::Custom(label.into()),
                    "Set up workspace",
                    false
                ),
                Consent::Unavailable
            );
        }
    }
}
