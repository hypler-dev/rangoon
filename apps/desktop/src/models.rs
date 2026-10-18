//! Desktop model orchestration. Only a native OS decision permits source transfer.
use crate::{
    AdmissionError,
    model_consent::{Consent, Decision, REVIEW_WINDOW, Review, SEND_LABEL},
    model_flight::{ModelFlight, ModelLease},
};
use rangoon_model_local::{CheckResult, Completion, LocalClient};
use rangoon_model_session::{
    Diagnostic, Freshness, LocalSession, Operation, PreparedView, SessionView, Transmission,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{
    AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
    ipc::{InvokeBody, Request},
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogResult};

#[derive(Default)]
pub struct NativeModel {
    session: LocalSession,
    consent: Consent<SendCustody>,
}

struct LocalPreparationCustody {
    ready: rangoon_model_session::RetainedPreparation,
    _flight: ModelLease,
}

struct SendCustody {
    send: Transmission,
    _flight: ModelLease,
}
impl NativeModel {
    pub(super) fn purge_for_workspace_close(&self) -> crate::workspace_close::Cleanup {
        let session = self.session.purge_for_workspace_close();
        let consent = self.consent.purge();
        crate::workspace_close::Cleanup {
            healthy: session && consent.healthy,
            keep_parent: consent.keep_parent,
        }
    }
}
impl SendCustody {
    fn ensure_current(&self) -> Result<(), Diagnostic> {
        if !self._flight.is_current() {
            return Err(Diagnostic::Cancelled);
        }
        self.send.operation().ensure_current()
    }
}
impl std::ops::Deref for SendCustody {
    type Target = Transmission;
    fn deref(&self) -> &Self::Target {
        &self.send
    }
}

/// IPC cancellation must reach retained blocking work before it can stage or
/// dispatch anything. The callback still owns its leases until it returns.
struct CancelOnDrop(rangoon_model_local::Cancellation);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Stamp {
    generation: Option<String>,
    run_id: Option<String>,
}
impl Stamp {
    fn idle(model: &NativeModel) -> Self {
        Self {
            generation: model.session.inspect().ok().map(|s| s.generation),
            run_id: None,
        }
    }
    fn operation(operation: &Operation) -> Self {
        Self {
            generation: Some(operation.generation()),
            run_id: Some(operation.run_id().to_owned()),
        }
    }
    fn result(self, result: Outcome) -> ModelResult {
        ModelResult {
            schema_version: "rangoon.local-session-result.v1",
            stamp: self,
            authority: "none",
            result,
        }
    }
    fn error(self, code: Code) -> ModelResult {
        self.result(
            if matches!(
                code,
                Code::Session(Diagnostic::Cancelled)
                    | Code::Transport(rangoon_model_local::Diagnostic::Cancelled)
            ) {
                Outcome::Cancelled
            } else {
                Outcome::Failed { code }
            },
        )
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelResult {
    schema_version: &'static str,
    #[serde(flatten)]
    stamp: Stamp,
    authority: &'static str,
    #[serde(flatten)]
    result: Outcome,
}
#[derive(Serialize)]
#[serde(
    tag = "outcome",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum Outcome {
    Unconfigured {
        session: SessionView,
    },
    Configured {
        session: SessionView,
    },
    Cleared {
        session: SessionView,
    },
    Prepared {
        prepared: Box<PreparedView>,
    },
    Checked {
        check: CheckResult,
    },
    Completed {
        completion: Box<Completion>,
        freshness: Freshness,
    },
    CancelRequested {
        session: SessionView,
    },
    Cancelled,
    Failed {
        code: Code,
    },
}
#[derive(Serialize)]
#[serde(untagged)]
enum Code {
    Session(Diagnostic),
    Transport(rangoon_model_local::Diagnostic),
    Native(NativeCode),
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum NativeCode {
    WorkspaceBusy,
    ConfirmationUnavailable,
}

fn raw(body: &InvokeBody, limit: usize) -> Result<&[u8], Diagnostic> {
    match body {
        InvokeBody::Raw(bytes) if bytes.len() <= limit => Ok(bytes),
        _ => Err(Diagnostic::InvalidRequest),
    }
}
fn empty(body: &InvokeBody) -> bool {
    matches!(body, InvokeBody::Json(value) if value.as_object().is_some_and(|o| o.is_empty()))
}
fn allowed(label: &str, expected: &str) -> Result<(), ()> {
    if label == expected { Ok(()) } else { Err(()) }
}
fn profile_result(model: &NativeModel) -> ModelResult {
    match model.session.inspect() {
        Ok(session) => Stamp {
            generation: Some(session.generation.clone()),
            run_id: None,
        }
        .result(if session.profile.is_some() {
            Outcome::Configured { session }
        } else {
            Outcome::Unconfigured { session }
        }),
        Err(error) => Stamp::idle(model).error(Code::Session(error)),
    }
}

#[tauri::command]
pub fn get_local_model_profile(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ModelResult, ()> {
    allowed(window.label(), "main")?;
    let model = app.state::<NativeModel>();
    Ok(if empty(request.body()) {
        match app.state::<ModelFlight>().read_for(&workspace) {
            Ok(lease) => {
                let result = profile_result(&model);
                if lease.is_current() {
                    result
                } else {
                    Stamp::idle(&model).error(Code::Session(Diagnostic::SessionUnavailable))
                }
            }
            Err(error) => Stamp::idle(&model).error(Code::Session(error)),
        }
    } else {
        Stamp::idle(&model).error(Code::Session(Diagnostic::InvalidRequest))
    })
}
#[tauri::command]
pub fn configure_local_model(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ModelResult, ()> {
    allowed(window.label(), "main")?;
    let model = app.state::<NativeModel>();
    let result = raw(request.body(), 1024).and_then(|bytes| {
        let flight = app.state::<ModelFlight>().begin_for(&workspace)?;
        if !flight.is_current() {
            return Err(Diagnostic::SessionUnavailable);
        }
        let result = model.session.configure(bytes)?;
        if !flight.is_current() {
            return Err(Diagnostic::SessionUnavailable);
        }
        Ok(result)
    });
    Ok(match result {
        Ok(session) => Stamp {
            generation: Some(session.generation.clone()),
            run_id: None,
        }
        .result(Outcome::Configured { session }),
        Err(error) => Stamp::idle(&model).error(Code::Session(error)),
    })
}
#[tauri::command]
pub fn clear_local_model(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ModelResult, ()> {
    allowed(window.label(), "main")?;
    let model = app.state::<NativeModel>();
    let _admission = match app.state::<ModelFlight>().read_for(&workspace) {
        Ok(lease) => lease,
        Err(error) => return Ok(Stamp::idle(&model).error(Code::Session(error))),
    };
    if !empty(request.body()) {
        return Ok(Stamp::idle(&model).error(Code::Session(Diagnostic::InvalidRequest)));
    }
    let result = model.session.clear();
    model.consent.purge();
    Ok(match result {
        Ok(session) => Stamp {
            generation: Some(session.generation.clone()),
            run_id: None,
        }
        .result(Outcome::Cleared { session }),
        Err(error) => Stamp::idle(&model).error(Code::Session(error)),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CancelInput {
    schema_version: String,
    run_id: String,
}

#[tauri::command]
pub fn cancel_local_model(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ModelResult, ()> {
    allowed(window.label(), "main")?;
    let model = app.state::<NativeModel>();
    let _admission = match app.state::<ModelFlight>().read_for(&workspace) {
        Ok(lease) => lease,
        Err(error) => return Ok(Stamp::idle(&model).error(Code::Session(error))),
    };
    let result = raw(request.body(), 256).and_then(|bytes| {
        let input: CancelInput =
            serde_json::from_slice(bytes).map_err(|_| Diagnostic::InvalidRequest)?;
        if input.schema_version != "rangoon.local-cancel.v1" {
            return Err(Diagnostic::InvalidRequest);
        }
        model.session.cancel(bytes)?;
        model.consent.cancel_run(Some(&input.run_id));
        model.session.inspect()
    });
    Ok(match result {
        Ok(session) => Stamp {
            generation: Some(session.generation.clone()),
            run_id: None,
        }
        .result(Outcome::CancelRequested { session }),
        Err(error) => Stamp::idle(&model).error(Code::Session(error)),
    })
}

#[tauri::command]
pub async fn prepare_local_model(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ModelResult, ()> {
    allowed(window.label(), "main")?;
    let model = app.state::<NativeModel>();
    let flight = match app.state::<ModelFlight>().begin_for(&workspace) {
        Ok(value) => value,
        Err(error) => return Ok(Stamp::idle(&model).error(Code::Session(error))),
    };
    // Every attempt invalidates prior preparation, including non-raw/oversized input.
    let preparation = match model
        .session
        .begin_prepare(raw(request.body(), 8192).unwrap_or_default())
    {
        Ok(value) => value,
        Err(error) => return Ok(Stamp::idle(&model).error(Code::Session(error))),
    };
    let stamp = Stamp::operation(preparation.operation());
    let _cancel = CancelOnDrop(preparation.operation().cancellation().clone());
    let guard = match app
        .state::<super::SourceAccess>()
        .for_request(&workspace)
        .begin_model_inputs()
    {
        Ok(value) => value,
        Err(AdmissionError::Busy) => {
            return Ok(stamp.error(Code::Native(NativeCode::WorkspaceBusy)));
        }
        Err(AdmissionError::Unavailable) => {
            return Ok(stamp.error(Code::Session(Diagnostic::SessionUnavailable)));
        }
    };
    let completed = tauri::async_runtime::spawn_blocking(move || {
        guard.with_inputs(|reader| {
            preparation
                .prepare_with_inputs(reader)
                .map(|ready| LocalPreparationCustody {
                    ready,
                    _flight: flight,
                })
        })
    })
    .await;
    Ok(match completed {
        Ok(completed) => match completed.deliver_using(
            |custody| {
                if !custody._flight.is_current() {
                    return Err(Diagnostic::Cancelled);
                }
                custody.ready.ensure_current()?;
                Ok(custody.ready.view().clone())
            },
            |custody| {
                if !custody._flight.is_current() {
                    return Err(Diagnostic::Cancelled);
                }
                custody.ready.publish()
            },
        ) {
            Ok(prepared) => stamp.result(Outcome::Prepared {
                prepared: Box::new(prepared),
            }),
            Err(error) => stamp.error(Code::Session(error)),
        },
        Err(_) => stamp.error(Code::Session(Diagnostic::SessionUnavailable)),
    })
}

#[tauri::command]
pub async fn check_local_model(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ModelResult, ()> {
    allowed(window.label(), "main")?;
    let model = app.state::<NativeModel>();
    if !empty(request.body()) {
        return Ok(Stamp::idle(&model).error(Code::Session(Diagnostic::InvalidRequest)));
    }
    let _flight = match app.state::<ModelFlight>().begin_for(&workspace) {
        Ok(value) => value,
        Err(error) => return Ok(Stamp::idle(&model).error(Code::Session(error))),
    };
    let operation = match model.session.begin_check() {
        Ok(value) => value,
        Err(error) => return Ok(Stamp::idle(&model).error(Code::Session(error))),
    };
    let stamp = Stamp::operation(&operation);
    let _cancel = CancelOnDrop(operation.cancellation().clone());
    if !_flight.is_current() {
        return Ok(stamp.error(Code::Session(Diagnostic::Cancelled)));
    }
    let result = LocalClient::new()
        .check(operation.profile(), operation.cancellation())
        .await;
    if !_flight.is_current() {
        return Ok(stamp.error(Code::Session(Diagnostic::Cancelled)));
    }
    Ok(match operation.ensure_current() {
        Err(error) => stamp.error(Code::Session(error)),
        Ok(()) => match result {
            Ok(check) => stamp.result(Outcome::Checked { check }),
            Err(error) => stamp.error(Code::Transport(error)),
        },
    })
}

struct ReviewGuard {
    app: AppHandle,
    run_id: String,
}

/// Cleanup belongs to the callback, including unwinding after a dialog failure.
struct PromptCompletion {
    app: AppHandle,
    run_id: String,
    approved: bool,
}
impl Drop for PromptCompletion {
    fn drop(&mut self) {
        if self
            .app
            .state::<NativeModel>()
            .consent
            .finish_prompt(&self.run_id, self.approved)
            && let Some(window) = self.app.get_webview_window(REVIEW_WINDOW)
        {
            let _ = window.close();
        }
    }
}
impl Drop for ReviewGuard {
    fn drop(&mut self) {
        if self.app.state::<NativeModel>().consent.remove(&self.run_id)
            && let Some(window) = self.app.get_webview_window(REVIEW_WINDOW)
        {
            let _ = window.close();
        }
    }
}
fn review_navigation(url: &tauri::Url) -> bool {
    crate::bundled_navigation(url)
        && url.path() == "/model-confirmation.html"
        && url.query().is_none()
        && url.fragment().is_none()
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
}
fn open_review(app: &AppHandle, run_id: &str) -> Result<(), ()> {
    let parent = app.get_webview_window("main").ok_or(())?;
    if app.get_webview_window(REVIEW_WINDOW).is_some() {
        return Err(());
    }
    let (width, height) = crate::parented_review_window_size(&parent, (1000.0, 780.0));
    let review_window = WebviewWindowBuilder::new(
        app,
        REVIEW_WINDOW,
        WebviewUrl::App("model-confirmation.html".into()),
    )
    .title("Rangoon — Review selected text")
    .inner_size(width, height)
    .min_inner_size(360.0, 480.0)
    .parent(&parent)
    .map_err(|_| ())?
    .on_navigation(review_navigation)
    .build()
    .map_err(|_| ())?;
    let event_app = app.clone();
    let event_run = run_id.to_owned();
    review_window.on_window_event(move |event| {
        let model = event_app.state::<NativeModel>();
        match event {
            WindowEvent::CloseRequested { api, .. } => {
                if model.consent.cancel_run(Some(&event_run)) {
                    api.prevent_close();
                }
            }
            WindowEvent::Destroyed => {
                model.consent.cancel_run(Some(&event_run));
            }
            _ => {}
        }
    });
    Ok(())
}

async fn freshness(app: AppHandle, send: &Arc<SendCustody>) -> Result<Freshness, Code> {
    let guard = app
        .state::<super::SourceAccess>()
        .for_model_callback(&send._flight)
        .begin_model_inputs()
        .map_err(|error| match error {
            AdmissionError::Busy => Code::Native(NativeCode::WorkspaceBusy),
            AdmissionError::Unavailable => Code::Session(Diagnostic::SessionUnavailable),
        })?;
    let retained = Arc::clone(send);
    let completed = tauri::async_runtime::spawn_blocking(move || {
        guard.with_inputs(|reader| {
            retained.ensure_current()?;
            retained.freshness_with_inputs(reader)
        })
    })
    .await
    .map_err(|_| Code::Session(Diagnostic::SessionUnavailable))?;
    completed.deliver().map_err(Code::Session)
}

#[tauri::command]
pub async fn send_local_model(
    workspace: crate::workspace_requests::WorkspaceRequest,
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ModelResult, ()> {
    allowed(window.label(), "main")?;
    let model = app.state::<NativeModel>();
    let flight = match app.state::<ModelFlight>().begin_for(&workspace) {
        Ok(value) => value,
        Err(error) => return Ok(Stamp::idle(&model).error(Code::Session(error))),
    };
    let send = match raw(request.body(), 512).and_then(|bytes| model.session.begin_send(bytes)) {
        Ok(send) => send,
        Err(error) => return Ok(Stamp::idle(&model).error(Code::Session(error))),
    };
    let stamp = Stamp::operation(send.operation());
    let send = Arc::new(SendCustody {
        send,
        _flight: flight,
    });
    let _cancel = CancelOnDrop(send.operation().cancellation().clone());
    let decision = match model.consent.install_guarded(
        Review::from_transmission(&send),
        send.operation().cancellation().clone(),
        Arc::clone(&send),
        send._flight.witness(),
    ) {
        Some(receiver) => receiver,
        None => return Ok(stamp.error(Code::Native(NativeCode::ConfirmationUnavailable))),
    };
    let _review = ReviewGuard {
        app: app.clone(),
        run_id: send.operation().run_id().to_owned(),
    };
    if open_review(&app, send.operation().run_id()).is_err() {
        return Ok(stamp.error(Code::Native(NativeCode::ConfirmationUnavailable)));
    }
    if let Err(error) = send.ensure_current() {
        return Ok(stamp.error(Code::Session(error)));
    }
    if !decision.await.unwrap_or(false) {
        return Ok(stamp.result(Outcome::Cancelled));
    }
    if let Err(error) = send.ensure_current() {
        return Ok(stamp.error(Code::Session(error)));
    }
    // Close the reviewed surface after the OS decision, before any request.
    drop(_review);
    match freshness(app.clone(), &send).await {
        Ok(Freshness::Current) => {}
        Ok(Freshness::Stale) => return Ok(stamp.error(Code::Session(Diagnostic::InputStale))),
        Ok(Freshness::Unavailable) => {
            return Ok(stamp.error(Code::Session(Diagnostic::InputUnavailable)));
        }
        Err(error) => return Ok(stamp.error(error)),
    }
    if let Err(error) = send.ensure_current() {
        return Ok(stamp.error(Code::Session(error)));
    }
    let response = LocalClient::new()
        .analyze(send.request(), send.operation().cancellation())
        .await;
    if let Err(error) = send.ensure_current() {
        return Ok(stamp.error(Code::Session(error)));
    }
    let completion = match response {
        Ok(value) => value,
        Err(error) => return Ok(stamp.error(Code::Transport(error))),
    };
    let observed = freshness(app, &send).await;
    if let Err(error) = send.ensure_current() {
        return Ok(stamp.error(Code::Session(error)));
    }
    Ok(stamp.result(Outcome::Completed {
        completion: Box::new(completion),
        freshness: observed.unwrap_or(Freshness::Unavailable),
    }))
}

#[derive(Serialize)]
#[serde(
    tag = "outcome",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ReviewResult {
    Ready {
        schema_version: &'static str,
        authority: &'static str,
        #[serde(flatten)]
        review: Review,
    },
    Unavailable {
        schema_version: &'static str,
        authority: &'static str,
    },
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStatus {
    schema_version: &'static str,
    outcome: &'static str,
    authority: &'static str,
}
fn review_status(finished: bool) -> ReviewStatus {
    ReviewStatus {
        schema_version: "rangoon.local-review-status.v1",
        outcome: if finished { "finished" } else { "unavailable" },
        authority: "none",
    }
}
#[tauri::command]
pub fn get_local_model_review(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ReviewResult, ()> {
    allowed(window.label(), REVIEW_WINDOW)?;
    let review = empty(request.body())
        .then(|| app.state::<NativeModel>().consent.inspect())
        .flatten();
    Ok(match review {
        Some(review) => ReviewResult::Ready {
            schema_version: "rangoon.local-review.v1",
            authority: "none",
            review,
        },
        None => ReviewResult::Unavailable {
            schema_version: "rangoon.local-review.v1",
            authority: "none",
        },
    })
}
#[tauri::command]
pub fn cancel_local_model_review(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ReviewStatus, ()> {
    allowed(window.label(), REVIEW_WINDOW)?;
    let decision = raw(request.body(), 512).ok().and_then(Decision::parse);
    Ok(review_status(decision.is_some_and(|d| {
        app.state::<NativeModel>().consent.cancel_decision(&d)
    })))
}
#[tauri::command]
pub async fn confirm_local_model_review(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ReviewStatus, ()> {
    allowed(window.label(), REVIEW_WINDOW)?;
    let decision = raw(request.body(), 512).ok().and_then(Decision::parse);
    let Some(review) = decision.and_then(|d| app.state::<NativeModel>().consent.prompt(&d)) else {
        return Ok(review_status(false));
    };
    let run_id = review.run_id.clone();
    let dialog_app = app.clone();
    let answer = tauri::async_runtime::spawn_blocking(move || {
        let mut completion = PromptCompletion {
            app: dialog_app.clone(),
            run_id,
            approved: false,
        };
        let answer = dialog_app
            .dialog()
            .message(review.question())
            .title("Send selected text?")
            .parent(&window)
            .buttons(MessageDialogButtons::OkCancelCustom(
                SEND_LABEL.into(),
                "Cancel".into(),
            ))
            .blocking_show_with_result();
        completion.approved =
            matches!(answer, MessageDialogResult::Custom(label) if label == SEND_LABEL);
    })
    .await;
    Ok(review_status(answer.is_ok()))
}

pub fn window_event(window: &tauri::Window, event: &WindowEvent) {
    if window.label() != "main" {
        return;
    }
    if let WindowEvent::CloseRequested { api, .. } = event {
        let model = window.state::<NativeModel>();
        let prompting = match model.session.clear() {
            Ok(session) => session
                .active
                .is_some_and(|active| model.consent.cancel_run(Some(&active.run_id))),
            Err(_) => model.consent.cancel_run(None),
        };
        if prompting {
            api.prevent_close();
        }
    }
}

#[cfg(test)]
#[path = "models_tests.rs"]
mod tests;

#[cfg(test)]
mod close_cleanup_tests {
    use super::*;
    #[test]
    fn native_local_close_removes_profile_and_cancels_active_work() {
        let model = NativeModel::default();
        model.session.configure(br#"{"schemaVersion":"rangoon.local-profile-request.v1","profileId":"local","host":"127.0.0.1","port":11434,"model":"fixture:v1","maxOutputTokens":128}"#).unwrap();
        let operation = model.session.begin_check().unwrap();
        let status = model.purge_for_workspace_close();
        assert!(status.healthy);
        assert!(!status.keep_parent);
        assert!(operation.cancellation().is_cancelled());
        assert!(operation.ensure_current().is_err());
        assert!(model.session.inspect().unwrap().profile.is_none());
        assert!(model.session.inspect().unwrap().active.is_some());
        drop(operation);
        assert!(model.session.inspect().unwrap().active.is_none());
    }
}
