//! Strict bounded JSON decoding. It validates only supplied syntax and shape.

use crate::records::{INVENTORY_KINDS, Window, record_fields, record_index};
use crate::{
    DecodeError, DecodedWindow, MAX_CANONICAL_BYTES, MAX_DEPTH, MAX_EDGES, MAX_EVENTS,
    MAX_INPUT_BYTES, MAX_INTEGER, MAX_LIST, MAX_NODES, MAX_OBJECT_FIELDS, MAX_PRODUCERS,
    MAX_RAW_ARRAY, MAX_VALUES, REDUCER_VERSION, WINDOW_SCHEMA,
};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub fn decode_window(input: &[u8]) -> Result<DecodedWindow, DecodeError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(DecodeError::InputLimit);
    }
    let source = std::str::from_utf8(input).map_err(|_| DecodeError::InvalidEncoding)?;
    scan_raw(source)?;
    let value: Value = serde_json::from_str(source).map_err(|_| DecodeError::InvalidJson)?;
    let root = object(&value)?;
    closed(root, &["schemaVersion", "snapshot", "tail"])?;
    if string(field(root, "schemaVersion")?)? != WINDOW_SCHEMA {
        return Err(DecodeError::UnsupportedSchema);
    }
    let (nodes, edges, producer_count, events, counts) = validate_window(root)?;
    let typed: Window = serde_json::from_value(value).map_err(|_| DecodeError::InvalidRecord)?;
    let canonical = serde_json::to_vec(&typed).map_err(|_| DecodeError::SerializationFailed)?;
    if canonical.len() > MAX_CANONICAL_BYTES {
        return Err(DecodeError::CanonicalLimit);
    }
    Ok(DecodedWindow::from_parts(
        canonical,
        producer_count,
        events,
        nodes,
        edges,
        counts,
    ))
}

fn validate_window(
    root: &Map<String, Value>,
) -> Result<(usize, usize, usize, usize, [usize; 19]), DecodeError> {
    let snapshot = object(field(root, "snapshot")?)?;
    closed(
        snapshot,
        &[
            "snapshotId",
            "reducerVersion",
            "scope",
            "topology",
            "prefixes",
            "asOfMs",
        ],
    )?;
    id(field(snapshot, "snapshotId")?)?;
    if string(field(snapshot, "reducerVersion")?)? != REDUCER_VERSION {
        return Err(DecodeError::InvalidRecord);
    }
    snapshot_scope(field(snapshot, "scope")?)?;
    let topology = object(field(snapshot, "topology")?)?;
    closed(
        topology,
        &[
            "revisionId",
            "nodes",
            "edges",
            "policyRevisionIds",
            "coverageRevisionIds",
        ],
    )?;
    id(field(topology, "revisionId")?)?;
    let nodes = list(field(topology, "nodes")?, MAX_NODES)?;
    for node in nodes {
        topology_node(node)?;
    }
    let edges = list(field(topology, "edges")?, MAX_EDGES)?;
    for edge in edges {
        topology_edge(edge)?;
    }
    ids(field(topology, "policyRevisionIds")?)?;
    ids(field(topology, "coverageRevisionIds")?)?;
    let prefixes = list(field(snapshot, "prefixes")?, MAX_PRODUCERS)?;
    let mut producers = BTreeSet::new();
    let mut events = 0usize;
    let mut counts = [0usize; 19];
    for prefix in prefixes {
        events += prefix_record(prefix, &mut counts, &mut producers)?;
    }
    let tail = list(field(root, "tail")?, MAX_EVENTS)?;
    for event in tail {
        let producer = event_record(event, &mut counts)?;
        producers.insert(producer.to_owned());
        events += 1;
    }
    if producers.len() > MAX_PRODUCERS || events > MAX_EVENTS {
        return Err(DecodeError::CollectionLimit);
    }
    uint(field(snapshot, "asOfMs")?)?;
    Ok((nodes.len(), edges.len(), producers.len(), events, counts))
}

fn prefix_record(
    value: &Value,
    counts: &mut [usize; 19],
    producers: &mut BTreeSet<String>,
) -> Result<usize, DecodeError> {
    let o = object(value)?;
    closed(o, &["producerId", "fenceId", "watermark", "events"])?;
    let producer = id(field(o, "producerId")?)?;
    id(field(o, "fenceId")?)?;
    optional(field(o, "watermark")?, cursor)?;
    let events = list(field(o, "events")?, MAX_EVENTS)?;
    producers.insert(producer.to_owned());
    for event in events {
        producers.insert(event_record(event, counts)?.to_owned());
    }
    Ok(events.len())
}
fn event_record<'a>(value: &'a Value, counts: &mut [usize; 19]) -> Result<&'a str, DecodeError> {
    let o = object(value)?;
    closed(
        o,
        &[
            "eventId",
            "producerId",
            "fenceId",
            "sequence",
            "previousEventId",
            "observedMs",
            "receivedMs",
            "correlation",
            "record",
        ],
    )?;
    id(field(o, "eventId")?)?;
    let producer = id(field(o, "producerId")?)?;
    id(field(o, "fenceId")?)?;
    positive(field(o, "sequence")?)?;
    optional(field(o, "previousEventId")?, id_ok)?;
    optional(field(o, "observedMs")?, uint_ok)?;
    optional(field(o, "receivedMs")?, uint_ok)?;
    let kind = record(field(o, "record")?, counts)?;
    match field(o, "correlation")? {
        Value::Null if INVENTORY_KINDS.contains(&kind) => {}
        Value::Null => return Err(DecodeError::InvalidRecord),
        v => correlation(v)?,
    }
    Ok(producer)
}

fn record<'a>(v: &'a Value, counts: &mut [usize; 19]) -> Result<&'a str, DecodeError> {
    let o = object(v)?;
    let kind = string(field(o, "kind")?)?;
    let i = record_index(kind).ok_or(DecodeError::InvalidRecord)?;
    closed(o, record_fields(kind).ok_or(DecodeError::InvalidRecord)?)?;
    id(field(o, "id")?)?;
    match kind {
        "actor" => {
            id(field(o, "systemId")?)?;
            id(field(o, "canonicalSubjectId")?)?;
            one(field(o, "actorKind")?, &["human", "agent", "service"])?;
            optional(field(o, "identitySourceRef")?, external)?;
            optional(field(o, "identityEvidenceId")?, id_ok)?;
            verification(field(o, "reportedVerification")?)?;
            optional(field(o, "observedMs")?, uint_ok)?;
            freshness(field(o, "freshness")?)?
        }
        "resource" => {
            id(field(o, "systemId")?)?;
            token(field(o, "resourceClass")?)?;
            optional(field(o, "externalRef")?, external)?
        }
        "tool" => {
            id(field(o, "systemId")?)?;
            token(field(o, "operationClass")?)?;
            optional(field(o, "externalRef")?, external)?
        }
        "workflow" => {
            id(field(o, "systemId")?)?;
            external(field(o, "externalRef")?)?;
            externals(field(o, "capabilityRefs")?)?
        }
        "engine" => {
            id(field(o, "systemId")?)?;
            external(field(o, "externalRef")?)?;
            digest(field(o, "artifactDigest")?)?
        }
        "policy_revision" => {
            id(field(o, "gatewayId")?)?;
            optional(field(o, "previousRevisionId")?, id_ok)?;
            digest(field(o, "artifactDigest")?)?;
            optional(field(o, "externalRef")?, external)?
        }
        "gateway_revision" => {
            id(field(o, "gatewayId")?)?;
            optional(field(o, "previousRevisionId")?, id_ok)?;
            id(field(o, "policyRevisionId")?)?;
            scope(field(o, "scope")?)?;
            boolean(field(o, "humanRequired")?)?;
            id(field(o, "changedByActorId")?)?;
            one(
                field(o, "state")?,
                &[
                    "draft",
                    "reported_admitted",
                    "reported_active",
                    "reported_revoked",
                    "reported_suspended",
                    "unknown",
                ],
            )?;
            ids(field(o, "evidenceIds")?)?
        }
        "assignment" => {
            for k in [
                "actorId",
                "gatewayRevisionId",
                "policyRevisionId",
                "changedByActorId",
            ] {
                id(field(o, k)?)?;
            }
            optional(field(o, "delegationId")?, id_ok)?;
            one(
                field(o, "state")?,
                &["declared", "reported_active", "reported_revoked", "unknown"],
            )?;
            ids(field(o, "evidenceIds")?)?
        }
        "delegation" => {
            for k in ["delegatorId", "delegateId", "changedByActorId"] {
                id(field(o, k)?)?;
            }
            optional(field(o, "parentDelegationId")?, id_ok)?;
            scope(field(o, "scope")?)?;
            optional(field(o, "expiresMs")?, uint_ok)?;
            one(
                field(o, "state")?,
                &["declared", "reported_active", "reported_revoked", "unknown"],
            )?;
            optional(field(o, "externalRef")?, external)?;
            verification(field(o, "reportedVerification")?)?;
            optional(field(o, "observedMs")?, uint_ok)?;
            freshness(field(o, "freshness")?)?;
            ids(field(o, "evidenceIds")?)?
        }
        "request" => {
            context(field(o, "context")?)?;
            for k in ["actorId", "gatewayRevisionId"] {
                id(field(o, k)?)?;
            }
            token(field(o, "operationClass")?)?;
            digest(field(o, "actionDigest")?)?;
            digest(field(o, "contextDigest")?)?;
            ids(field(o, "resourceIds")?)?;
            optional(field(o, "workflowRef")?, external)?;
            optional(field(o, "expiresMs")?, uint_ok)?
        }
        "evaluation" => {
            context(field(o, "context")?)?;
            one(field(o, "evaluationKind")?, &["initial", "successor"])?;
            for k in [
                "gatewayRevisionId",
                "evaluatorId",
                "inputSnapshotId",
                "contextSnapshotId",
                "lineageFenceId",
            ] {
                id(field(o, k)?)?;
            }
            digest(field(o, "evaluatorArtifactDigest")?)?;
            token(field(o, "inputCanonicalization")?)?;
            for k in ["inputDigest", "contextDigest"] {
                digest(field(o, k)?)?;
            }
            optional(field(o, "previousEvaluationId")?, id_ok)?;
            optional(field(o, "expectedEffectiveEvaluationId")?, id_ok)?;
            one(field(o, "result")?, &["allow", "deny", "unknown"])?;
            optional(field(o, "expiresMs")?, uint_ok)?;
            reason(field(o, "reason")?)?;
            optional(field(o, "externalRef")?, external)?;
            ids(field(o, "evidenceIds")?)?
        }
        "approval_plan" => {
            context(field(o, "context")?)?;
            id(field(o, "evaluationId")?)?;
            optional(field(o, "previousPlanId")?, id_ok)?;
            id(field(o, "changedByActorId")?)?;
            optional(field(o, "expiresMs")?, uint_ok)?;
            stages(field(o, "stages")?)?;
            ids(field(o, "evidenceIds")?)?
        }
        "approval_decision" => {
            context(field(o, "context")?)?;
            for k in ["evaluationId", "planId", "stageId", "actorId"] {
                id(field(o, k)?)?;
            }
            optional(field(o, "delegationId")?, id_ok)?;
            one(field(o, "decision")?, &["approve", "deny", "abstain"])?;
            optional(field(o, "supersedesDecisionId")?, id_ok)?;
            reason(field(o, "reason")?)?;
            ids(field(o, "evidenceIds")?)?
        }
        "authorization" => {
            context(field(o, "context")?)?;
            id(field(o, "evaluationId")?)?;
            optional(field(o, "planId")?, id_ok)?;
            ids(field(o, "decisionIds")?)?;
            optional(field(o, "previousObservationId")?, id_ok)?;
            one(
                field(o, "state")?,
                &[
                    "reported_issued",
                    "reported_refused",
                    "reported_expired",
                    "reported_revoked",
                    "unknown",
                ],
            )?;
            scope(field(o, "scope")?)?;
            optional(field(o, "expiresMs")?, uint_ok)?;
            reason(field(o, "reason")?)?;
            optional(field(o, "externalRef")?, external)?;
            ids(field(o, "evidenceIds")?)?
        }
        "attempt" => {
            context(field(o, "context")?)?;
            optional(field(o, "authorizationId")?, id_ok)?;
            id(field(o, "executorId")?)?;
            optional(field(o, "previousObservationId")?, id_ok)?;
            id(field(o, "attemptId")?)?;
            one(
                field(o, "state")?,
                &[
                    "reported_accepted",
                    "reported_active",
                    "reported_completed",
                    "reported_failed",
                    "reported_cancelled",
                    "outcome_unknown",
                    "reported_reconciled_completed",
                    "reported_reconciled_failed",
                ],
            )?;
            reason(field(o, "reason")?)?;
            ids(field(o, "evidenceIds")?)?
        }
        "suspension" => {
            id(field(o, "gatewayRevisionId")?)?;
            id(field(o, "requestedByActorId")?)?;
            scope(field(o, "scope")?)?;
            one(
                field(o, "state")?,
                &[
                    "requested",
                    "reported_acknowledged",
                    "reported_partial",
                    "unknown",
                    "reported_resumed",
                ],
            )?;
            optional(field(o, "previousObservationId")?, id_ok)?;
            reason(field(o, "reason")?)?;
            ids(field(o, "evidenceIds")?)?
        }
        "metric" => {
            context(field(o, "context")?)?;
            optional(field(o, "attemptId")?, id_ok)?;
            ids(field(o, "resourceIds")?)?;
            optional(field(o, "latencyMs")?, uint_ok)?;
            optional(field(o, "cost")?, money)?;
            reason(field(o, "reason")?)?
        }
        "evidence" => {
            id(field(o, "producerId")?)?;
            optional(field(o, "externalRef")?, external)?;
            optional(field(o, "artifactDigest")?, digest)?;
            one(field(o, "state")?, &["unverified"])?;
            reason(field(o, "reason")?)?
        }
        "coverage" => {
            for k in ["systemId", "engineId", "gatewayRevisionId", "environmentId"] {
                id(field(o, k)?)?;
            }
            token(field(o, "operationClass")?)?;
            one(
                field(o, "targetOs")?,
                &["macos", "windows", "linux", "other", "unknown"],
            )?;
            optional(field(o, "observedMs")?, uint_ok)?;
            freshness(field(o, "freshness")?)?;
            ids(field(o, "targetResourceIds")?)?;
            digest(field(o, "adapterArtifactDigest")?)?;
            digest(field(o, "engineArtifactDigest")?)?;
            one(
                field(o, "reportedLevel")?,
                &[
                    "unknown",
                    "unavailable",
                    "advisory",
                    "partial_mediation",
                    "fully_mediated",
                ],
            )?;
            checks(field(o, "checks")?)?;
            optional(field(o, "expiresMs")?, uint_ok)?;
            ids(field(o, "evidenceIds")?)?
        }
        _ => unreachable!(),
    }
    counts[i] += 1;
    Ok(kind)
}

fn scope(v: &Value) -> Result<(), DecodeError> {
    let o = object(v)?;
    closed(
        o,
        &[
            "systemId",
            "environmentIds",
            "operationClasses",
            "resourceIds",
            "policyRevisionIds",
        ],
    )?;
    id(field(o, "systemId")?)?;
    ids(field(o, "environmentIds")?)?;
    tokens(field(o, "operationClasses")?)?;
    ids(field(o, "resourceIds")?)?;
    ids(field(o, "policyRevisionIds")?)?;
    Ok(())
}
fn snapshot_scope(v: &Value) -> Result<(), DecodeError> {
    let o = object(v)?;
    closed(
        o,
        &["workspaceId", "systemIds", "environmentIds", "engineIds"],
    )?;
    id(field(o, "workspaceId")?)?;
    ids(field(o, "systemIds")?)?;
    ids(field(o, "environmentIds")?)?;
    ids(field(o, "engineIds")?)?;
    Ok(())
}
fn context(v: &Value) -> Result<(), DecodeError> {
    let o = object(v)?;
    closed(
        o,
        &[
            "workspaceId",
            "systemId",
            "environmentId",
            "engineId",
            "requestId",
            "traceId",
            "spanId",
            "policyRevisionId",
        ],
    )?;
    for k in [
        "workspaceId",
        "systemId",
        "environmentId",
        "engineId",
        "requestId",
        "traceId",
        "spanId",
        "policyRevisionId",
    ] {
        id(field(o, k)?)?;
    }
    Ok(())
}
fn correlation(v: &Value) -> Result<(), DecodeError> {
    let o = object(v)?;
    closed(
        o,
        &[
            "workspaceId",
            "systemId",
            "environmentId",
            "engineId",
            "traceId",
            "spanId",
            "requestId",
            "evaluationId",
            "attemptId",
            "policyRevisionId",
        ],
    )?;
    for k in [
        "workspaceId",
        "systemId",
        "environmentId",
        "engineId",
        "traceId",
        "spanId",
    ] {
        id(field(o, k)?)?;
    }
    for k in ["requestId", "evaluationId", "attemptId", "policyRevisionId"] {
        optional(field(o, k)?, id_ok)?;
    }
    Ok(())
}
fn topology_node(v: &Value) -> Result<(), DecodeError> {
    let o = object(v)?;
    closed(o, &["id", "kind", "recordId"])?;
    id(field(o, "id")?)?;
    one(
        field(o, "kind")?,
        &[
            "actor",
            "workflow",
            "tool",
            "resource",
            "gateway",
            "engine",
            "approval_stage",
        ],
    )?;
    id(field(o, "recordId")?)?;
    Ok(())
}
fn topology_edge(v: &Value) -> Result<(), DecodeError> {
    let o = object(v)?;
    closed(o, &["id", "kind", "from", "to"])?;
    id(field(o, "id")?)?;
    one(
        field(o, "kind")?,
        &[
            "declared_dependency",
            "request_flow",
            "delegation_observation",
            "mediation_observation",
            "evidence_lineage",
        ],
    )?;
    id(field(o, "from")?)?;
    id(field(o, "to")?)?;
    Ok(())
}
fn cursor(v: &Value) -> Result<(), DecodeError> {
    let o = object(v)?;
    closed(o, &["sequence", "eventId"])?;
    positive(field(o, "sequence")?)?;
    id(field(o, "eventId")?)?;
    Ok(())
}
fn external(v: &Value) -> Result<(), DecodeError> {
    let o = object(v)?;
    closed(o, &["systemId", "contractVersion", "externalId"])?;
    id(field(o, "systemId")?)?;
    token(field(o, "contractVersion")?)?;
    token(field(o, "externalId")?)
}
fn externals(v: &Value) -> Result<(), DecodeError> {
    for x in list(v, MAX_LIST)? {
        external(x)?;
    }
    Ok(())
}
fn stages(v: &Value) -> Result<(), DecodeError> {
    for x in list(v, MAX_LIST)? {
        let o = object(x)?;
        closed(
            o,
            &[
                "id",
                "dependsOnStageIds",
                "approverIds",
                "humanRequired",
                "mode",
                "quorum",
                "expiresMs",
                "escalationActorIds",
            ],
        )?;
        id(field(o, "id")?)?;
        ids(field(o, "dependsOnStageIds")?)?;
        ids(field(o, "approverIds")?)?;
        boolean(field(o, "humanRequired")?)?;
        one(field(o, "mode")?, &["all", "quorum"])?;
        optional(field(o, "quorum")?, uint_ok)?;
        optional(field(o, "expiresMs")?, uint_ok)?;
        ids(field(o, "escalationActorIds")?)?;
    }
    Ok(())
}
fn money(v: &Value) -> Result<(), DecodeError> {
    let o = object(v)?;
    closed(o, &["kind", "currency", "amountMicros", "sourceEvidenceId"])?;
    one(field(o, "kind")?, &["estimate", "observed", "final"])?;
    let c = string(field(o, "currency")?)?;
    if c.len() != 3 || !c.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err(DecodeError::InvalidRecord);
    };
    uint(field(o, "amountMicros")?)?;
    optional(field(o, "sourceEvidenceId")?, id_ok)
}
fn checks(v: &Value) -> Result<(), DecodeError> {
    let o = object(v)?;
    let f = [
        "policyDecision",
        "approvalDuties",
        "agentDelegation",
        "expiryRevocation",
        "executionConsumption",
        "replayProtection",
        "outcomeEvidence",
        "reconciliation",
    ];
    closed(o, &f)?;
    for k in f {
        one(field(o, k)?, &["yes", "no", "unknown"])?;
    }
    Ok(())
}
fn verification(v: &Value) -> Result<(), DecodeError> {
    one(
        v,
        &[
            "unknown",
            "reported_unverified",
            "reported_verified",
            "reported_rejected",
        ],
    )
}
fn freshness(v: &Value) -> Result<(), DecodeError> {
    one(v, &["unknown", "reported_current", "reported_stale"])
}
fn reason(v: &Value) -> Result<(), DecodeError> {
    one(
        v,
        &[
            "pending",
            "policy_denied",
            "scope_mismatch",
            "human_required",
            "delegation_missing",
            "unsupported",
            "expired",
            "revoked",
            "suspended",
            "stale",
            "conflict",
            "unknown_outcome",
            "failed",
            "cancelled",
            "none",
        ],
    )
}
fn ids(v: &Value) -> Result<(), DecodeError> {
    for x in list(v, MAX_LIST)? {
        id(x)?;
    }
    Ok(())
}
fn tokens(v: &Value) -> Result<(), DecodeError> {
    for x in list(v, MAX_LIST)? {
        token(x)?;
    }
    Ok(())
}
fn id(v: &Value) -> Result<&str, DecodeError> {
    let s = string(v)?;
    if s.len() != 68
        || !s.starts_with("ops:")
        || !s[4..]
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    {
        return Err(DecodeError::InvalidRecord);
    }
    Ok(s)
}
fn digest(v: &Value) -> Result<(), DecodeError> {
    let s = string(v)?;
    if s.len() != 64
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    {
        return Err(DecodeError::InvalidRecord);
    }
    Ok(())
}
fn token(v: &Value) -> Result<(), DecodeError> {
    let s = string(v)?;
    if s.is_empty()
        || s.len() > 128
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b':' | b'-'))
    {
        return Err(DecodeError::InvalidRecord);
    }
    Ok(())
}
fn uint(v: &Value) -> Result<u64, DecodeError> {
    v.as_u64()
        .filter(|n| *n <= MAX_INTEGER)
        .ok_or(DecodeError::InvalidRecord)
}
fn positive(v: &Value) -> Result<u64, DecodeError> {
    let n = uint(v)?;
    if n == 0 {
        Err(DecodeError::InvalidRecord)
    } else {
        Ok(n)
    }
}
fn boolean(v: &Value) -> Result<(), DecodeError> {
    if v.is_boolean() {
        Ok(())
    } else {
        Err(DecodeError::InvalidRecord)
    }
}
fn one(v: &Value, allowed: &[&str]) -> Result<(), DecodeError> {
    if allowed.contains(&string(v)?) {
        Ok(())
    } else {
        Err(DecodeError::InvalidRecord)
    }
}
fn optional(v: &Value, f: fn(&Value) -> Result<(), DecodeError>) -> Result<(), DecodeError> {
    if v.is_null() { Ok(()) } else { f(v) }
}
fn id_ok(v: &Value) -> Result<(), DecodeError> {
    id(v).map(|_| ())
}
fn uint_ok(v: &Value) -> Result<(), DecodeError> {
    uint(v).map(|_| ())
}
fn object(v: &Value) -> Result<&Map<String, Value>, DecodeError> {
    v.as_object().ok_or(DecodeError::InvalidRecord)
}
fn list(v: &Value, max: usize) -> Result<&Vec<Value>, DecodeError> {
    let a = v.as_array().ok_or(DecodeError::InvalidRecord)?;
    if a.len() > max {
        Err(DecodeError::CollectionLimit)
    } else {
        Ok(a)
    }
}
fn string(v: &Value) -> Result<&str, DecodeError> {
    v.as_str().ok_or(DecodeError::InvalidRecord)
}
fn field<'a>(o: &'a Map<String, Value>, name: &str) -> Result<&'a Value, DecodeError> {
    o.get(name).ok_or(DecodeError::InvalidRecord)
}
fn closed(o: &Map<String, Value>, fields: &[&str]) -> Result<(), DecodeError> {
    if o.len() != fields.len() || fields.iter().any(|k| !o.contains_key(*k)) {
        Err(DecodeError::InvalidRecord)
    } else {
        Ok(())
    }
}

fn scan_raw(source: &str) -> Result<(), DecodeError> {
    let mut s = Scanner {
        b: source.as_bytes(),
        p: 0,
        values: 0,
    };
    s.ws();
    s.value(1)?;
    s.ws();
    if s.p != s.b.len() {
        return Err(DecodeError::InvalidJson);
    }
    Ok(())
}
struct Scanner<'a> {
    b: &'a [u8],
    p: usize,
    values: usize,
}
impl Scanner<'_> {
    fn ws(&mut self) {
        while self.p < self.b.len() && self.b[self.p].is_ascii_whitespace() {
            self.p += 1
        }
    }
    fn value(&mut self, d: usize) -> Result<(), DecodeError> {
        if d > MAX_DEPTH {
            return Err(DecodeError::DepthLimit);
        }
        self.values += 1;
        if self.values > MAX_VALUES {
            return Err(DecodeError::ValueLimit);
        }
        self.ws();
        match self.b.get(self.p) {
            Some(b'{') => self.obj(d),
            Some(b'[') => self.arr(d),
            Some(b'\"') => {
                self.str()?;
                Ok(())
            }
            Some(b't') => self.word(b"true"),
            Some(b'f') => self.word(b"false"),
            Some(b'n') => self.word(b"null"),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(DecodeError::InvalidJson),
        }
    }
    fn obj(&mut self, d: usize) -> Result<(), DecodeError> {
        self.p += 1;
        self.ws();
        let mut keys = BTreeSet::new();
        let mut n = 0;
        if self.b.get(self.p) == Some(&b'}') {
            self.p += 1;
            return Ok(());
        }
        loop {
            self.ws();
            let start = self.p;
            self.str()?;
            let key: String = serde_json::from_slice(&self.b[start..self.p])
                .map_err(|_| DecodeError::InvalidJson)?;
            if !keys.insert(key) {
                return Err(DecodeError::DuplicateField);
            }
            n += 1;
            if n > MAX_OBJECT_FIELDS {
                return Err(DecodeError::CollectionLimit);
            }
            self.ws();
            if self.b.get(self.p) != Some(&b':') {
                return Err(DecodeError::InvalidJson);
            }
            self.p += 1;
            self.value(d + 1)?;
            self.ws();
            match self.b.get(self.p) {
                Some(b',') => self.p += 1,
                Some(b'}') => {
                    self.p += 1;
                    return Ok(());
                }
                _ => return Err(DecodeError::InvalidJson),
            }
        }
    }
    fn arr(&mut self, d: usize) -> Result<(), DecodeError> {
        self.p += 1;
        self.ws();
        let mut n = 0;
        if self.b.get(self.p) == Some(&b']') {
            self.p += 1;
            return Ok(());
        }
        loop {
            n += 1;
            if n > MAX_RAW_ARRAY {
                return Err(DecodeError::CollectionLimit);
            }
            self.value(d + 1)?;
            self.ws();
            match self.b.get(self.p) {
                Some(b',') => self.p += 1,
                Some(b']') => {
                    self.p += 1;
                    return Ok(());
                }
                _ => return Err(DecodeError::InvalidJson),
            }
        }
    }
    fn str(&mut self) -> Result<(), DecodeError> {
        if self.b.get(self.p) != Some(&b'\"') {
            return Err(DecodeError::InvalidJson);
        }
        self.p += 1;
        while let Some(&c) = self.b.get(self.p) {
            match c {
                b'\"' => {
                    self.p += 1;
                    return Ok(());
                }
                b'\\' => {
                    self.p += 1;
                    match self.b.get(self.p) {
                        Some(b'\"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => {
                            self.p += 1
                        }
                        Some(b'u') => {
                            self.p += 1;
                            for _ in 0..4 {
                                if !self.b.get(self.p).is_some_and(uhex) {
                                    return Err(DecodeError::InvalidJson);
                                }
                                self.p += 1
                            }
                        }
                        _ => return Err(DecodeError::InvalidJson),
                    }
                }
                0..=31 => return Err(DecodeError::InvalidJson),
                _ => self.p += 1,
            }
        }
        Err(DecodeError::InvalidJson)
    }
    fn word(&mut self, w: &[u8]) -> Result<(), DecodeError> {
        if self.b.get(self.p..self.p + w.len()) == Some(w) {
            self.p += w.len();
            Ok(())
        } else {
            Err(DecodeError::InvalidJson)
        }
    }
    fn number(&mut self) -> Result<(), DecodeError> {
        let start = self.p;
        if self.b.get(self.p) == Some(&b'-') {
            self.p += 1
        }
        if self.b.get(self.p) == Some(&b'0') {
            self.p += 1
        } else {
            let p = self.p;
            while self.b.get(self.p).is_some_and(|c| c.is_ascii_digit()) {
                self.p += 1
            }
            if p == self.p {
                return Err(DecodeError::InvalidJson);
            }
        }
        if self.b.get(self.p) == Some(&b'.') {
            self.p += 1;
            let p = self.p;
            while self.b.get(self.p).is_some_and(|c| c.is_ascii_digit()) {
                self.p += 1
            }
            if p == self.p {
                return Err(DecodeError::InvalidJson);
            }
        }
        if matches!(self.b.get(self.p), Some(b'e' | b'E')) {
            self.p += 1;
            if matches!(self.b.get(self.p), Some(b'+' | b'-')) {
                self.p += 1
            }
            let p = self.p;
            while self.b.get(self.p).is_some_and(|c| c.is_ascii_digit()) {
                self.p += 1
            }
            if p == self.p {
                return Err(DecodeError::InvalidJson);
            }
        }
        if self.p == start {
            Err(DecodeError::InvalidJson)
        } else {
            Ok(())
        }
    }
}
fn uhex(c: &u8) -> bool {
    c.is_ascii_hexdigit()
}
