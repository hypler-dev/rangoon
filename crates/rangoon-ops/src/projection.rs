//! Structural observations only; nothing here evaluates or consumes authority.

use crate::DecodedWindow;
use crate::projection_types::*;
use crate::records::{Event, Snapshot, Window};
use rangoon_domain::byte_digest;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

const CODES: [&str; 18] = [
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
type Flags = [bool; 18];
type Groups<'a> = BTreeMap<&'a str, Vec<usize>>;

struct Observation<'a> {
    event: &'a Event,
    location: Location,
    record: Value,
    event_bytes: Vec<u8>,
    record_bytes: Vec<u8>,
    flags: Flags,
}

impl<'a> Observation<'a> {
    fn new(event: &'a Event, location: Location) -> Result<Self, ProjectionError> {
        Ok(Self {
            event,
            location,
            record: serde_json::to_value(&event.record)
                .map_err(|_| ProjectionError::SerializationFailed)?,
            event_bytes: serde_json::to_vec(event)
                .map_err(|_| ProjectionError::SerializationFailed)?,
            record_bytes: serde_json::to_vec(&event.record)
                .map_err(|_| ProjectionError::SerializationFailed)?,
            flags: [false; 18],
        })
    }
    fn field(&self, key: &str) -> Option<&str> {
        self.record.get(key).and_then(Value::as_str)
    }
    fn record_id(&self) -> &str {
        self.field("id").expect("private validated record identity")
    }
    fn record_kind(&self) -> &str {
        self.field("kind").expect("private validated record kind")
    }
}

/// Projects an exact immutable window. Structural checks cannot qualify authority.
pub fn project_window(decoded: &DecodedWindow) -> Result<Projection, ProjectionError> {
    // Only O1a's private validated canonical bytes enter this typed parser.
    let window: Window = serde_json::from_slice(decoded.canonical_bytes())
        .map_err(|_| ProjectionError::SerializationFailed)?;
    let mut observations = Vec::with_capacity(decoded.inventory().event_count());
    let mut prefix_ordinals = Vec::with_capacity(window.snapshot.prefixes.len());
    for (prefix_index, prefix) in window.snapshot.prefixes.iter().enumerate() {
        let mut ordinals = Vec::with_capacity(prefix.events.len());
        for (event_index, event) in prefix.events.iter().enumerate() {
            ordinals.push(observations.len());
            observations.push(Observation::new(
                event,
                Location::Prefix {
                    prefix_index,
                    event_index,
                },
            )?);
        }
        prefix_ordinals.push(ordinals);
    }
    for (event_index, event) in window.tail.iter().enumerate() {
        observations.push(Observation::new(event, Location::Tail { event_index })?);
    }
    let mut snapshot_flags = [false; 18];
    check_identities_and_streams(&mut observations);
    check_prefixes(
        &window.snapshot,
        &prefix_ordinals,
        &mut observations,
        &mut snapshot_flags,
    );
    for observation in &mut observations {
        observation.flags[10] = outside_scope(observation, &window.snapshot);
        observation.flags[11] = correlation_mismatch(observation);
    }
    let (nodes, edges, node_flags, edge_flags) = graph(&window.snapshot, &observations);
    let mut counts = [0; 18];
    let mut incomplete = false;
    let mut conflict = false;
    for flags in std::iter::once(&snapshot_flags)
        .chain(observations.iter().map(|o| &o.flags))
        .chain(node_flags.iter())
        .chain(edge_flags.iter())
    {
        for (i, flag) in flags.iter().enumerate() {
            if *flag {
                counts[i] += 1;
                match i {
                    0 => (),
                    4 | 5 | 8 | 13 | 16 => incomplete = true,
                    _ => conflict = true,
                }
            }
        }
    }
    let structural_state = if conflict {
        "conflict"
    } else if incomplete {
        "incomplete"
    } else {
        "structural_unverified"
    };
    let mut timeline = observations
        .iter()
        .enumerate()
        .map(|(ordinal, observation)| {
            let event = observation.event;
            TimelineEntry {
                input_ordinal: ordinal,
                location: observation.location.clone(),
                event_id: event.event_id.clone(),
                producer_id: event.producer_id.clone(),
                fence_id: event.fence_id.clone(),
                sequence: event.sequence,
                previous_event_id: event.previous_event_id.clone(),
                observed_ms: event.observed_ms,
                received_ms: event.received_ms,
                correlation: event.correlation.clone(),
                record_id: observation.record_id().to_owned(),
                record_kind: observation.record_kind().to_owned(),
                event_digest: byte_digest(&observation.event_bytes),
                reported_value: [
                    "state",
                    "result",
                    "decision",
                    "reportedVerification",
                    "reportedLevel",
                ]
                .iter()
                .find_map(|key| observation.field(key))
                .map(str::to_owned),
                diagnostics: codes(&observation.flags),
            }
        })
        .collect::<Vec<_>>();
    timeline.sort_by(|a, b| {
        (
            &a.producer_id,
            &a.fence_id,
            a.sequence,
            &a.event_id,
            a.input_ordinal,
        )
            .cmp(&(
                &b.producer_id,
                &b.fence_id,
                b.sequence,
                &b.event_id,
                b.input_ordinal,
            ))
    });
    let snapshot = &window.snapshot;
    let mut prefixes = snapshot
        .prefixes
        .iter()
        .enumerate()
        .map(|(index, prefix)| ReplayPrefix {
            input_prefix_index: index,
            producer_id: prefix.producer_id.clone(),
            fence_id: prefix.fence_id.clone(),
            watermark: prefix.watermark.clone(),
        })
        .collect::<Vec<_>>();
    prefixes.sort_by(|a, b| {
        (&a.producer_id, &a.fence_id, a.input_prefix_index).cmp(&(
            &b.producer_id,
            &b.fence_id,
            b.input_prefix_index,
        ))
    });
    let document = ProjectionDocument {
        schema_version: PROJECTION_SCHEMA,
        inventory: decoded.inventory().clone(),
        replay: Replay {
            canonical_digest: decoded.canonical_digest().to_owned(),
            snapshot_id: snapshot.snapshot_id.clone(),
            reducer_version: snapshot.reducer_version.clone(),
            as_of_ms: snapshot.as_of_ms,
            scope: snapshot.scope.clone(),
            topology_revision_id: snapshot.topology.revision_id.clone(),
            policy_revision_ids: snapshot.topology.policy_revision_ids.clone(),
            coverage_revision_ids: snapshot.topology.coverage_revision_ids.clone(),
            prefixes,
        },
        authority: "none",
        authenticity: "unverified",
        execution: "unavailable",
        semantic_assessment: "unavailable",
        authoritative_eligibility: "unknown",
        authoritative_effective_evaluation_id: None,
        structural_state,
        snapshot_diagnostics: codes(&snapshot_flags),
        diagnostic_counts: DiagnosticCounts::from_array(counts),
        timeline,
        nodes,
        edges,
    };
    let canonical = bounded_json(&document)?;
    Ok(Projection {
        canonical,
        document,
    })
}

fn codes(flags: &Flags) -> Vec<String> {
    CODES
        .iter()
        .zip(flags)
        .filter(|(_, flag)| **flag)
        .map(|(code, _)| (*code).to_owned())
        .collect()
}

fn check_identities_and_streams(observations: &mut [Observation<'_>]) {
    // The indexes contain only references to the bounded typed window and bounded ordinals.
    let events = grouped(observations, |o| &o.event.event_id);
    let records = grouped(observations, |o| o.record_id());
    let mut sequences = BTreeMap::<(&str, &str, u64), Vec<usize>>::new();
    let mut streams = BTreeMap::<(&str, &str), BTreeMap<u64, Vec<usize>>>::new();
    for (i, o) in observations.iter().enumerate() {
        sequences
            .entry((&o.event.producer_id, &o.event.fence_id, o.event.sequence))
            .or_default()
            .push(i);
        streams
            .entry((&o.event.producer_id, &o.event.fence_id))
            .or_default()
            .entry(o.event.sequence)
            .or_default()
            .push(i);
    }
    // Collect marks before mutation; map keys borrow the immutable observation identities.
    let mut marks = Vec::<(usize, usize)>::new();
    for ordinals in events.values() {
        if ordinals.len() > 1 {
            let different = ordinals
                .iter()
                .any(|&i| observations[i].event_bytes != observations[ordinals[0]].event_bytes);
            marks.extend(ordinals.iter().map(|&i| (i, if different { 1 } else { 0 })));
        }
    }
    for ordinals in sequences.values() {
        if ordinals
            .iter()
            .any(|&i| observations[i].event_bytes != observations[ordinals[0]].event_bytes)
        {
            marks.extend(ordinals.iter().map(|&i| (i, 2)));
        }
    }
    for ordinals in records.values() {
        if ordinals
            .iter()
            .any(|&i| observations[i].record_bytes != observations[ordinals[0]].record_bytes)
        {
            marks.extend(ordinals.iter().map(|&i| (i, 3)));
        }
    }
    for stream in streams.values() {
        let mut expected = 1;
        for (&sequence, ordinals) in stream {
            if sequence != expected {
                marks.extend(ordinals.iter().map(|&i| (i, 4)));
            }
            expected = sequence + 1; // O1a's integer ceiling leaves ample u64 space.
        }
    }
    for (i, o) in observations.iter().enumerate() {
        match &o.event.previous_event_id {
            None if o.event.sequence > 1 => marks.push((i, 5)),
            None => (),
            Some(previous) => {
                if o.event.sequence == 1 {
                    marks.push((i, 6));
                }
                match events.get(previous.as_str()) {
                    None => marks.push((i, 5)),
                    Some(ordinals) => {
                        let predecessor = &observations[ordinals[0]];
                        if ordinals
                            .iter()
                            .any(|&j| observations[j].event_bytes != predecessor.event_bytes)
                            || predecessor.event.producer_id != o.event.producer_id
                            || predecessor.event.fence_id != o.event.fence_id
                            || predecessor.event.sequence.checked_add(1) != Some(o.event.sequence)
                        {
                            marks.push((i, 6));
                        }
                    }
                }
            }
        }
    }
    drop((events, records, sequences, streams));
    for (ordinal, flag) in marks {
        observations[ordinal].flags[flag] = true;
    }
}

fn grouped<'a>(
    observations: &'a [Observation<'_>],
    key: impl Fn(&'a Observation<'_>) -> &'a str,
) -> Groups<'a> {
    let mut groups = BTreeMap::new();
    for (i, o) in observations.iter().enumerate() {
        groups.entry(key(o)).or_insert_with(Vec::new).push(i);
    }
    groups
}

fn check_prefixes(
    snapshot: &Snapshot,
    ordinals: &[Vec<usize>],
    observations: &mut [Observation<'_>],
    snapshot_flags: &mut Flags,
) {
    let mut pairs = BTreeMap::<(&str, &str), Vec<usize>>::new();
    for (i, prefix) in snapshot.prefixes.iter().enumerate() {
        pairs
            .entry((&prefix.producer_id, &prefix.fence_id))
            .or_default()
            .push(i);
        let greatest = prefix.events.iter().map(|event| event.sequence).max();
        let ids = prefix
            .events
            .iter()
            .filter(|e| Some(e.sequence) == greatest)
            .map(|e| e.event_id.as_str())
            .collect::<BTreeSet<_>>();
        let watermark_matches = match (&prefix.watermark, greatest) {
            (None, None) => true,
            (Some(watermark), Some(sequence)) => {
                watermark.sequence == sequence
                    && ids.len() == 1
                    && ids.contains(watermark.event_id.as_str())
            }
            _ => false,
        };
        if !watermark_matches {
            snapshot_flags[8] = true;
            for &ordinal in &ordinals[i] {
                observations[ordinal].flags[8] = true;
            }
        }
        for &ordinal in &ordinals[i] {
            let event = observations[ordinal].event;
            if event.producer_id != prefix.producer_id || event.fence_id != prefix.fence_id {
                observations[ordinal].flags[7] = true;
            }
        }
    }
    for indexes in pairs.values().filter(|indexes| indexes.len() > 1) {
        snapshot_flags[9] = true;
        for &index in indexes {
            for &ordinal in &ordinals[index] {
                observations[ordinal].flags[9] = true;
            }
        }
    }
}

fn outside_scope(o: &Observation<'_>, snapshot: &Snapshot) -> bool {
    let scope = &snapshot.scope;
    let policy_ids = &snapshot.topology.policy_revision_ids;
    let member = |key: &str, values: &[String]| {
        o.field(key)
            .is_some_and(|id| !values.iter().any(|v| v == id))
    };
    if member("systemId", &scope.system_ids)
        || member("engineId", &scope.engine_ids)
        || member("environmentId", &scope.environment_ids)
        || member("policyRevisionId", policy_ids)
    {
        return true;
    }
    if let Some(c) = &o.event.correlation {
        if c.workspace_id != scope.workspace_id
            || !scope.system_ids.contains(&c.system_id)
            || !scope.environment_ids.contains(&c.environment_id)
            || !scope.engine_ids.contains(&c.engine_id)
        {
            return true;
        }
    }
    if let Some(context) = o.record.get("context") {
        if context["workspaceId"].as_str() != Some(scope.workspace_id.as_str())
            || !contains(&scope.system_ids, &context["systemId"])
            || !contains(&scope.environment_ids, &context["environmentId"])
            || !contains(&scope.engine_ids, &context["engineId"])
            || !contains(policy_ids, &context["policyRevisionId"])
        {
            return true;
        }
    }
    if let Some(action) = o.record.get("scope") {
        if !contains(&scope.system_ids, &action["systemId"])
            || !subset(&action["environmentIds"], &scope.environment_ids)
            || !subset(&action["policyRevisionIds"], policy_ids)
        {
            return true;
        }
    }
    false
}
fn contains(values: &[String], value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|v| values.iter().any(|item| item == v))
}
fn subset(value: &Value, values: &[String]) -> bool {
    value
        .as_array()
        .is_some_and(|items| items.iter().all(|item| contains(values, item)))
}

fn correlation_mismatch(o: &Observation<'_>) -> bool {
    if o.record_kind() == "evidence" && o.field("producerId") != Some(o.event.producer_id.as_str())
    {
        return true;
    }
    let Some(c) = &o.event.correlation else {
        return false;
    };
    for (key, expected) in [
        ("systemId", Some(c.system_id.as_str())),
        ("environmentId", Some(c.environment_id.as_str())),
        ("engineId", Some(c.engine_id.as_str())),
        ("policyRevisionId", c.policy_revision_id.as_deref()),
    ] {
        if let Some(actual) = o.field(key) {
            if Some(actual) != expected {
                return true;
            }
        }
    }
    if let Some(context) = o.record.get("context") {
        for (key, expected) in [
            ("workspaceId", Some(c.workspace_id.as_str())),
            ("systemId", Some(c.system_id.as_str())),
            ("environmentId", Some(c.environment_id.as_str())),
            ("engineId", Some(c.engine_id.as_str())),
            ("requestId", c.request_id.as_deref()),
            ("traceId", Some(c.trace_id.as_str())),
            ("spanId", Some(c.span_id.as_str())),
            ("policyRevisionId", c.policy_revision_id.as_deref()),
        ] {
            if context[key].as_str() != expected {
                return true;
            }
        }
    }
    if let Some(action) = o.record.get("scope") {
        let environments = action["environmentIds"]
            .as_array()
            .expect("private validated action scope");
        let policies = action["policyRevisionIds"]
            .as_array()
            .expect("private validated action scope");
        if action["systemId"].as_str() != Some(c.system_id.as_str())
            || !environments
                .iter()
                .any(|v| v.as_str() == Some(c.environment_id.as_str()))
            || c.policy_revision_id
                .as_deref()
                .is_none_or(|id| !policies.iter().any(|v| v.as_str() == Some(id)))
        {
            return true;
        }
    }
    let evaluation = match o.record_kind() {
        "evaluation" => Some(o.record_id()),
        "approval_plan" | "approval_decision" | "authorization" => o.field("evaluationId"),
        _ => None,
    };
    if evaluation.is_some_and(|id| c.evaluation_id.as_deref() != Some(id)) {
        return true;
    }
    if o.record_kind() == "request" && c.request_id.as_deref() != Some(o.record_id()) {
        return true;
    }
    if matches!(o.record_kind(), "attempt" | "metric") {
        if let Some(id) = o.field("attemptId") {
            if c.attempt_id.as_deref() != Some(id) {
                return true;
            }
        }
    }
    false
}

fn graph(
    snapshot: &Snapshot,
    observations: &[Observation<'_>],
) -> (
    Vec<ProjectedNode>,
    Vec<ProjectedEdge>,
    Vec<Flags>,
    Vec<Flags>,
) {
    let records = grouped(observations, |o| o.record_id());
    let mut node_ids = BTreeMap::<&str, usize>::new();
    let mut edge_ids = BTreeMap::<&str, usize>::new();
    for node in &snapshot.topology.nodes {
        *node_ids.entry(&node.id).or_default() += 1;
    }
    for edge in &snapshot.topology.edges {
        *edge_ids.entry(&edge.id).or_default() += 1;
    }
    let mut node_flags = Vec::new();
    let mut edge_flags = Vec::new();
    let nodes = snapshot
        .topology
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let mut flags = [false; 18];
            flags[12] = node_ids[node.id.as_str()] > 1;
            let ordinals = records
                .get(node.record_id.as_str())
                .cloned()
                .unwrap_or_default();
            flags[13] = ordinals.is_empty();
            let expected_kind = match node.kind.as_str() {
                "gateway" => "gateway_revision",
                "approval_stage" => "approval_plan",
                other => other,
            };
            flags[14] = ordinals
                .iter()
                .any(|&i| observations[i].record_kind() != expected_kind);
            node_flags.push(flags);
            ProjectedNode {
                input_node_index: index,
                id: node.id.clone(),
                kind: node.kind.clone(),
                record_id: node.record_id.clone(),
                matching_event_ordinals: ordinals,
                diagnostics: codes(&flags),
            }
        })
        .collect();
    let edges = snapshot
        .topology
        .edges
        .iter()
        .enumerate()
        .map(|(index, edge)| {
            let mut flags = [false; 18];
            flags[15] = edge_ids[edge.id.as_str()] > 1;
            let from = node_ids.get(edge.from.as_str()).copied().unwrap_or(0);
            let to = node_ids.get(edge.to.as_str()).copied().unwrap_or(0);
            flags[16] = from == 0 || to == 0;
            flags[17] = from > 1 || to > 1;
            edge_flags.push(flags);
            ProjectedEdge {
                input_edge_index: index,
                id: edge.id.clone(),
                kind: edge.kind.clone(),
                from: edge.from.clone(),
                to: edge.to.clone(),
                diagnostics: codes(&flags),
            }
        })
        .collect();
    (nodes, edges, node_flags, edge_flags)
}

struct LimitedWriter {
    bytes: Vec<u8>,
    limit_hit: bool,
}
impl LimitedWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(MAX_PROJECTION_BYTES),
            limit_hit: false,
        }
    }
}
impl Write for LimitedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_PROJECTION_BYTES - self.bytes.len() {
            self.limit_hit = true;
            return Err(std::io::Error::other("projection_limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(crate) fn bounded_json<T: Serialize>(value: &T) -> Result<Vec<u8>, ProjectionError> {
    let mut writer = LimitedWriter::new();
    if serde_json::to_writer(&mut writer, value).is_err() {
        return Err(if writer.limit_hit {
            ProjectionError::Limit
        } else {
            ProjectionError::SerializationFailed
        });
    }
    Ok(writer.bytes)
}

#[cfg(test)]
mod writer_tests {
    use super::*;
    #[test]
    fn exact_writer_limit_and_atomic_overflow() {
        let mut writer = LimitedWriter::new();
        assert_eq!(
            writer.write(&vec![b'x'; MAX_PROJECTION_BYTES]).unwrap(),
            MAX_PROJECTION_BYTES
        );
        assert!(writer.write(b"x").is_err());
        assert_eq!(writer.bytes.len(), MAX_PROJECTION_BYTES);
        assert!(writer.limit_hit);
        assert_eq!(
            bounded_json(&"x".repeat(MAX_PROJECTION_BYTES)).unwrap_err(),
            ProjectionError::Limit
        );
        let fitting = "x".repeat(MAX_PROJECTION_BYTES - 2);
        assert_eq!(bounded_json(&fitting).unwrap().len(), MAX_PROJECTION_BYTES);
    }
}
