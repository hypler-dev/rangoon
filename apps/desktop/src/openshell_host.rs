//! Native custody for one bounded OpenShell observation.
//!
//! This module owns consent and workspace-generation custody only. It does
//! not obtain credentials, connect to a gateway, persist an observation, or
//! expose a Tauri command. Production observation start is deliberately
//! unavailable until the separately accepted native-only credential packet.

// D2b-2a intentionally leaves native UI/credential entry unmounted. Keep its
// closed custody contract compiled and tested without manufacturing a renderer
// call merely to satisfy dead-code linting.
#![allow(dead_code)]

#[path = "openshell_credential.rs"]
mod credential;
use credential::{CredentialInspection, CredentialSelection};
#[path = "openshell_native_input.rs"]
mod native_input;
#[path = "openshell_session_view.rs"]
mod session_view;

use crate::{AdmissionError, workspace_requests::WorkspaceRequest};
use rangoon_desktop::workspace_lifecycle::{Gate, GenerationWitness, Lane, Lease};
#[cfg(test)]
use rangoon_openshell_client::BearerLease;
use rangoon_openshell_client::{
    Authenticity, CertificatePin, ClientConfig, ClientError, ConnectionCheck, LogWindow,
    PolicyRead, WatchControl, WatchWindow,
};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

const PRODUCT_VERSION: &str = "v0.1.4";
const MAX_CERTIFICATE_BYTES: usize = 256 * 1024;
const MAX_ORIGIN_BYTES: usize = 512;

/// Closed observation method choice. This is never an arbitrary RPC name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReadKind {
    ProtectedCheck,
    PolicyStatus,
    RecentLogs,
    RecentWatch,
}

/// D2b-2a has no durable observation path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Retention {
    SessionOnly,
}

/// Fixed minimized data classes. The renderer cannot add fields to a consent
/// review by sending descriptive text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DataClass {
    ProtectedRead,
    PolicyRevision,
    RedactedLog,
    BoundedWatch,
}

/// Exact target data retained until one consented attempt completes or stops.
/// Construction validates the client configuration before workspace admission.
pub(crate) struct ProposedTarget {
    config: ClientConfig,
    origin: String,
    selected_peer: SocketAddr,
    installation: String,
    workspace: String,
    sandbox: String,
    root_sha256: String,
    peer_pin_sha256: String,
}

impl ProposedTarget {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        origin: String,
        selected_peer: SocketAddr,
        installation: String,
        workspace: String,
        sandbox: String,
        trusted_root_certificate_der: Vec<u8>,
        peer_certificate_der: Vec<u8>,
    ) -> Result<Self, ClientError> {
        if origin.len() > MAX_ORIGIN_BYTES
            || trusted_root_certificate_der.is_empty()
            || trusted_root_certificate_der.len() > MAX_CERTIFICATE_BYTES
            || peer_certificate_der.is_empty()
            || peer_certificate_der.len() > MAX_CERTIFICATE_BYTES
        {
            return Err(ClientError::InvalidConfiguration);
        }
        let root_sha256 = sha256_hex(&trusted_root_certificate_der);
        let peer_pin_sha256 = sha256_hex(&peer_certificate_der);
        let peer_pin = CertificatePin::from_certificate_der(&peer_certificate_der)?;
        let config = ClientConfig::new(
            origin.clone(),
            selected_peer,
            installation.clone(),
            workspace.clone(),
            sandbox.clone(),
            PRODUCT_VERSION.to_owned(),
            trusted_root_certificate_der,
            peer_pin,
        )?;
        Ok(Self {
            config,
            origin,
            selected_peer,
            installation,
            workspace,
            sandbox,
            root_sha256,
            peer_pin_sha256,
        })
    }
}

/// Native review data binds one target, one bounded kind and one retention
/// mode. It carries only SHA-256 identities for selected trust material, never
/// a credential, raw DER, pin bytes, or provider result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Review {
    pub(crate) id: String,
    pub(crate) origin: String,
    pub(crate) selected_peer: SocketAddr,
    pub(crate) installation: String,
    pub(crate) workspace: String,
    pub(crate) sandbox: String,
    pub(crate) product_version: &'static str,
    pub(crate) root_sha256: String,
    pub(crate) peer_pin_sha256: String,
    pub(crate) kind: ReadKind,
    pub(crate) retention: Retention,
    pub(crate) data_classes: Vec<DataClass>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostError {
    Busy,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostStatus {
    Unavailable,
    AwaitingConsent,
    ReadyForNativeCredential,
    Running,
    Closing,
    Cancelled,
    Completed,
    Error(ClientError),
}

/// Only the client's bounded, minimized native-read projections may occupy
/// this session slot. It is not serialized to IPC or persisted by D2b-2a.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Observation {
    ProtectedCheck(ConnectionCheck),
    PolicyStatus(PolicyRead),
    RecentLogs(LogWindow),
    RecentWatch(WatchWindow),
}

impl Observation {
    fn matches_review(&self, review: &Review) -> bool {
        let (kind, authenticity, installation, workspace, sandbox) = match self {
            Self::ProtectedCheck(value) => (
                ReadKind::ProtectedCheck,
                value.authenticity,
                value.installation.as_str(),
                value.workspace.as_str(),
                value.sandbox.as_str(),
            ),
            Self::PolicyStatus(value) => (
                ReadKind::PolicyStatus,
                value.authenticity,
                value.installation.as_str(),
                value.workspace.as_str(),
                value.sandbox.as_str(),
            ),
            Self::RecentLogs(value) => (
                ReadKind::RecentLogs,
                value.authenticity,
                value.installation.as_str(),
                value.workspace.as_str(),
                value.sandbox.as_str(),
            ),
            Self::RecentWatch(value) => (
                ReadKind::RecentWatch,
                value.authenticity,
                value.installation.as_str(),
                value.workspace.as_str(),
                value.sandbox.as_str(),
            ),
        };
        kind == review.kind
            && authenticity == Authenticity::Unqualified
            && installation == review.installation
            && workspace == review.workspace
            && sandbox == review.sandbox
    }
}

/// A successful bounded read must be paired with a protected connection
/// check from the same consented target. This type is test-only until the
/// host-to-real-client composition has independent transport proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReadOutcome {
    pub(crate) protected_check: ConnectionCheck,
    pub(crate) observation: Observation,
}

impl ReadOutcome {
    fn matches_review(&self, review: &Review) -> bool {
        let check = &self.protected_check;
        check.authenticity == Authenticity::Unqualified
            && check.installation == review.installation
            && check.workspace == review.workspace
            && check.sandbox == review.sandbox
            && check.reported_product_version == review.product_version
            && matches!(
                check.health,
                rangoon_openshell_client::HealthStatus::Healthy
                    | rangoon_openshell_client::HealthStatus::Degraded
            )
            && self.observation.matches_review(review)
            && !matches!(&self.observation, Observation::ProtectedCheck(value) if value != check)
    }
}

#[derive(Clone, Copy)]
enum CallbackStart {
    Confirmed,
    NativeReview,
}

enum Pending {
    Awaiting {
        review: Review,
        target: ProposedTarget,
        admission: Lease,
        selection: CredentialSelection,
    },
    Confirmed {
        review: Review,
        target: ProposedTarget,
        admission: Lease,
        selection: CredentialSelection,
    },
}

impl Pending {
    fn review(&self) -> &Review {
        match self {
            Self::Awaiting { review, .. } | Self::Confirmed { review, .. } => review,
        }
    }

    fn selection(&self) -> &CredentialSelection {
        match self {
            Self::Awaiting { selection, .. } | Self::Confirmed { selection, .. } => selection,
        }
    }

    fn is_current(&self) -> bool {
        match self {
            Self::Awaiting { admission, .. } | Self::Confirmed { admission, .. } => {
                admission.is_current().unwrap_or(false)
            }
        }
    }

    /// Readback reports the retained phase without consuming consent or custody.
    /// Expired selections remain owned until Stop or the existing close path.
    fn status(&self) -> HostStatus {
        if !self.is_current() {
            return HostStatus::Unavailable;
        }
        if let Err(error) = self.selection().check() {
            return HostStatus::Error(error);
        }
        match self {
            Self::Awaiting { .. } => HostStatus::AwaitingConsent,
            Self::Confirmed { .. } => HostStatus::ReadyForNativeCredential,
        }
    }
}

struct Active {
    review_id: String,
    kind: ReadKind,
    witness: GenerationWitness,
    control: WatchControl,
    state: Mutex<ActiveState>,
    cancelled: AtomicBool,
    parent_label: Option<String>,
}

struct ActiveState {
    /// Kept by actual callback, never IPC waiter. Dropping after completion
    /// lets the workspace drain prove quiescence.
    admission: Option<Lease>,
    result: Option<Result<ReadOutcome, ClientError>>,
    credential_deadline: Option<std::time::Instant>,
}

/// Finalize callback custody after its stack has unwound. This guard is
/// created before invoking the client, so client arguments (including the
/// secret lease) are dropped before admission is released on a panic.
/// It does not catch panics, change the panic hook, or handle process abort.
struct CallbackCompletion(Arc<Active>);

impl Drop for CallbackCompletion {
    fn drop(&mut self) {
        // finish is single-assignment. A normal result stays intact, while an
        // unwound callback becomes a fixed failure; cancellation still wins.
        self.0.finish(Err(ClientError::Source));
    }
}

impl Active {
    fn new(review_id: String, kind: ReadKind, admission: Lease, control: WatchControl) -> Self {
        Self {
            review_id,
            kind,
            witness: admission.witness(),
            control,
            state: Mutex::new(ActiveState {
                admission: Some(admission),
                result: None,
                credential_deadline: None,
            }),
            cancelled: AtomicBool::new(false),
            parent_label: None,
        }
    }

    fn accept_credential(&self, deadline: std::time::Instant) -> Result<(), ClientError> {
        if self.cancelled.load(Ordering::Acquire) || !self.witness.is_current().unwrap_or(false) {
            return Err(ClientError::Cancelled);
        }
        credential::check_deadline(deadline)
    }

    fn bind_credential_deadline(&self, deadline: std::time::Instant) -> Result<(), ClientError> {
        let mut state = self.state.lock().map_err(|_| ClientError::Source)?;
        let bound = state
            .credential_deadline
            .map_or(deadline, |old| old.min(deadline));
        state.credential_deadline = Some(bound);
        drop(state);
        self.accept_credential(bound)
    }

    fn finish(&self, result: Result<ReadOutcome, ClientError>) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if state.result.is_none() {
            state.result = Some(if self.cancelled.load(Ordering::Acquire) {
                Err(ClientError::Cancelled)
            } else if state
                .credential_deadline
                .is_some_and(|deadline| credential::check_deadline(deadline).is_err())
            {
                Err(ClientError::CredentialLeaseExpired)
            } else {
                result
            });
            state.admission.take();
        }
    }

    fn status(&self) -> HostStatus {
        let Ok(state) = self.state.lock() else {
            return HostStatus::Unavailable;
        };
        match state.result {
            None if self.cancelled.load(Ordering::Acquire) => HostStatus::Closing,
            None => HostStatus::Running,
            Some(Ok(_)) => HostStatus::Completed,
            Some(Err(ClientError::Cancelled)) => HostStatus::Cancelled,
            Some(Err(error)) => HostStatus::Error(error),
        }
    }

    fn is_complete(&self) -> bool {
        self.state.lock().is_ok_and(|state| state.result.is_some())
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.control.cancel();
        if let Ok(mut state) = self.state.lock()
            && matches!(state.result, Some(Ok(_)))
        {
            state.result = Some(Err(ClientError::Cancelled));
        }
    }

    fn completed_observation(&self) -> Option<ReadOutcome> {
        if self.cancelled.load(Ordering::Acquire) || !self.witness.is_current().ok()? {
            return None;
        }
        let state = self.state.lock().ok()?;
        let observation = state.result.as_ref()?.as_ref().ok()?.clone();
        (!self.cancelled.load(Ordering::Acquire) && self.witness.is_current().ok()?)
            .then_some(observation)
    }
}

#[derive(Default)]
struct State {
    pending: Option<Pending>,
    active: Option<Arc<Active>>,
}

/// One native custody owner. It is not an OpenShell client and cannot make a
/// provider request in production D2b-2a.
#[derive(Clone)]
pub(crate) struct Host {
    gate: Gate,
    state: Arc<Mutex<State>>,
}

impl Host {
    pub(crate) fn new(gate: Gate) -> Self {
        Self {
            gate,
            state: Arc::new(Mutex::new(State::default())),
        }
    }

    /// Validate target before admission, then retain exactly one pending review
    /// under the current workspace generation.
    pub(crate) fn prepare(
        &self,
        workspace: &WorkspaceRequest,
        target: ProposedTarget,
        kind: ReadKind,
        retention: Retention,
    ) -> Result<Review, HostError> {
        // D2b-2a observation custody is available only through a managed,
        // generation-bound workspace request. Legacy and refused requests
        // cannot downgrade that binding to ordinary Gate admission.
        if !matches!(workspace, WorkspaceRequest::Managed(_)) {
            return Err(HostError::Unavailable);
        }
        let admission = workspace
            .admit(&self.gate, Lane::Read)
            .map_err(map_admission)?;
        if !admission.is_current().unwrap_or(false) {
            return Err(HostError::Unavailable);
        }
        self.retain_review(admission, target, kind, retention)
    }

    fn retain_review(
        &self,
        admission: Lease,
        target: ProposedTarget,
        kind: ReadKind,
        retention: Retention,
    ) -> Result<Review, HostError> {
        let review = Review {
            id: review_id()?,
            origin: target.origin.clone(),
            selected_peer: target.selected_peer,
            installation: target.installation.clone(),
            workspace: target.workspace.clone(),
            sandbox: target.sandbox.clone(),
            product_version: PRODUCT_VERSION,
            root_sha256: target.root_sha256.clone(),
            peer_pin_sha256: target.peer_pin_sha256.clone(),
            kind,
            retention,
            data_classes: data_classes(kind).to_vec(),
        };
        let mut state = self.state.lock().map_err(|_| HostError::Unavailable)?;
        // Close fences admission before taking this mutex to purge pending
        // reviews. A preparation that lost that race must drop its lease here.
        if !admission.is_current().unwrap_or(false) {
            return Err(HostError::Unavailable);
        }
        if state.pending.is_some() || state.active.is_some() {
            return Err(HostError::Busy);
        }
        let selection = CredentialSelection::new(&review)?;
        state.pending = Some(Pending::Awaiting {
            review: review.clone(),
            target,
            admission,
            selection,
        });
        Ok(review)
    }

    pub(crate) fn inspect_review(&self) -> Option<Review> {
        let state = self.state.lock().ok()?;
        state
            .pending
            .as_ref()
            .filter(|pending| pending.is_current())
            .map(|pending| pending.review().clone())
    }

    /// An inspection reference cannot acquire a secret or supply native consent.
    fn credential_inspection(&self, review_id: &str) -> Option<CredentialInspection> {
        let state = self.state.lock().ok()?;
        let pending = state.pending.as_ref()?;
        (pending.review().id == review_id && pending.is_current())
            .then(|| pending.selection().inspect())
    }

    /// Internal readback only; future UI needs its own reviewed IPC contract.
    /// Current Managed admission and the original review ID must both match.
    pub(crate) fn completed_observation(
        &self,
        workspace: &WorkspaceRequest,
        review_id: &str,
    ) -> Option<ReadOutcome> {
        if !matches!(workspace, WorkspaceRequest::Managed(_)) {
            return None;
        }
        let admission = workspace.admit(&self.gate, Lane::Read).ok()?;
        let state = self.state.lock().ok()?;
        let active = state.active.as_ref()?;
        if active.review_id != review_id || !admission.is_current().ok()? {
            return None;
        }
        let observation = active.completed_observation()?;
        admission.is_current().ok()?.then_some(observation)
    }

    /// Claim native consent once. A refusal releases admission immediately.
    pub(crate) fn confirm(&self, review_id: &str, approved: bool) -> HostStatus {
        let Ok(mut state) = self.state.lock() else {
            return HostStatus::Unavailable;
        };
        let Some(pending) = state.pending.as_ref() else {
            return HostStatus::Unavailable;
        };
        if pending.review().id != review_id {
            return HostStatus::Unavailable;
        }
        if !matches!(pending, Pending::Awaiting { .. }) {
            return HostStatus::Unavailable;
        }
        let pending = state.pending.take().expect("pending checked above");
        if !approved {
            return HostStatus::Cancelled;
        }
        let Pending::Awaiting {
            review,
            target,
            admission,
            selection,
        } = pending
        else {
            unreachable!("pending phase checked before ownership transfer");
        };
        if !admission.is_current().unwrap_or(false) {
            return HostStatus::Unavailable;
        }
        if let Err(error) = selection.check() {
            return HostStatus::Error(error);
        }
        state.pending = Some(Pending::Confirmed {
            review,
            target,
            admission,
            selection,
        });
        HostStatus::ReadyForNativeCredential
    }

    /// Production credential acquisition and provider contact remain closed.
    /// This function intentionally constructs neither a bearer lease nor a
    /// client, even after a valid one-shot consent decision.
    pub(crate) fn begin_production(&self, review_id: &str) -> HostStatus {
        let Ok(state) = self.state.lock() else {
            return HostStatus::Unavailable;
        };
        match state.pending.as_ref() {
            Some(Pending::Confirmed { review, .. }) if review.id == review_id => {
                HostStatus::Unavailable
            }
            Some(Pending::Awaiting { review, .. }) if review.id == review_id => {
                HostStatus::AwaitingConsent
            }
            _ => HostStatus::Unavailable,
        }
    }

    /// Unmounted native path. Only the explicit disposable qualification target
    /// calls this until production mounting receives separate acceptance.
    pub(crate) fn begin_native<R: tauri::Runtime>(
        &self,
        review_id: &str,
        parent: tauri::Window<R>,
    ) -> HostStatus {
        native_input::begin(self, review_id, parent)
    }

    /// Called by the single existing native window hook. It cancels only the
    /// flight bound to this exact initiating window, never another window's UI.
    pub(crate) fn parent_closed(&self, label: &str) {
        if let Ok(state) = self.state.lock()
            && let Some(active) = &state.active
            && active.parent_label.as_deref() == Some(label)
        {
            active.cancel();
        }
    }

    /// Stop is safe before, during, or after a synthetic callback. A running
    /// callback remains counted until it itself reports a result.
    pub(crate) fn stop(&self) -> HostStatus {
        let Ok(mut state) = self.state.lock() else {
            return HostStatus::Unavailable;
        };
        if state.pending.take().is_some() {
            return HostStatus::Cancelled;
        }
        let Some(active) = state.active.as_ref() else {
            return HostStatus::Unavailable;
        };
        active.cancel();
        if active.is_complete() {
            state.active = None;
            HostStatus::Cancelled
        } else {
            HostStatus::Closing
        }
    }

    /// Exit may proceed only after active callback custody has drained. Never
    /// block the native event loop waiting for a provider callback.
    pub(crate) fn cancel_and_check_exit(&self) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        state.pending = None;
        match state.active.as_ref() {
            None => true,
            Some(active) => {
                active.cancel();
                if active.is_complete() {
                    state.active = None;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// First close pass must cancel but retain an active callback. Final close
    /// may clear host state only after caller proves drain and callback reports.
    pub(crate) fn purge_for_workspace_close(&self, drained: bool) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        state.pending = None;
        let Some(active) = state.active.as_ref() else {
            return true;
        };
        active.cancel();
        if !drained {
            return true;
        }
        if !active.is_complete() {
            return false;
        }
        state.active = None;
        true
    }

    #[cfg(test)]
    fn begin_synthetic(
        &self,
        review_id: &str,
        lease: BearerLease,
        client: Box<dyn SyntheticReadClient>,
    ) -> HostStatus {
        self.begin_callback(
            review_id,
            None,
            CallbackStart::Confirmed,
            None,
            move |config, kind, control, _, _| client.run(config, lease, kind, control),
        )
    }

    /// C0 is test-only: the acquirer and peer contain synthetic data only.
    #[cfg(test)]
    fn begin_acquisition(
        &self,
        inspection: &CredentialInspection,
        acquirer: Option<Box<dyn credential::CredentialAcquirer>>,
        client: Box<dyn SyntheticReadClient>,
    ) -> HostStatus {
        self.begin_callback(
            &inspection.review.id,
            Some(inspection),
            CallbackStart::Confirmed,
            None,
            move |config, kind, control, active, selection| {
                let deadline = selection.deadline;
                active.bind_credential_deadline(deadline)?;
                let acquisition = acquirer
                    .ok_or(ClientError::Source)?
                    .acquire(selection.claim());
                active.accept_credential(deadline)?;
                let acquired = acquisition.map_err(|error| match error {
                    ClientError::Cancelled | ClientError::CredentialLeaseExpired => error,
                    _ => ClientError::Source,
                })?;
                let deadline = acquired.deadline();
                active.bind_credential_deadline(deadline)?;
                let result = client.run(config, acquired.into_lease(), kind, control);
                // A callback can outlive transport success. Expiry or cancellation
                // before actual return still rejects the retained observation.
                active.accept_credential(deadline)?;
                result
            },
        )
    }

    fn begin_callback<F>(
        &self,
        review_id: &str,
        inspection: Option<&CredentialInspection>,
        start: CallbackStart,
        parent_label: Option<String>,
        run: F,
    ) -> HostStatus
    where
        F: FnOnce(
                ClientConfig,
                ReadKind,
                WatchControl,
                &Arc<Active>,
                CredentialSelection,
            ) -> Result<ReadOutcome, ClientError>
            + Send
            + 'static,
    {
        let Ok(mut state) = self.state.lock() else {
            return HostStatus::Unavailable;
        };
        let Some(pending) = state.pending.as_ref() else {
            return HostStatus::Unavailable;
        };
        if pending.review().id != review_id {
            return HostStatus::Unavailable;
        }
        if let Some(candidate) = inspection
            && !pending.selection().matches(candidate, pending.review())
        {
            return HostStatus::Unavailable;
        }
        if !matches!(
            (start, pending),
            (CallbackStart::Confirmed, Pending::Confirmed { .. })
                | (CallbackStart::NativeReview, Pending::Awaiting { .. })
        ) {
            return HostStatus::AwaitingConsent;
        }
        let pending = state.pending.take().expect("pending checked above");
        let (Pending::Awaiting {
            review,
            target,
            admission,
            selection,
        }
        | Pending::Confirmed {
            review,
            target,
            admission,
            selection,
        }) = pending;
        if !admission.is_current().unwrap_or(false) {
            return HostStatus::Unavailable;
        }
        if let Err(error) = selection.check() {
            return HostStatus::Error(error);
        }
        let control = WatchControl::default();
        let mut active = Active::new(review.id.clone(), review.kind, admission, control.clone());
        active.parent_label = parent_label;
        let active = Arc::new(active);
        let callback = Arc::clone(&active);
        let kind = review.kind;
        let spawned = std::thread::Builder::new()
            .name("rangoon-openshell-observation".into())
            .spawn(move || {
                let completion = CallbackCompletion(callback);
                let result = run(target.config, kind, control, &completion.0, selection).and_then(
                    |outcome| {
                        outcome
                            .matches_review(&review)
                            .then_some(outcome)
                            .ok_or(ClientError::Source)
                    },
                );
                completion.0.finish(result);
            });
        if spawned.is_err() {
            return HostStatus::Unavailable;
        }
        state.active = Some(active);
        HostStatus::Running
    }

    /// Snapshot one current Managed session. This is not an IPC delivery guard.
    pub(crate) fn inspect_session(
        &self,
        workspace: &WorkspaceRequest,
    ) -> session_view::SessionView {
        self.inspect_session_inner(
            workspace,
            #[cfg(test)]
            None,
        )
    }

    fn inspect_session_inner(
        &self,
        workspace: &WorkspaceRequest,
        #[cfg(test)] probe: Option<&session_view::SnapshotProbe>,
    ) -> session_view::SessionView {
        use session_view::SessionView;
        if !matches!(workspace, WorkspaceRequest::Managed(_)) {
            return SessionView::unavailable();
        }
        let Ok(admission) = workspace.admit(&self.gate, Lane::Read) else {
            return SessionView::unavailable();
        };
        #[cfg(test)]
        if let Some(probe) = probe {
            session_view::cross_probe(&probe.before_lock);
        }
        // Stop and close purge use Host -> Active. Neither lock spans transport,
        // toolkit dispatch, await, or a user callback. Projection is bounded.
        let Ok(state) = self.state.lock() else {
            return SessionView::unavailable();
        };
        if let Some(active) = state.active.as_ref() {
            let Ok(active_state) = active.state.lock() else {
                return SessionView::unavailable();
            };
            let candidate = match &active_state.result {
                None => SessionView::phase(HostStatus::Running, Some(active.kind)),
                Some(Ok(outcome)) => SessionView::completed(active.kind, outcome),
                Some(Err(ClientError::Cancelled)) => {
                    SessionView::phase(HostStatus::Cancelled, Some(active.kind))
                }
                Some(Err(error)) => {
                    SessionView::phase(HostStatus::Error(*error), Some(active.kind))
                }
            };
            #[cfg(test)]
            if let Some(probe) = probe {
                session_view::cross_probe(&probe.before_final);
            }
            if !active.witness.is_current().unwrap_or(false)
                || !admission.is_current().unwrap_or(false)
            {
                return SessionView::unavailable();
            }
            // Last witness check above defines this instantaneous snapshot;
            // cancellation is checked while both captured states stay protected.
            let view = if active.cancelled.load(Ordering::Acquire) {
                let phase = if active_state.result.is_none() {
                    HostStatus::Closing
                } else {
                    HostStatus::Cancelled
                };
                SessionView::phase(phase, Some(active.kind))
            } else {
                candidate
            };
            #[cfg(test)]
            if let Some(probe) = probe {
                session_view::cross_probe(&probe.after_final);
            }
            return view;
        }
        #[cfg(test)]
        if let Some(probe) = probe {
            session_view::cross_probe(&probe.before_final);
        }
        if !admission.is_current().unwrap_or(false) {
            return SessionView::unavailable();
        }
        // Pending::status performs the last retained witness check, then checks
        // the immutable selection deadline. Do not cache a phase before this
        // boundary: a queued inspection may outlive native review consent.
        let candidate = state
            .pending
            .as_ref()
            .map_or_else(SessionView::unavailable, |pending| {
                SessionView::phase(pending.status(), Some(pending.review().kind))
            });
        #[cfg(test)]
        if let Some(probe) = probe {
            session_view::cross_probe(&probe.after_final);
        }
        candidate
    }

    pub(crate) fn status(&self) -> HostStatus {
        let Ok(state) = self.state.lock() else {
            return HostStatus::Unavailable;
        };
        match state.active.as_ref() {
            Some(active) => active.status(),
            None => state
                .pending
                .as_ref()
                .map_or(HostStatus::Unavailable, Pending::status),
        }
    }

    #[cfg(test)]
    fn cancellation_signalled(&self) -> bool {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.active.as_ref().cloned())
            .is_some_and(|active| active.cancelled.load(Ordering::Acquire))
    }
}

fn map_admission(error: AdmissionError) -> HostError {
    match error {
        AdmissionError::Busy => HostError::Busy,
        AdmissionError::Unavailable => HostError::Unavailable,
    }
}

fn review_id() -> Result<String, HostError> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| HostError::Unavailable)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn data_classes(kind: ReadKind) -> &'static [DataClass] {
    match kind {
        ReadKind::ProtectedCheck => &[DataClass::ProtectedRead],
        ReadKind::PolicyStatus => &[DataClass::PolicyRevision],
        ReadKind::RecentLogs => &[DataClass::RedactedLog],
        ReadKind::RecentWatch => &[DataClass::BoundedWatch],
    }
}

/// Test-only closed dispatch seam. Production has no implementation of this
/// trait, no synthetic client injection, and no route to `OpenShellReadClient`.
/// The transport crate separately proves its real synthetic TLS gRPC behavior.
#[cfg(test)]
trait SyntheticReadClient: Send + 'static {
    fn run(
        self: Box<Self>,
        config: ClientConfig,
        lease: BearerLease,
        kind: ReadKind,
        control: WatchControl,
    ) -> Result<ReadOutcome, ClientError>;
}

#[cfg(test)]
#[path = "openshell_host_tests.rs"]
mod tests;

#[cfg(all(test, feature = "native-exit-qualification"))]
#[path = "native_exit_qualification.rs"]
pub(crate) mod native_exit_qualification;

#[cfg(all(test, feature = "native-credential-qualification"))]
#[path = "native_credential_qualification.rs"]
pub(crate) mod native_credential_qualification;
