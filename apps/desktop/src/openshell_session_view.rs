//! App-private, minimized display of one instantaneous native session.
//! No provider record, target, credential or inbound replacement is serializable.
use super::{HostStatus, Observation, PRODUCT_VERSION, ReadKind, ReadOutcome};
use rangoon_openshell_client::{
    ClientError, Continuity, HealthStatus, Loss, PolicyLoadStatus, ProducerKind, WatchEventKind,
};
use serde::Serialize;
use std::io::{self, Write};

const MAX_ENCODED_BYTES: usize = 8_192;
const MAX_REVISIONS: usize = 64;
const MAX_LOG_LINES: usize = 64;
const MAX_WATCH_EVENTS: usize = 128;
const MAX_LOSSES: usize = 12;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionView {
    schema: &'static str,
    provider: &'static str,
    product_version: &'static str,
    phase: Phase,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    read_kind: Option<SelectedRead>,
    start_available: bool,
    retention: &'static str,
    provenance: Provenance,
    #[serde(skip_serializing_if = "Option::is_none")]
    observation: Option<Summary>,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Unavailable,
    AwaitingConsent,
    ReadyForNativeCredential,
    Running,
    Closing,
    Cancelled,
    Completed,
    Error,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum SelectedRead {
    ProtectedCheck,
    PolicyStatus,
    RecentLogs,
    RecentWatch,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Provenance {
    authenticity: &'static str,
    authority: &'static str,
    execution: &'static str,
    action_outcome: &'static str,
    coverage: &'static str,
    omissions: [&'static str; 5],
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Summary {
    #[serde(rename_all = "camelCase")]
    ProtectedCheck {
        health: Health,
        policy_load: LoadState,
    },
    #[serde(rename_all = "camelCase")]
    PolicyStatus {
        active_version: u32,
        queried_version: u32,
        load_state: LoadState,
        revision_count: usize,
    },
    #[serde(rename_all = "camelCase")]
    RecentLogs {
        returned_line_count: u32,
        provider_buffer_total: u32,
    },
    #[serde(rename_all = "camelCase")]
    RecentWatch {
        event_count: usize,
        kinds: EventCounts,
        producers: ProducerCounts,
        producer_absent_count: usize,
        continuity: WindowContinuity,
        loss_reasons: Vec<LossReason>,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Health {
    Unspecified,
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum LoadState {
    Unspecified,
    Pending,
    Loaded,
    Failed,
    Superseded,
    Unknown,
}

#[derive(Default, Serialize)]
struct EventCounts {
    status: usize,
    log: usize,
    platform: usize,
    warning: usize,
}

#[derive(Default, Serialize)]
struct ProducerCounts {
    gateway: usize,
    sandbox: usize,
    kubernetes: usize,
    docker: usize,
    process: usize,
    unknown: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum WindowContinuity {
    Bounded,
    Gap,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum LossReason {
    PolicyHashMissing,
    PolicyHashInvalid,
    PolicyBodyRedacted,
    PolicyLoadErrorRedacted,
    PolicyProvenanceRedacted,
    LogTextRedacted,
    LogFieldsRedacted,
    PlatformTextRedacted,
    ProducerInferredFromLegacyEmpty,
    ProducerUnrecognized,
    MissingNativeEventId,
    StreamWarning,
}

impl SessionView {
    pub(super) fn unavailable() -> Self {
        Self::phase(HostStatus::Unavailable, None)
    }

    /// Non-completed phases never carry a summary. A bare Completed is invalid.
    pub(super) fn phase(status: HostStatus, kind: Option<ReadKind>) -> Self {
        let (phase, error) = match status {
            HostStatus::Unavailable | HostStatus::Completed => (Phase::Unavailable, None),
            HostStatus::AwaitingConsent => (Phase::AwaitingConsent, None),
            HostStatus::ReadyForNativeCredential => (Phase::ReadyForNativeCredential, None),
            HostStatus::Running => (Phase::Running, None),
            HostStatus::Closing => (Phase::Closing, None),
            HostStatus::Cancelled => (Phase::Cancelled, None),
            HostStatus::Error(error) => (Phase::Error, Some(error.code())),
        };
        let read_kind = if matches!(phase, Phase::Unavailable) {
            None
        } else {
            kind.map(SelectedRead::from)
        };
        Self {
            schema: "rangoon.openshell.session-view.v1",
            provider: "openshell",
            product_version: PRODUCT_VERSION,
            phase,
            error,
            read_kind,
            start_available: false,
            retention: "session_only",
            provenance: Provenance {
                authenticity: "unqualified",
                authority: "none",
                execution: "unavailable",
                action_outcome: "unknown",
                coverage: "bounded_partial",
                omissions: [
                    "native_identifiers",
                    "source_timestamps",
                    "policy_hashes",
                    "provider_payloads",
                    "target_and_credential_references",
                ],
            },
            observation: None,
        }
    }

    pub(super) fn completed(kind: ReadKind, outcome: &ReadOutcome) -> Self {
        match Summary::from_outcome(kind, outcome) {
            Ok(summary) => {
                let mut view = Self::phase(HostStatus::Running, Some(kind));
                view.phase = Phase::Completed;
                view.observation = Some(summary);
                // Bound even app-private schema growth before publishing a view.
                if view.encode().is_err() {
                    return Self::phase(HostStatus::Error(ClientError::Bounds), Some(kind));
                }
                view
            }
            Err(error) => Self::phase(HostStatus::Error(error), Some(kind)),
        }
    }

    /// Bounded in-memory encoding; failure returns no prefix or partial JSON.
    pub(crate) fn encode(&self) -> Result<Vec<u8>, ClientError> {
        let mut writer = BoundedJson(Vec::new());
        serde_json::to_writer(&mut writer, self).map_err(|_| ClientError::Bounds)?;
        Ok(writer.0)
    }
}

impl Summary {
    fn from_outcome(kind: ReadKind, outcome: &ReadOutcome) -> Result<Self, ClientError> {
        let check = &outcome.protected_check;
        if check.reported_product_version != PRODUCT_VERSION
            || !matches!(check.health, HealthStatus::Healthy | HealthStatus::Degraded)
        {
            return Err(ClientError::Source);
        }
        bounded_losses(&check.protected_read.losses)?;
        let scope_matches = |installation: &str, workspace: &str, sandbox: &str| {
            check.installation == installation
                && check.workspace == workspace
                && check.sandbox == sandbox
        };
        match (&outcome.observation, kind) {
            (Observation::ProtectedCheck(value), ReadKind::ProtectedCheck) if value == check => {
                Ok(Self::ProtectedCheck {
                    health: Health::from(value.health),
                    policy_load: LoadState::from(value.protected_read.status),
                })
            }
            (Observation::PolicyStatus(value), ReadKind::PolicyStatus)
                if scope_matches(&value.installation, &value.workspace, &value.sandbox) =>
            {
                if value.revisions.len() > MAX_REVISIONS {
                    return Err(ClientError::Bounds);
                }
                bounded_losses(&value.queried.losses)?;
                for revision in &value.revisions {
                    bounded_losses(&revision.losses)?;
                }
                Ok(Self::PolicyStatus {
                    active_version: value.active_version,
                    queried_version: value.queried.version,
                    load_state: LoadState::from(value.queried.status),
                    revision_count: value.revisions.len(),
                })
            }
            (Observation::RecentLogs(value), ReadKind::RecentLogs)
                if scope_matches(&value.installation, &value.workspace, &value.sandbox) =>
            {
                let count = value.line_count as usize;
                if count > MAX_LOG_LINES
                    || value.source_times_millis.len() != count
                    || value.producers.len() != count
                {
                    return Err(ClientError::Bounds);
                }
                bounded_losses(&value.losses)?;
                Ok(Self::RecentLogs {
                    returned_line_count: value.line_count,
                    provider_buffer_total: value.buffer_total,
                })
            }
            (Observation::RecentWatch(value), ReadKind::RecentWatch)
                if scope_matches(&value.installation, &value.workspace, &value.sandbox) =>
            {
                if value.events.len() > MAX_WATCH_EVENTS {
                    return Err(ClientError::Bounds);
                }
                let mut kinds = EventCounts::default();
                let mut producers = ProducerCounts::default();
                let mut producer_absent_count = 0;
                let mut seen_losses = [false; MAX_LOSSES];
                for event in &value.events {
                    bounded_losses(&event.losses)?;
                    match event.kind {
                        WatchEventKind::Status => kinds.status += 1,
                        WatchEventKind::Log => kinds.log += 1,
                        WatchEventKind::Platform => kinds.platform += 1,
                        WatchEventKind::Warning => kinds.warning += 1,
                    }
                    match event.producer {
                        Some(ProducerKind::Gateway) => producers.gateway += 1,
                        Some(ProducerKind::Sandbox) => producers.sandbox += 1,
                        Some(ProducerKind::Kubernetes) => producers.kubernetes += 1,
                        Some(ProducerKind::Docker) => producers.docker += 1,
                        Some(ProducerKind::Process) => producers.process += 1,
                        Some(ProducerKind::Unknown) => producers.unknown += 1,
                        None => producer_absent_count += 1,
                    }
                    for loss in &event.losses {
                        seen_losses[loss_index(*loss)] = true;
                    }
                }
                if kinds.warning != 0 && value.continuity != Continuity::Gap {
                    return Err(ClientError::Source);
                }
                let loss_reasons = LOSS_REASONS
                    .into_iter()
                    .enumerate()
                    .filter_map(|(index, reason)| seen_losses[index].then_some(reason))
                    .collect();
                Ok(Self::RecentWatch {
                    event_count: value.events.len(),
                    kinds,
                    producers,
                    producer_absent_count,
                    continuity: match value.continuity {
                        Continuity::Bounded => WindowContinuity::Bounded,
                        Continuity::Gap => WindowContinuity::Gap,
                    },
                    loss_reasons,
                })
            }
            _ => Err(ClientError::Source),
        }
    }
}

fn bounded_losses(losses: &[Loss]) -> Result<(), ClientError> {
    if losses.len() > MAX_LOSSES {
        Err(ClientError::Bounds)
    } else {
        Ok(())
    }
}

impl From<ReadKind> for SelectedRead {
    fn from(value: ReadKind) -> Self {
        match value {
            ReadKind::ProtectedCheck => Self::ProtectedCheck,
            ReadKind::PolicyStatus => Self::PolicyStatus,
            ReadKind::RecentLogs => Self::RecentLogs,
            ReadKind::RecentWatch => Self::RecentWatch,
        }
    }
}
impl From<HealthStatus> for Health {
    fn from(value: HealthStatus) -> Self {
        match value {
            HealthStatus::Unspecified => Self::Unspecified,
            HealthStatus::Healthy => Self::Healthy,
            HealthStatus::Degraded => Self::Degraded,
            HealthStatus::Unhealthy => Self::Unhealthy,
            HealthStatus::Unknown => Self::Unknown,
        }
    }
}
impl From<PolicyLoadStatus> for LoadState {
    fn from(value: PolicyLoadStatus) -> Self {
        match value {
            PolicyLoadStatus::Unspecified => Self::Unspecified,
            PolicyLoadStatus::Pending => Self::Pending,
            PolicyLoadStatus::Loaded => Self::Loaded,
            PolicyLoadStatus::Failed => Self::Failed,
            PolicyLoadStatus::Superseded => Self::Superseded,
            PolicyLoadStatus::Unknown => Self::Unknown,
        }
    }
}

const LOSS_REASONS: [LossReason; MAX_LOSSES] = [
    LossReason::PolicyHashMissing,
    LossReason::PolicyHashInvalid,
    LossReason::PolicyBodyRedacted,
    LossReason::PolicyLoadErrorRedacted,
    LossReason::PolicyProvenanceRedacted,
    LossReason::LogTextRedacted,
    LossReason::LogFieldsRedacted,
    LossReason::PlatformTextRedacted,
    LossReason::ProducerInferredFromLegacyEmpty,
    LossReason::ProducerUnrecognized,
    LossReason::MissingNativeEventId,
    LossReason::StreamWarning,
];
fn loss_index(value: Loss) -> usize {
    match value {
        Loss::PolicyHashMissing => 0,
        Loss::PolicyHashInvalid => 1,
        Loss::PolicyBodyRedacted => 2,
        Loss::PolicyLoadErrorRedacted => 3,
        Loss::PolicyProvenanceRedacted => 4,
        Loss::LogTextRedacted => 5,
        Loss::LogFieldsRedacted => 6,
        Loss::PlatformTextRedacted => 7,
        Loss::ProducerInferredFromLegacyEmpty => 8,
        Loss::ProducerUnrecognized => 9,
        Loss::MissingNativeEventId => 10,
        Loss::StreamWarning => 11,
    }
}

struct BoundedJson(Vec<u8>);
impl Write for BoundedJson {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_ENCODED_BYTES.saturating_sub(self.0.len()) {
            return Err(io::Error::other("session view exceeds encoded bound"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[derive(Default)]
pub(super) struct SnapshotProbe {
    pub(super) before_lock: Option<(std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>)>,
    pub(super) before_final: Option<(std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>)>,
    pub(super) after_final: Option<(std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>)>,
}
#[cfg(test)]
pub(super) fn cross_probe(
    boundary: &Option<(std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>)>,
) {
    if let Some((entered, release)) = boundary {
        let _ = entered.send(());
        // Sender drop also releases a test that unwinds; never wait forever.
        let _ = release.recv_timeout(std::time::Duration::from_secs(10));
    }
}

#[cfg(test)]
#[path = "openshell_session_view_tests.rs"]
mod tests;
