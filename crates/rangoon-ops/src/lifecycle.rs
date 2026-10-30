//! Pure, bounded inspection of reported lifecycle observations.

use crate::approval_types::ApprovalError;
use crate::delegation_types::DelegationError;
use crate::lifecycle_types::*;
use crate::projection_types::ProjectionError;
use crate::records::{Event, Window};
use crate::relationship_types::{ReferenceLink, RelationshipError};
use crate::{DecodedWindow, inspect_approvals, inspect_delegations, project_window, relate_window};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

const CODES: [&str; 22] = [
    "structural_incomplete",
    "reference_missing",
    "reference_ambiguous",
    "reference_kind_mismatch",
    "context_mismatch",
    "binding_mismatch",
    "predecessor_missing",
    "predecessor_ambiguous",
    "predecessor_wrong_kind",
    "sequence_mismatch",
    "stream_mismatch",
    "multiple_roots",
    "branch",
    "cycle",
    "state_transition_invalid",
    "effect_unknown",
    "reconciliation_evidence_missing",
    "coverage_freshness_unknown",
    "coverage_stale",
    "coverage_expired",
    "coverage_conflict",
    "coverage_evidence_missing",
];
const STREAM_TAINT: [&str; 11] = [
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
];
const FLOW_KEYS: [&str; 7] = [
    "workspaceId",
    "systemId",
    "environmentId",
    "engineId",
    "requestId",
    "traceId",
    "policyRevisionId",
];
type Flags = [bool; 22];

struct Observation<'a> {
    event: &'a Event,
    value: Value,
    bytes: Vec<u8>,
}

impl Observation<'_> {
    fn text(&self, key: &str) -> Option<&str> {
        self.value.get(key).and_then(Value::as_str)
    }
    fn id(&self) -> Option<&str> {
        self.text("id")
    }
    fn kind(&self) -> Option<&str> {
        self.text("kind")
    }
    fn selected(&self) -> bool {
        matches!(
            self.kind(),
            Some("authorization" | "attempt" | "suspension" | "metric" | "evidence" | "coverage")
        )
    }
    fn predecessor(&self) -> Option<&str> {
        self.value
            .get("previousObservationId")
            .and_then(Value::as_str)
    }
}

#[derive(Default)]
struct IndexedRecord {
    first: usize,
    kind: String,
    bytes: Vec<u8>,
    ordinals: Vec<usize>,
    ambiguous: bool,
    streams: BTreeSet<(String, String)>,
}

#[derive(Default)]
struct Budget {
    references: usize,
    matches: usize,
    steps: usize,
}

impl Budget {
    fn reserve(
        &mut self,
        references: usize,
        matches: usize,
        steps: usize,
    ) -> Result<(), LifecycleError> {
        let next = [
            self.references.checked_add(references),
            self.matches.checked_add(matches),
            self.steps.checked_add(steps),
        ];
        let Some(next_refs) = next[0] else {
            return Err(LifecycleError::Limit);
        };
        let Some(next_matches) = next[1] else {
            return Err(LifecycleError::Limit);
        };
        let Some(next_steps) = next[2] else {
            return Err(LifecycleError::Limit);
        };
        if next_refs > MAX_LIFECYCLE_REFERENCES
            || next_matches > MAX_LIFECYCLE_MATCHES
            || next_steps > MAX_LIFECYCLE_PATH_STEPS
        {
            return Err(LifecycleError::Limit);
        }
        self.references = next_refs;
        self.matches = next_matches;
        self.steps = next_steps;
        Ok(())
    }
}

struct LimitedWriter {
    bytes: Vec<u8>,
    limit_hit: bool,
}

impl LimitedWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            limit_hit: false,
        }
    }
}

impl Write for LimitedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_LIFECYCLE_BYTES.saturating_sub(self.bytes.len()) {
            self.limit_hit = true;
            return Err(std::io::Error::other("lifecycle_limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn bounded_json<T: Serialize>(value: &T) -> Result<Vec<u8>, LifecycleError> {
    let mut writer = LimitedWriter::new();
    if serde_json::to_writer(&mut writer, value).is_err() {
        return Err(if writer.limit_hit {
            LifecycleError::Limit
        } else {
            LifecycleError::SerializationFailed
        });
    }
    Ok(writer.bytes)
}

fn code_index(code: &str) -> Option<usize> {
    CODES.iter().position(|candidate| *candidate == code)
}
fn mark(flags: &mut Flags, code: &str) {
    if let Some(index) = code_index(code) {
        flags[index] = true;
    }
}
fn codes(flags: &Flags) -> Vec<&'static str> {
    CODES
        .iter()
        .zip(flags)
        .filter_map(|(code, set)| set.then_some(*code))
        .collect()
}
fn union(into: &mut Flags, from: &Flags) {
    for (a, b) in into.iter_mut().zip(from) {
        *a |= *b;
    }
}
fn incomplete(index: usize) -> bool {
    matches!(index, 0 | 1 | 3 | 6 | 8 | 15 | 16 | 17 | 18 | 19 | 21)
}
fn assessment(flags: &Flags) -> &'static str {
    if flags
        .iter()
        .enumerate()
        .any(|(index, flag)| *flag && !incomplete(index))
    {
        "conflict"
    } else if flags.iter().any(|flag| *flag) {
        "incomplete"
    } else {
        "linked_unverified"
    }
}
fn selected_assessment(selected: bool, flags: &Flags) -> &'static str {
    if selected {
        assessment(flags)
    } else {
        "not_assessed"
    }
}

fn flow_equal(a: &Value, b: &Value) -> bool {
    let Some(a) = a.get("context") else {
        return false;
    };
    let Some(b) = b.get("context") else {
        return false;
    };
    FLOW_KEYS.iter().all(|key| a.get(*key) == b.get(*key))
}
fn stream_key<'a>(item: &Observation<'a>) -> (&'a str, &'a str) {
    (&item.event.producer_id, &item.event.fence_id)
}
fn earliest_sequence(record: &IndexedRecord, observations: &[Observation<'_>]) -> Option<u64> {
    record
        .ordinals
        .iter()
        .map(|ordinal| observations[*ordinal].event.sequence)
        .min()
}

fn status(index: &BTreeMap<String, IndexedRecord>, id: &str, expected: &str) -> &'static str {
    match index.get(id) {
        None => "missing",
        Some(record) if record.ambiguous => "ambiguous",
        Some(record) if record.kind != expected => "wrong_kind",
        Some(_) => "unique_unverified",
    }
}
fn status_flags(flags: &mut Flags, state: &str) {
    match state {
        "missing" => mark(flags, "reference_missing"),
        "ambiguous" => mark(flags, "reference_ambiguous"),
        "wrong_kind" => mark(flags, "reference_kind_mismatch"),
        _ => (),
    }
}

struct ReferenceRequest<'a> {
    field: &'static str,
    list_index: Option<usize>,
    id: &'a str,
    expected: &'static str,
}

fn reference(
    index: &BTreeMap<String, IndexedRecord>,
    tainted: &[bool],
    budget: &mut Budget,
    flags: &mut Flags,
    request: ReferenceRequest<'_>,
) -> Result<ReferenceLink, LifecycleError> {
    let ordinals = index
        .get(request.id)
        .map(|record| record.ordinals.as_slice())
        .unwrap_or(&[]);
    budget.reserve(1, ordinals.len(), 0)?;
    let state = status(index, request.id, request.expected);
    status_flags(flags, state);
    if ordinals.iter().any(|ordinal| tainted[*ordinal]) {
        mark(flags, "structural_incomplete");
    }
    Ok(ReferenceLink {
        field: request.field,
        index: request.list_index,
        record_id: request.id.to_owned(),
        expected_kind: request.expected,
        status: state,
        matching_event_ordinals: ordinals.to_vec(),
    })
}

fn attempt_reference(
    observations: &[Observation<'_>],
    index: &BTreeMap<String, IndexedRecord>,
    tainted: &[bool],
    budget: &mut Budget,
    flags: &mut Flags,
    source: &Observation<'_>,
    selector: &str,
) -> Result<ReferenceLink, LifecycleError> {
    let ordinals: Vec<usize> = observations
        .iter()
        .enumerate()
        .filter_map(|(ordinal, item)| {
            (item.kind() == Some("attempt") && item.text("attemptId") == Some(selector))
                .then_some(ordinal)
        })
        .collect();
    budget.reserve(1, ordinals.len(), 0)?;
    if ordinals.iter().any(|ordinal| tainted[*ordinal]) {
        mark(flags, "structural_incomplete");
    }
    let mut ids = BTreeSet::new();
    let mut bindings = BTreeSet::new();
    for ordinal in &ordinals {
        if let Some(id) = observations[*ordinal].id() {
            ids.insert(id.to_owned());
        }
        bindings.insert(binding(&observations[*ordinal]));
    }
    let state = if ordinals.is_empty() {
        "missing"
    } else if !coherent_attempt_group(observations, index, tainted, &ordinals, &ids, &bindings) {
        "ambiguous"
    } else {
        "unique_unverified"
    };
    status_flags(flags, state);
    if state == "unique_unverified"
        && ordinals
            .first()
            .is_some_and(|ordinal| !flow_equal(&source.value, &observations[*ordinal].value))
    {
        mark(flags, "context_mismatch");
    }
    Ok(ReferenceLink {
        field: "attemptId",
        index: None,
        record_id: selector.to_owned(),
        expected_kind: "attempt",
        status: state,
        matching_event_ordinals: ordinals,
    })
}

fn coherent_attempt_group(
    observations: &[Observation<'_>],
    index: &BTreeMap<String, IndexedRecord>,
    tainted: &[bool],
    ordinals: &[usize],
    ids: &BTreeSet<String>,
    bindings: &BTreeSet<String>,
) -> bool {
    if ids.is_empty() || bindings.len() != 1 {
        return false;
    }
    let mut nodes = BTreeMap::<&str, Vec<usize>>::new();
    for ordinal in ordinals {
        let item = &observations[*ordinal];
        let Some(id) = item.id() else { return false };
        nodes.entry(id).or_default().push(*ordinal);
    }
    let mut roots = 0usize;
    let mut successors = BTreeMap::<&str, usize>::new();
    for (id, node_ordinals) in &nodes {
        let first = &observations[node_ordinals[0]];
        if node_ordinals.iter().any(|ordinal| {
            let item = &observations[*ordinal];
            tainted[*ordinal] || item.bytes != first.bytes || stream_key(item) != stream_key(first)
        }) {
            return false;
        }
        if first
            .text("authorizationId")
            .is_some_and(|id| status(index, id, "authorization") != "unique_unverified")
            || !attempt_evidence_coherent(first, index)
        {
            return false;
        }
        match first.predecessor() {
            None => {
                if !transition_ok(
                    "attempt",
                    None,
                    first.text("state").unwrap_or_default(),
                    false,
                ) {
                    return false;
                }
                roots += 1;
            }
            Some(parent) => {
                let Some(parent_ordinals) = nodes.get(parent) else {
                    return false;
                };
                let parent_item = &observations[parent_ordinals[0]];
                let Some(first_sequence) = node_ordinals
                    .iter()
                    .map(|ordinal| observations[*ordinal].event.sequence)
                    .min()
                else {
                    return false;
                };
                let Some(parent_sequence) = parent_ordinals
                    .iter()
                    .map(|ordinal| observations[*ordinal].event.sequence)
                    .min()
                else {
                    return false;
                };
                if stream_key(first) != stream_key(parent_item)
                    || first_sequence <= parent_sequence
                    || binding(first) != binding(parent_item)
                    || !transition_ok(
                        "attempt",
                        parent_item.text("state"),
                        first.text("state").unwrap_or_default(),
                        first
                            .value
                            .get("evidenceIds")
                            .and_then(Value::as_array)
                            .is_some_and(|ids| {
                                ids.iter().any(|entry| {
                                    entry.as_str().is_some_and(|id| {
                                        status(index, id, "evidence") == "unique_unverified"
                                    })
                                })
                            }),
                    )
                {
                    return false;
                }
                let count = successors.entry(parent).or_default();
                *count += 1;
                if *count > 1 {
                    return false;
                }
            }
        }
        if id.is_empty() {
            return false;
        }
    }
    if roots != 1 {
        return false;
    }
    for id in nodes.keys() {
        let mut current = *id;
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert(current) {
                return false;
            }
            let Some(node) = nodes.get(current).and_then(|matches| matches.first()) else {
                return false;
            };
            let Some(parent) = observations[*node].predecessor() else {
                break;
            };
            current = parent;
        }
    }
    true
}

fn attempt_evidence_coherent(
    item: &Observation<'_>,
    index: &BTreeMap<String, IndexedRecord>,
) -> bool {
    let Some(ids) = item.value.get("evidenceIds").and_then(Value::as_array) else {
        return false;
    };
    let unique = |entry: &Value| {
        entry
            .as_str()
            .is_some_and(|id| status(index, id, "evidence") == "unique_unverified")
    };
    if matches!(
        item.text("state"),
        Some("reported_reconciled_completed" | "reported_reconciled_failed")
    ) {
        ids.iter().any(unique)
    } else {
        ids.iter().all(unique)
    }
}

fn scope_json(value: &Value) -> Result<String, LifecycleError> {
    let scope = value
        .get("scope")
        .ok_or(LifecycleError::SerializationFailed)?;
    let encode = |field: &str| {
        serde_json::to_string(scope.get(field).unwrap_or(&Value::Null))
            .map_err(|_| LifecycleError::SerializationFailed)
    };
    Ok(format!(
        "{{\"systemId\":{},\"environmentIds\":{},\"operationClasses\":{},\"resourceIds\":{},\"policyRevisionIds\":{}}}",
        encode("systemId")?,
        encode("environmentIds")?,
        encode("operationClasses")?,
        encode("resourceIds")?,
        encode("policyRevisionIds")?
    ))
}
fn json_string(value: Option<&str>) -> Result<String, LifecycleError> {
    serde_json::to_string(&value).map_err(|_| LifecycleError::SerializationFailed)
}
fn group_id(item: &Observation<'_>) -> Result<Option<(u8, String)>, LifecycleError> {
    let Some(kind) = item.kind() else {
        return Ok(None);
    };
    match kind {
        "attempt" => Ok(item.text("attemptId").map(|id| (1, id.to_owned()))),
        "authorization" => {
            let context = item
                .value
                .get("context")
                .ok_or(LifecycleError::SerializationFailed)?;
            let mut values = vec![json_string(Some("authorization"))?];
            for key in FLOW_KEYS {
                values.push(json_string(context.get(key).and_then(Value::as_str))?);
            }
            values.push(json_string(item.text("evaluationId"))?);
            values.push(json_string(
                item.value.get("planId").and_then(Value::as_str),
            )?);
            values.push(scope_json(&item.value)?);
            let compact = format!("[{}]", values.join(","));
            Ok(Some((
                0,
                format!("ops:{}", rangoon_domain::byte_digest(compact.as_bytes())),
            )))
        }
        "suspension" => {
            let scope = scope_json(&item.value)?;
            let compact = format!(
                "[{},{},{}]",
                json_string(Some("suspension"))?,
                json_string(item.text("gatewayRevisionId"))?,
                scope
            );
            Ok(Some((
                2,
                format!("ops:{}", rangoon_domain::byte_digest(compact.as_bytes())),
            )))
        }
        _ => Ok(None),
    }
}

fn binding(item: &Observation<'_>) -> String {
    let mut values = Vec::new();
    match item.kind() {
        Some("authorization") => {
            let context = item.value.get("context");
            for key in FLOW_KEYS {
                values.push(
                    context
                        .and_then(|v| v.get(key))
                        .cloned()
                        .unwrap_or(Value::Null),
                );
            }
            values.push(
                item.value
                    .get("evaluationId")
                    .cloned()
                    .unwrap_or(Value::Null),
            );
            values.push(item.value.get("planId").cloned().unwrap_or(Value::Null));
            values.push(item.value.get("scope").cloned().unwrap_or(Value::Null));
        }
        Some("attempt") => {
            let context = item.value.get("context");
            for key in FLOW_KEYS {
                values.push(
                    context
                        .and_then(|v| v.get(key))
                        .cloned()
                        .unwrap_or(Value::Null),
                );
            }
            values.push(item.value.get("attemptId").cloned().unwrap_or(Value::Null));
            values.push(
                item.value
                    .get("authorizationId")
                    .cloned()
                    .unwrap_or(Value::Null),
            );
            values.push(item.value.get("executorId").cloned().unwrap_or(Value::Null));
        }
        Some("suspension") => {
            values.push(
                item.value
                    .get("gatewayRevisionId")
                    .cloned()
                    .unwrap_or(Value::Null),
            );
            values.push(item.value.get("scope").cloned().unwrap_or(Value::Null));
        }
        _ => (),
    }
    serde_json::to_string(&values).unwrap_or_default()
}

fn transition_ok(
    kind: &str,
    prior: Option<&str>,
    current: &str,
    reconciliation_evidence: bool,
) -> bool {
    match (kind, prior) {
        ("authorization", None) => {
            matches!(current, "reported_issued" | "reported_refused" | "unknown")
        }
        ("authorization", Some("reported_issued")) => {
            matches!(current, "reported_expired" | "reported_revoked" | "unknown")
        }
        ("attempt", None) => matches!(current, "reported_accepted" | "outcome_unknown"),
        ("attempt", Some("reported_accepted")) => matches!(
            current,
            "reported_active"
                | "reported_completed"
                | "reported_failed"
                | "reported_cancelled"
                | "outcome_unknown"
        ),
        ("attempt", Some("reported_active")) => matches!(
            current,
            "reported_completed" | "reported_failed" | "reported_cancelled" | "outcome_unknown"
        ),
        ("attempt", Some("outcome_unknown")) => {
            matches!(
                current,
                "reported_reconciled_completed" | "reported_reconciled_failed"
            ) && reconciliation_evidence
        }
        ("suspension", None) => current == "requested",
        ("suspension", Some("requested")) => matches!(
            current,
            "reported_acknowledged" | "reported_partial" | "unknown"
        ),
        ("suspension", Some("reported_acknowledged" | "reported_partial")) => {
            current == "reported_resumed"
        }
        _ => false,
    }
}

fn event_refs(
    ordinal: usize,
    observations: &[Observation<'_>],
    index: &BTreeMap<String, IndexedRecord>,
    tainted: &[bool],
    flags: &mut Flags,
    budget: &mut Budget,
) -> Result<Vec<ReferenceLink>, LifecycleError> {
    let item = &observations[ordinal];
    let Some(kind) = item.kind() else {
        return Err(LifecycleError::SerializationFailed);
    };
    let mut refs = Vec::new();
    match kind {
        "authorization" => {
            if let Some(id) = item.text("evaluationId") {
                refs.push(reference(
                    index,
                    tainted,
                    budget,
                    flags,
                    ReferenceRequest {
                        field: "evaluationId",
                        list_index: None,
                        id,
                        expected: "evaluation",
                    },
                )?);
            }
            if let Some(id) = item.text("planId") {
                refs.push(reference(
                    index,
                    tainted,
                    budget,
                    flags,
                    ReferenceRequest {
                        field: "planId",
                        list_index: None,
                        id,
                        expected: "approval_plan",
                    },
                )?);
            }
            if let Some(ids) = item.value.get("decisionIds").and_then(Value::as_array) {
                for (i, id) in ids.iter().enumerate() {
                    if let Some(id) = id.as_str() {
                        refs.push(reference(
                            index,
                            tainted,
                            budget,
                            flags,
                            ReferenceRequest {
                                field: "decisionIds",
                                list_index: Some(i),
                                id,
                                expected: "approval_decision",
                            },
                        )?);
                    }
                }
            }
        }
        "attempt" => {
            if let Some(id) = item.text("authorizationId") {
                refs.push(reference(
                    index,
                    tainted,
                    budget,
                    flags,
                    ReferenceRequest {
                        field: "authorizationId",
                        list_index: None,
                        id,
                        expected: "authorization",
                    },
                )?);
            }
        }
        "suspension" | "coverage" => {
            if let Some(id) = item.text("gatewayRevisionId") {
                refs.push(reference(
                    index,
                    tainted,
                    budget,
                    flags,
                    ReferenceRequest {
                        field: "gatewayRevisionId",
                        list_index: None,
                        id,
                        expected: "gateway_revision",
                    },
                )?);
            }
        }
        "metric" => {
            if let Some(id) = item.text("attemptId") {
                refs.push(attempt_reference(
                    observations,
                    index,
                    tainted,
                    budget,
                    flags,
                    item,
                    id,
                )?);
            }
            if let Some(id) = item
                .value
                .get("cost")
                .and_then(|cost| cost.get("sourceEvidenceId"))
                .and_then(Value::as_str)
            {
                refs.push(reference(
                    index,
                    tainted,
                    budget,
                    flags,
                    ReferenceRequest {
                        field: "cost.sourceEvidenceId",
                        list_index: None,
                        id,
                        expected: "evidence",
                    },
                )?);
            }
        }
        _ => (),
    }
    if matches!(kind, "authorization" | "attempt" | "suspension")
        && let Some(id) = item.predecessor()
    {
        let expected_kind: &'static str = match kind {
            "authorization" => "authorization",
            "attempt" => "attempt",
            "suspension" => "suspension",
            _ => return Err(LifecycleError::SerializationFailed),
        };
        refs.push(reference(
            index,
            tainted,
            budget,
            flags,
            ReferenceRequest {
                field: "previousObservationId",
                list_index: None,
                id,
                expected: expected_kind,
            },
        )?);
    }
    if matches!(
        kind,
        "authorization" | "attempt" | "suspension" | "coverage"
    ) && let Some(ids) = item.value.get("evidenceIds").and_then(Value::as_array)
    {
        for (i, id) in ids.iter().enumerate() {
            if let Some(id) = id.as_str() {
                refs.push(reference(
                    index,
                    tainted,
                    budget,
                    flags,
                    ReferenceRequest {
                        field: "evidenceIds",
                        list_index: Some(i),
                        id,
                        expected: "evidence",
                    },
                )?);
            }
        }
    }
    for link in &refs {
        if link.status == "unique_unverified"
            && link.expected_kind != "evidence"
            && link.expected_kind != "gateway_revision"
            && link.field != "previousObservationId"
            && let Some(record) = index.get(&link.record_id)
            && let Some(first) = observations.get(record.first)
            && first.value.get("context").is_some()
            && item.value.get("context").is_some()
            && !flow_equal(&item.value, &first.value)
        {
            mark(flags, "context_mismatch");
        }
    }
    Ok(refs)
}

fn coverage_checks(value: &Value) -> ReportedChecks {
    let checks = value.get("checks").unwrap_or(&Value::Null);
    let field = |name| {
        checks
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned()
    };
    ReportedChecks {
        policy_decision: field("policyDecision"),
        approval_duties: field("approvalDuties"),
        agent_delegation: field("agentDelegation"),
        expiry_revocation: field("expiryRevocation"),
        execution_consumption: field("executionConsumption"),
        replay_protection: field("replayProtection"),
        outcome_evidence: field("outcomeEvidence"),
        reconciliation: field("reconciliation"),
    }
}

fn coverage_selector(value: &Value) -> Result<String, LifecycleError> {
    let fields = [
        "systemId",
        "engineId",
        "gatewayRevisionId",
        "operationClass",
        "environmentId",
        "targetOs",
        "targetResourceIds",
    ];
    let values: Vec<Value> = fields
        .iter()
        .map(|field| value.get(*field).cloned().unwrap_or(Value::Null))
        .collect();
    serde_json::to_string(&values).map_err(|_| LifecycleError::SerializationFailed)
}
fn coverage_comparison(value: &Value) -> Result<String, LifecycleError> {
    let fields = [
        "systemId",
        "engineId",
        "gatewayRevisionId",
        "operationClass",
        "environmentId",
        "targetOs",
        "targetResourceIds",
        "freshness",
        "expiresMs",
        "adapterArtifactDigest",
        "engineArtifactDigest",
        "reportedLevel",
        "checks",
        "evidenceIds",
    ];
    let values: Vec<Value> = fields
        .iter()
        .map(|field| value.get(*field).cloned().unwrap_or(Value::Null))
        .collect();
    serde_json::to_string(&values).map_err(|_| LifecycleError::SerializationFailed)
}

fn map_projection(error: ProjectionError) -> LifecycleError {
    match error {
        ProjectionError::Limit => LifecycleError::StructuralLimit,
        ProjectionError::SerializationFailed => LifecycleError::SerializationFailed,
    }
}
fn map_relationship(error: RelationshipError) -> LifecycleError {
    match error {
        RelationshipError::SerializationFailed => LifecycleError::SerializationFailed,
        RelationshipError::StructuralLimit => LifecycleError::StructuralLimit,
        RelationshipError::Limit => LifecycleError::RelationshipLimit,
    }
}
fn map_delegation(error: DelegationError) -> LifecycleError {
    match error {
        DelegationError::SerializationFailed => LifecycleError::SerializationFailed,
        DelegationError::StructuralLimit => LifecycleError::StructuralLimit,
        DelegationError::Limit => LifecycleError::DelegationLimit,
    }
}
fn map_approval(error: ApprovalError) -> LifecycleError {
    match error {
        ApprovalError::SerializationFailed => LifecycleError::SerializationFailed,
        ApprovalError::StructuralLimit => LifecycleError::StructuralLimit,
        ApprovalError::RelationshipLimit => LifecycleError::RelationshipLimit,
        ApprovalError::DelegationLimit => LifecycleError::DelegationLimit,
        ApprovalError::Limit => LifecycleError::ApprovalLimit,
    }
}

/// Inspects source-reported lifecycles without accepting, executing, or authorizing anything.
pub fn inspect_lifecycle(decoded: &DecodedWindow) -> Result<LifecycleInspection, LifecycleError> {
    let projection = project_window(decoded).map_err(map_projection)?;
    let relationships = relate_window(decoded).map_err(map_relationship)?;
    let delegations = inspect_delegations(decoded).map_err(map_delegation)?;
    let approvals = inspect_approvals(decoded).map_err(map_approval)?;
    let window: Window = serde_json::from_slice(decoded.canonical_bytes())
        .map_err(|_| LifecycleError::SerializationFailed)?;
    let observations: Vec<Observation<'_>> = window
        .snapshot
        .prefixes
        .iter()
        .flat_map(|prefix| &prefix.events)
        .chain(&window.tail)
        .map(|event| {
            Ok(Observation {
                event,
                value: serde_json::to_value(&event.record)
                    .map_err(|_| LifecycleError::SerializationFailed)?,
                bytes: serde_json::to_vec(&event.record)
                    .map_err(|_| LifecycleError::SerializationFailed)?,
            })
        })
        .collect::<Result<_, LifecycleError>>()?;
    let mut index = BTreeMap::new();
    for (ordinal, item) in observations.iter().enumerate() {
        let Some(id) = item.id() else {
            return Err(LifecycleError::SerializationFailed);
        };
        let Some(kind) = item.kind() else {
            return Err(LifecycleError::SerializationFailed);
        };
        let record = index.entry(id.to_owned()).or_insert_with(|| IndexedRecord {
            first: ordinal,
            kind: kind.to_owned(),
            bytes: item.bytes.clone(),
            ordinals: Vec::new(),
            ambiguous: false,
            streams: BTreeSet::new(),
        });
        record.ambiguous |= record.bytes != item.bytes;
        record.ordinals.push(ordinal);
        record
            .streams
            .insert((item.event.producer_id.clone(), item.event.fence_id.clone()));
    }
    let global_taint = projection
        .document()
        .snapshot_diagnostics
        .iter()
        .any(|code| {
            matches!(
                code.as_str(),
                "prefix_watermark_mismatch" | "prefix_identity_conflict"
            )
        });
    let mut tainted = vec![global_taint; observations.len()];
    for item in &projection.document().timeline {
        if item
            .diagnostics
            .iter()
            .any(|code| STREAM_TAINT.contains(&code.as_str()))
        {
            let Some(target) = tainted.get_mut(item.input_ordinal) else {
                return Err(LifecycleError::SerializationFailed);
            };
            *target = true;
        }
    }
    let mut flags = vec![[false; 22]; observations.len()];
    let mut references = (0..observations.len())
        .map(|_| Vec::new())
        .collect::<Vec<Vec<ReferenceLink>>>();
    let mut budget = Budget::default();
    for (ordinal, item) in observations.iter().enumerate() {
        if item.selected() {
            if tainted[ordinal] {
                mark(&mut flags[ordinal], "structural_incomplete");
            }
            if let Some(id) = item.id() {
                status_flags(
                    &mut flags[ordinal],
                    status(&index, id, item.kind().unwrap_or_default()),
                );
            }
            references[ordinal] = event_refs(
                ordinal,
                &observations,
                &index,
                &tainted,
                &mut flags[ordinal],
                &mut budget,
            )?;
            if item.kind() == Some("attempt")
                && matches!(
                    item.text("state"),
                    Some("outcome_unknown" | "reported_cancelled")
                )
            {
                mark(&mut flags[ordinal], "effect_unknown");
            }
        }
    }
    let mut groups: BTreeMap<(u8, String), Vec<usize>> = BTreeMap::new();
    for (ordinal, item) in observations.iter().enumerate() {
        if let Some(key) = group_id(item)? {
            groups.entry(key).or_default().push(ordinal);
        }
    }
    let mut chains = Vec::new();
    for ((order, group_label), mut ordinals) in groups {
        let kind = match order {
            0 => "authorization",
            1 => "attempt",
            _ => "suspension",
        };
        ordinals.sort_unstable();
        let mut chain_flags = [false; 22];
        let mut nodes: BTreeMap<String, usize> = BTreeMap::new();
        for ordinal in &ordinals {
            let item = &observations[*ordinal];
            let Some(id) = item.id() else {
                return Err(LifecycleError::SerializationFailed);
            };
            if index.get(id).is_some_and(|record| record.ambiguous) {
                mark(&mut chain_flags, "reference_ambiguous");
            }
            if index.get(id).is_some_and(|record| record.streams.len() > 1) {
                mark(&mut chain_flags, "stream_mismatch");
            }
            if tainted[*ordinal] {
                mark(&mut chain_flags, "structural_incomplete");
            }
            nodes.entry(id.to_owned()).or_insert(*ordinal);
        }
        let mut roots = Vec::new();
        let mut successors: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (id, ordinal) in &nodes {
            let item = &observations[*ordinal];
            let Some(parent_id) = item.predecessor() else {
                roots.push(id.clone());
                if !transition_ok(kind, None, item.text("state").unwrap_or_default(), false) {
                    mark(&mut chain_flags, "predecessor_missing");
                }
                continue;
            };
            match status(&index, parent_id, kind) {
                "missing" => mark(&mut chain_flags, "predecessor_missing"),
                "ambiguous" => mark(&mut chain_flags, "predecessor_ambiguous"),
                "wrong_kind" => mark(&mut chain_flags, "predecessor_wrong_kind"),
                _ => {
                    let Some(parent_record) = index.get(parent_id) else {
                        return Err(LifecycleError::SerializationFailed);
                    };
                    let Some(parent_ordinal) = parent_record.ordinals.first().copied() else {
                        return Err(LifecycleError::SerializationFailed);
                    };
                    let parent = &observations[parent_ordinal];
                    if group_id(parent)?.as_ref().map(|key| key.1.as_str())
                        != Some(group_label.as_str())
                    {
                        mark(&mut chain_flags, "binding_mismatch");
                    }
                    if stream_key(item) != stream_key(parent) {
                        mark(&mut chain_flags, "stream_mismatch");
                    } else if let (Some(current_seq), Some(parent_seq)) = (
                        index
                            .get(id)
                            .and_then(|record| earliest_sequence(record, &observations)),
                        earliest_sequence(parent_record, &observations),
                    ) && current_seq <= parent_seq
                    {
                        mark(&mut chain_flags, "sequence_mismatch");
                    }
                    if binding(item) != binding(parent) {
                        mark(&mut chain_flags, "binding_mismatch");
                    }
                    successors
                        .entry(parent_id.to_owned())
                        .or_default()
                        .push(id.clone());
                    let evidence = item
                        .value
                        .get("evidenceIds")
                        .and_then(Value::as_array)
                        .is_some_and(|ids| {
                            ids.iter().any(|entry| {
                                entry.as_str().is_some_and(|evidence_id| {
                                    status(&index, evidence_id, "evidence") == "unique_unverified"
                                })
                            })
                        });
                    if !transition_ok(
                        kind,
                        parent.text("state"),
                        item.text("state").unwrap_or_default(),
                        evidence,
                    ) {
                        mark(&mut chain_flags, "state_transition_invalid");
                        if kind == "attempt" && parent.text("state") == Some("outcome_unknown") {
                            mark(&mut chain_flags, "reconciliation_evidence_missing");
                        }
                    }
                }
            }
        }
        if roots.len() != 1 {
            mark(&mut chain_flags, "multiple_roots");
        }
        if successors.values().any(|children| children.len() > 1) {
            mark(&mut chain_flags, "branch");
        }
        for id in nodes.keys() {
            let mut current = id.as_str();
            let mut active = BTreeSet::new();
            loop {
                budget.reserve(0, 0, 1)?;
                if !active.insert(current) {
                    mark(&mut chain_flags, "cycle");
                    break;
                }
                let Some(record) = index.get(current) else {
                    break;
                };
                if record.ambiguous || record.kind != kind {
                    break;
                }
                let Some(node_ordinal) = record.ordinals.first().copied() else {
                    break;
                };
                let node = &observations[node_ordinal];
                if group_id(node)?.as_ref().map(|key| key.1.as_str()) != Some(group_label.as_str())
                {
                    break;
                }
                let Some(parent) = node.predecessor() else {
                    break;
                };
                current = parent;
            }
        }
        let mut ordered_ids = nodes.keys().cloned().collect::<Vec<_>>();
        let coherent = !chain_flags.iter().any(|flag| *flag);
        let mut tip_id = None;
        let mut tip_state = None;
        if coherent {
            let mut current = roots
                .first()
                .cloned()
                .ok_or(LifecycleError::SerializationFailed)?;
            let mut path = Vec::new();
            loop {
                budget.reserve(0, 0, 1)?;
                path.push(current.clone());
                let Some(next) = successors
                    .get(&current)
                    .and_then(|entries| entries.first())
                    .cloned()
                else {
                    break;
                };
                current = next;
            }
            if path.len() != nodes.len() {
                mark(&mut chain_flags, "branch");
            } else {
                tip_state = nodes
                    .get(&current)
                    .and_then(|ordinal| observations[*ordinal].text("state"))
                    .map(str::to_owned);
                tip_id = Some(current);
                ordered_ids = path;
            }
        }
        for ordinal in &ordinals {
            union(&mut flags[*ordinal], &chain_flags);
        }
        chains.push(LifecycleChain {
            kind,
            group_id: group_label,
            matching_event_ordinals: ordinals,
            record_ids: ordered_ids,
            reported_chain_status: if chain_flags.iter().any(|flag| *flag) {
                "unknown_or_conflict"
            } else {
                "single_chain_unverified"
            },
            reported_tip_record_id: if chain_flags.iter().any(|flag| *flag) {
                None
            } else {
                tip_id
            },
            reported_tip_state: if chain_flags.iter().any(|flag| *flag) {
                None
            } else {
                tip_state
            },
            diagnostics: codes(&chain_flags),
        });
    }
    let mut coverage_flags = vec![[false; 22]; observations.len()];
    let mut coverage_groups: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (ordinal, item) in observations
        .iter()
        .enumerate()
        .filter(|(_, item)| item.kind() == Some("coverage"))
    {
        let selector = coverage_selector(&item.value)?;
        coverage_groups
            .entry(selector)
            .or_default()
            .insert(coverage_comparison(&item.value)?);
        union(&mut coverage_flags[ordinal], &flags[ordinal]);
        if item
            .value
            .get("observedMs")
            .and_then(Value::as_u64)
            .is_none()
            || item
                .value
                .get("expiresMs")
                .and_then(Value::as_u64)
                .is_none()
            || item.text("freshness") == Some("unknown")
        {
            mark(&mut coverage_flags[ordinal], "coverage_freshness_unknown");
        }
        if matches!(item.text("freshness"), Some("stale" | "reported_stale")) {
            mark(&mut coverage_flags[ordinal], "coverage_stale");
        }
        if item
            .value
            .get("expiresMs")
            .and_then(Value::as_u64)
            .is_some_and(|expires| expires <= window.snapshot.as_of_ms)
        {
            mark(&mut coverage_flags[ordinal], "coverage_expired");
        }
        let unresolved = references[ordinal].iter().any(|reference| {
            reference.field == "evidenceIds" && reference.status != "unique_unverified"
        });
        let absent = item
            .value
            .get("evidenceIds")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty);
        if absent || unresolved {
            mark(&mut coverage_flags[ordinal], "coverage_evidence_missing");
        }
    }
    for (ordinal, item) in observations
        .iter()
        .enumerate()
        .filter(|(_, item)| item.kind() == Some("coverage"))
    {
        if coverage_groups
            .get(&coverage_selector(&item.value)?)
            .is_some_and(|variants| variants.len() > 1)
        {
            mark(&mut coverage_flags[ordinal], "coverage_conflict");
        }
    }
    for (ordinal, item) in observations.iter().enumerate() {
        if item.kind() == Some("coverage") {
            union(&mut flags[ordinal], &coverage_flags[ordinal]);
        }
    }
    let mut counts = [0usize; 22];
    let mut event_assessments = Vec::with_capacity(observations.len());
    for (ordinal, item) in observations.iter().enumerate() {
        if item.selected() {
            for (count, flag) in counts.iter_mut().zip(flags[ordinal]) {
                *count += usize::from(flag);
            }
        }
        event_assessments.push(LifecycleEvent {
            input_ordinal: ordinal,
            event_id: item.event.event_id.clone(),
            record_id: item.id().unwrap_or_default().to_owned(),
            record_kind: item.kind().unwrap_or_default().to_owned(),
            assessment: selected_assessment(item.selected(), &flags[ordinal]),
            reported_state: matches!(
                item.kind(),
                Some("authorization" | "attempt" | "suspension" | "evidence")
            )
            .then(|| item.text("state").unwrap_or("unverified").to_owned()),
            reported_attempt_id: (item.kind() == Some("attempt"))
                .then(|| item.text("attemptId").unwrap_or_default().to_owned())
                .or_else(|| {
                    (item.kind() == Some("metric"))
                        .then(|| item.text("attemptId").map(str::to_owned))
                        .flatten()
                }),
            diagnostics: if item.selected() {
                codes(&flags[ordinal])
            } else {
                Vec::new()
            },
            references: std::mem::take(&mut references[ordinal]),
        });
    }
    for chain in &chains {
        for code in &chain.diagnostics {
            if let Some(index) = code_index(code) {
                counts[index] += 1;
            }
        }
    }
    let mut coverage_assessments = Vec::new();
    for (ordinal, item) in observations
        .iter()
        .enumerate()
        .filter(|(_, item)| item.kind() == Some("coverage"))
    {
        for (count, flag) in counts.iter_mut().zip(coverage_flags[ordinal]) {
            *count += usize::from(flag);
        }
        coverage_assessments.push(CoverageAssessment {
            input_ordinal: ordinal,
            record_id: item.id().unwrap_or_default().to_owned(),
            reported_level: item.text("reportedLevel").unwrap_or_default().to_owned(),
            reported_freshness: item.text("freshness").unwrap_or_default().to_owned(),
            observed_ms: item.value.get("observedMs").and_then(Value::as_u64),
            expires_ms: item.value.get("expiresMs").and_then(Value::as_u64),
            target_os: item.text("targetOs").unwrap_or_default().to_owned(),
            checks: coverage_checks(&item.value),
            assessment: assessment(&coverage_flags[ordinal]),
            diagnostics: codes(&coverage_flags[ordinal]),
            references: event_assessments[ordinal]
                .references
                .iter()
                .filter(|reference| {
                    reference.field == "gatewayRevisionId" || reference.field == "evidenceIds"
                })
                .cloned()
                .collect(),
            qualified_level: "unknown",
        });
    }
    let semantic_assessment = if counts
        .iter()
        .enumerate()
        .any(|(index, count)| *count > 0 && !incomplete(index))
    {
        "conflict"
    } else if counts.iter().any(|count| *count > 0) {
        "incomplete"
    } else {
        "linked_unverified"
    };
    let document = LifecycleDocument {
        schema_version: LIFECYCLE_SCHEMA,
        input_canonical_digest: decoded.canonical_digest().to_owned(),
        structural_projection_digest: projection.canonical_digest(),
        relationship_digest: relationships.canonical_digest(),
        delegation_digest: delegations.canonical_digest(),
        approval_digest: approvals.canonical_digest(),
        authority: "none",
        authenticity: "unverified",
        execution: "unavailable",
        semantic_assessment,
        authoritative_eligibility: "unknown",
        authoritative_outcome: "unknown",
        qualified_coverage: "unknown",
        diagnostic_counts: CODES
            .iter()
            .zip(counts)
            .map(|(code, count)| LifecycleCount { code, count })
            .collect(),
        event_assessments,
        reported_chains: chains,
        coverage_assessments,
    };
    let canonical = bounded_json(&document)?;
    Ok(LifecycleInspection {
        canonical,
        document,
    })
}
