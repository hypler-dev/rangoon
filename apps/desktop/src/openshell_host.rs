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

enum Pending {
    Awaiting {
        review: Review,
        target: ProposedTarget,
        admission: Lease,
    },
    Confirmed {
        review: Review,
        target: ProposedTarget,
        admission: Lease,
    },
}

impl Pending {
    fn review(&self) -> &Review {
        match self {
            Self::Awaiting { review, .. } | Self::Confirmed { review, .. } => review,
        }
    }

    fn is_current(&self) -> bool {
        match self {
            Self::Awaiting { admission, .. } | Self::Confirmed { admission, .. } => {
                admission.is_current().unwrap_or(false)
            }
        }
    }
}

struct Active {
    review_id: String,
    witness: GenerationWitness,
    control: WatchControl,
    state: Mutex<ActiveState>,
    cancelled: AtomicBool,
}

struct ActiveState {
    /// Kept by actual callback, never IPC waiter. Dropping after completion
    /// lets the workspace drain prove quiescence.
    admission: Option<Lease>,
    result: Option<Result<ReadOutcome, ClientError>>,
}

/// Finalize callback custody after its stack has unwound. This guard is
/// created before invoking the client, so client arguments (including the
/// secret lease) are dropped before admission is released on a panic.
/// It does not catch panics, change the panic hook, or handle process abort.
#[cfg(test)]
struct CallbackCompletion(Arc<Active>);

#[cfg(test)]
impl Drop for CallbackCompletion {
    fn drop(&mut self) {
        // finish is single-assignment. A normal result stays intact, while an
        // unwound callback becomes a fixed failure; cancellation still wins.
        self.0.finish(Err(ClientError::Source));
    }
}

impl Active {
    fn new(review_id: String, admission: Lease, control: WatchControl) -> Self {
        Self {
            review_id,
            witness: admission.witness(),
            control,
            state: Mutex::new(ActiveState {
                admission: Some(admission),
                result: None,
            }),
            cancelled: AtomicBool::new(false),
        }
    }

    fn finish(&self, result: Result<ReadOutcome, ClientError>) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if state.result.is_none() {
            state.result = Some(if self.cancelled.load(Ordering::Acquire) {
                Err(ClientError::Cancelled)
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
        if state.pending.is_some() || state.active.is_some() {
            return Err(HostError::Busy);
        }
        state.pending = Some(Pending::Awaiting {
            review: review.clone(),
            target,
            admission,
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
        } = pending
        else {
            unreachable!("pending phase checked before ownership transfer");
        };
        if !admission.is_current().unwrap_or(false) {
            return HostStatus::Unavailable;
        }
        state.pending = Some(Pending::Confirmed {
            review,
            target,
            admission,
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
        let Ok(mut state) = self.state.lock() else {
            return HostStatus::Unavailable;
        };
        let Some(pending) = state.pending.as_ref() else {
            return HostStatus::Unavailable;
        };
        if pending.review().id != review_id {
            return HostStatus::Unavailable;
        }
        if !matches!(pending, Pending::Confirmed { .. }) {
            return HostStatus::AwaitingConsent;
        }
        let pending = state.pending.take().expect("pending checked above");
        let Pending::Confirmed {
            review,
            target,
            admission,
        } = pending
        else {
            unreachable!("pending phase checked before ownership transfer");
        };
        if !admission.is_current().unwrap_or(false) {
            return HostStatus::Unavailable;
        }
        let control = WatchControl::default();
        let active = Arc::new(Active::new(review.id.clone(), admission, control.clone()));
        let callback = Arc::clone(&active);
        let kind = review.kind;
        let spawned = std::thread::Builder::new()
            .name("rangoon-openshell-synthetic-read".into())
            .spawn(move || {
                let completion = CallbackCompletion(callback);
                let result = client
                    .run(target.config, lease, kind, control)
                    .and_then(|outcome| {
                        outcome
                            .matches_review(&review)
                            .then_some(outcome)
                            .ok_or(ClientError::Source)
                    });
                completion.0.finish(result);
            });
        if spawned.is_err() {
            return HostStatus::Unavailable;
        }
        state.active = Some(active);
        HostStatus::Running
    }

    #[cfg(test)]
    fn status(&self) -> HostStatus {
        let Ok(state) = self.state.lock() else {
            return HostStatus::Unavailable;
        };
        match state.active.as_ref() {
            Some(active) => active.status(),
            None if state.pending.is_some() => HostStatus::AwaitingConsent,
            None => HostStatus::Unavailable,
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
