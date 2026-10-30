//! Renderer-safe, allowlisted operations observations. This module never exposes O1 documents.

use crate::{
    ApprovalInspection, DecodedWindow, DelegationInspection, LifecycleInspection, Projection,
    RECORD_KINDS, Relationships,
};
use serde::Serialize;
use std::collections::BTreeMap;

pub const OPERATOR_VIEW_SCHEMA: &str = "rangoon.ops.operator-view.v1";
pub const MAX_OPERATOR_VIEW_BYTES: usize = 4_194_304;

const STRUCTURAL_CODES: [&str; 18] = [
    "duplicate_observation",
    "event_identity_conflict",
    "sequence_conflict",
    "record_identity_conflict",
    "sequence_gap",
    "predecessor_missing",
    "predecessor_conflict",
    "prefix_fence_mismatch",
    "prefix_watermark_mismatch",
    "prefix_identity_conflict",
    "scope_mismatch",
    "correlation_mismatch",
    "duplicate_node",
    "node_record_missing",
    "node_record_kind_mismatch",
    "duplicate_edge",
    "edge_endpoint_missing",
    "edge_endpoint_ambiguous",
];
const ISSUE_STAGES: [&str; 5] = [
    "structural",
    "relationships",
    "delegations",
    "approvals",
    "lifecycle",
];
const NODE_KINDS: [&str; 7] = [
    "actor",
    "workflow",
    "tool",
    "resource",
    "gateway",
    "engine",
    "approval_stage",
];
const EDGE_KINDS: [&str; 5] = [
    "declared_dependency",
    "request_flow",
    "delegation_observation",
    "mediation_observation",
    "evidence_lineage",
];
const REFERENCE_STATUSES: [&str; 4] = ["missing", "ambiguous", "wrong_kind", "unique_unverified"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperatorViewError {
    StageMismatch,
    InvalidView,
    Limit,
    SerializationFailed,
}

impl OperatorViewError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::StageMismatch => "operator_view_stage_mismatch",
            Self::InvalidView => "operator_view_invalid",
            Self::Limit => "operator_view_limit",
            Self::SerializationFailed => "operator_view_serialization_failed",
        }
    }
}

impl std::fmt::Display for OperatorViewError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for OperatorViewError {}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperatorViewDocument {
    schema_version: &'static str,
    authority: &'static str,
    authenticity: &'static str,
    eligibility: &'static str,
    execution: &'static str,
    outcome: &'static str,
    coverage: &'static str,
    structural_state: String,
    source_counts: SourceCounts,
    diagnostic_counts: DiagnosticCounts,
    decision_counts: DecisionCounts,
    events: Vec<EventView>,
    topology: TopologyView,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceCounts {
    event_count: usize,
    producer_count: usize,
    topology_node_count: usize,
    topology_edge_count: usize,
    record_counts: Vec<RecordCount>,
}

#[derive(Clone, Debug, Serialize)]
struct RecordCount {
    kind: &'static str,
    count: usize,
}

#[derive(Clone, Debug, Serialize)]
struct DiagnosticCounts {
    structural: usize,
    relationships: usize,
    delegations: usize,
    approvals: usize,
    lifecycle: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DecisionCounts {
    evaluation: EvaluationDecisionCounts,
    approval_decision: ApprovalDecisionCounts,
    authorization: AuthorizationCounts,
    attempt: AttemptCounts,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EvaluationDecisionCounts {
    allow: usize,
    deny: usize,
    unknown: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
struct ApprovalDecisionCounts {
    approve: usize,
    deny: usize,
    abstain: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorizationCounts {
    reported_issued: usize,
    reported_refused: usize,
    reported_expired: usize,
    reported_revoked: usize,
    unknown: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct AttemptCounts {
    reported_accepted: usize,
    reported_active: usize,
    reported_completed: usize,
    reported_failed: usize,
    reported_cancelled: usize,
    outcome_unknown: usize,
    reported_reconciled_completed: usize,
    reported_reconciled_failed: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EventView {
    input_ordinal: usize,
    producer_ordinal: usize,
    fence_ordinal: usize,
    sequence: u64,
    record_kind: String,
    reported_value: Option<String>,
    structural_diagnostics: Vec<String>,
    issue_stages: Vec<&'static str>,
    references: Vec<ReferenceView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceView {
    reference_index: usize,
    status: String,
    matching_event_ordinals: Vec<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TopologyView {
    nodes: Vec<TopologyNodeView>,
    edges: Vec<TopologyEdgeView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TopologyNodeView {
    input_node_index: usize,
    kind: String,
    matching_event_ordinals: Vec<usize>,
    has_issue: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TopologyEdgeView {
    input_edge_index: usize,
    kind: String,
    from_node_indices: Vec<usize>,
    to_node_indices: Vec<usize>,
    has_issue: bool,
}

pub struct OperatorView {
    canonical: Vec<u8>,
    document: OperatorViewDocument,
}

impl OperatorView {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }

    pub fn document(&self) -> &OperatorViewDocument {
        &self.document
    }

    pub fn into_document(self) -> OperatorViewDocument {
        self.document
    }
}

impl std::fmt::Debug for OperatorView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OperatorView")
            .field("bytes", &self.canonical.len())
            .finish()
    }
}

/// Builds the only renderer-safe operations payload from one exact O1 inspection set.
pub fn build_operator_view(
    decoded: &DecodedWindow,
    projection: &Projection,
    relationships: &Relationships,
    delegations: &DelegationInspection,
    approvals: &ApprovalInspection,
    lifecycle: &LifecycleInspection,
) -> Result<OperatorView, OperatorViewError> {
    ensure_same_window(
        decoded,
        projection,
        relationships,
        delegations,
        approvals,
        lifecycle,
    )?;

    let projection_document = projection.document();
    let relationships_document = relationships.document();
    let delegations_document = delegations.document();
    let approvals_document = approvals.document();
    let lifecycle_document = lifecycle.document();
    let event_count = decoded.inventory().event_count();

    if projection_document.timeline.len() != event_count
        || projection_document.nodes.len() != decoded.inventory().topology_node_count()
        || projection_document.edges.len() != decoded.inventory().topology_edge_count()
    {
        return Err(OperatorViewError::StageMismatch);
    }

    let mut timeline = projection_document.timeline.iter().collect::<Vec<_>>();
    timeline.sort_by_key(|event| event.input_ordinal);
    let mut record_counts = [0usize; RECORD_KINDS.len()];
    for (ordinal, event) in timeline.iter().enumerate() {
        if event.input_ordinal != ordinal {
            return Err(OperatorViewError::InvalidView);
        }
        let Some(kind_index) = RECORD_KINDS
            .iter()
            .position(|kind| *kind == event.record_kind.as_str())
        else {
            return Err(OperatorViewError::InvalidView);
        };
        record_counts[kind_index] += 1;
    }
    if RECORD_KINDS
        .iter()
        .zip(record_counts)
        .any(|(kind, count)| decoded.inventory().record_counts().get(*kind).copied() != Some(count))
    {
        return Err(OperatorViewError::StageMismatch);
    }

    let relationship_events = indexed_stage(
        relationships_document
            .event_links
            .iter()
            .map(|event| (event.input_ordinal, event.diagnostics.as_slice())),
        event_count,
    )?;
    let delegation_events = indexed_stage(
        delegations_document
            .event_assessments
            .iter()
            .map(|event| (event.input_ordinal, event.diagnostics.as_slice())),
        event_count,
    )?;
    let approval_events = indexed_stage(
        approvals_document
            .event_assessments
            .iter()
            .map(|event| (event.input_ordinal, event.diagnostics.as_slice())),
        event_count,
    )?;
    let lifecycle_events = indexed_stage(
        lifecycle_document
            .event_assessments
            .iter()
            .map(|event| (event.input_ordinal, event.diagnostics.as_slice())),
        event_count,
    )?;

    let mut relationship_references = BTreeMap::new();
    let mut reference_total = 0usize;
    for event in &relationships_document.event_links {
        if event.input_ordinal >= event_count
            || relationship_references.contains_key(&event.input_ordinal)
        {
            return Err(OperatorViewError::StageMismatch);
        }
        let mut references = Vec::with_capacity(event.references.len());
        for (index, reference) in event.references.iter().enumerate() {
            reference_total = reference_total
                .checked_add(1)
                .ok_or(OperatorViewError::Limit)?;
            if reference_total > 8_192
                || !REFERENCE_STATUSES.contains(&reference.status)
                || !sorted_unique_in_bounds(&reference.matching_event_ordinals, event_count)
            {
                return Err(OperatorViewError::InvalidView);
            }
            references.push(ReferenceView {
                reference_index: index,
                status: reference.status.to_owned(),
                matching_event_ordinals: reference.matching_event_ordinals.clone(),
            });
        }
        relationship_references.insert(event.input_ordinal, references);
    }

    let mut producer_ordinals = BTreeMap::<&str, usize>::new();
    let mut fence_ordinals = BTreeMap::<(&str, &str), usize>::new();
    let mut events = Vec::with_capacity(event_count);
    for event in timeline {
        if event
            .diagnostics
            .iter()
            .any(|code| !STRUCTURAL_CODES.contains(&code.as_str()))
            || !matches!(
                event.record_kind.as_str(),
                "actor"
                    | "resource"
                    | "tool"
                    | "workflow"
                    | "engine"
                    | "policy_revision"
                    | "gateway_revision"
                    | "assignment"
                    | "delegation"
                    | "request"
                    | "evaluation"
                    | "approval_plan"
                    | "approval_decision"
                    | "authorization"
                    | "attempt"
                    | "suspension"
                    | "metric"
                    | "evidence"
                    | "coverage"
            )
        {
            return Err(OperatorViewError::InvalidView);
        }
        let producer_next = producer_ordinals.len();
        let producer_ordinal = *producer_ordinals
            .entry(event.producer_id.as_str())
            .or_insert(producer_next);
        let fence_next = fence_ordinals
            .keys()
            .filter(|(producer, _)| *producer == event.producer_id.as_str())
            .count();
        let fence_ordinal = *fence_ordinals
            .entry((event.producer_id.as_str(), event.fence_id.as_str()))
            .or_insert(fence_next);
        let stage_diagnostics = [
            !event.diagnostics.is_empty(),
            relationship_events
                .get(&event.input_ordinal)
                .is_some_and(|diagnostics| !diagnostics.is_empty()),
            delegation_events
                .get(&event.input_ordinal)
                .is_some_and(|diagnostics| !diagnostics.is_empty()),
            approval_events
                .get(&event.input_ordinal)
                .is_some_and(|diagnostics| !diagnostics.is_empty()),
            lifecycle_events
                .get(&event.input_ordinal)
                .is_some_and(|diagnostics| !diagnostics.is_empty()),
        ];
        events.push(EventView {
            input_ordinal: event.input_ordinal,
            producer_ordinal,
            fence_ordinal,
            sequence: event.sequence,
            record_kind: event.record_kind.clone(),
            reported_value: reported_value(&event.record_kind, event.reported_value.as_deref())?,
            structural_diagnostics: event.diagnostics.clone(),
            issue_stages: ISSUE_STAGES
                .iter()
                .zip(stage_diagnostics)
                .filter_map(|(stage, has_issue)| has_issue.then_some(*stage))
                .collect(),
            references: relationship_references
                .remove(&event.input_ordinal)
                .unwrap_or_default(),
        });
    }

    let mut nodes = projection_document.nodes.iter().collect::<Vec<_>>();
    nodes.sort_by_key(|node| node.input_node_index);
    if nodes.iter().enumerate().any(|(index, node)| {
        node.input_node_index != index
            || !NODE_KINDS.contains(&node.kind.as_str())
            || !sorted_unique_in_bounds(&node.matching_event_ordinals, event_count)
    }) {
        return Err(OperatorViewError::InvalidView);
    }
    let node_ids = nodes
        .iter()
        .map(|node| (node.id.as_str(), node.input_node_index))
        .fold(
            BTreeMap::<&str, Vec<usize>>::new(),
            |mut index, (id, ordinal)| {
                index.entry(id).or_default().push(ordinal);
                index
            },
        );
    let node_views = nodes
        .iter()
        .map(|node| TopologyNodeView {
            input_node_index: node.input_node_index,
            kind: node.kind.clone(),
            matching_event_ordinals: node.matching_event_ordinals.clone(),
            has_issue: !node.diagnostics.is_empty(),
        })
        .collect();

    let mut edges = projection_document.edges.iter().collect::<Vec<_>>();
    edges.sort_by_key(|edge| edge.input_edge_index);
    if edges.iter().enumerate().any(|(index, edge)| {
        edge.input_edge_index != index || !EDGE_KINDS.contains(&edge.kind.as_str())
    }) {
        return Err(OperatorViewError::InvalidView);
    }
    let edge_views = edges
        .iter()
        .map(|edge| TopologyEdgeView {
            input_edge_index: edge.input_edge_index,
            kind: edge.kind.clone(),
            from_node_indices: node_ids
                .get(edge.from.as_str())
                .cloned()
                .unwrap_or_default(),
            to_node_indices: node_ids.get(edge.to.as_str()).cloned().unwrap_or_default(),
            has_issue: !edge.diagnostics.is_empty(),
        })
        .collect();

    let decision_counts = decision_counts(&events, decoded)?;
    let document = OperatorViewDocument {
        schema_version: OPERATOR_VIEW_SCHEMA,
        authority: "none",
        authenticity: "unverified",
        eligibility: "unknown",
        execution: "unavailable",
        outcome: "unknown",
        coverage: "unknown",
        structural_state: checked_structural_state(projection_document.structural_state)?,
        source_counts: SourceCounts {
            event_count,
            producer_count: decoded.inventory().producer_count(),
            topology_node_count: decoded.inventory().topology_node_count(),
            topology_edge_count: decoded.inventory().topology_edge_count(),
            record_counts: RECORD_KINDS
                .iter()
                .map(|kind| {
                    Ok(RecordCount {
                        kind,
                        count: *decoded
                            .inventory()
                            .record_counts()
                            .get(*kind)
                            .ok_or(OperatorViewError::StageMismatch)?,
                    })
                })
                .collect::<Result<Vec<_>, OperatorViewError>>()?,
        },
        diagnostic_counts: DiagnosticCounts {
            structural: structural_count(projection_document),
            relationships: relationship_count(relationships_document),
            delegations: delegation_count(delegations_document),
            approvals: approvals_document
                .diagnostic_counts
                .iter()
                .map(|count| count.count)
                .sum(),
            lifecycle: lifecycle_document
                .diagnostic_counts
                .iter()
                .map(|count| count.count)
                .sum(),
        },
        decision_counts,
        events,
        topology: TopologyView {
            nodes: node_views,
            edges: edge_views,
        },
    };
    let canonical = bounded_serialize(&document)?;
    Ok(OperatorView {
        canonical,
        document,
    })
}

fn bounded_serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, OperatorViewError> {
    let canonical =
        serde_json::to_vec(value).map_err(|_| OperatorViewError::SerializationFailed)?;
    if canonical.len() > MAX_OPERATOR_VIEW_BYTES {
        return Err(OperatorViewError::Limit);
    }
    Ok(canonical)
}

fn ensure_same_window(
    decoded: &DecodedWindow,
    projection: &Projection,
    relationships: &Relationships,
    delegations: &DelegationInspection,
    approvals: &ApprovalInspection,
    lifecycle: &LifecycleInspection,
) -> Result<(), OperatorViewError> {
    let source = decoded.canonical_digest();
    let projection_digest = projection.canonical_digest();
    let relationship_digest = relationships.canonical_digest();
    let delegation_digest = delegations.canonical_digest();
    let approval_digest = approvals.canonical_digest();
    let relationship_document = relationships.document();
    let delegation_document = delegations.document();
    let approval_document = approvals.document();
    let lifecycle_document = lifecycle.document();
    if relationship_document.input_canonical_digest != source
        || delegation_document.input_canonical_digest != source
        || approval_document.input_canonical_digest != source
        || lifecycle_document.input_canonical_digest != source
        || relationship_document.structural_projection_digest != projection_digest
        || delegation_document.structural_projection_digest != projection_digest
        || approval_document.structural_projection_digest != projection_digest
        || lifecycle_document.structural_projection_digest != projection_digest
        || approval_document.relationship_digest != relationship_digest
        || lifecycle_document.relationship_digest != relationship_digest
        || approval_document.delegation_digest != delegation_digest
        || lifecycle_document.delegation_digest != delegation_digest
        || lifecycle_document.approval_digest != approval_digest
    {
        return Err(OperatorViewError::StageMismatch);
    }
    Ok(())
}

fn indexed_stage<'a>(
    entries: impl Iterator<Item = (usize, &'a [&'a str])>,
    event_count: usize,
) -> Result<BTreeMap<usize, &'a [&'a str]>, OperatorViewError> {
    let mut result = BTreeMap::new();
    for (ordinal, diagnostics) in entries {
        if ordinal >= event_count || result.insert(ordinal, diagnostics).is_some() {
            return Err(OperatorViewError::StageMismatch);
        }
    }
    Ok(result)
}

fn sorted_unique_in_bounds(values: &[usize], event_count: usize) -> bool {
    values.iter().all(|ordinal| *ordinal < event_count)
        && values.windows(2).all(|pair| pair[0] < pair[1])
}

fn reported_value(kind: &str, value: Option<&str>) -> Result<Option<String>, OperatorViewError> {
    let allowed: &[&str] = match kind {
        "evaluation" => &["allow", "deny", "unknown"],
        "approval_decision" => &["approve", "deny", "abstain"],
        "authorization" => &[
            "reported_issued",
            "reported_refused",
            "reported_expired",
            "reported_revoked",
            "unknown",
        ],
        "attempt" => &[
            "reported_accepted",
            "reported_active",
            "reported_completed",
            "reported_failed",
            "reported_cancelled",
            "outcome_unknown",
            "reported_reconciled_completed",
            "reported_reconciled_failed",
        ],
        _ => return Ok(None),
    };
    match value {
        Some(value) if allowed.contains(&value) => Ok(Some(value.to_owned())),
        _ => Err(OperatorViewError::InvalidView),
    }
}

fn decision_counts(
    events: &[EventView],
    decoded: &DecodedWindow,
) -> Result<DecisionCounts, OperatorViewError> {
    let mut counts = DecisionCounts {
        evaluation: EvaluationDecisionCounts::default(),
        approval_decision: ApprovalDecisionCounts::default(),
        authorization: AuthorizationCounts::default(),
        attempt: AttemptCounts::default(),
    };
    for event in events {
        match (event.record_kind.as_str(), event.reported_value.as_deref()) {
            ("evaluation", Some("allow")) => counts.evaluation.allow += 1,
            ("evaluation", Some("deny")) => counts.evaluation.deny += 1,
            ("evaluation", Some("unknown")) => counts.evaluation.unknown += 1,
            ("approval_decision", Some("approve")) => counts.approval_decision.approve += 1,
            ("approval_decision", Some("deny")) => counts.approval_decision.deny += 1,
            ("approval_decision", Some("abstain")) => counts.approval_decision.abstain += 1,
            ("authorization", Some("reported_issued")) => counts.authorization.reported_issued += 1,
            ("authorization", Some("reported_refused")) => {
                counts.authorization.reported_refused += 1
            }
            ("authorization", Some("reported_expired")) => {
                counts.authorization.reported_expired += 1
            }
            ("authorization", Some("reported_revoked")) => {
                counts.authorization.reported_revoked += 1
            }
            ("authorization", Some("unknown")) => counts.authorization.unknown += 1,
            ("attempt", Some("reported_accepted")) => counts.attempt.reported_accepted += 1,
            ("attempt", Some("reported_active")) => counts.attempt.reported_active += 1,
            ("attempt", Some("reported_completed")) => counts.attempt.reported_completed += 1,
            ("attempt", Some("reported_failed")) => counts.attempt.reported_failed += 1,
            ("attempt", Some("reported_cancelled")) => counts.attempt.reported_cancelled += 1,
            ("attempt", Some("outcome_unknown")) => counts.attempt.outcome_unknown += 1,
            ("attempt", Some("reported_reconciled_completed")) => {
                counts.attempt.reported_reconciled_completed += 1
            }
            ("attempt", Some("reported_reconciled_failed")) => {
                counts.attempt.reported_reconciled_failed += 1
            }
            ("evaluation" | "approval_decision" | "authorization" | "attempt", _) => {
                return Err(OperatorViewError::InvalidView);
            }
            _ => {}
        }
    }
    let inventory = decoded.inventory().record_counts();
    let equal = |kind: &str, total: usize| inventory.get(kind).copied() == Some(total);
    if !equal(
        "evaluation",
        counts.evaluation.allow + counts.evaluation.deny + counts.evaluation.unknown,
    ) || !equal(
        "approval_decision",
        counts.approval_decision.approve
            + counts.approval_decision.deny
            + counts.approval_decision.abstain,
    ) || !equal(
        "authorization",
        counts.authorization.reported_issued
            + counts.authorization.reported_refused
            + counts.authorization.reported_expired
            + counts.authorization.reported_revoked
            + counts.authorization.unknown,
    ) || !equal(
        "attempt",
        counts.attempt.reported_accepted
            + counts.attempt.reported_active
            + counts.attempt.reported_completed
            + counts.attempt.reported_failed
            + counts.attempt.reported_cancelled
            + counts.attempt.outcome_unknown
            + counts.attempt.reported_reconciled_completed
            + counts.attempt.reported_reconciled_failed,
    ) {
        return Err(OperatorViewError::StageMismatch);
    }
    Ok(counts)
}

fn checked_structural_state(value: &str) -> Result<String, OperatorViewError> {
    match value {
        "conflict" | "incomplete" | "structural_unverified" => Ok(value.to_owned()),
        _ => Err(OperatorViewError::InvalidView),
    }
}

fn structural_count(document: &crate::ProjectionDocument) -> usize {
    let counts = &document.diagnostic_counts;
    counts.duplicate_observation
        + counts.event_identity_conflict
        + counts.sequence_conflict
        + counts.record_identity_conflict
        + counts.sequence_gap
        + counts.predecessor_missing
        + counts.predecessor_conflict
        + counts.prefix_fence_mismatch
        + counts.prefix_watermark_mismatch
        + counts.prefix_identity_conflict
        + counts.scope_mismatch
        + counts.correlation_mismatch
        + counts.duplicate_node
        + counts.node_record_missing
        + counts.node_record_kind_mismatch
        + counts.duplicate_edge
        + counts.edge_endpoint_missing
        + counts.edge_endpoint_ambiguous
}

fn relationship_count(document: &crate::RelationshipDocument) -> usize {
    let counts = &document.diagnostic_counts;
    counts.structural_incomplete
        + counts.reference_missing
        + counts.reference_ambiguous
        + counts.reference_kind_mismatch
        + counts.context_mismatch
        + counts.gateway_mismatch
        + counts.context_digest_mismatch
        + counts.gateway_policy_mismatch
        + counts.evaluation_selector_mismatch
        + counts.lineage_fence_mismatch
        + counts.evaluation_sequence_mismatch
        + counts.evaluation_branch
        + counts.evaluation_cycle
        + counts.evaluation_incomplete_chain
        + counts.binding_mismatch
        + counts.non_tip_evaluation
}

fn delegation_count(document: &crate::DelegationDocument) -> usize {
    let counts = &document.diagnostic_counts;
    counts.structural_incomplete
        + counts.reference_missing
        + counts.reference_ambiguous
        + counts.reference_kind_mismatch
        + counts.subject_kind_conflict
        + counts.system_mismatch
        + counts.self_delegation
        + counts.self_expansion
        + counts.parent_actor_mismatch
        + counts.scope_incomplete
        + counts.scope_expansion
        + counts.expiry_incomplete
        + counts.expiry_expansion
        + counts.attenuation_missing
        + counts.delegation_cycle
        + counts.subject_cycle
}

#[cfg(test)]
mod bounded_serialize_tests {
    use super::{MAX_OPERATOR_VIEW_BYTES, OperatorViewError, bounded_serialize};

    #[test]
    fn accepts_exact_cap_and_rejects_one_serialized_byte_more() {
        let exact = "x".repeat(MAX_OPERATOR_VIEW_BYTES - 2);
        assert_eq!(
            bounded_serialize(&exact).unwrap().len(),
            MAX_OPERATOR_VIEW_BYTES
        );

        let overflow = "x".repeat(MAX_OPERATOR_VIEW_BYTES - 1);
        assert_eq!(
            bounded_serialize(&overflow).unwrap_err(),
            OperatorViewError::Limit
        );
    }
}
