//! Bounded, offline mapping of already-decoded synthetic OpenShell metadata.
//! This module has no transport, filesystem, policy, authorization, or execution authority.

use serde::Serialize;
use std::collections::BTreeMap;

pub const OPEN_SHELL_VERSION: &str = "0.1.4";
pub const MAX_OPEN_SHELL_ITEMS: usize = 128;
pub const MAX_OPEN_SHELL_POLICY_REVISIONS: usize = 32;
pub const MAX_OPEN_SHELL_OUTPUT_BYTES: usize = 131_072;
pub const MAX_OPEN_SHELL_TIME_MS: u64 = 9_007_199_254_740_991;
pub const MAX_OPEN_SHELL_CURSOR_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenShellObservationError {
    InputLimit,
    InvalidInput,
    UnsupportedVersion,
    OutputLimit,
    SerializationFailed,
}

impl OpenShellObservationError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InputLimit => "openshell_window_limit",
            Self::InvalidInput => "openshell_invalid_input",
            Self::UnsupportedVersion => "openshell_unsupported_version",
            Self::OutputLimit => "openshell_output_limit",
            Self::SerializationFailed => "openshell_serialization_failed",
        }
    }
}

impl std::fmt::Display for OpenShellObservationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for OpenShellObservationError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenShellWindow {
    pub version: String,
    pub scope: OpenShellScope,
    pub prior_stream: Option<OpenShellStreamPosition>,
    pub policy_revisions: Vec<PolicyRevision>,
    pub items: Vec<OpenShellItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenShellScope {
    pub installation_id: String,
    pub gateway_display_name: String,
    pub workspace_id: String,
    pub sandbox_id: String,
    pub source_kind: OpenShellSourceKind,
    pub replica_id: Option<String>,
    pub cursor_space: Option<String>,
    pub cursor: Option<String>,
}

/// Prior opaque stream metadata supplied by the caller only to mark a continuity break.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenShellStreamPosition {
    pub replica_id: Option<String>,
    pub cursor_space: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenShellSourceKind {
    SandboxPolicyStatus,
    SandboxLog,
    SandboxStream,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyRevision {
    pub revision_id: String,
    pub status: PolicyLoadStatus,
    pub active_version: Option<String>,
    pub provider_time_ms: Option<u64>,
    pub receipt_time_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyLoadStatus {
    Pending,
    Loaded,
    Failed,
    Superseded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenShellItem {
    Report(ReportedEvent),
    Watch(WatchStatus),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportedEvent {
    pub event_id: Option<String>,
    pub native_id: Option<String>,
    pub producer: OpenShellProducer,
    pub result: ReportedResult,
    pub policy_revision: Option<String>,
    pub provider_time_ms: Option<u64>,
    pub receipt_time_ms: Option<u64>,
    pub recent_buffer_truncated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenShellProducer {
    PolicyStatus,
    SandboxLog,
    PlatformStatus,
    StreamStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportedResult {
    Allowed,
    Denied,
    Uncertain,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchStatus {
    pub event_id: Option<String>,
    pub condition: WatchCondition,
    pub provider_time_ms: Option<u64>,
    pub receipt_time_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchCondition {
    Warning,
    OutOfRange,
    InvalidArgument,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MappedOpenShellWindow {
    pub schema_version: &'static str,
    pub source: &'static str,
    pub version: &'static str,
    pub scope: MappedScope,
    pub policy_revisions: Vec<MappedPolicyRevision>,
    pub items: Vec<MappedItem>,
    pub diagnostics: Vec<MappedDiagnostic>,
    pub losses: Vec<&'static str>,
    pub authority: &'static str,
    pub lnsat_authorization: &'static str,
    pub execution: &'static str,
    pub verified_effect: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MappedScope {
    pub installation_id: String,
    pub workspace_id: String,
    pub sandbox_id: String,
    pub source_kind: OpenShellSourceKind,
    pub replica_id: Option<String>,
    pub cursor_space: Option<String>,
    pub cursor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MappedPolicyRevision {
    pub revision_id: String,
    pub status: PolicyLoadStatus,
    pub active_version: Option<String>,
    pub active_version_matches: Option<bool>,
    pub provider_time_ms: Option<u64>,
    pub receipt_time_ms: Option<u64>,
    pub effective_policy: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MappedItem {
    pub event_id: Option<String>,
    pub kind: &'static str,
    pub native_id: Option<String>,
    pub producer: Option<OpenShellProducer>,
    pub reported_result: Option<ReportedResult>,
    pub policy_revision: Option<String>,
    pub watch_condition: Option<WatchCondition>,
    pub stream_state: &'static str,
    pub provider_time_ms: Option<u64>,
    pub receipt_time_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MappedDiagnostic {
    pub code: &'static str,
    pub event_id: Option<String>,
}

/// Maps one already-decoded synthetic window. It never parses provider bytes or performs I/O.
pub fn map_openshell_window(
    window: &OpenShellWindow,
) -> Result<MappedOpenShellWindow, OpenShellObservationError> {
    if window.items.len() > MAX_OPEN_SHELL_ITEMS
        || window.policy_revisions.len() > MAX_OPEN_SHELL_POLICY_REVISIONS
    {
        return Err(OpenShellObservationError::InputLimit);
    }
    if window.version != OPEN_SHELL_VERSION {
        return Err(OpenShellObservationError::UnsupportedVersion);
    }
    validate_scope(&window.scope)?;
    validate_source_shape(window)?;
    if let Some(prior) = &window.prior_stream {
        validate_stream_position(prior)?;
    }

    let mut diagnostics = Vec::new();
    let mut revisions = Vec::with_capacity(window.policy_revisions.len());
    let mut revision_ids = BTreeMap::<&str, &PolicyRevision>::new();
    for revision in &window.policy_revisions {
        token(&revision.revision_id)?;
        time(revision.provider_time_ms)?;
        time(revision.receipt_time_ms)?;
        if let Some(active) = &revision.active_version {
            token(active)?;
        }
        if revision_ids
            .insert(&revision.revision_id, revision)
            .is_some()
        {
            return Err(OpenShellObservationError::InvalidInput);
        }
        let matches = revision
            .active_version
            .as_deref()
            .map(|active| active == revision.revision_id);
        if matches == Some(false) {
            diagnostics.push(MappedDiagnostic {
                code: "active_version_mismatch",
                event_id: None,
            });
        }
        revisions.push(MappedPolicyRevision {
            revision_id: revision.revision_id.clone(),
            status: revision.status,
            active_version: revision.active_version.clone(),
            active_version_matches: matches,
            provider_time_ms: revision.provider_time_ms,
            receipt_time_ms: revision.receipt_time_ms,
            effective_policy: if revision.status == PolicyLoadStatus::Loaded
                && matches == Some(true)
            {
                "reported_loaded"
            } else {
                "unassessed"
            },
        });
    }
    if revisions.len() > 1 {
        diagnostics.push(MappedDiagnostic {
            code: "policy_revision_changed",
            event_id: None,
        });
    }

    let mut items = Vec::with_capacity(window.items.len());
    let mut losses = Vec::new();
    if let Some(prior) = &window.prior_stream {
        if prior.replica_id != window.scope.replica_id {
            losses.push("stream_continuity_replica_changed");
        }
        if prior.cursor_space != window.scope.cursor_space {
            losses.push("stream_continuity_cursor_space_changed");
        }
    }
    let mut event_fingerprints = BTreeMap::<String, String>::new();
    let mut first_producer: Option<OpenShellProducer> = None;
    for item in &window.items {
        let mapped = map_item(item, &revision_ids, &mut losses)?;
        if let Some(producer) = mapped.producer {
            if let Some(first) = first_producer {
                if first != producer
                    && !diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.code == "mixed_producer")
                {
                    diagnostics.push(MappedDiagnostic {
                        code: "mixed_producer",
                        event_id: None,
                    });
                }
            } else {
                first_producer = Some(producer);
            }
        }
        let fingerprint = serde_json::to_string(&mapped)
            .map_err(|_| OpenShellObservationError::SerializationFailed)?;
        if let Some(event_id) = mapped.event_id.as_deref()
            && let Some(previous) =
                event_fingerprints.insert(event_id.to_owned(), fingerprint.clone())
        {
            diagnostics.push(MappedDiagnostic {
                code: if previous == fingerprint {
                    "repeated_event_id"
                } else {
                    "conflicting_event_id"
                },
                event_id: Some(event_id.to_owned()),
            });
        }
        items.push(mapped);
    }

    let mapped = MappedOpenShellWindow {
        schema_version: "rangoon.openshell.observation.v0.1",
        source: "openshell_offline_synthetic",
        version: OPEN_SHELL_VERSION,
        scope: MappedScope {
            installation_id: window.scope.installation_id.clone(),
            workspace_id: window.scope.workspace_id.clone(),
            sandbox_id: window.scope.sandbox_id.clone(),
            source_kind: window.scope.source_kind,
            replica_id: window.scope.replica_id.clone(),
            cursor_space: window.scope.cursor_space.clone(),
            cursor: window.scope.cursor.clone(),
        },
        policy_revisions: revisions,
        items,
        diagnostics,
        losses,
        authority: "none",
        lnsat_authorization: "unavailable",
        execution: "unavailable",
        verified_effect: "unavailable",
    };
    mapped.minimized_json_bytes()?;
    Ok(mapped)
}

impl MappedOpenShellWindow {
    /// Bounded minimized JSON for isolated inspection. This is not canonical identity data.
    pub fn minimized_json_bytes(&self) -> Result<Vec<u8>, OpenShellObservationError> {
        let bytes =
            serde_json::to_vec(self).map_err(|_| OpenShellObservationError::SerializationFailed)?;
        if bytes.len() > MAX_OPEN_SHELL_OUTPUT_BYTES {
            Err(OpenShellObservationError::OutputLimit)
        } else {
            Ok(bytes)
        }
    }
}

fn map_item(
    item: &OpenShellItem,
    revisions: &BTreeMap<&str, &PolicyRevision>,
    losses: &mut Vec<&'static str>,
) -> Result<MappedItem, OpenShellObservationError> {
    match item {
        OpenShellItem::Report(report) => {
            if let Some(event_id) = &report.event_id {
                token(event_id)?;
            } else {
                losses.push("native_event_id_missing");
            }
            if let Some(native_id) = &report.native_id {
                token(native_id)?;
            }
            if let Some(revision) = &report.policy_revision {
                token(revision)?;
                if !revisions.contains_key(revision.as_str()) {
                    losses.push("reported_policy_revision_not_in_window");
                }
            }
            time(report.provider_time_ms)?;
            time(report.receipt_time_ms)?;
            if report.recent_buffer_truncated {
                losses.push("recent_buffer_truncated");
            }
            Ok(MappedItem {
                event_id: report.event_id.clone(),
                kind: "reported_event",
                native_id: report.native_id.clone(),
                producer: Some(report.producer),
                reported_result: Some(report.result),
                policy_revision: report.policy_revision.clone(),
                watch_condition: None,
                stream_state: "reported_only",
                provider_time_ms: report.provider_time_ms,
                receipt_time_ms: report.receipt_time_ms,
            })
        }
        OpenShellItem::Watch(watch) => {
            if let Some(event_id) = &watch.event_id {
                token(event_id)?;
            } else {
                losses.push("native_event_id_missing");
            }
            time(watch.provider_time_ms)?;
            time(watch.receipt_time_ms)?;
            let stream_state = match watch.condition {
                WatchCondition::Warning => {
                    losses.push("watch_gap_warning");
                    "watch_warning"
                }
                WatchCondition::OutOfRange => {
                    losses.push("watch_gap_out_of_range");
                    "gap"
                }
                WatchCondition::InvalidArgument => "invalid_input",
            };
            Ok(MappedItem {
                event_id: watch.event_id.clone(),
                kind: "watch_status",
                native_id: None,
                producer: None,
                reported_result: None,
                policy_revision: None,
                watch_condition: Some(watch.condition),
                stream_state,
                provider_time_ms: watch.provider_time_ms,
                receipt_time_ms: watch.receipt_time_ms,
            })
        }
    }
}

fn validate_scope(scope: &OpenShellScope) -> Result<(), OpenShellObservationError> {
    token(&scope.installation_id)?;
    token(&scope.workspace_id)?;
    token(&scope.sandbox_id)?;
    if scope.gateway_display_name.len() > 256 {
        return Err(OpenShellObservationError::InvalidInput);
    }
    validate_stream_position(&OpenShellStreamPosition {
        replica_id: scope.replica_id.clone(),
        cursor_space: scope.cursor_space.clone(),
    })?;
    if let Some(cursor) = &scope.cursor {
        opaque_cursor(cursor)?;
    }
    if scope.cursor.is_some() && scope.cursor_space.is_none() {
        return Err(OpenShellObservationError::InvalidInput);
    }
    Ok(())
}

fn validate_source_shape(window: &OpenShellWindow) -> Result<(), OpenShellObservationError> {
    let has_stream_position = window.scope.replica_id.is_some()
        || window.scope.cursor_space.is_some()
        || window.scope.cursor.is_some()
        || window.prior_stream.is_some();
    match window.scope.source_kind {
        OpenShellSourceKind::SandboxPolicyStatus => {
            if !window.items.is_empty() || has_stream_position {
                return Err(OpenShellObservationError::InvalidInput);
            }
        }
        OpenShellSourceKind::SandboxLog => {
            if !window.policy_revisions.is_empty()
                || has_stream_position
                || !window.items.iter().all(|item| {
                    matches!(item, OpenShellItem::Report(report) if report.producer == OpenShellProducer::SandboxLog)
                })
            {
                return Err(OpenShellObservationError::InvalidInput);
            }
        }
        OpenShellSourceKind::SandboxStream => {
            if !window.policy_revisions.is_empty()
                || !window.items.iter().all(|item| {
                    matches!(item, OpenShellItem::Watch(_)
                        | OpenShellItem::Report(ReportedEvent {
                            producer: OpenShellProducer::SandboxLog
                                | OpenShellProducer::PlatformStatus
                                | OpenShellProducer::StreamStatus,
                            ..
                        }))
                })
            {
                return Err(OpenShellObservationError::InvalidInput);
            }
        }
    }
    Ok(())
}

fn validate_stream_position(
    position: &OpenShellStreamPosition,
) -> Result<(), OpenShellObservationError> {
    for token_value in [&position.replica_id, &position.cursor_space]
        .into_iter()
        .flatten()
    {
        token(token_value)?;
    }
    Ok(())
}

fn token(value: &str) -> Result<(), OpenShellObservationError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'))
    {
        Err(OpenShellObservationError::InvalidInput)
    } else {
        Ok(())
    }
}

fn opaque_cursor(value: &str) -> Result<(), OpenShellObservationError> {
    if value.is_empty()
        || value.len() > MAX_OPEN_SHELL_CURSOR_BYTES
        || value.chars().any(char::is_control)
    {
        Err(OpenShellObservationError::InvalidInput)
    } else {
        Ok(())
    }
}

fn time(value: Option<u64>) -> Result<(), OpenShellObservationError> {
    if value.is_some_and(|millis| millis > MAX_OPEN_SHELL_TIME_MS) {
        Err(OpenShellObservationError::InvalidInput)
    } else {
        Ok(())
    }
}
