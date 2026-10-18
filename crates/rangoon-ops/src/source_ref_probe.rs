//! Candidate source-reference extraction from an already validated O1 window.
//!
//! This probe is deliberately narrower than an adapter. It neither identifies
//! a provider nor emits a D0a observation, and its output conveys no authority.

use crate::DecodedWindow;
use crate::records::{Event, ExternalRef, FlowContext, Record, Window};
use serde::Serialize;

/// Maximum candidate/refusal rows. One validated O1 event yields one row.
pub const MAX_SOURCE_REFERENCE_ROWS: usize = crate::MAX_EVENTS;

/// Fixed failures from the closed decoded-window boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceReferenceProbeError {
    InternalInconsistency,
}

impl SourceReferenceProbeError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InternalInconsistency => "source_reference_internal_inconsistency",
        }
    }
}

impl std::fmt::Display for SourceReferenceProbeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SourceReferenceProbeError {}

/// A closed result. It is source shape only, never provider identity or proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceReferenceProbeOutcome {
    CandidateUnverifiedSourceShape,
    RefuseNoSameRecordFlowContext,
    RefuseUnsupportedRecordKind,
    RefuseMissingExternalReference,
    RefuseSystemMismatch,
    RefuseInvalidReference,
}

impl SourceReferenceProbeOutcome {
    pub const fn code(self) -> &'static str {
        match self {
            Self::CandidateUnverifiedSourceShape => "candidate_unverified_source_shape",
            Self::RefuseNoSameRecordFlowContext => "refuse_no_same_record_flow_context",
            Self::RefuseUnsupportedRecordKind => "refuse_unsupported_record_kind",
            Self::RefuseMissingExternalReference => "refuse_missing_external_reference",
            Self::RefuseSystemMismatch => "refuse_system_mismatch",
            Self::RefuseInvalidReference => "refuse_invalid_reference",
        }
    }
}

/// Five opaque source-reference parts copied byte-for-byte from one O1 record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceReference {
    system_id: String,
    environment_id: String,
    engine_id: String,
    contract_version: String,
    native_id: String,
}

impl SourceReference {
    pub fn system_id(&self) -> &str {
        &self.system_id
    }

    pub fn environment_id(&self) -> &str {
        &self.environment_id
    }

    pub fn engine_id(&self) -> &str {
        &self.engine_id
    }

    pub fn contract_version(&self) -> &str {
        &self.contract_version
    }

    pub fn native_id(&self) -> &str {
        &self.native_id
    }
}

/// One candidate/refusal result, tied to its original O1 event and record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceReferenceProbeRow {
    event_id: String,
    record_id: String,
    #[serde(rename = "result")]
    outcome: SourceReferenceProbeOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    reference: Option<SourceReference>,
}

impl SourceReferenceProbeRow {
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    pub fn record_id(&self) -> &str {
        &self.record_id
    }

    pub const fn outcome(&self) -> SourceReferenceProbeOutcome {
        self.outcome
    }

    pub fn reference(&self) -> Option<&SourceReference> {
        self.reference.as_ref()
    }
}

/// Immutable ordered results from one decoded O1 window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceReferenceProbe {
    rows: Vec<SourceReferenceProbeRow>,
}

impl SourceReferenceProbe {
    pub fn rows(&self) -> &[SourceReferenceProbeRow] {
        &self.rows
    }
}

/// Extracts candidate source-reference parts from one already validated O1 window.
///
/// The event traversal follows input prefix order followed by input tail order.
/// Event-level correlation is never read. A successful candidate is unverified
/// source shape only and cannot be used as an authorization or execution result.
pub fn probe_source_references(
    decoded: &DecodedWindow,
) -> Result<SourceReferenceProbe, SourceReferenceProbeError> {
    let window: Window = serde_json::from_slice(decoded.canonical_bytes())
        .map_err(|_| SourceReferenceProbeError::InternalInconsistency)?;
    let mut rows = Vec::with_capacity(decoded.inventory().event_count());

    for prefix in &window.snapshot.prefixes {
        for event in &prefix.events {
            rows.push(probe_event(event));
        }
    }
    for event in &window.tail {
        rows.push(probe_event(event));
    }

    debug_assert_eq!(rows.len(), decoded.inventory().event_count());
    debug_assert!(rows.len() <= MAX_SOURCE_REFERENCE_ROWS);
    Ok(SourceReferenceProbe { rows })
}

fn probe_event(event: &Event) -> SourceReferenceProbeRow {
    let (record_id, outcome, reference) = match &event.record {
        Record::Evaluation {
            id,
            context,
            external_ref,
            ..
        } => probe_evaluation(id, context, external_ref.as_ref()),
        record if record_context(record).is_some() => (
            record_id(record),
            SourceReferenceProbeOutcome::RefuseUnsupportedRecordKind,
            None,
        ),
        record => (
            record_id(record),
            SourceReferenceProbeOutcome::RefuseNoSameRecordFlowContext,
            None,
        ),
    };

    SourceReferenceProbeRow {
        event_id: event.event_id.clone(),
        record_id: record_id.to_owned(),
        outcome,
        reference,
    }
}

fn probe_evaluation<'a>(
    record_id: &'a str,
    context: &'a FlowContext,
    external_ref: Option<&'a ExternalRef>,
) -> (
    &'a str,
    SourceReferenceProbeOutcome,
    Option<SourceReference>,
) {
    let Some(external_ref) = external_ref else {
        return (
            record_id,
            SourceReferenceProbeOutcome::RefuseMissingExternalReference,
            None,
        );
    };
    if external_ref.system_id != context.system_id {
        return (
            record_id,
            SourceReferenceProbeOutcome::RefuseSystemMismatch,
            None,
        );
    }
    if !is_opaque_token(&external_ref.system_id)
        || !is_opaque_token(&context.environment_id)
        || !is_opaque_token(&context.engine_id)
        || !is_opaque_token(&external_ref.contract_version)
        || !is_opaque_token(&external_ref.external_id)
    {
        return (
            record_id,
            SourceReferenceProbeOutcome::RefuseInvalidReference,
            None,
        );
    }
    (
        record_id,
        SourceReferenceProbeOutcome::CandidateUnverifiedSourceShape,
        Some(SourceReference {
            system_id: external_ref.system_id.clone(),
            environment_id: context.environment_id.clone(),
            engine_id: context.engine_id.clone(),
            contract_version: external_ref.contract_version.clone(),
            native_id: external_ref.external_id.clone(),
        }),
    )
}

pub(crate) fn is_opaque_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'))
}

fn record_context(record: &Record) -> Option<&FlowContext> {
    match record {
        Record::Request { context, .. }
        | Record::Evaluation { context, .. }
        | Record::ApprovalPlan { context, .. }
        | Record::ApprovalDecision { context, .. }
        | Record::Authorization { context, .. }
        | Record::Attempt { context, .. }
        | Record::Metric { context, .. } => Some(context),
        _ => None,
    }
}

fn record_id(record: &Record) -> &str {
    match record {
        Record::Actor { id, .. }
        | Record::Resource { id, .. }
        | Record::Tool { id, .. }
        | Record::Workflow { id, .. }
        | Record::Engine { id, .. }
        | Record::PolicyRevision { id, .. }
        | Record::GatewayRevision { id, .. }
        | Record::Assignment { id, .. }
        | Record::Delegation { id, .. }
        | Record::Request { id, .. }
        | Record::Evaluation { id, .. }
        | Record::ApprovalPlan { id, .. }
        | Record::ApprovalDecision { id, .. }
        | Record::Authorization { id, .. }
        | Record::Attempt { id, .. }
        | Record::Suspension { id, .. }
        | Record::Metric { id, .. }
        | Record::Evidence { id, .. }
        | Record::Coverage { id, .. } => id,
    }
}
