//! Native cloud orchestration. A retained request is never transfer permission.
use crate::{
    AdmissionError, begin_operation,
    cloud_custody::{Custody, Lease, Phase},
    cloud_model_consent::{Consent, Decision, Purpose, REVIEW_WINDOW, Review},
    cloud_store::{self, OsStore, Secret},
    model_flight::{ModelFlight, ModelLease},
    workspace,
};
use rangoon_model_cloud::{Cancellation, CheckRequest, CheckResult, CloudClient, Completion};
use rangoon_model_session::{
    Diagnostic, Freshness,
    cloud::{CloudSession, Operation, PreparedView, Resolution, SessionView, Transmission},
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::{
    AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
    ipc::{InvokeBody, Request},
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogResult};

#[derive(Default)]
pub struct NativeCloud {
    state: Mutex<State>,
    consent: Consent<Flow>,
}
#[derive(Default)]
struct State {
    session: CloudSession,
    staged: Option<Envelope>,
}
struct Envelope {
    prepared_id: String,
    request_id: String,
    secret: Secret,
}
impl NativeCloud {
    fn lock(&self) -> Result<MutexGuard<'_, State>, Diagnostic> {
        self.state
            .lock()
            .map_err(|_| Diagnostic::SessionUnavailable)
    }
    fn inspect(&self) -> Result<SessionView, Diagnostic> {
        self.lock()?.session.inspect()
    }
    fn configure(&self, raw: &[u8]) -> Result<SessionView, Diagnostic> {
        let mut state = self.lock()?;
        let view = state.session.configure(raw)?;
        state.staged = None;
        Ok(view)
    }
    fn clear(&self) -> Result<SessionView, Diagnostic> {
        let mut state = self.lock()?;
        state.staged = None;
        state.session.clear()
    }
    fn resolution(&self, raw: &[u8]) -> Result<Resolution, Diagnostic> {
        let mut state = self.lock()?;
        state.staged = None;
        state.session.begin_resolution(raw)
    }
    fn stage(
        &self,
        ready: rangoon_model_session::cloud::RetainedPreparation,
        secret: Secret,
    ) -> Result<PreparedView, Diagnostic> {
        let mut state = self.lock()?;
        ready.ensure_current()?;
        let view = ready.view();
        if view.credential_revision != secret.revision() {
            return Err(Diagnostic::StalePrepared);
        }
        state.staged = Some(Envelope {
            prepared_id: view.prepared_id.clone(),
            request_id: view.request_id.clone(),
            secret,
        });
        // The portable operation is released only after pairing, while the native
        // lock still excludes clear/configure/competing preparation/claim.
        Ok(ready.into_view())
    }
    fn claim(&self, raw: &[u8]) -> Result<(Transmission, Secret), Diagnostic> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Claim {
            schema_version: String,
            prepared_id: String,
            request_id: String,
        }
        if raw.len() > 512 {
            return Err(Diagnostic::InvalidRequest);
        }
        let input: Claim = serde_json::from_slice(raw).map_err(|_| Diagnostic::InvalidRequest)?;
        if input.schema_version != "rangoon.cloud-send.v1" {
            return Err(Diagnostic::InvalidRequest);
        }
        let mut state = self.lock()?;
        let send = state.session.begin_send(raw)?;
        let pair = state.staged.take().ok_or(Diagnostic::StalePrepared)?;
        if pair.prepared_id != input.prepared_id
            || pair.request_id != input.request_id
            || pair.request_id != send.request().request_id()
            || pair.secret.revision() != send.credential_revision()
        {
            return Err(Diagnostic::StalePrepared);
        }
        Ok((send, pair.secret))
    }
    fn cancel(&self, raw: &[u8]) -> Result<(SessionView, String), Diagnostic> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Input {
            schema_version: String,
            run_id: String,
        }
        if raw.len() > 256 {
            return Err(Diagnostic::InvalidRequest);
        }
        let input: Input = serde_json::from_slice(raw).map_err(|_| Diagnostic::InvalidRequest)?;
        if input.schema_version != "rangoon.cloud-cancel.v1" {
            return Err(Diagnostic::InvalidRequest);
        }
        let mut state = self.lock()?;
        state.session.cancel(raw)?;
        state.staged = None;
        Ok((state.session.inspect()?, input.run_id))
    }
}

struct Resources {
    _flight: ModelLease,
    credential: Lease,
}
impl Resources {
    fn acquire(app: &AppHandle) -> Result<Arc<Self>, Code> {
        let flight = app.state::<ModelFlight>().begin().map_err(Code::Session)?;
        let (credential, receiver) = app
            .state::<Custody>()
            .begin(Phase::Using)
            .map_err(store_code)?;
        drop(receiver);
        Ok(Arc::new(Self {
            _flight: flight,
            credential,
        }))
    }
    fn ensure_current(&self) -> Result<(), Code> {
        if self.credential.is_current() {
            Ok(())
        } else {
            Err(Code::Session(Diagnostic::Cancelled))
        }
    }
}
struct CancelOnDrop(Cancellation);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
enum Work {
    Analysis(Transmission),
    Check {
        operation: Operation,
        request: Box<CheckRequest>,
    },
}
struct Flow {
    work: Work,
    secret: Secret,
    resources: Arc<Resources>,
}
impl Flow {
    fn operation(&self) -> &Operation {
        match &self.work {
            Work::Analysis(send) => send.operation(),
            Work::Check { operation, .. } => operation,
        }
    }
    fn ensure_current(&self) -> Result<(), Code> {
        self.operation().ensure_current().map_err(Code::Session)?;
        self.resources.ensure_current()
    }
    fn review(&self) -> Review {
        let operation = self.operation();
        let mut review = Review {
            generation: operation.generation(),
            run_id: operation.run_id().to_owned(),
            request_id: String::new(),
            origin: operation.profile().origin().to_owned(),
            model: operation.profile().model().to_owned(),
            purpose: Purpose::ModelCheck,
            method: "GET".into(),
            path: String::new(),
            credential_revision: self.secret.revision(),
            body_sha256: rangoon_domain::byte_digest(b""),
            body_bytes: 0,
            body_json: String::new(),
            pack_id: None,
            inputs: vec![],
        };
        match &self.work {
            Work::Analysis(send) => {
                review.purpose = Purpose::Analysis;
                review.request_id = send.request().request_id().to_owned();
                review.method = "POST".into();
                review.path = "/v1/responses".into();
                review.body_json = send.request().body_json().to_owned();
                review.body_bytes = review.body_json.len();
                review.body_sha256 = rangoon_domain::byte_digest(review.body_json.as_bytes());
                review.pack_id = Some(send.pack_id().to_owned());
                review.inputs = send.inputs().cloned().collect();
            }
            Work::Check { request, .. } => {
                review.request_id = request.request_id().to_owned();
                review.path = request.path().to_owned();
            }
        }
        review
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Stamp {
    generation: Option<String>,
    run_id: Option<String>,
}
impl Stamp {
    fn idle(model: &NativeCloud) -> Self {
        Self {
            generation: model.inspect().ok().map(|s| s.generation),
            run_id: None,
        }
    }
    fn operation(operation: &Operation) -> Self {
        Self {
            generation: Some(operation.generation()),
            run_id: Some(operation.run_id().to_owned()),
        }
    }
    fn result(self, outcome: Outcome) -> ModelResult {
        ModelResult {
            schema_version: "rangoon.cloud-session-result.v1",
            stamp: self,
            authority: "none",
            outcome,
        }
    }
    fn error(self, code: Code) -> ModelResult {
        self.result(
            if matches!(
                code,
                Code::Session(Diagnostic::Cancelled)
                    | Code::Transport(rangoon_model_cloud::Diagnostic::Cancelled)
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
    outcome: Outcome,
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
    Transport(rangoon_model_cloud::Diagnostic),
    Native(NativeCode),
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum NativeCode {
    WorkspaceBusy,
    ConfirmationUnavailable,
    CredentialUnavailable,
    CredentialMissing,
    InvalidStoredCredential,
    CredentialChanged,
}
fn store_code(error: cloud_store::Error) -> Code {
    match error {
        cloud_store::Error::Busy => Code::Session(Diagnostic::Busy),
        cloud_store::Error::InvalidStoredCredential => {
            Code::Native(NativeCode::InvalidStoredCredential)
        }
        cloud_store::Error::Changed => Code::Native(NativeCode::CredentialChanged),
        _ => Code::Native(NativeCode::CredentialUnavailable),
    }
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
fn allowed(actual: &str, expected: &str) -> Result<(), ()> {
    if actual == expected { Ok(()) } else { Err(()) }
}
fn profile_result(model: &NativeCloud) -> ModelResult {
    match model.inspect() {
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
fn read_secret(app: &AppHandle) -> Result<Secret, Code> {
    let store = OsStore::open(&app.config().identifier).map_err(store_code)?;
    cloud_store::inspect(&store)
        .map_err(store_code)?
        .ok_or(Code::Native(NativeCode::CredentialMissing))
}
fn cancel_result(model: &NativeCloud, bytes: &[u8]) -> ModelResult {
    match model.cancel(bytes) {
        Ok((session, run_id)) => {
            model.consent.cancel_run(Some(&run_id));
            Stamp {
                generation: Some(session.generation.clone()),
                run_id: Some(run_id),
            }
            .result(Outcome::CancelRequested { session })
        }
        Err(error) => Stamp::idle(model).error(Code::Session(error)),
    }
}

#[tauri::command]
pub fn get_cloud_model_profile(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ModelResult, ()> {
    allowed(window.label(), "main")?;
    let model = app.state::<NativeCloud>();
    Ok(if empty(request.body()) {
        profile_result(&model)
    } else {
        Stamp::idle(&model).error(Code::Session(Diagnostic::InvalidRequest))
    })
}
#[tauri::command]
pub fn configure_cloud_model(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ModelResult, ()> {
    allowed(window.label(), "main")?;
    let model = app.state::<NativeCloud>();
    let result = raw(request.body(), 1024).and_then(|bytes| {
        let _flight = app.state::<ModelFlight>().begin()?;
        model.configure(bytes)
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
pub fn clear_cloud_model(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ModelResult, ()> {
    allowed(window.label(), "main")?;
    let model = app.state::<NativeCloud>();
    if !empty(request.body()) {
        return Ok(Stamp::idle(&model).error(Code::Session(Diagnostic::InvalidRequest)));
    }
    let result = model.clear();
    match &result {
        Ok(view) => {
            if let Some(active) = &view.active {
                model.consent.cancel_run(Some(&active.run_id));
            }
        }
        Err(_) => {
            model.consent.cancel_run(None);
        }
    }
    Ok(match result {
        Ok(session) => Stamp {
            generation: Some(session.generation.clone()),
            run_id: None,
        }
        .result(Outcome::Cleared { session }),
        Err(error) => Stamp::idle(&model).error(Code::Session(error)),
    })
}
#[tauri::command]
pub fn cancel_cloud_model(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ModelResult, ()> {
    allowed(window.label(), "main")?;
    let model = app.state::<NativeCloud>();
    Ok(match raw(request.body(), 256) {
        Ok(bytes) => cancel_result(&model, bytes),
        Err(error) => Stamp::idle(&model).error(Code::Session(error)),
    })
}
#[tauri::command]
pub async fn prepare_cloud_model(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ModelResult, ()> {
    allowed(window.label(), "main")?;
    let model = app.state::<NativeCloud>();
    let resolution = match model.resolution(raw(request.body(), 8192).unwrap_or_default()) {
        Ok(value) => value,
        Err(error) => return Ok(Stamp::idle(&model).error(Code::Session(error))),
    };
    let stamp = Stamp::operation(resolution.operation());
    let _cancel = CancelOnDrop(resolution.operation().cancellation().clone());
    let resources = match Resources::acquire(&app) {
        Ok(value) => value,
        Err(error) => return Ok(stamp.error(error)),
    };
    let failure = stamp.clone();
    Ok(tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            resources.ensure_current()?;
            resolution
                .operation()
                .ensure_current()
                .map_err(Code::Session)?;
            let secret = read_secret(&app)?;
            resources.ensure_current()?;
            let preparation = resolution
                .with_credential_revision(&secret.revision())
                .map_err(Code::Session)?;
            let _guard = begin_operation(&app).map_err(|error| match error {
                AdmissionError::Busy => Code::Native(NativeCode::WorkspaceBusy),
                AdmissionError::Unavailable => Code::Session(Diagnostic::SessionUnavailable),
            })?;
            let store = workspace(&app).map_err(|_| Code::Session(Diagnostic::InputUnavailable))?;
            let ready = preparation
                .prepare_retained(&store)
                .map_err(Code::Session)?;
            resources.ensure_current()?;
            app.state::<NativeCloud>()
                .stage(ready, secret)
                .map_err(Code::Session)
        })();
        match result {
            Ok(prepared) => stamp.result(Outcome::Prepared {
                prepared: Box::new(prepared),
            }),
            Err(error) => stamp.error(error),
        }
    })
    .await
    .unwrap_or_else(|_| failure.error(Code::Session(Diagnostic::SessionUnavailable))))
}

#[tauri::command]
pub async fn check_cloud_model(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ModelResult, ()> {
    allowed(window.label(), "main")?;
    let model = app.state::<NativeCloud>();
    if !empty(request.body()) {
        return Ok(Stamp::idle(&model).error(Code::Session(Diagnostic::InvalidRequest)));
    }
    let operation = match model.lock().and_then(|state| state.session.begin_check()) {
        Ok(value) => value,
        Err(error) => return Ok(Stamp::idle(&model).error(Code::Session(error))),
    };
    let stamp = Stamp::operation(&operation);
    let _cancel = CancelOnDrop(operation.cancellation().clone());
    let resources = match Resources::acquire(&app) {
        Ok(value) => value,
        Err(error) => return Ok(stamp.error(error)),
    };
    let callback_app = app.clone();
    let flow = tauri::async_runtime::spawn_blocking(move || {
        resources.ensure_current()?;
        operation.ensure_current().map_err(Code::Session)?;
        let secret = read_secret(&callback_app)?;
        let request =
            CheckRequest::new(operation.profile(), &secret.revision()).map_err(Code::Transport)?;
        let flow = Arc::new(Flow {
            work: Work::Check {
                operation,
                request: Box::new(request),
            },
            secret,
            resources,
        });
        flow.ensure_current()?;
        Ok::<_, Code>(flow)
    })
    .await;
    let flow = match flow {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => return Ok(stamp.error(error)),
        Err(_) => return Ok(stamp.error(Code::Session(Diagnostic::SessionUnavailable))),
    };
    Ok(match run_flow(app, flow).await {
        Ok(outcome) => stamp.result(outcome),
        Err(error) => stamp.error(error),
    })
}
#[tauri::command]
pub async fn send_cloud_model(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ModelResult, ()> {
    allowed(window.label(), "main")?;
    let model = app.state::<NativeCloud>();
    let (send, secret) = match raw(request.body(), 512).and_then(|bytes| model.claim(bytes)) {
        Ok(value) => value,
        Err(error) => return Ok(Stamp::idle(&model).error(Code::Session(error))),
    };
    let stamp = Stamp::operation(send.operation());
    let _cancel = CancelOnDrop(send.operation().cancellation().clone());
    let resources = match Resources::acquire(&app) {
        Ok(value) => value,
        Err(error) => return Ok(stamp.error(error)),
    };
    let flow = Arc::new(Flow {
        work: Work::Analysis(send),
        secret,
        resources,
    });
    Ok(match run_flow(app, flow).await {
        Ok(outcome) => stamp.result(outcome),
        Err(error) => stamp.error(error),
    })
}

async fn freshness(app: AppHandle, flow: Arc<Flow>) -> Result<Freshness, Code> {
    let guard = match begin_operation(&app) {
        Ok(lease) => lease,
        Err(AdmissionError::Busy) => {
            return Err(Code::Native(NativeCode::WorkspaceBusy));
        }
        Err(AdmissionError::Unavailable) => {
            return Err(Code::Session(Diagnostic::SessionUnavailable));
        }
    };
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        flow.ensure_current()?;
        let Work::Analysis(send) = &flow.work else {
            return Err(Code::Session(Diagnostic::InvalidRequest));
        };
        let store = workspace(&app).map_err(|_| Code::Session(Diagnostic::InputUnavailable))?;
        send.freshness(&store).map_err(Code::Session)
    })
    .await
    .map_err(|_| Code::Session(Diagnostic::SessionUnavailable))?
}
async fn run_flow(app: AppHandle, flow: Arc<Flow>) -> Result<Outcome, Code> {
    flow.ensure_current()?;
    let decision = app
        .state::<NativeCloud>()
        .consent
        .install(
            flow.review(),
            flow.operation().cancellation().clone(),
            Arc::clone(&flow),
        )
        .ok_or(Code::Native(NativeCode::ConfirmationUnavailable))?;
    let review_guard = ReviewGuard {
        app: app.clone(),
        run_id: flow.operation().run_id().to_owned(),
    };
    open_review(&app, flow.operation().run_id())
        .map_err(|_| Code::Native(NativeCode::ConfirmationUnavailable))?;
    flow.ensure_current()?;
    if !decision.await.unwrap_or(false) {
        return Ok(Outcome::Cancelled);
    }
    flow.ensure_current()?;
    drop(review_guard);
    let retained = Arc::clone(&flow);
    let callback_app = app.clone();
    let credential = tauri::async_runtime::spawn_blocking(move || {
        retained.ensure_current()?;
        let store = OsStore::open(&callback_app.config().identifier).map_err(store_code)?;
        let credential =
            cloud_store::transport_credential(&store, &retained.secret).map_err(store_code)?;
        retained.ensure_current()?;
        Ok::<_, Code>(credential)
    })
    .await
    .map_err(|_| Code::Session(Diagnostic::SessionUnavailable))??;
    flow.ensure_current()?;
    if matches!(flow.work, Work::Analysis(_)) {
        match freshness(app.clone(), Arc::clone(&flow)).await? {
            Freshness::Current => {}
            Freshness::Stale => return Err(Code::Session(Diagnostic::InputStale)),
            Freshness::Unavailable => return Err(Code::Session(Diagnostic::InputUnavailable)),
        }
    }
    flow.ensure_current()?;
    let client = CloudClient::new();
    match &flow.work {
        Work::Check { request, .. } => {
            let result = client
                .check(request, &credential, flow.operation().cancellation())
                .await;
            flow.ensure_current()?;
            Ok(Outcome::Checked {
                check: result.map_err(Code::Transport)?,
            })
        }
        Work::Analysis(send) => {
            let result = client
                .analyze(send.request(), &credential, flow.operation().cancellation())
                .await;
            flow.ensure_current()?;
            let completion = result.map_err(Code::Transport)?;
            let observed = freshness(app, Arc::clone(&flow))
                .await
                .unwrap_or(Freshness::Unavailable);
            flow.ensure_current()?;
            Ok(Outcome::Completed {
                completion: Box::new(completion),
                freshness: observed,
            })
        }
    }
}

struct ReviewGuard {
    app: AppHandle,
    run_id: String,
}
impl Drop for ReviewGuard {
    fn drop(&mut self) {
        if self.app.state::<NativeCloud>().consent.remove(&self.run_id)
            && let Some(window) = self.app.get_webview_window(REVIEW_WINDOW)
        {
            let _ = window.close();
        }
    }
}
struct PromptCompletion {
    app: AppHandle,
    run_id: String,
    approved: bool,
}
impl Drop for PromptCompletion {
    fn drop(&mut self) {
        if self
            .app
            .state::<NativeCloud>()
            .consent
            .finish_prompt(&self.run_id, self.approved)
            && let Some(window) = self.app.get_webview_window(REVIEW_WINDOW)
        {
            let _ = window.close();
        }
    }
}
fn review_navigation(url: &tauri::Url) -> bool {
    crate::bundled_navigation(url)
        && url.path() == "/cloud-model-confirmation.html"
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
    let review_window = WebviewWindowBuilder::new(
        app,
        REVIEW_WINDOW,
        WebviewUrl::App("cloud-model-confirmation.html".into()),
    )
    .title("Rangoon — Review cloud transfer")
    .inner_size(1000.0, 780.0)
    .min_inner_size(360.0, 480.0)
    .parent(&parent)
    .map_err(|_| ())?
    .on_navigation(review_navigation)
    .build()
    .map_err(|_| ())?;
    let owner = app.clone();
    let id = run_id.to_owned();
    review_window.on_window_event(move |event| {
        let model = owner.state::<NativeCloud>();
        match event {
            WindowEvent::CloseRequested { api, .. } => {
                if model.consent.cancel_run(Some(&id)) {
                    api.prevent_close();
                }
            }
            WindowEvent::Destroyed => {
                model.consent.cancel_run(Some(&id));
            }
            _ => {}
        }
    });
    Ok(())
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
        review: Box<Review>,
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
        schema_version: "rangoon.cloud-review-status.v1",
        outcome: if finished { "finished" } else { "unavailable" },
        authority: "none",
    }
}
#[tauri::command]
pub fn get_cloud_model_review(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ReviewResult, ()> {
    allowed(window.label(), REVIEW_WINDOW)?;
    let review = empty(request.body())
        .then(|| app.state::<NativeCloud>().consent.inspect())
        .flatten();
    Ok(match review {
        Some(review) => ReviewResult::Ready {
            schema_version: "rangoon.cloud-review.v1",
            authority: "none",
            review: Box::new(review),
        },
        None => ReviewResult::Unavailable {
            schema_version: "rangoon.cloud-review.v1",
            authority: "none",
        },
    })
}
#[tauri::command]
pub fn cancel_cloud_model_review(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ReviewStatus, ()> {
    allowed(window.label(), REVIEW_WINDOW)?;
    let decision = raw(request.body(), 512).ok().and_then(Decision::parse);
    Ok(review_status(decision.is_some_and(|d| {
        app.state::<NativeCloud>().consent.cancel_decision(&d)
    })))
}
#[tauri::command]
pub async fn confirm_cloud_model_review(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<ReviewStatus, ()> {
    allowed(window.label(), REVIEW_WINDOW)?;
    let decision = raw(request.body(), 512).ok().and_then(Decision::parse);
    let Some(review) = decision.and_then(|d| app.state::<NativeCloud>().consent.prompt(&d)) else {
        return Ok(review_status(false));
    };
    let dialog_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut completion = PromptCompletion {
            app: dialog_app.clone(),
            run_id: review.run_id.clone(),
            approved: false,
        };
        let label = review.send_label();
        let answer = dialog_app
            .dialog()
            .message(review.question())
            .title("Allow this cloud transfer?")
            .parent(&window)
            .buttons(MessageDialogButtons::OkCancelCustom(
                label.into(),
                "Cancel".into(),
            ))
            .blocking_show_with_result();
        completion.approved =
            matches!(answer, MessageDialogResult::Custom(answer) if answer == label);
    })
    .await;
    Ok(review_status(result.is_ok()))
}
pub fn window_event(window: &tauri::Window, event: &WindowEvent) {
    if window.label() != "main" {
        return;
    }
    if let WindowEvent::CloseRequested { api, .. } = event {
        let model = window.state::<NativeCloud>();
        let prompting = match model.clear() {
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
#[path = "cloud_models_tests.rs"]
mod tests;
