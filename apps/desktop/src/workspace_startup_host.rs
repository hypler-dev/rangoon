//! Source-free local startup messages. Exact matching is request bookkeeping;
//! only native closed-control custody and confirmation can permit an operation.
#[cfg(any(test, feature = "encrypted-workspace"))]
use rangoon_desktop::workspace_keys::WorkspaceId;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, WebviewWindow, ipc::Request};

#[cfg(feature = "encrypted-workspace")]
use crate::workspace_close::{
    failure::WorkerCleanup,
    setup::{Consent, dialog_consent},
};
#[cfg(feature = "encrypted-workspace")]
use tauri::Manager;
#[cfg(feature = "encrypted-workspace")]
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

/// Native-only confirmation purpose. It is not an IPC action or an execution
/// approval; adoption additionally checks its retained capture and real lease.
#[cfg(feature = "encrypted-workspace")]
#[derive(Clone, Copy)]
pub(crate) enum StartupConfirmation {
    CopyExisting,
    VerifyCopy,
    Recovery {
        step: crate::workspace_adoption::RecoveryStep,
        workspace: [u8; 64],
    },
}

#[cfg(feature = "encrypted-workspace")]
pub(crate) fn native_startup_consent(
    app: &AppHandle,
    cleanup: &WorkerCleanup,
    purpose: StartupConfirmation,
) -> Consent {
    if !cleanup.alive() {
        return Consent::Unavailable;
    }
    let Some(parent) = app.get_webview_window("main") else {
        return Consent::Unavailable;
    };
    let (title, question, affirmative) = match purpose {
        StartupConfirmation::CopyExisting => (
            "Copy existing workspace?",
            "Copy the reviewed existing Rangoon workspace to its fixed encrypted workspace and save a new key in this user's operating-system credential store? The original copy is kept. It remains unencrypted. Rangoon must close and reopen before verifying the encrypted copy. This does not authorize external calls or execution.".to_owned(),
            "Copy workspace",
        ),
        StartupConfirmation::VerifyCopy => (
            "Verify encrypted copy?",
            "Read the existing key from this user's operating-system credential store and compare the encrypted copy with the retained original workspace? A matching copy may be selected for later use. The original copy is kept. It remains unencrypted. Verification does not unlock the workbench or authorize external calls or execution.".to_owned(),
            "Verify copy",
        ),
        StartupConfirmation::Recovery { step, workspace } => {
            use crate::workspace_adoption::RecoveryStep;
            let Ok(identity) = std::str::from_utf8(&workspace) else {
                return Consent::Unavailable;
            };
            if WorkspaceId::parse(identity).is_err() {
                return Consent::Unavailable;
            }
            let (title, explanation, affirmative) = match step {
                RecoveryStep::Inspect => ("Inspect interrupted copy?", "Read its existing operating-system key and inspect this fixed destination? This does not restore records or select it for use.", "Inspect copy"),
                RecoveryStep::InitializeEmpty => ("Finish empty encrypted setup?", "Create only the missing empty encrypted database using the exact existing key and inspected unfinished attempt? No key is created or replaced. A separate unlock and restore are required before the retained original can be copied.", "Initialize empty copy"),
                RecoveryStep::Reconcile => ("Reconcile copy metadata?", "Read the existing key again and reconcile the exact inspected CreateEmpty or Restore operation metadata? This may repair its ready marker and operation record. It does not replay that operation or restore records.", "Reconcile metadata"),
                RecoveryStep::Unlock => ("Unlock copy for recovery?", "Read the existing key and unlock this fixed destination for native recovery inspection only? This does not open the workbench.", "Inspect destination"),
                RecoveryStep::Restore => ("Resume copy into empty destination?", "Restore the retained original into this exact proven-empty encrypted destination, check complete parity and record the copy? No existing records may be overwritten. Close and reopen Rangoon afterward to verify it.", "Resume copy"),
                RecoveryStep::RecordCopied => ("Record matching encrypted copy?", "Record the already matching complete encrypted copy without repeating Restore? Close and reopen Rangoon afterward to verify it.", "Record copy"),
                RecoveryStep::CompleteSelection => ("Complete verified-copy selection?", "Read the existing key, recheck complete parity and append only the missing selection record, preserving its earlier verifier identity? This does not unlock the workbench.", "Complete selection"),
            };
            (title, format!("Workspace {identity}\n\n{explanation}\n\nThe original copy is kept. It remains unencrypted. Recovery cannot create a replacement key and does not authorize external calls or execution."), affirmative)
        }
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
        use crate::workspace_adoption::RecoveryStep;
        let purpose = match purpose {
            StartupConfirmation::CopyExisting => "copy_existing",
            StartupConfirmation::VerifyCopy => "verify_copy",
            StartupConfirmation::Recovery { step, .. } => match step {
                RecoveryStep::Inspect => "recovery_inspect",
                RecoveryStep::InitializeEmpty => "recovery_initialize_empty",
                RecoveryStep::Reconcile => "recovery_reconcile",
                RecoveryStep::Unlock => "recovery_unlock",
                RecoveryStep::Restore => "recovery_restore",
                RecoveryStep::RecordCopied => "recovery_record_copied",
                RecoveryStep::CompleteSelection => "recovery_complete_selection",
            },
        };
        crate::native_encrypted_qualification::consent_observed(purpose, consent);
    }
    consent
}

const VIEW_SCHEMA: &str = "rangoon.workspace-startup.v1";
const ACTION_SCHEMA: &str = "rangoon.workspace-startup-action.v1";
const MAX_REQUEST: usize = 512;
#[cfg(any(test, feature = "encrypted-workspace"))]
const MAX_RESPONSE: usize = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    Invalid,
    #[cfg(any(test, feature = "encrypted-workspace"))]
    Unavailable,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    not(feature = "encrypted-workspace"),
    allow(
        dead_code,
        reason = "Frozen DTO vocabulary; builds without the backend can emit only unavailable"
    )
)]
pub(super) enum Stage {
    Fresh,
    LegacyFound,
    ConversionReview,
    Copying,
    RestartRequired,
    VerifyRequired,
    RecoveryRequired,
    Handoff,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Action {
    Setup,
    ReviewConversion,
    Copy,
    Cancel,
    Verify,
    Recover,
    Quit,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    not(feature = "encrypted-workspace"),
    allow(
        dead_code,
        reason = "Frozen DTO vocabulary; builds without the backend cannot assess original data"
    )
)]
pub(super) enum Original {
    Unassessed,
    PresentUnencrypted,
    NotVerified,
    NotApplicable,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(
    dead_code,
    reason = "Frozen local DTO vocabulary; the synchronous host does not fabricate intermediate copy progress"
)]
pub(super) enum CopyPhase {
    CreatingDestination,
    CopyingRecords,
    CheckingCopy,
    RecordingSelection,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(
    not(feature = "encrypted-workspace"),
    allow(
        dead_code,
        reason = "Frozen DTO vocabulary; builds without the backend can report only backend unavailability"
    )
)]
pub(super) enum Diagnostic {
    #[serde(rename = "startup_unavailable")]
    Unavailable,
    #[serde(rename = "backend_unavailable")]
    BackendUnavailable,
    #[serde(rename = "startup_layout_invalid")]
    LayoutInvalid,
    #[serde(rename = "startup_changed")]
    Changed,
    #[serde(rename = "startup_busy")]
    Busy,
    #[serde(rename = "startup_key_missing")]
    #[allow(
        dead_code,
        reason = "Reserved diagnostic; unavailable custody must not imply proven key loss"
    )]
    KeyMissing,
    #[serde(rename = "startup_copy_interrupted")]
    #[allow(
        dead_code,
        reason = "Reserved diagnostic; uncertain recovery must not imply a proven interruption"
    )]
    CopyInterrupted,
    #[serde(rename = "startup_recovery_required")]
    RecoveryRequired,
    #[serde(rename = "startup_control_unavailable")]
    ControlUnavailable,
}

#[cfg(any(test, feature = "encrypted-workspace"))]
pub(super) struct Identity {
    startup_id: String,
    attempt_id: String,
    epoch: String,
}
#[cfg(any(test, feature = "encrypted-workspace"))]
impl std::fmt::Debug for Identity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("StartupIdentity(<redacted>)")
    }
}

fn identity(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn epoch(value: &str) -> bool {
    !value.starts_with('0')
        && value.len() <= 20
        && value
            .parse::<u64>()
            .is_ok_and(|number| number > 0 && number.to_string() == value)
}
#[cfg(any(test, feature = "encrypted-workspace"))]
impl Identity {
    /// The supplied cycle must come from the native close coordinator. This
    /// token never replaces that coordinator's actual current lease checks.
    pub(crate) fn new(attempt_id: &str, cycle_epoch: &str) -> Result<Self, Error> {
        if !identity(attempt_id) || !epoch(cycle_epoch) {
            return Err(Error::Invalid);
        }
        Ok(Self {
            startup_id: WorkspaceId::generate()
                .map_err(|_| Error::Unavailable)?
                .as_hex(),
            attempt_id: attempt_id.to_owned(),
            epoch: cycle_epoch.to_owned(),
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ActionRequest {
    schema_version: String,
    startup_id: String,
    attempt_id: String,
    epoch: String,
    #[cfg_attr(
        not(feature = "encrypted-workspace"),
        allow(
            dead_code,
            reason = "The action is validated but no backend exists to dispatch it"
        )
    )]
    action: Action,
}
impl std::fmt::Debug for ActionRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("StartupAction(<redacted>)")
    }
}
impl ActionRequest {
    pub(super) fn parse(raw: &[u8]) -> Result<Self, Error> {
        if raw.len() > MAX_REQUEST {
            return Err(Error::Invalid);
        }
        let value: Self = serde_json::from_slice(raw).map_err(|_| Error::Invalid)?;
        if value.schema_version != ACTION_SCHEMA
            || !identity(&value.startup_id)
            || !identity(&value.attempt_id)
            || !epoch(&value.epoch)
        {
            return Err(Error::Invalid);
        }
        Ok(value)
    }
    #[cfg(any(test, feature = "encrypted-workspace"))]
    pub(super) fn matches(&self, expected: &Identity) -> bool {
        self.startup_id == expected.startup_id
            && self.attempt_id == expected.attempt_id
            && self.epoch == expected.epoch
    }
    #[cfg(any(test, feature = "encrypted-workspace"))]
    pub(crate) fn action(&self) -> Action {
        self.action
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct View {
    schema_version: &'static str,
    state: Stage,
    startup_id: Option<String>,
    attempt_id: Option<String>,
    epoch: Option<String>,
    actions: Vec<Action>,
    copy_phase: Option<CopyPhase>,
    retained_original: Original,
    diagnostic: Option<Diagnostic>,
}
#[cfg(any(test, feature = "encrypted-workspace"))]
impl Stage {
    fn actions(self) -> &'static [Action] {
        use Action::*;
        match self {
            Self::Fresh => &[Setup, Quit],
            Self::LegacyFound => &[ReviewConversion, Quit],
            Self::ConversionReview => &[Copy, Cancel, Quit],
            Self::Copying | Self::RestartRequired | Self::Unavailable => &[Quit],
            Self::VerifyRequired => &[Verify, Quit],
            Self::RecoveryRequired => &[Recover, Quit],
            Self::Handoff => &[],
        }
    }
    fn permits_original(self, original: Original) -> bool {
        use Original::*;
        match self {
            Self::Fresh => original == NotApplicable,
            Self::LegacyFound => original == Unassessed,
            Self::ConversionReview | Self::Copying | Self::RestartRequired => {
                original == PresentUnencrypted
            }
            Self::VerifyRequired => matches!(original, PresentUnencrypted | NotVerified),
            Self::RecoveryRequired => original == NotVerified,
            Self::Handoff => matches!(original, NotApplicable | PresentUnencrypted | NotVerified),
            Self::Unavailable => matches!(original, NotApplicable | NotVerified),
        }
    }
}
impl View {
    #[cfg(feature = "encrypted-workspace")]
    fn completed_handoff(&self) -> bool {
        self.schema_version == VIEW_SCHEMA
            && self.state == Stage::Handoff
            && self.startup_id.is_none()
            && self.attempt_id.is_none()
            && self.epoch.is_none()
            && self.actions.is_empty()
            && self.copy_phase.is_none()
            && Stage::Handoff.permits_original(self.retained_original)
            && self.diagnostic.is_none()
    }

    /// A completed native routing handoff has no outstanding startup action.
    /// It must not invent a still-current close cycle after normal unlock.
    #[cfg(any(test, feature = "encrypted-workspace"))]
    pub(super) fn handoff(retained_original: Original) -> Result<Self, Error> {
        if !Stage::Handoff.permits_original(retained_original) {
            return Err(Error::Invalid);
        }
        let result = Self {
            schema_version: VIEW_SCHEMA,
            state: Stage::Handoff,
            startup_id: None,
            attempt_id: None,
            epoch: None,
            actions: Vec::new(),
            copy_phase: None,
            retained_original,
            diagnostic: None,
        };
        result.encode()?;
        Ok(result)
    }
    pub(super) fn unavailable(diagnostic: Diagnostic) -> Self {
        Self {
            schema_version: VIEW_SCHEMA,
            state: Stage::Unavailable,
            startup_id: None,
            attempt_id: None,
            epoch: None,
            actions: Vec::new(),
            copy_phase: None,
            retained_original: Original::NotApplicable,
            diagnostic: Some(diagnostic),
        }
    }
    #[cfg(any(test, feature = "encrypted-workspace"))]
    pub(super) fn for_cycle(
        identity: &Identity,
        state: Stage,
        retained_original: Original,
        actions: &[Action],
        copy_phase: Option<CopyPhase>,
        diagnostic: Option<Diagnostic>,
    ) -> Result<Self, Error> {
        let permitted = state.actions();
        let mut previous = None;
        for action in actions {
            let index = permitted
                .iter()
                .position(|candidate| candidate == action)
                .ok_or(Error::Invalid)?;
            if previous.is_some_and(|prior| index <= prior) {
                return Err(Error::Invalid);
            }
            previous = Some(index);
        }
        if actions.len() > 3
            || (state == Stage::Copying) != copy_phase.is_some()
            || !state.permits_original(retained_original)
        {
            return Err(Error::Invalid);
        }
        let result = Self {
            schema_version: VIEW_SCHEMA,
            state,
            startup_id: Some(identity.startup_id.clone()),
            attempt_id: Some(identity.attempt_id.clone()),
            epoch: Some(identity.epoch.clone()),
            actions: actions.to_vec(),
            copy_phase,
            retained_original,
            diagnostic,
        };
        result.encode()?;
        Ok(result)
    }
    #[cfg(any(test, feature = "encrypted-workspace"))]
    pub(super) fn encode(&self) -> Result<Vec<u8>, Error> {
        let bytes = serde_json::to_vec(self).map_err(|_| Error::Invalid)?;
        if bytes.len() > MAX_RESPONSE {
            return Err(Error::Invalid);
        }
        Ok(bytes)
    }
}

#[cfg(test)]
#[path = "workspace_startup_host_tests.rs"]
mod tests;

/// The host retains real closed-control custody with each native preview. It
/// does not own a second content store or any ordinary client-publication path.
#[cfg(feature = "encrypted-workspace")]
mod runtime {
    use super::*;
    use crate::{
        workspace_adoption::{
            self as adoption, Conversion, Recovery, RecoveryOutcome, Verification,
        },
        workspace_close::{StartupFlight, WorkspaceClose},
        workspace_source_access::ControlKind,
        workspace_startup::{BindingProof, Inspection, StartupRoots, State as RootState},
    };
    use rangoon_desktop::workspace_session::Vault;
    use std::{
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    enum Payload {
        Fresh(Inspection),
        Residue(BindingProof),
        Legacy(Inspection),
        Conversion(Box<Conversion>),
        Verification(Box<Verification>),
        Recovery(Box<Recovery>),
    }
    struct Plan {
        payload: Option<Payload>,
        flight: Option<StartupFlight>,
        identity: Identity,
        stage: Stage,
        original: Original,
    }
    impl Plan {
        fn view(&self) -> Result<View, Error> {
            View::for_cycle(
                &self.identity,
                self.stage,
                self.original,
                self.stage.actions(),
                None,
                None,
            )
        }
    }
    impl Drop for Plan {
        fn drop(&mut self) {
            // Drop capture/locked verification Owner before actual drain custody.
            drop(self.payload.take());
            drop(self.flight.take());
        }
    }
    #[derive(Default)]
    struct State {
        busy: Option<String>,
        plan: Option<Plan>,
        outcome: Option<View>,
        terminal: bool,
        quit_requested: bool,
    }
    struct Inner {
        anchor: PathBuf,
        close: WorkspaceClose,
        state: Mutex<State>,
    }
    #[derive(Clone)]
    pub(crate) struct Host(Arc<Inner>);
    /// Move this into a real blocking cleanup worker. Its contained Flight
    /// keeps exit drain pending until capture and Owner cleanup actually return.
    pub(crate) struct Retired(Option<Plan>);
    impl Drop for Retired {
        fn drop(&mut self) {
            drop(self.0.take());
        }
    }
    struct Work {
        host: Host,
        token: String,
        plan: Option<Plan>,
        disarmed: bool,
    }

    impl Work {
        fn current(&self) -> bool {
            self.host
                .0
                .state
                .lock()
                .is_ok_and(|state| !state.terminal && state.busy.as_deref() == Some(&self.token))
        }
        fn finish(mut self, view: View) -> View {
            if let Some(plan) = self.plan.as_mut() {
                drop(plan.payload.take());
            }
            let accepted = self.host.0.state.lock().is_ok_and(|mut state| {
                if state.busy.as_deref() != Some(&self.token) {
                    return false;
                }
                let accepted = !state.terminal;
                if accepted {
                    // A read before completed C0 has no retained plan to
                    // consume. Keep that refusal retryable on this same fixed
                    // Host; never replace managed state to recover timing.
                    state.outcome = if view.state == Stage::Unavailable && view.startup_id.is_none()
                    {
                        None
                    } else {
                        Some(view.clone())
                    };
                }
                state.busy = None;
                accepted
            });
            // The host no longer claims retained content before Flight's
            // finalizer can call native drained-cleanup inspection.
            drop(self.plan.take());
            self.disarmed = true;
            if accepted {
                view
            } else {
                View::unavailable(Diagnostic::ControlUnavailable)
            }
        }
        fn retain(mut self, stage: Stage, original: Original, rotate: bool) -> Result<View, Error> {
            if !self.current() {
                return Err(Error::Unavailable);
            }
            let mut plan = self.plan.take().ok_or(Error::Unavailable)?;
            let flight = plan.flight.as_ref().ok_or(Error::Unavailable)?;
            flight.startup_permit().map_err(|_| Error::Unavailable)?;
            if rotate {
                let (id, epoch) = flight.startup_cycle().map_err(|_| Error::Unavailable)?;
                plan.identity = Identity::new(&id, &epoch)?;
            }
            plan.stage = stage;
            plan.original = original;
            let view = plan.view()?;
            let mut state = self.host.0.state.lock().map_err(|_| Error::Unavailable)?;
            if state.terminal || state.busy.as_deref() != Some(&self.token) || state.plan.is_some()
            {
                drop(state);
                self.plan = Some(plan);
                return Err(Error::Unavailable);
            }
            state.plan = Some(plan);
            state.busy = None;
            self.disarmed = true;
            Ok(view)
        }
        fn unavailable(self, diagnostic: Diagnostic) -> View {
            let view = self
                .plan
                .as_ref()
                .and_then(|plan| {
                    let original = if plan.original == Original::NotApplicable {
                        Original::NotApplicable
                    } else {
                        Original::NotVerified
                    };
                    View::for_cycle(
                        &plan.identity,
                        Stage::Unavailable,
                        original,
                        &[Action::Quit],
                        None,
                        Some(diagnostic),
                    )
                    .ok()
                })
                .unwrap_or_else(|| View::unavailable(diagnostic));
            self.finish(view)
        }
    }
    impl Drop for Work {
        fn drop(&mut self) {
            if self.disarmed {
                return;
            }
            if let Some(plan) = self.plan.as_mut() {
                drop(plan.payload.take());
            }
            match self.host.0.state.lock() {
                Ok(mut state) => {
                    if state.busy.as_deref() == Some(&self.token) {
                        state.busy = None;
                        state.terminal = true;
                    }
                }
                Err(poisoned) => poisoned.into_inner().terminal = true,
            }
            drop(self.plan.take());
        }
    }
    impl Host {
        /// Native application-data anchor only. Construction performs no I/O,
        /// directory creation, credential lookup or content publication.
        pub(crate) fn new(anchor: PathBuf, close: WorkspaceClose) -> Self {
            Self(Arc::new(Inner {
                anchor,
                close,
                state: Mutex::new(State::default()),
            }))
        }
        fn claim(&self) -> Result<Work, Error> {
            let token = WorkspaceId::generate()
                .map_err(|_| Error::Unavailable)?
                .as_hex();
            let mut state = self.0.state.lock().map_err(|_| Error::Unavailable)?;
            if state.terminal || state.busy.is_some() {
                return Err(Error::Unavailable);
            }
            state.busy = Some(token.clone());
            Ok(Work {
                host: self.clone(),
                token,
                plan: state.plan.take(),
                disarmed: false,
            })
        }
        /// Called before terminal close/failure cancellation, not from ordinary
        /// C0/C1 cleanup. Never drop the returned custody under this mutex.
        pub(crate) fn retire(&self) -> Option<Retired> {
            let mut state = match self.0.state.lock() {
                Ok(state) => state,
                Err(error) => error.into_inner(),
            };
            state.terminal = true;
            state.outcome = None;
            state.plan.take().map(|plan| Retired(Some(plan)))
        }
        /// Ordinary Lock may keep only a completed, idle, non-effectful handoff.
        /// Classification and retirement share the claim/finish mutex; an
        /// active Work must see terminal retirement before it can publish.
        pub(crate) fn retire_for_ordinary_close(&self) -> Option<Retired> {
            let (mut state, healthy) = match self.0.state.lock() {
                Ok(state) => (state, true),
                Err(error) => (error.into_inner(), false),
            };
            if healthy
                && !state.terminal
                && state.busy.is_none()
                && state.plan.is_none()
                && state.outcome.as_ref().is_some_and(View::completed_handoff)
            {
                return None;
            }
            state.terminal = true;
            state.outcome = None;
            state.plan.take().map(|plan| Retired(Some(plan)))
        }
        pub(crate) fn cleanup_consistent(&self, drained: bool) -> bool {
            self.0
                .state
                .lock()
                .is_ok_and(|state| !drained || (state.plan.is_none() && state.busy.is_none()))
        }
        pub(crate) fn quit_requested(&self) -> bool {
            self.0.state.lock().is_ok_and(|state| state.quit_requested)
        }
        /// The waiter services a real renderer acknowledgment independently of
        /// this worker. Its boolean alone permits nothing: the actual permit is
        /// rechecked afterward. No host mutex spans the wait or native work.
        pub(crate) fn status(
            &self,
            cleanup: impl FnOnce(bool) -> bool + Send + 'static,
            wait_for_ack: impl FnOnce(&WorkspaceClose) -> bool,
        ) -> View {
            if let Ok(state) = self.0.state.lock() {
                if state.terminal {
                    return View::unavailable(Diagnostic::ControlUnavailable);
                }
                if let Some(view) = &state.outcome {
                    return view.clone();
                }
            }
            let Ok(mut work) = self.claim() else {
                return View::unavailable(Diagnostic::Busy);
            };
            if let Some(plan) = work.plan.as_mut() {
                let checked = (|| {
                    let permit = plan
                        .flight
                        .as_ref()
                        .ok_or(Error::Unavailable)?
                        .startup_permit()
                        .map_err(|_| Error::Unavailable)?;
                    match plan.payload.as_mut().ok_or(Error::Unavailable)? {
                        Payload::Fresh(proof) | Payload::Legacy(proof) => {
                            StartupRoots::new(self.0.anchor.clone())
                                .revalidate(proof)
                                .map_err(|_| Error::Unavailable)?;
                        }
                        Payload::Residue(proof) => {
                            proof.check_before().map_err(|_| Error::Unavailable)?;
                        }
                        Payload::Conversion(value) => value
                            .revalidate_native(&permit)
                            .map_err(|_| Error::Unavailable)?,
                        Payload::Verification(value) => value
                            .revalidate_native(&permit)
                            .map_err(|_| Error::Unavailable)?,
                        Payload::Recovery(value) => value
                            .revalidate_native(&permit)
                            .map_err(|_| Error::Unavailable)?,
                    }
                    Ok::<_, Error>((plan.stage, plan.original))
                })();
                return match checked {
                    Ok((stage, original)) => work
                        .retain(stage, original, false)
                        .unwrap_or_else(|_| View::unavailable(Diagnostic::ControlUnavailable)),
                    Err(_) => work.unavailable(Diagnostic::Changed),
                };
            }
            let started = (|| {
                let (id, epoch) = self
                    .0
                    .close
                    .locked_startup_cycle()
                    .map_err(|_| Error::Unavailable)?;
                let flight = self
                    .0
                    .close
                    .start_control_for(&id, &epoch, ControlKind::Startup, cleanup)
                    .map_err(|_| Error::Unavailable)?;
                let (id, epoch) = flight.startup_cycle().map_err(|_| Error::Unavailable)?;
                let identity = Identity::new(&id, &epoch)?;
                work.plan = Some(Plan {
                    payload: None,
                    flight: Some(flight),
                    identity,
                    stage: Stage::Unavailable,
                    original: Original::NotApplicable,
                });
                if !wait_for_ack(&self.0.close) || !work.current() {
                    return Err(Error::Unavailable);
                }
                work.plan
                    .as_ref()
                    .and_then(|plan| plan.flight.as_ref())
                    .ok_or(Error::Unavailable)?
                    .startup_permit()
                    .map_err(|_| Error::Unavailable)?;
                Ok::<_, Error>(())
            })();
            if started.is_err() {
                return work.unavailable(Diagnostic::ControlUnavailable);
            }
            self.classify(work)
        }
        fn classify(&self, mut work: Work) -> View {
            let roots = StartupRoots::new(self.0.anchor.clone());
            let route = adoption::route(&self.0.anchor);
            match route {
                adoption::Route::Selected(selection) => {
                    work.plan.as_mut().unwrap().original = Original::NotVerified;
                    let matches = work
                        .plan
                        .as_ref()
                        .and_then(|plan| plan.flight.as_ref())
                        .and_then(|flight| flight.startup_permit().ok())
                        .is_some_and(|permit| selection.original_matches(&permit) == Ok(true));
                    let original = if matches {
                        Original::PresentUnencrypted
                    } else {
                        Original::NotVerified
                    };
                    return self.bind_existing(work, Some(selection), original);
                }
                adoption::Route::VerifyRequired => {
                    let prepared = (|| {
                        let permit = work
                            .plan
                            .as_ref()
                            .and_then(|plan| plan.flight.as_ref())
                            .ok_or(Error::Unavailable)?
                            .startup_permit()
                            .map_err(|_| Error::Unavailable)?;
                        Verification::prepare_native(self.0.anchor.clone(), &permit)
                            .map_err(|_| Error::Unavailable)
                    })();
                    return match prepared {
                        Ok(value) => {
                            work.plan.as_mut().unwrap().payload =
                                Some(Payload::Verification(Box::new(value)));
                            work.retain(Stage::VerifyRequired, Original::PresentUnencrypted, true)
                                .unwrap_or_else(|_| {
                                    View::unavailable(Diagnostic::ControlUnavailable)
                                })
                        }
                        Err(_) => work.unavailable(Diagnostic::RecoveryRequired),
                    };
                }
                adoption::Route::RestartRequired => {
                    work.plan.as_mut().unwrap().original = Original::NotVerified;
                    let matches = work
                        .plan
                        .as_ref()
                        .and_then(|plan| plan.flight.as_ref())
                        .and_then(|flight| flight.startup_permit().ok())
                        .is_some_and(|permit| {
                            adoption::original_matches(&self.0.anchor, &permit) == Ok(true)
                        });
                    if !matches {
                        return work.unavailable(Diagnostic::Changed);
                    }
                    let plan = work.plan.as_ref().unwrap();
                    let view = View::for_cycle(
                        &plan.identity,
                        Stage::RestartRequired,
                        Original::PresentUnencrypted,
                        &[Action::Quit],
                        None,
                        None,
                    );
                    return work.finish(
                        view.unwrap_or_else(|_| View::unavailable(Diagnostic::Unavailable)),
                    );
                }
                adoption::Route::RecoveryRequired => {
                    work.plan.as_mut().unwrap().original = Original::NotVerified;
                    return work.unavailable(Diagnostic::RecoveryRequired);
                }
                adoption::Route::Recovery(recovery) => {
                    return self.offer_recovery(work, recovery);
                }
                adoption::Route::Unavailable => return work.unavailable(Diagnostic::Unavailable),
                adoption::Route::NoAttempt => {}
            }
            let inspection = roots.inspect();
            let (payload, stage, original) = match inspection.state() {
                RootState::FreshCandidate => (
                    Payload::Fresh(inspection),
                    Stage::Fresh,
                    Original::NotApplicable,
                ),
                RootState::LegacyCandidate => (
                    Payload::Legacy(inspection),
                    Stage::LegacyFound,
                    Original::Unassessed,
                ),
                RootState::ManagedCandidate => {
                    return self.bind_existing(work, None, Original::NotApplicable);
                }
                _ => match roots.setup_residue_binding() {
                    Ok(proof) => (
                        Payload::Residue(proof),
                        Stage::Fresh,
                        Original::NotApplicable,
                    ),
                    Err(_) => return work.unavailable(Diagnostic::LayoutInvalid),
                },
            };
            work.plan.as_mut().unwrap().payload = Some(payload);
            work.retain(stage, original, true)
                .unwrap_or_else(|_| View::unavailable(Diagnostic::ControlUnavailable))
        }
        fn offer_recovery(&self, mut work: Work, recovery: Recovery) -> View {
            work.plan.as_mut().unwrap().original = Original::NotVerified;
            let prepared = (|| {
                let permit = work
                    .plan
                    .as_ref()
                    .and_then(|plan| plan.flight.as_ref())
                    .ok_or(Error::Unavailable)?
                    .startup_permit()
                    .map_err(|_| Error::Unavailable)?;
                recovery
                    .bind_native(&permit)
                    .map_err(|_| Error::Unavailable)
            })();
            match prepared {
                Ok(recovery) => {
                    work.plan.as_mut().unwrap().payload =
                        Some(Payload::Recovery(Box::new(recovery)));
                    work.retain(Stage::RecoveryRequired, Original::NotVerified, true)
                        .unwrap_or_else(|_| View::unavailable(Diagnostic::ControlUnavailable))
                }
                Err(_) => work.unavailable(Diagnostic::RecoveryRequired),
            }
        }
        fn bind_existing(
            &self,
            mut work: Work,
            selection: Option<adoption::Selection>,
            original: Original,
        ) -> View {
            let checked = (|| {
                if !work.current() {
                    return Err(Error::Unavailable);
                }
                let roots = StartupRoots::new(self.0.anchor.clone());
                if let Some(selected) = &selection {
                    selected.check().map_err(|_| Error::Unavailable)?;
                }
                let proof = roots.existing_binding().map_err(|_| Error::Unavailable)?;
                let flight = work
                    .plan
                    .as_mut()
                    .and_then(|plan| plan.flight.as_mut())
                    .ok_or(Error::Unavailable)?;
                flight.startup_permit().map_err(|_| Error::Unavailable)?;
                flight
                    .bind_locked_owner(proof)
                    .map_err(|_| Error::Unavailable)?;
                if let Some(selected) = &selection {
                    flight
                        .check_bound_identity(selected.destination())
                        .map_err(|_| Error::Unavailable)?;
                    selected.check().map_err(|_| Error::Unavailable)?;
                }
                Ok::<_, Error>(())
            })();
            if checked.is_err() {
                // A bound destination whose selected proof changed cannot be
                // exposed through a later direct ordinary unlock request.
                self.0.close.fail_startup();
                return work.unavailable(Diagnostic::RecoveryRequired);
            }
            drop(selection);
            work.finish(
                View::handoff(original)
                    .unwrap_or_else(|_| View::unavailable(Diagnostic::Unavailable)),
            )
        }
        pub(crate) fn action(
            &self,
            request: ActionRequest,
            vault: &impl Vault,
            mut confirm: impl FnMut(StartupConfirmation) -> Consent,
        ) -> View {
            // A completed Copy may retain only a restart/quit observation, no
            // source preview or Flight. Match that exact outcome before Quit.
            if let Ok(mut state) = self.0.state.lock()
                && !state.terminal
                && state.busy.is_none()
                && let Some(view) = &state.outcome
            {
                let view = view.clone();
                let quit = request.action() == Action::Quit
                    && view.actions.contains(&Action::Quit)
                    && view.startup_id.as_deref() == Some(&request.startup_id)
                    && view.attempt_id.as_deref() == Some(&request.attempt_id)
                    && view.epoch.as_deref() == Some(&request.epoch);
                if quit {
                    state.quit_requested = true;
                    state.terminal = true;
                    state.outcome = None;
                    return View::unavailable(Diagnostic::ControlUnavailable);
                }
                return view;
            }
            let Ok(mut work) = self.claim() else {
                return View::unavailable(Diagnostic::Busy);
            };
            let valid = work.plan.as_ref().is_some_and(|plan| {
                request.matches(&plan.identity) && plan.stage.actions().contains(&request.action())
            });
            if !valid {
                // A malformed/stale action cannot consume a valid retained plan.
                if let Some(plan) = work.plan.as_ref() {
                    let (stage, original) = (plan.stage, plan.original);
                    return work
                        .retain(stage, original, false)
                        .unwrap_or_else(|_| View::unavailable(Diagnostic::ControlUnavailable));
                }
                return work.unavailable(Diagnostic::ControlUnavailable);
            }
            if request.action() == Action::Quit {
                if let Ok(mut state) = self.0.state.lock() {
                    state.quit_requested = true;
                }
                let _ = self.retire();
                return work.unavailable(Diagnostic::ControlUnavailable);
            }
            let result = (|| {
                if !work.current() {
                    return Err(Error::Unavailable);
                }
                let plan = work.plan.as_mut().ok_or(Error::Unavailable)?;
                let flight = plan.flight.as_mut().ok_or(Error::Unavailable)?;
                let permit = flight.startup_permit().map_err(|_| Error::Unavailable)?;
                let payload = plan.payload.take().ok_or(Error::Unavailable)?;
                match (request.action(), payload) {
                    (Action::Setup, Payload::Fresh(inspection)) => {
                        let proof = StartupRoots::new(self.0.anchor.clone())
                            .fresh_binding(inspection)
                            .map_err(|_| Error::Unavailable)?;
                        permit.checkpoint().map_err(|_| Error::Unavailable)?;
                        flight
                            .bind_locked_owner(proof)
                            .map_err(|_| Error::Unavailable)?;
                        Ok(None)
                    }
                    (Action::Setup, Payload::Residue(proof)) => {
                        permit.checkpoint().map_err(|_| Error::Unavailable)?;
                        flight
                            .bind_locked_owner(proof)
                            .map_err(|_| Error::Unavailable)?;
                        Ok(None)
                    }
                    (Action::ReviewConversion, Payload::Legacy(inspection)) => {
                        StartupRoots::new(self.0.anchor.clone())
                            .revalidate(&inspection)
                            .map_err(|_| Error::Unavailable)?;
                        let conversion = Conversion::prepare_native(self.0.anchor.clone(), &permit)
                            .map_err(|_| Error::Unavailable)?;
                        plan.payload = Some(Payload::Conversion(Box::new(conversion)));
                        Ok(Some((
                            Stage::ConversionReview,
                            Original::PresentUnencrypted,
                        )))
                    }
                    (Action::Cancel, Payload::Conversion(value)) => {
                        drop(value);
                        Ok(Some((Stage::LegacyFound, Original::Unassessed)))
                    }
                    (Action::Copy, Payload::Conversion(value)) => {
                        match value
                            .confirm_native(&permit, || confirm(StartupConfirmation::CopyExisting))
                            .map_err(|_| Error::Unavailable)?
                        {
                            Some((value, consent)) => {
                                value
                                    .copy_native(consent, vault, &permit)
                                    .map_err(|_| Error::Unavailable)?;
                                Ok(Some((Stage::RestartRequired, Original::PresentUnencrypted)))
                            }
                            None => Ok(Some((Stage::LegacyFound, Original::Unassessed))),
                        }
                    }
                    (Action::Verify, Payload::Verification(value)) => {
                        match value
                            .confirm_native(&permit, || confirm(StartupConfirmation::VerifyCopy))
                            .map_err(|_| Error::Unavailable)?
                        {
                            Some((value, consent)) => {
                                value
                                    .select_native(consent, vault, &permit)
                                    .map_err(|_| Error::Unavailable)?;
                                Ok(None)
                            }
                            None => {
                                // Explicit Cancel retires the old one-shot
                                // preview, not the valid Copied journal. Rebuild
                                // native custody without reading a key; a later
                                // attempt still needs its own confirmation.
                                let verification =
                                    Verification::prepare_native(self.0.anchor.clone(), &permit)
                                        .map_err(|_| Error::Unavailable)?;
                                plan.payload = Some(Payload::Verification(Box::new(verification)));
                                Ok(Some((Stage::VerifyRequired, Original::PresentUnencrypted)))
                            }
                        }
                    }
                    (Action::Recover, Payload::Recovery(value)) => {
                        match value
                            .recover_native(&permit, vault, |step, destination| {
                                let Ok(workspace) = destination.as_bytes().try_into() else {
                                    return Consent::Unavailable;
                                };
                                confirm(StartupConfirmation::Recovery { step, workspace })
                            })
                            .map_err(|_| Error::Unavailable)?
                        {
                            RecoveryOutcome::Cancelled => {
                                Ok(Some((Stage::RecoveryRequired, Original::NotVerified)))
                            }
                            RecoveryOutcome::Copied => {
                                Ok(Some((Stage::RestartRequired, Original::PresentUnencrypted)))
                            }
                            RecoveryOutcome::Selected => Ok(None),
                        }
                    }
                    _ => Err(Error::Invalid),
                }
            })();
            match result {
                Ok(Some((Stage::RecoveryRequired, _))) => match adoption::route(&self.0.anchor) {
                    adoption::Route::Recovery(recovery) => self.offer_recovery(work, recovery),
                    _ => work.unavailable(Diagnostic::RecoveryRequired),
                },
                Ok(Some((Stage::LegacyFound, original))) => {
                    let proof = StartupRoots::new(self.0.anchor.clone()).inspect();
                    if proof.state() != RootState::LegacyCandidate {
                        return work.unavailable(Diagnostic::Changed);
                    }
                    work.plan.as_mut().unwrap().payload = Some(Payload::Legacy(proof));
                    work.retain(Stage::LegacyFound, original, true)
                        .unwrap_or_else(|_| View::unavailable(Diagnostic::ControlUnavailable))
                }
                Ok(Some((Stage::RestartRequired, original))) => {
                    let plan = work.plan.as_ref().unwrap();
                    let view = View::for_cycle(
                        &plan.identity,
                        Stage::RestartRequired,
                        original,
                        &[Action::Quit],
                        None,
                        None,
                    )
                    .unwrap_or_else(|_| View::unavailable(Diagnostic::Unavailable));
                    work.finish(view)
                }
                Ok(Some((stage, original))) => work
                    .retain(stage, original, true)
                    .unwrap_or_else(|_| View::unavailable(Diagnostic::ControlUnavailable)),
                Ok(None) => {
                    if matches!(request.action(), Action::Verify | Action::Recover) {
                        match adoption::route(&self.0.anchor) {
                            adoption::Route::Selected(selected) => self.bind_existing(
                                work,
                                Some(selected),
                                Original::PresentUnencrypted,
                            ),
                            _ => work.unavailable(Diagnostic::RecoveryRequired),
                        }
                    } else if request.action() == Action::Setup {
                        work.finish(
                            View::handoff(Original::NotApplicable)
                                .unwrap_or_else(|_| View::unavailable(Diagnostic::Unavailable)),
                        )
                    } else {
                        work.unavailable(Diagnostic::RecoveryRequired)
                    }
                }
                Err(_) => work.unavailable(Diagnostic::RecoveryRequired),
            }
        }
    }
    #[cfg(test)]
    mod ordinary_close_state_tests {
        use super::*;

        fn host_with_handoff(original: Original) -> Host {
            let source = crate::SourceAccess::legacy(crate::Gate::new(), Ok(PathBuf::new()));
            let host = Host::new(PathBuf::new(), WorkspaceClose::new(source));
            host.0.state.lock().unwrap().outcome = Some(View::handoff(original).unwrap());
            host
        }

        #[test]
        fn only_exact_completed_handoffs_survive_repeated_ordinary_close() {
            for original in [
                Original::NotApplicable,
                Original::PresentUnencrypted,
                Original::NotVerified,
            ] {
                let host = host_with_handoff(original);
                for _ in 0..3 {
                    assert!(host.retire_for_ordinary_close().is_none());
                    let state = host.0.state.lock().unwrap();
                    assert!(!state.terminal);
                    assert!(state.outcome.as_ref().unwrap().completed_handoff());
                }
            }
        }

        #[test]
        fn partial_or_effectful_handoffs_are_terminally_retired() {
            let mutations: [fn(&mut View); 9] = [
                |view| view.schema_version = "not-the-startup-contract",
                |view| view.state = Stage::Fresh,
                |view| view.startup_id = Some("a".repeat(64)),
                |view| view.attempt_id = Some("b".repeat(64)),
                |view| view.epoch = Some("1".to_owned()),
                |view| view.actions.push(Action::Quit),
                |view| view.copy_phase = Some(CopyPhase::CreatingDestination),
                |view| view.retained_original = Original::Unassessed,
                |view| view.diagnostic = Some(Diagnostic::Unavailable),
            ];
            for mutate in mutations {
                let host = host_with_handoff(Original::NotApplicable);
                mutate(host.0.state.lock().unwrap().outcome.as_mut().unwrap());
                assert!(host.retire_for_ordinary_close().is_none());
                let state = host.0.state.lock().unwrap();
                assert!(state.terminal);
                assert!(state.outcome.is_none());
            }
        }

        #[test]
        fn busy_work_cannot_publish_an_old_handoff_after_ordinary_close() {
            let host = host_with_handoff(Original::NotApplicable);
            let work = host.claim().unwrap();
            assert!(work.current());
            assert!(host.retire_for_ordinary_close().is_none());
            assert!(!work.current());
            assert!(!host.cleanup_consistent(true));
            let result = work.finish(View::handoff(Original::NotApplicable).unwrap());
            assert!(result.state == Stage::Unavailable);
            assert!(host.cleanup_consistent(true));
            let state = host.0.state.lock().unwrap();
            assert!(state.terminal);
            assert!(state.outcome.is_none());
        }

        #[test]
        fn ordinary_close_fences_a_handoff_after_its_last_flight_check() {
            struct UnusedVault;
            impl Vault for UnusedVault {
                type Slot = rangoon_desktop::workspace_keys::OsSlot;

                fn open(
                    &self,
                    _: &WorkspaceId,
                ) -> Result<Self::Slot, rangoon_desktop::workspace_keys::Error> {
                    panic!("this lifecycle regression must not access a vault")
                }
            }

            fn value(close: &WorkspaceClose) -> serde_json::Value {
                serde_json::to_value(close.inspect(|_| true)).unwrap()
            }

            fn acknowledge(close: &WorkspaceClose) -> serde_json::Value {
                let view = value(close);
                let bytes = serde_json::to_vec(&serde_json::json!({
                    "schemaVersion": "rangoon.workspace-purge-ack.v1",
                    "attemptId": view["attemptId"],
                    "epoch": view["epoch"],
                }))
                .unwrap();
                serde_json::to_value(close.acknowledge(&bytes, |_| true)).unwrap()
            }

            let gate = crate::Gate::new();
            let source =
                crate::SourceAccess::managed_deferred(gate.clone(), PathBuf::new(), UnusedVault);
            let close = WorkspaceClose::new(source.clone());
            let host = Host::new(PathBuf::new(), close.clone());
            close.begin(|_| true);
            assert_eq!(acknowledge(&close)["state"], "locked");
            let (id, epoch) = close.locked_startup_cycle().unwrap();
            let flight = close
                .start_control_for(&id, &epoch, ControlKind::Startup, |_| true)
                .unwrap();
            assert_eq!(acknowledge(&close)["state"], "closing");
            let (id, epoch) = flight.startup_cycle().unwrap();
            let mut work = host.claim().unwrap();
            work.plan = Some(Plan {
                payload: None,
                flight: Some(flight),
                identity: Identity::new(&id, &epoch).unwrap(),
                stage: Stage::Fresh,
                original: Original::NotApplicable,
            });

            let (checked, checked_rx) = std::sync::mpsc::channel();
            let (release, release_rx) = std::sync::mpsc::channel();
            let worker = std::thread::spawn(move || {
                // Pause at the actual Work commit boundary after its last live
                // Flight check, without a production test hook or fake lease.
                work.plan
                    .as_ref()
                    .unwrap()
                    .flight
                    .as_ref()
                    .unwrap()
                    .startup_permit()
                    .unwrap();
                assert!(work.current());
                checked.send(()).unwrap();
                release_rx
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .unwrap();
                work.finish(View::handoff(Original::NotApplicable).unwrap())
            });
            checked_rx
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap();

            let retired = host.retire_for_ordinary_close();
            assert!(retired.is_none());
            assert_eq!(
                serde_json::to_value(close.begin(|_| true)).unwrap()["state"],
                "closing"
            );
            assert!(!host.cleanup_consistent(true));
            assert_eq!(value(&close)["state"], "closing");
            assert!(
                gate.begin(rangoon_desktop::workspace_lifecycle::Lane::Read)
                    .is_err()
            );
            assert!(serde_json::to_value(source.request_view()).unwrap()["clientId"].is_null());

            // Complete between source cancellation and detached-custody drop:
            // the old final-check/publication gap must not restore Handoff.
            release.send(()).unwrap();
            assert!(worker.join().unwrap().state == Stage::Unavailable);
            drop(retired);
            assert!(host.cleanup_consistent(true));
            assert_eq!(value(&close)["state"], "closing");
            assert_eq!(acknowledge(&close)["state"], "locked");
            assert!(
                gate.begin(rangoon_desktop::workspace_lifecycle::Lane::Read)
                    .is_err()
            );
            assert!(serde_json::to_value(source.request_view()).unwrap()["clientId"].is_null());
            for _ in 0..2 {
                assert!(host.retire_for_ordinary_close().is_none());
                let state = host.0.state.lock().unwrap();
                assert!(state.terminal);
                assert!(state.busy.is_none());
                assert!(state.outcome.is_none());
                assert!(state.plan.is_none());
            }
        }

        #[test]
        fn ordinary_close_without_a_completed_outcome_remains_terminal() {
            let host = host_with_handoff(Original::NotApplicable);
            host.0.state.lock().unwrap().outcome = None;
            assert!(host.retire_for_ordinary_close().is_none());
            assert!(host.0.state.lock().unwrap().terminal);
        }

        #[test]
        fn terminal_retirement_wins_a_race_with_idle_handoff_retention() {
            let host = host_with_handoff(Original::NotApplicable);
            let barrier = Arc::new(std::sync::Barrier::new(3));
            let ordinary_host = host.clone();
            let ordinary_barrier = Arc::clone(&barrier);
            let ordinary = std::thread::spawn(move || {
                ordinary_barrier.wait();
                assert!(ordinary_host.retire_for_ordinary_close().is_none());
            });
            let terminal_host = host.clone();
            let terminal_barrier = Arc::clone(&barrier);
            let terminal = std::thread::spawn(move || {
                terminal_barrier.wait();
                assert!(terminal_host.retire().is_none());
            });
            barrier.wait();
            ordinary.join().unwrap();
            terminal.join().unwrap();
            for _ in 0..2 {
                assert!(host.retire_for_ordinary_close().is_none());
                let state = host.0.state.lock().unwrap();
                assert!(state.terminal);
                assert!(state.outcome.is_none());
            }
        }

        #[test]
        fn poisoned_mutex_cannot_preserve_an_otherwise_completed_handoff() {
            let host = host_with_handoff(Original::NotApplicable);
            let worker_host = host.clone();
            assert!(
                std::thread::spawn(move || {
                    let _guard = worker_host.0.state.lock().unwrap();
                    panic!("poison the disposable host mutex");
                })
                .join()
                .is_err()
            );
            assert!(host.retire_for_ordinary_close().is_none());
            let state = match host.0.state.lock() {
                Ok(_) => panic!("expected a poisoned host mutex"),
                Err(error) => error.into_inner(),
            };
            assert!(state.terminal);
            assert!(state.outcome.is_none());
        }
    }
}

#[cfg(feature = "encrypted-workspace")]
pub(super) use runtime::Host;

/// Terminal lifecycle hooks call this after closing source admission. Moving
/// retained custody to a real blocking task keeps the event loop responsive;
/// dropping its JoinHandle does not count as completion or release the Flight.
pub(crate) fn retire_for_close(app: &tauri::AppHandle) {
    #[cfg(feature = "encrypted-workspace")]
    if let Some(host) = app.try_state::<Host>()
        && let Some(retired) = host.retire()
    {
        tauri::async_runtime::spawn_blocking(move || drop(retired));
    }
    #[cfg(not(feature = "encrypted-workspace"))]
    let _ = app;
}

/// Cleanup custody only; an empty guard conveys no approval or permission.
/// Keep this named guard alive until ordinary close has fenced admission.
pub(crate) struct OrdinaryCloseRetirement {
    #[cfg(feature = "encrypted-workspace")]
    retained: Option<runtime::Retired>,
}

impl Drop for OrdinaryCloseRetirement {
    fn drop(&mut self) {
        #[cfg(feature = "encrypted-workspace")]
        if let Some(retired) = self.retained.take() {
            tauri::async_runtime::spawn_blocking(move || drop(retired));
        }
    }
}

/// Classify before cancellation can unblock Work and publish a late handoff.
/// The caller then closes admission before dropping this cleanup guard. No
/// Host mutex is held across source closure, callbacks or custody destruction.
pub(crate) fn retire_for_ordinary_close(app: &tauri::AppHandle) -> OrdinaryCloseRetirement {
    #[cfg(not(feature = "encrypted-workspace"))]
    let _ = app;
    OrdinaryCloseRetirement {
        #[cfg(feature = "encrypted-workspace")]
        retained: app
            .try_state::<Host>()
            .and_then(|host| host.retire_for_ordinary_close()),
    }
}

pub(super) fn cleanup_consistent(app: &tauri::AppHandle, drained: bool) -> bool {
    #[cfg(feature = "encrypted-workspace")]
    {
        app.try_state::<Host>()
            .is_none_or(|host| host.cleanup_consistent(drained))
    }
    #[cfg(not(feature = "encrypted-workspace"))]
    {
        let _ = (app, drained);
        true
    }
}

#[cfg(feature = "encrypted-workspace")]
fn wait_for_startup_ack(close: &crate::WorkspaceClose, cleanup: &WorkerCleanup) -> bool {
    let started = std::time::Instant::now();
    while cleanup.alive() && started.elapsed() < std::time::Duration::from_secs(30) {
        match close.startup_acknowledged() {
            Ok(true) => return true,
            Ok(false) => std::thread::sleep(std::time::Duration::from_millis(25)),
            Err(_) => return false,
        }
    }
    false
}

fn main_window(label: &str) -> Result<(), ()> {
    (label == "main").then_some(()).ok_or(())
}

fn empty_object(body: &tauri::ipc::InvokeBody) -> Result<(), ()> {
    matches!(body, tauri::ipc::InvokeBody::Json(value)
        if value.as_object().is_some_and(|object| object.is_empty()))
    .then_some(())
    .ok_or(())
}

fn request_quit(
    label: &str,
    body: &tauri::ipc::InvokeBody,
    request_exit: impl FnOnce(),
) -> Result<(), ()> {
    main_window(label)?;
    empty_object(body)?;
    request_exit();
    Ok(())
}

/// Failure cards may have no current plan identity. Request the same exit path
/// as native window close; this does not prove drain or force process exit.
#[tauri::command]
pub(crate) fn workspace_startup_quit(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<(), ()> {
    request_quit(window.label(), request.body(), || {
        app.exit(crate::workspace_close::failure::CLEAN_EXIT_CODE);
    })
}

/// Source-free commands have their own bounded native lifecycle path. They do
/// not accept a content client, workspace path, credential or replacement data.
#[tauri::command]
pub(crate) async fn workspace_startup_status(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<View, ()> {
    main_window(window.label())?;
    empty_object(request.body())?;
    #[cfg(feature = "encrypted-workspace")]
    {
        let Some(host) = app.try_state::<Host>().map(|state| state.inner().clone()) else {
            return Ok(View::unavailable(Diagnostic::BackendUnavailable));
        };
        let Some(cleanup) = app
            .try_state::<WorkerCleanup>()
            .map(|state| state.inner().clone())
            .filter(WorkerCleanup::alive)
        else {
            return Ok(View::unavailable(Diagnostic::ControlUnavailable));
        };
        let finalizer = cleanup.clone();
        let view = tauri::async_runtime::spawn_blocking(move || {
            host.status(
                move |drained| finalizer.cleanup(drained),
                |close| wait_for_startup_ack(close, &cleanup),
            )
        })
        .await
        .map_err(|_| ())?;
        #[cfg(all(test, feature = "native-encrypted-qualification", target_os = "macos"))]
        crate::native_encrypted_qualification::startup_observed(&app, &view);
        Ok(view)
    }
    #[cfg(not(feature = "encrypted-workspace"))]
    {
        let _ = app;
        Ok(View::unavailable(Diagnostic::BackendUnavailable))
    }
}

#[tauri::command]
pub(crate) async fn workspace_startup_action(
    app: AppHandle,
    window: WebviewWindow,
    request: Request<'_>,
) -> Result<View, ()> {
    main_window(window.label())?;
    let tauri::ipc::InvokeBody::Raw(raw) = request.body() else {
        return Err(());
    };
    let action = ActionRequest::parse(raw).map_err(|_| ())?;
    #[cfg(feature = "encrypted-workspace")]
    {
        let Some(host) = app.try_state::<Host>().map(|state| state.inner().clone()) else {
            return Ok(View::unavailable(Diagnostic::BackendUnavailable));
        };
        let Some(cleanup) = app
            .try_state::<WorkerCleanup>()
            .map(|state| state.inner().clone())
            .filter(WorkerCleanup::alive)
        else {
            return Ok(View::unavailable(Diagnostic::ControlUnavailable));
        };
        tauri::async_runtime::spawn_blocking(move || {
            let vault = crate::workspace_native_bootstrap::native_startup_vault(&app);
            let view = host.action(action, &vault, |purpose| {
                native_startup_consent(&app, &cleanup, purpose)
            });
            #[cfg(all(test, feature = "native-encrypted-qualification", target_os = "macos"))]
            crate::native_encrypted_qualification::startup_observed(&app, &view);
            if host.quit_requested() {
                // This requests the existing native exit coordinator. It does
                // not bypass callback drain, cleanup or the exit-code proof.
                app.exit(crate::workspace_close::failure::CLEAN_EXIT_CODE);
            }
            view
        })
        .await
        .map_err(|_| ())
    }
    #[cfg(not(feature = "encrypted-workspace"))]
    {
        let _ = (app, action);
        Ok(View::unavailable(Diagnostic::BackendUnavailable))
    }
}
