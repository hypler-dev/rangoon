//! Private memory-only close protocol. Production Legacy storage cannot enter it.
//! Renderer acknowledgment is cooperative cleanup evidence, never authorization.
use crate::{
    SourceAccess, cloud_custody, cloud_models, models, workspace_source_access::CloseTicket,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{
    AppHandle, Manager, WebviewWindow,
    ipc::{InvokeBody, Request},
};

pub(super) struct Cleanup {
    pub healthy: bool,
    pub keep_parent: bool,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Unavailable,
    Open,
    Closing,
    Locked,
    RecoveryRequired,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct View {
    schema_version: &'static str,
    storage: &'static str,
    state: Phase,
    attempt_id: Option<String>,
    epoch: Option<String>,
    diagnostic: Option<&'static str>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Acknowledgment {
    schema_version: String,
    attempt_id: String,
    epoch: String,
}
impl Acknowledgment {
    fn parse(raw: &[u8]) -> Option<Self> {
        if raw.len() > 256 {
            return None;
        }
        let value: Self = serde_json::from_slice(raw).ok()?;
        (value.schema_version == "rangoon.workspace-purge-ack.v1"
            && value.attempt_id.len() == 64
            && value
                .attempt_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && !value.epoch.starts_with('0')
            && value.epoch.len() <= 20
            && value
                .epoch
                .parse::<u64>()
                .is_ok_and(|n| n > 0 && n.to_string() == value.epoch))
        .then_some(value)
    }
}

#[derive(Default)]
struct State {
    epoch: u64,
    failed: bool,
    attempt: Option<Arc<Attempt>>,
    control: Option<control::Retained>,
    #[cfg(test)]
    fail_control_after_swap: bool,
}
struct Attempt {
    ticket: CloseTicket,
    id: Option<String>,
    epoch: Option<String>,
    progress: Mutex<Progress>,
}
struct Progress {
    initial_done: bool,
    finishing: bool,
    native_done: bool,
    healthy: bool,
    acknowledged: bool,
}

#[derive(Clone)]
pub(super) struct WorkspaceClose {
    source: SourceAccess,
    state: Arc<Mutex<State>>,
}
impl WorkspaceClose {
    pub(super) fn new(source: SourceAccess) -> Self {
        Self {
            source,
            state: Arc::new(Mutex::new(State::default())),
        }
    }
    fn snapshot(&self, diagnostic: Option<&'static str>) -> View {
        let encrypted = self.source.encrypted_close_available();
        let mut view = View {
            schema_version: "rangoon.workspace-close-state.v1",
            storage: if encrypted {
                "managed_encrypted"
            } else {
                "legacy_plaintext"
            },
            state: if encrypted {
                Phase::Open
            } else {
                Phase::Unavailable
            },
            attempt_id: None,
            epoch: None,
            diagnostic: diagnostic.or((!encrypted).then_some("encrypted_close_unavailable")),
        };
        if !encrypted {
            return view;
        }
        let (state, poisoned) = match self.state.lock() {
            Ok(state) => (state, false),
            Err(error) => (error.into_inner(), true),
        };
        if state.failed || poisoned {
            view.state = Phase::RecoveryRequired;
            view.diagnostic = Some("workspace_close_incomplete");
        }
        if let Some(attempt) = &state.attempt {
            view.attempt_id = attempt.id.clone();
            view.epoch = attempt.epoch.clone();
            let progress = match attempt.progress.lock() {
                Ok(progress) => progress,
                Err(_) => {
                    view.state = Phase::RecoveryRequired;
                    view.diagnostic = Some("workspace_close_incomplete");
                    return view;
                }
            };
            if !state.failed && !poisoned {
                view.state = if !progress.healthy {
                    view.diagnostic = Some("workspace_close_incomplete");
                    Phase::RecoveryRequired
                } else if state.control.is_some() {
                    Phase::Closing
                } else if progress.native_done && progress.acknowledged {
                    Phase::Locked
                } else {
                    Phase::Closing
                };
            }
        }
        view
    }
    pub(super) fn rejected(&self) -> View {
        self.snapshot(Some("invalid_workspace_close_request"))
    }
    pub(super) fn begin(&self, cleanup: impl FnOnce(bool) -> bool) -> View {
        self.begin_with_entropy(cleanup, |bytes| getrandom::fill(bytes).map_err(|_| ()))
    }
    fn poisoned_cleanup(&self, cleanup: impl FnOnce(bool) -> bool) -> View {
        self.purge_poisoned_control();
        // Recover no operational capability from poison. Only admission closure,
        // cancellation, retained-data deletion and proven-drained key drop remain.
        let ticket = self.source.request_close().ok();
        let drained = ticket
            .as_ref()
            .is_some_and(|t| t.is_quiescent() == Ok(true));
        let _ = self.source.purge_retained();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cleanup(drained)));
        if drained && let Some(ticket) = ticket {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ticket.release_key()));
        }
        self.snapshot(None)
    }
    fn begin_with_entropy(
        &self,
        cleanup: impl FnOnce(bool) -> bool,
        entropy: impl FnOnce(&mut [u8]) -> Result<(), ()>,
    ) -> View {
        // This is an immutable host-mode check, not a backend or vault lookup.
        if !self.source.encrypted_close_available() {
            return self.snapshot(None);
        }
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(error) => {
                drop(error.into_inner());
                return self.poisoned_cleanup(cleanup);
            }
        };
        if state.failed {
            drop(state);
            return self.snapshot(None);
        }
        if state.control.is_some() {
            drop(state);
            return self.cancel_any_control(cleanup);
        }
        if state.attempt.is_some() {
            drop(state);
            return self.inspect(cleanup);
        }
        let ticket = match self.source.request_close() {
            Ok(ticket) => ticket,
            Err(()) => {
                state.failed = true;
                drop(state);
                let _ = self.source.purge_retained();
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cleanup(false)));
                return self.snapshot(None);
            }
        };
        let (id, epoch) = next_identity(&mut state, entropy);
        let healthy = id.is_some() && epoch.is_some();
        let attempt = Arc::new(Attempt {
            ticket,
            id,
            epoch,
            progress: Mutex::new(Progress {
                initial_done: false,
                finishing: false,
                native_done: false,
                healthy,
                acknowledged: false,
            }),
        });
        state.attempt = Some(Arc::clone(&attempt));
        drop(state);
        let purged = attempt.ticket.purge();
        let cleaned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cleanup(false)))
            .unwrap_or(false);
        if let Ok(mut progress) = attempt.progress.lock() {
            progress.healthy &= purged && cleaned;
            progress.initial_done = true;
        }
        // Final native cleanup is performed on a subsequent explicit poll/ack.
        self.snapshot(None)
    }
    pub(super) fn inspect(&self, cleanup: impl FnOnce(bool) -> bool) -> View {
        if !self.source.encrypted_close_available() {
            return self.snapshot(None);
        }
        let attempt = match self.state.lock() {
            Ok(state) => state.attempt.clone(),
            Err(error) => {
                drop(error.into_inner());
                return self.poisoned_cleanup(cleanup);
            }
        };
        let Some(attempt) = attempt else {
            return self.snapshot(None);
        };
        let mut progress = match attempt.progress.lock() {
            Ok(progress) => progress,
            Err(error) => {
                drop(error.into_inner());
                return self.poisoned_cleanup(cleanup);
            }
        };
        if !progress.initial_done || progress.native_done || progress.finishing {
            drop(progress);
            return self.snapshot(None);
        }
        match attempt.ticket.is_quiescent() {
            Ok(false) => {
                drop(progress);
                return self.snapshot(None);
            }
            Err(()) => {
                progress.healthy = false;
                drop(progress);
                return self.snapshot(None);
            }
            Ok(true) => {}
        }
        progress.finishing = true;
        drop(progress);
        let purged = attempt.ticket.purge();
        let cleaned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cleanup(true)))
            .unwrap_or(false);
        // Even unhealthy cleanup must attempt key release after proven drain.
        let released = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            attempt.ticket.release_key()
        }))
        .ok()
        .and_then(Result::ok)
        .unwrap_or(false);
        if let Ok(mut progress) = attempt.progress.lock() {
            progress.healthy &= purged && cleaned && released;
            progress.native_done = true;
            progress.finishing = false;
        }
        self.snapshot(None)
    }
    pub(super) fn acknowledge(&self, raw: &[u8], cleanup: impl FnOnce(bool) -> bool) -> View {
        if !self.source.encrypted_close_available() {
            return self.snapshot(None);
        }
        let Some(ack) = Acknowledgment::parse(raw) else {
            return self.rejected();
        };
        let attempt = self.state.lock().ok().and_then(|s| s.attempt.clone());
        let Some(attempt) = attempt else {
            return self.rejected();
        };
        if attempt.id.as_deref() != Some(ack.attempt_id.as_str())
            || attempt.epoch.as_deref() != Some(ack.epoch.as_str())
        {
            return self.rejected();
        }
        let accepted = match attempt.progress.lock() {
            Ok(mut progress) if !progress.acknowledged => {
                progress.acknowledged = true;
                true
            }
            _ => false,
        };
        if !accepted {
            return self.rejected();
        }
        self.inspect(cleanup)
    }
}

// The private control supervisor is not registered as a command yet.
#[allow(dead_code)]
#[path = "workspace_close_control.rs"]
mod control;

fn next_identity(
    state: &mut State,
    entropy: impl FnOnce(&mut [u8]) -> Result<(), ()>,
) -> (Option<String>, Option<String>) {
    let mut bytes = [0; 32];
    let id = entropy(&mut bytes)
        .ok()
        .map(|()| bytes.iter().map(|b| format!("{b:02x}")).collect());
    let epoch = state.epoch.checked_add(1);
    if let Some(epoch) = epoch {
        state.epoch = epoch;
    }
    (id, epoch.map(|n| n.to_string()))
}
fn native_cleanup(app: &AppHandle, drained: bool) -> bool {
    let local = app
        .state::<models::NativeModel>()
        .purge_for_workspace_close();
    let cloud = app
        .state::<cloud_models::NativeCloud>()
        .purge_for_workspace_close();
    let credential = app
        .state::<cloud_custody::Custody>()
        .purge_for_workspace_close(drained);
    let mut healthy = local.healthy && cloud.healthy && credential.healthy;
    for (label, keep_parent) in [
        (crate::model_consent::REVIEW_WINDOW, local.keep_parent),
        (crate::cloud_model_consent::REVIEW_WINDOW, cloud.keep_parent),
        (cloud_custody::EDIT_WINDOW, credential.keep_parent),
    ] {
        if !keep_parent {
            if let Some(window) = app.get_webview_window(label) {
                healthy &= window.destroy().is_ok();
            }
        } else if drained {
            healthy = false;
        }
    }
    healthy
}
fn empty(body: &InvokeBody) -> bool {
    matches!(body, InvokeBody::Json(v) if v.as_object().is_some_and(|o| o.is_empty()))
}
fn allowed(label: &str) -> Result<(), ()> {
    if label == "main" { Ok(()) } else { Err(()) }
}
#[tauri::command]
pub(super) fn get_workspace_close_state(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<View, ()> {
    allowed(window.label())?;
    let state = app.state::<WorkspaceClose>();
    Ok(if empty(request.body()) {
        state.inspect(|drained| native_cleanup(&app, drained))
    } else {
        state.rejected()
    })
}
#[tauri::command]
pub(super) fn begin_workspace_close(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<View, ()> {
    allowed(window.label())?;
    let state = app.state::<WorkspaceClose>();
    Ok(if empty(request.body()) {
        state.begin(|drained| native_cleanup(&app, drained))
    } else {
        state.rejected()
    })
}
#[tauri::command]
pub(super) fn acknowledge_workspace_purge(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<View, ()> {
    allowed(window.label())?;
    let state = app.state::<WorkspaceClose>();
    Ok(match request.body() {
        InvokeBody::Raw(raw) => state.acknowledge(raw, |drained| native_cleanup(&app, drained)),
        _ => state.rejected(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rangoon_desktop::workspace_lifecycle::{Gate, Lane};
    use serde_json::{Value, json};

    fn state(view: View) -> Value {
        serde_json::to_value(view).unwrap()
    }
    fn fixture() -> (WorkspaceClose, Gate) {
        let gate = Gate::new();
        (
            WorkspaceClose::new(SourceAccess::close_protocol_fixture(gate.clone())),
            gate,
        )
    }
    #[test]
    fn acknowledgment_parser_refuses_noncanonical_and_oversized_fields() {
        let good = json!({"schemaVersion":"rangoon.workspace-purge-ack.v1","attemptId":"ab".repeat(32),"epoch":"1"});
        assert!(Acknowledgment::parse(&serde_json::to_vec(&good).unwrap()).is_some());
        for epoch in [
            "",
            "0",
            "01",
            "+1",
            "-1",
            "1.0",
            " 1",
            "18446744073709551616",
            "١",
        ] {
            let mut bad = good.clone();
            bad["epoch"] = json!(epoch);
            assert!(Acknowledgment::parse(&serde_json::to_vec(&bad).unwrap()).is_none());
        }
        for id in [
            "A".repeat(64),
            "a".repeat(63),
            "a".repeat(65),
            "g".repeat(64),
        ] {
            let mut bad = good.clone();
            bad["attemptId"] = json!(id);
            assert!(Acknowledgment::parse(&serde_json::to_vec(&bad).unwrap()).is_none());
        }
        let mut extra = good.clone();
        extra["approved"] = json!(true);
        assert!(Acknowledgment::parse(&serde_json::to_vec(&extra).unwrap()).is_none());
        assert!(Acknowledgment::parse(&[b' '; 257]).is_none());
        assert!(Acknowledgment::parse(br#"{"schemaVersion":"rangoon.workspace-purge-ack.v1","attemptId":"abababababababababababababababababababababababababababababababab","epoch":"1","epoch":"2"}"#).is_none());
    }
    #[test]
    fn entropy_or_epoch_failure_stays_closed_and_never_reports_locked() {
        for overflow in [false, true] {
            let (close, gate) = fixture();
            if overflow {
                close.state.lock().unwrap().epoch = u64::MAX;
            }
            let first = state(close.begin_with_entropy(
                |_| true,
                |bytes| {
                    if overflow {
                        bytes.fill(1);
                        Ok(())
                    } else {
                        Err(())
                    }
                },
            ));
            assert_eq!(first["state"], "recovery_required");
            assert!(gate.begin(Lane::Read).is_err());
            assert_eq!(state(close.inspect(|_| true))["state"], "recovery_required");
        }
    }
    #[test]
    fn cleanup_failure_or_panic_cannot_be_repaired_by_a_renderer_ack() {
        for panic in [false, true] {
            let (close, gate) = fixture();
            let first = state(close.begin(|_| {
                if panic {
                    panic!("synthetic cleanup panic");
                }
                false
            }));
            let ack = serde_json::to_vec(&json!({"schemaVersion":"rangoon.workspace-purge-ack.v1","attemptId":first["attemptId"],"epoch":first["epoch"]})).unwrap();
            assert_eq!(
                state(close.acknowledge(&ack, |_| true))["state"],
                "recovery_required"
            );
            assert!(gate.begin(Lane::Read).is_err());
        }
    }
    #[test]
    fn poisoned_coordination_cancels_only_and_never_reopens() {
        let (close, gate) = fixture();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = close.state.lock().unwrap();
            panic!("synthetic close coordinator poison");
        }));
        let mut count = 0;
        assert_eq!(
            state(close.begin(|_| {
                count += 1;
                true
            }))["state"],
            "recovery_required"
        );
        assert!(gate.begin(Lane::Read).is_err());
        assert_eq!(
            state(close.inspect(|_| {
                count += 1;
                true
            }))["state"],
            "recovery_required"
        );
        assert_eq!(count, 2);
        assert!(close.state.lock().is_err());
    }
    #[test]
    fn close_commands_are_strictly_main_window_and_input_scoped() {
        assert!(allowed("main").is_ok());
        for label in [
            "model-confirmation",
            "cloud-model-confirmation",
            "cloud-credential-entry",
            "other",
        ] {
            assert!(allowed(label).is_err());
        }
        assert!(empty(&InvokeBody::Json(json!({}))));
        for body in [
            InvokeBody::Json(json!([])),
            InvokeBody::Json(json!({"extra":true})),
            InvokeBody::Raw(b"{}".to_vec()),
        ] {
            assert!(!empty(&body));
        }
        let main: Value =
            serde_json::from_str(include_str!("../capabilities/source-analysis.json")).unwrap();
        assert_eq!(main["windows"], json!(["main"]));
        assert_eq!(main["local"], true);
        let manifest = include_str!("../build.rs");
        for command in [
            "get_workspace_close_state",
            "begin_workspace_close",
            "acknowledge_workspace_purge",
        ] {
            assert!(manifest.contains(&format!("\"{command}\"")));
            let allow = format!("allow-{}", command.replace('_', "-"));
            assert!(
                main["permissions"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(allow))
            );
            for private in [
                include_str!("../capabilities/model-confirmation.json"),
                include_str!("../capabilities/cloud-model-confirmation.json"),
                include_str!("../capabilities/cloud-credential-entry.json"),
            ] {
                assert!(!private.contains(&allow));
            }
        }
    }
}
