//! Closed, inert operations record vocabulary.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Window {
    pub schema_version: String,
    pub snapshot: Snapshot,
    pub tail: Vec<Event>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Snapshot {
    pub snapshot_id: String,
    pub reducer_version: String,
    pub scope: SnapshotScope,
    pub topology: Topology,
    pub prefixes: Vec<Prefix>,
    pub as_of_ms: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SnapshotScope {
    pub workspace_id: String,
    pub system_ids: Vec<String>,
    pub environment_ids: Vec<String>,
    pub engine_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Topology {
    pub revision_id: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub policy_revision_ids: Vec<String>,
    pub coverage_revision_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Node {
    pub id: String,
    pub kind: String,
    pub record_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Edge {
    pub id: String,
    pub kind: String,
    pub from: String,
    pub to: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Prefix {
    pub producer_id: String,
    pub fence_id: String,
    pub watermark: Option<Cursor>,
    pub events: Vec<Event>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Cursor {
    pub sequence: u64,
    pub event_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Event {
    pub event_id: String,
    pub producer_id: String,
    pub fence_id: String,
    pub sequence: u64,
    pub previous_event_id: Option<String>,
    pub observed_ms: Option<u64>,
    pub received_ms: Option<u64>,
    pub correlation: Option<Correlation>,
    pub record: Record,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Correlation {
    pub workspace_id: String,
    pub system_id: String,
    pub environment_id: String,
    pub engine_id: String,
    pub trace_id: String,
    pub span_id: String,
    pub request_id: Option<String>,
    pub evaluation_id: Option<String>,
    pub attempt_id: Option<String>,
    pub policy_revision_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FlowContext {
    pub workspace_id: String,
    pub system_id: String,
    pub environment_id: String,
    pub engine_id: String,
    pub request_id: String,
    pub trace_id: String,
    pub span_id: String,
    pub policy_revision_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ActionScope {
    pub system_id: String,
    pub environment_ids: Vec<String>,
    pub operation_classes: Vec<String>,
    pub resource_ids: Vec<String>,
    pub policy_revision_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExternalRef {
    pub system_id: String,
    pub contract_version: String,
    pub external_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Stage {
    pub id: String,
    pub depends_on_stage_ids: Vec<String>,
    pub approver_ids: Vec<String>,
    pub human_required: bool,
    pub mode: String,
    pub quorum: Option<u64>,
    pub expires_ms: Option<u64>,
    pub escalation_actor_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Money {
    pub kind: String,
    pub currency: String,
    pub amount_micros: u64,
    pub source_evidence_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Checks {
    pub policy_decision: String,
    pub approval_duties: String,
    pub agent_delegation: String,
    pub expiry_revocation: String,
    pub execution_consumption: String,
    pub replay_protection: String,
    pub outcome_evidence: String,
    pub reconciliation: String,
}

pub(crate) const INVENTORY_KINDS: [&str; 5] = ["actor", "resource", "tool", "workflow", "engine"];

/// Closed private vocabulary retained for the later O1b reducer.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum Record {
    Actor {
        id: String,
        system_id: String,
        canonical_subject_id: String,
        actor_kind: String,
        identity_source_ref: Option<ExternalRef>,
        identity_evidence_id: Option<String>,
        reported_verification: String,
        observed_ms: Option<u64>,
        freshness: String,
    },
    Resource {
        id: String,
        system_id: String,
        resource_class: String,
        external_ref: Option<ExternalRef>,
    },
    Tool {
        id: String,
        system_id: String,
        operation_class: String,
        external_ref: Option<ExternalRef>,
    },
    Workflow {
        id: String,
        system_id: String,
        external_ref: ExternalRef,
        capability_refs: Vec<ExternalRef>,
    },
    Engine {
        id: String,
        system_id: String,
        external_ref: ExternalRef,
        artifact_digest: String,
    },
    PolicyRevision {
        id: String,
        gateway_id: String,
        previous_revision_id: Option<String>,
        artifact_digest: String,
        external_ref: Option<ExternalRef>,
    },
    GatewayRevision {
        id: String,
        gateway_id: String,
        previous_revision_id: Option<String>,
        policy_revision_id: String,
        scope: ActionScope,
        human_required: bool,
        changed_by_actor_id: String,
        state: String,
        evidence_ids: Vec<String>,
    },
    Assignment {
        id: String,
        actor_id: String,
        gateway_revision_id: String,
        policy_revision_id: String,
        delegation_id: Option<String>,
        state: String,
        changed_by_actor_id: String,
        evidence_ids: Vec<String>,
    },
    Delegation {
        id: String,
        delegator_id: String,
        delegate_id: String,
        parent_delegation_id: Option<String>,
        scope: ActionScope,
        expires_ms: Option<u64>,
        state: String,
        changed_by_actor_id: String,
        external_ref: Option<ExternalRef>,
        reported_verification: String,
        observed_ms: Option<u64>,
        freshness: String,
        evidence_ids: Vec<String>,
    },
    Request {
        id: String,
        context: FlowContext,
        actor_id: String,
        gateway_revision_id: String,
        operation_class: String,
        action_digest: String,
        context_digest: String,
        resource_ids: Vec<String>,
        workflow_ref: Option<ExternalRef>,
        expires_ms: Option<u64>,
    },
    Evaluation {
        id: String,
        context: FlowContext,
        evaluation_kind: String,
        gateway_revision_id: String,
        evaluator_id: String,
        evaluator_artifact_digest: String,
        input_snapshot_id: String,
        input_canonicalization: String,
        input_digest: String,
        context_snapshot_id: String,
        context_digest: String,
        previous_evaluation_id: Option<String>,
        expected_effective_evaluation_id: Option<String>,
        lineage_fence_id: String,
        result: String,
        expires_ms: Option<u64>,
        reason: String,
        external_ref: Option<ExternalRef>,
        evidence_ids: Vec<String>,
    },
    ApprovalPlan {
        id: String,
        context: FlowContext,
        evaluation_id: String,
        previous_plan_id: Option<String>,
        changed_by_actor_id: String,
        expires_ms: Option<u64>,
        stages: Vec<Stage>,
        evidence_ids: Vec<String>,
    },
    ApprovalDecision {
        id: String,
        context: FlowContext,
        evaluation_id: String,
        plan_id: String,
        stage_id: String,
        actor_id: String,
        delegation_id: Option<String>,
        decision: String,
        supersedes_decision_id: Option<String>,
        reason: String,
        evidence_ids: Vec<String>,
    },
    Authorization {
        id: String,
        context: FlowContext,
        evaluation_id: String,
        plan_id: Option<String>,
        decision_ids: Vec<String>,
        previous_observation_id: Option<String>,
        state: String,
        scope: ActionScope,
        expires_ms: Option<u64>,
        reason: String,
        external_ref: Option<ExternalRef>,
        evidence_ids: Vec<String>,
    },
    Attempt {
        id: String,
        context: FlowContext,
        authorization_id: Option<String>,
        executor_id: String,
        previous_observation_id: Option<String>,
        attempt_id: String,
        state: String,
        reason: String,
        evidence_ids: Vec<String>,
    },
    Suspension {
        id: String,
        gateway_revision_id: String,
        requested_by_actor_id: String,
        scope: ActionScope,
        state: String,
        previous_observation_id: Option<String>,
        reason: String,
        evidence_ids: Vec<String>,
    },
    Metric {
        id: String,
        context: FlowContext,
        attempt_id: Option<String>,
        resource_ids: Vec<String>,
        latency_ms: Option<u64>,
        cost: Option<Money>,
        reason: String,
    },
    Evidence {
        id: String,
        producer_id: String,
        external_ref: Option<ExternalRef>,
        artifact_digest: Option<String>,
        state: String,
        reason: String,
    },
    Coverage {
        id: String,
        system_id: String,
        engine_id: String,
        gateway_revision_id: String,
        operation_class: String,
        environment_id: String,
        target_os: String,
        observed_ms: Option<u64>,
        freshness: String,
        target_resource_ids: Vec<String>,
        adapter_artifact_digest: String,
        engine_artifact_digest: String,
        reported_level: String,
        checks: Checks,
        expires_ms: Option<u64>,
        evidence_ids: Vec<String>,
    },
}

pub(crate) fn record_index(kind: &str) -> Option<usize> {
    match kind {
        "actor" => Some(0),
        "resource" => Some(1),
        "tool" => Some(2),
        "workflow" => Some(3),
        "engine" => Some(4),
        "policy_revision" => Some(5),
        "gateway_revision" => Some(6),
        "assignment" => Some(7),
        "delegation" => Some(8),
        "request" => Some(9),
        "evaluation" => Some(10),
        "approval_plan" => Some(11),
        "approval_decision" => Some(12),
        "authorization" => Some(13),
        "attempt" => Some(14),
        "suspension" => Some(15),
        "metric" => Some(16),
        "evidence" => Some(17),
        "coverage" => Some(18),
        _ => None,
    }
}

pub(crate) fn record_fields(kind: &str) -> Option<&'static [&'static str]> {
    Some(match kind {
        "actor" => &[
            "kind",
            "id",
            "systemId",
            "canonicalSubjectId",
            "actorKind",
            "identitySourceRef",
            "identityEvidenceId",
            "reportedVerification",
            "observedMs",
            "freshness",
        ],
        "resource" => &["kind", "id", "systemId", "resourceClass", "externalRef"],
        "tool" => &["kind", "id", "systemId", "operationClass", "externalRef"],
        "workflow" => &["kind", "id", "systemId", "externalRef", "capabilityRefs"],
        "engine" => &["kind", "id", "systemId", "externalRef", "artifactDigest"],
        "policy_revision" => &[
            "kind",
            "id",
            "gatewayId",
            "previousRevisionId",
            "artifactDigest",
            "externalRef",
        ],
        "gateway_revision" => &[
            "kind",
            "id",
            "gatewayId",
            "previousRevisionId",
            "policyRevisionId",
            "scope",
            "humanRequired",
            "changedByActorId",
            "state",
            "evidenceIds",
        ],
        "assignment" => &[
            "kind",
            "id",
            "actorId",
            "gatewayRevisionId",
            "policyRevisionId",
            "delegationId",
            "state",
            "changedByActorId",
            "evidenceIds",
        ],
        "delegation" => &[
            "kind",
            "id",
            "delegatorId",
            "delegateId",
            "parentDelegationId",
            "scope",
            "expiresMs",
            "state",
            "changedByActorId",
            "externalRef",
            "reportedVerification",
            "observedMs",
            "freshness",
            "evidenceIds",
        ],
        "request" => &[
            "kind",
            "id",
            "context",
            "actorId",
            "gatewayRevisionId",
            "operationClass",
            "actionDigest",
            "contextDigest",
            "resourceIds",
            "workflowRef",
            "expiresMs",
        ],
        "evaluation" => &[
            "kind",
            "id",
            "context",
            "evaluationKind",
            "gatewayRevisionId",
            "evaluatorId",
            "evaluatorArtifactDigest",
            "inputSnapshotId",
            "inputCanonicalization",
            "inputDigest",
            "contextSnapshotId",
            "contextDigest",
            "previousEvaluationId",
            "expectedEffectiveEvaluationId",
            "lineageFenceId",
            "result",
            "expiresMs",
            "reason",
            "externalRef",
            "evidenceIds",
        ],
        "approval_plan" => &[
            "kind",
            "id",
            "context",
            "evaluationId",
            "previousPlanId",
            "changedByActorId",
            "expiresMs",
            "stages",
            "evidenceIds",
        ],
        "approval_decision" => &[
            "kind",
            "id",
            "context",
            "evaluationId",
            "planId",
            "stageId",
            "actorId",
            "delegationId",
            "decision",
            "supersedesDecisionId",
            "reason",
            "evidenceIds",
        ],
        "authorization" => &[
            "kind",
            "id",
            "context",
            "evaluationId",
            "planId",
            "decisionIds",
            "previousObservationId",
            "state",
            "scope",
            "expiresMs",
            "reason",
            "externalRef",
            "evidenceIds",
        ],
        "attempt" => &[
            "kind",
            "id",
            "context",
            "authorizationId",
            "executorId",
            "previousObservationId",
            "attemptId",
            "state",
            "reason",
            "evidenceIds",
        ],
        "suspension" => &[
            "kind",
            "id",
            "gatewayRevisionId",
            "requestedByActorId",
            "scope",
            "state",
            "previousObservationId",
            "reason",
            "evidenceIds",
        ],
        "metric" => &[
            "kind",
            "id",
            "context",
            "attemptId",
            "resourceIds",
            "latencyMs",
            "cost",
            "reason",
        ],
        "evidence" => &[
            "kind",
            "id",
            "producerId",
            "externalRef",
            "artifactDigest",
            "state",
            "reason",
        ],
        "coverage" => &[
            "kind",
            "id",
            "systemId",
            "engineId",
            "gatewayRevisionId",
            "operationClass",
            "environmentId",
            "targetOs",
            "observedMs",
            "freshness",
            "targetResourceIds",
            "adapterArtifactDigest",
            "engineArtifactDigest",
            "reportedLevel",
            "checks",
            "expiresMs",
            "evidenceIds",
        ],
        _ => return None,
    })
}
