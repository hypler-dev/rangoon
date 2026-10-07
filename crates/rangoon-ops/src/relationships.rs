//! Bounded assessment of reported links, never policy evaluation or authorization.
use crate::projection::bounded_json;
use crate::records::{Event, Window};
use crate::relationship_types::*;
use crate::{DecodedWindow, ProjectionError, project_window};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const CODES: [&str; 16] = [
    "structural_incomplete",
    "reference_missing",
    "reference_ambiguous",
    "reference_kind_mismatch",
    "context_mismatch",
    "gateway_mismatch",
    "context_digest_mismatch",
    "gateway_policy_mismatch",
    "evaluation_selector_mismatch",
    "lineage_fence_mismatch",
    "evaluation_sequence_mismatch",
    "evaluation_branch",
    "evaluation_cycle",
    "evaluation_incomplete_chain",
    "binding_mismatch",
    "non_tip_evaluation",
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
type Flags = [bool; 16];
type RecordIndex<'a> = BTreeMap<&'a str, Vec<usize>>;

struct Observation<'a> {
    event: &'a Event,
    value: Value,
    bytes: Vec<u8>,
}
impl Observation<'_> {
    fn field(&self, key: &str) -> Option<&str> {
        self.value.get(key).and_then(Value::as_str)
    }
    fn id(&self) -> &str {
        self.field("id").expect("private validated identity")
    }
    fn kind(&self) -> &str {
        self.field("kind").expect("private validated kind")
    }
    fn context(&self) -> Option<&Value> {
        self.value.get("context")
    }
    fn selected(&self) -> bool {
        matches!(
            self.kind(),
            "request" | "evaluation" | "approval_plan" | "approval_decision" | "authorization"
        )
    }
}
#[derive(Default)]
struct Budget {
    references: usize,
    matches: usize,
}
impl Budget {
    fn reserve(&mut self, references: usize, matches: usize) -> Result<(), RelationshipError> {
        let next_refs = self
            .references
            .checked_add(references)
            .ok_or(RelationshipError::Limit)?;
        let next_matches = self
            .matches
            .checked_add(matches)
            .ok_or(RelationshipError::Limit)?;
        if next_refs > MAX_RELATIONSHIP_REFERENCES || next_matches > MAX_RELATIONSHIP_MATCHES {
            return Err(RelationshipError::Limit);
        }
        self.references = next_refs;
        self.matches = next_matches;
        Ok(())
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
    for (target, flag) in into.iter_mut().zip(from) {
        *target |= flag;
    }
}
fn core(flags: &Flags) -> bool {
    flags[..15].iter().any(|flag| *flag)
}
fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .expect("private validated selector")
}
fn flow_equal(a: &Value, b: &Value) -> bool {
    FLOW_KEYS.iter().all(|key| a.get(key) == b.get(key))
}
fn status(id: &str, kind: &str, index: &RecordIndex<'_>, obs: &[Observation<'_>]) -> &'static str {
    let Some(ordinals) = index.get(id) else {
        return "missing";
    };
    let first = &obs[ordinals[0]];
    if ordinals
        .iter()
        .any(|ordinal| obs[*ordinal].bytes != first.bytes)
    {
        "ambiguous"
    } else if first.kind() != kind {
        "wrong_kind"
    } else {
        "unique_unverified"
    }
}
fn unique(id: &str, kind: &str, index: &RecordIndex<'_>, obs: &[Observation<'_>]) -> Option<usize> {
    (status(id, kind, index, obs) == "unique_unverified").then(|| index[id][0])
}
fn status_flag(flags: &mut Flags, value: &str) {
    match value {
        "missing" => flags[1] = true,
        "ambiguous" => flags[2] = true,
        "wrong_kind" => flags[3] = true,
        _ => (),
    }
}

/// Assesses an exact immutable observation window without creating authority.
pub fn relate_window(decoded: &DecodedWindow) -> Result<Relationships, RelationshipError> {
    let projection = project_window(decoded).map_err(|error| match error {
        ProjectionError::Limit => RelationshipError::StructuralLimit,
        ProjectionError::SerializationFailed => RelationshipError::SerializationFailed,
    })?;
    let window: Window = serde_json::from_slice(decoded.canonical_bytes())
        .map_err(|_| RelationshipError::SerializationFailed)?;
    let obs: Vec<Observation<'_>> = window
        .snapshot
        .prefixes
        .iter()
        .flat_map(|prefix| &prefix.events)
        .chain(&window.tail)
        .map(|event| {
            Ok(Observation {
                event,
                value: serde_json::to_value(&event.record)
                    .map_err(|_| RelationshipError::SerializationFailed)?,
                bytes: serde_json::to_vec(&event.record)
                    .map_err(|_| RelationshipError::SerializationFailed)?,
            })
        })
        .collect::<Result<_, RelationshipError>>()?;
    let mut index = BTreeMap::new();
    for (ordinal, item) in obs.iter().enumerate() {
        index
            .entry(item.id())
            .or_insert_with(Vec::new)
            .push(ordinal);
    }
    let mut tainted = BTreeSet::new();
    for item in &projection.document().timeline {
        if item
            .diagnostics
            .iter()
            .any(|code| STREAM_TAINT.contains(&code.as_str()))
        {
            tainted.insert((item.producer_id.as_str(), item.fence_id.as_str()));
        }
    }
    let global = projection
        .document()
        .snapshot_diagnostics
        .iter()
        .any(|code| {
            matches!(
                code.as_str(),
                "prefix_watermark_mismatch" | "prefix_identity_conflict"
            )
        });
    let stream_bad = |ordinal: usize| {
        let event = obs[ordinal].event;
        global || tainted.contains(&(event.producer_id.as_str(), event.fence_id.as_str()))
    };
    let mut flags = vec![[false; 16]; obs.len()];
    let mut references: Vec<Vec<ReferenceLink>> = (0..obs.len()).map(|_| Vec::new()).collect();
    let mut budget = Budget::default();
    for (ordinal, item) in obs.iter().enumerate().filter(|(_, item)| item.selected()) {
        flags[ordinal][0] = stream_bad(ordinal);
        for (field, list_index, id, kind) in reference_selectors(item) {
            let ordinals = index.get(id).map(Vec::as_slice).unwrap_or(&[]);
            budget.reserve(1, ordinals.len())?;
            let state = status(id, kind, &index, &obs);
            status_flag(&mut flags[ordinal], state);
            if ordinals.iter().any(|matching| stream_bad(*matching)) {
                flags[ordinal][0] = true;
            }
            if state == "unique_unverified" {
                if let Some(context) = obs[ordinals[0]].context() {
                    if !flow_equal(item.context().expect("selected context"), context) {
                        flags[ordinal][4] = true;
                    }
                }
            }
            references[ordinal].push(ReferenceLink {
                field,
                index: list_index,
                record_id: id.to_owned(),
                expected_kind: kind,
                status: state,
                matching_event_ordinals: ordinals.to_vec(),
            });
        }
        check_bindings(ordinal, &obs, &index, &mut flags);
    }
    let mut groups: BTreeMap<(&str, &str), Vec<usize>> = BTreeMap::new();
    for (ordinal, item) in obs.iter().enumerate().filter(|(_, item)| item.selected()) {
        let context = item.context().expect("selected context");
        groups
            .entry((text(context, "requestId"), text(context, "engineId")))
            .or_default()
            .push(ordinal);
    }
    let mut chains = Vec::with_capacity(groups.len());
    for ((request_id, engine_id), ordinals) in groups {
        let chain = chain(
            request_id,
            engine_id,
            &ordinals,
            &obs,
            &index,
            &mut flags,
            &mut budget,
        )?;
        if let Some(tip) = &chain.reported_chain_tip_id {
            for ordinal in &ordinals {
                let item = &obs[*ordinal];
                if matches!(
                    item.kind(),
                    "approval_plan" | "approval_decision" | "authorization"
                ) && item.field("evaluationId") != Some(tip.as_str())
                    && item.field("evaluationId").is_some_and(|id| {
                        chain
                            .evaluation_ids
                            .iter()
                            .any(|evaluation| evaluation == id)
                            && unique(id, "evaluation", &index, &obs).is_some()
                    })
                {
                    flags[*ordinal][15] = true;
                }
            }
        }
        chains.push(chain);
    }
    let mut counts = [0; 16];
    let mut event_links = Vec::with_capacity(obs.len());
    for (ordinal, (item, refs)) in obs.iter().zip(references).enumerate() {
        for (count, flag) in counts.iter_mut().zip(flags[ordinal]) {
            *count += usize::from(flag);
        }
        event_links.push(EventLink {
            input_ordinal: ordinal,
            event_id: item.event.event_id.clone(),
            record_id: item.id().to_owned(),
            record_kind: item.kind().to_owned(),
            assessment: assessment(item.selected(), &flags[ordinal]),
            diagnostics: codes(&flags[ordinal]),
            references: refs,
        });
    }
    for chain in &chains {
        for (count, code) in counts.iter_mut().zip(CODES) {
            *count += usize::from(chain.diagnostics.contains(&code));
        }
    }
    let document = RelationshipDocument {
        schema_version: RELATIONSHIP_SCHEMA,
        input_canonical_digest: decoded.canonical_digest().to_owned(),
        structural_projection_digest: projection.canonical_digest(),
        authority: "none",
        authenticity: "unverified",
        execution: "unavailable",
        semantic_assessment: "reported_links_only",
        authoritative_eligibility: "unknown",
        authoritative_effective_evaluation_id: None,
        diagnostic_counts: RelationshipCounts::from_array(counts),
        event_links,
        chains,
    };
    let canonical = bounded_json(&document).map_err(|error| match error {
        ProjectionError::Limit => RelationshipError::Limit,
        ProjectionError::SerializationFailed => RelationshipError::SerializationFailed,
    })?;
    Ok(Relationships {
        canonical,
        document,
    })
}

type Selector<'a> = (&'static str, Option<usize>, &'a str, &'static str);
fn reference_selectors<'a>(item: &'a Observation<'_>) -> Vec<Selector<'a>> {
    let mut result = Vec::new();
    let context = item.context().expect("selected context");
    if item.kind() != "request" {
        result.push((
            "context.requestId",
            None,
            text(context, "requestId"),
            "request",
        ));
    }
    result.push((
        "context.engineId",
        None,
        text(context, "engineId"),
        "engine",
    ));
    result.push((
        "context.policyRevisionId",
        None,
        text(context, "policyRevisionId"),
        "policy_revision",
    ));
    let table: &[(&str, &str, bool)] = match item.kind() {
        "request" => &[
            ("actorId", "actor", false),
            ("gatewayRevisionId", "gateway_revision", false),
            ("resourceIds", "resource", true),
        ],
        "evaluation" => &[
            ("gatewayRevisionId", "gateway_revision", false),
            ("evaluatorId", "actor", false),
            ("previousEvaluationId", "evaluation", false),
            ("expectedEffectiveEvaluationId", "evaluation", false),
            ("evidenceIds", "evidence", true),
        ],
        "approval_plan" => &[
            ("evaluationId", "evaluation", false),
            ("previousPlanId", "approval_plan", false),
            ("changedByActorId", "actor", false),
            ("evidenceIds", "evidence", true),
        ],
        "approval_decision" => &[
            ("evaluationId", "evaluation", false),
            ("planId", "approval_plan", false),
            ("actorId", "actor", false),
            ("delegationId", "delegation", false),
            ("supersedesDecisionId", "approval_decision", false),
            ("evidenceIds", "evidence", true),
        ],
        "authorization" => &[
            ("evaluationId", "evaluation", false),
            ("planId", "approval_plan", false),
            ("decisionIds", "approval_decision", true),
            ("previousObservationId", "authorization", false),
            ("scope.policyRevisionIds", "policy_revision", true),
            ("scope.resourceIds", "resource", true),
            ("evidenceIds", "evidence", true),
        ],
        _ => &[],
    };
    for (field, kind, list) in table {
        let value = if let Some(key) = field.strip_prefix("scope.") {
            item.value.get("scope").and_then(|scope| scope.get(key))
        } else {
            item.value.get(*field)
        };
        if *list {
            for (index, id) in value
                .and_then(Value::as_array)
                .expect("private validated list")
                .iter()
                .enumerate()
            {
                result.push((
                    *field,
                    Some(index),
                    id.as_str().expect("private validated reference"),
                    *kind,
                ));
            }
        } else if let Some(id) = value.and_then(Value::as_str) {
            result.push((*field, None, id, *kind));
        }
    }
    result
}
fn check_bindings(
    ordinal: usize,
    obs: &[Observation<'_>],
    index: &RecordIndex<'_>,
    flags: &mut [Flags],
) {
    let item = &obs[ordinal];
    let context = item.context().expect("selected context");
    let resolve = |id: &str, kind: &str| unique(id, kind, index, obs).map(|i| &obs[i]);
    match item.kind() {
        "request" => {
            if let Some(gateway) =
                resolve(item.field("gatewayRevisionId").unwrap(), "gateway_revision")
            {
                if gateway.field("policyRevisionId") != Some(text(context, "policyRevisionId")) {
                    flags[ordinal][7] = true;
                }
                if let Some(policy) = resolve(text(context, "policyRevisionId"), "policy_revision")
                {
                    if policy.field("gatewayId") != gateway.field("gatewayId") {
                        flags[ordinal][7] = true;
                    }
                }
            }
        }
        "evaluation" => {
            if let Some(request) = resolve(text(context, "requestId"), "request") {
                flags[ordinal][5] |=
                    item.field("gatewayRevisionId") != request.field("gatewayRevisionId");
                flags[ordinal][6] |= item.field("contextDigest") != request.field("contextDigest");
            }
            let previous = item.field("previousEvaluationId");
            let expected = item.field("expectedEffectiveEvaluationId");
            flags[ordinal][8] = match (item.field("evaluationKind"), previous, expected) {
                (Some("initial"), None, None) => false,
                (Some("successor"), Some(previous), Some(expected)) => previous != expected,
                _ => true,
            };
            let occurrences = &index[item.id()];
            let first = obs[occurrences[0]].event;
            flags[ordinal][9] |= occurrences.iter().any(|i| {
                let event = obs[*i].event;
                event.producer_id != first.producer_id
                    || event.fence_id != first.fence_id
                    || Some(event.fence_id.as_str()) != item.field("lineageFenceId")
            });
            if let Some(previous) = previous.and_then(|id| unique(id, "evaluation", index, obs)) {
                let predecessor = &obs[previous];
                flags[ordinal][5] |=
                    item.field("gatewayRevisionId") != predecessor.field("gatewayRevisionId");
                flags[ordinal][9] |= item.field("lineageFenceId")
                    != predecessor.field("lineageFenceId")
                    || item.event.producer_id != predecessor.event.producer_id
                    || item.event.fence_id != predecessor.event.fence_id;
                let earliest = |id: &str| {
                    index[id]
                        .iter()
                        .map(|i| obs[*i].event.sequence)
                        .min()
                        .unwrap()
                };
                flags[ordinal][10] |= earliest(item.id()) <= earliest(predecessor.id());
            }
        }
        "approval_decision" => {
            if let Some(plan) = resolve(item.field("planId").unwrap(), "approval_plan") {
                let stages = plan.value.get("stages").and_then(Value::as_array).unwrap();
                flags[ordinal][14] |= item.field("evaluationId") != plan.field("evaluationId")
                    || stages
                        .iter()
                        .filter(|stage| {
                            stage.get("id").and_then(Value::as_str) == item.field("stageId")
                        })
                        .count()
                        != 1;
            }
        }
        "authorization" => {
            let decisions = item
                .value
                .get("decisionIds")
                .and_then(Value::as_array)
                .unwrap();
            let plan = item
                .field("planId")
                .and_then(|id| resolve(id, "approval_plan"));
            if !decisions.is_empty() && plan.is_none() {
                flags[ordinal][14] = true;
            }
            if let Some(plan) = plan {
                flags[ordinal][14] |= plan.field("evaluationId") != item.field("evaluationId");
            }
            for id in decisions {
                if let Some(decision) = resolve(id.as_str().unwrap(), "approval_decision") {
                    flags[ordinal][14] |= decision.field("evaluationId")
                        != item.field("evaluationId")
                        || decision.field("planId") != item.field("planId");
                }
            }
        }
        _ => (),
    }
}
fn assessment(selected: bool, flags: &Flags) -> &'static str {
    if !selected {
        "not_assessed"
    } else if flags[2] {
        "ambiguous"
    } else if flags[4..15].iter().any(|f| *f) {
        "conflicting"
    } else if flags[1] || flags[3] {
        "missing"
    } else if flags[0] {
        "structurally_incomplete"
    } else {
        "linked_unverified"
    }
}

fn chain(
    request_id: &str,
    engine_id: &str,
    ordinals: &[usize],
    obs: &[Observation<'_>],
    index: &RecordIndex<'_>,
    flags: &mut [Flags],
    budget: &mut Budget,
) -> Result<ReportedChain, RelationshipError> {
    let request_ordinals = index.get(request_id).map(Vec::as_slice).unwrap_or(&[]);
    budget.reserve(0, request_ordinals.len())?;
    let mut group_flags = [false; 16];
    status_flag(&mut group_flags, status(request_id, "request", index, obs));
    if let Some(request) = unique(request_id, "request", index, obs) {
        union(&mut group_flags, &flags[request]);
        let context = obs[request].context().unwrap();
        group_flags[4] |=
            text(context, "requestId") != request_id || text(context, "engineId") != engine_id;
    }
    let mut evaluations: BTreeMap<&str, usize> = BTreeMap::new();
    for ordinal in ordinals {
        if obs[*ordinal].kind() == "evaluation" {
            evaluations.entry(obs[*ordinal].id()).or_insert(*ordinal);
        }
        if matches!(obs[*ordinal].kind(), "request" | "evaluation") {
            union(&mut group_flags, &flags[*ordinal]);
        }
    }
    let mut roots = Vec::new();
    let mut successors: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (id, ordinal) in &evaluations {
        let item = &obs[*ordinal];
        if status(id, "evaluation", index, obs) == "ambiguous" {
            group_flags[2] = true;
        }
        if let Some(previous) = item.field("previousEvaluationId") {
            successors.entry(previous).or_default().insert(id);
        } else if item.field("evaluationKind") == Some("initial")
            && item.field("expectedEffectiveEvaluationId").is_none()
        {
            roots.push(*id);
        }
    }
    group_flags[11] = roots.len() > 1 || successors.values().any(|next| next.len() > 1);
    // Walking each predecessor path bounds cycle detection and avoids recursion.
    for start in evaluations.keys() {
        let mut visited = BTreeSet::new();
        let mut current = Some(*start);
        while let Some(id) = current {
            if !visited.insert(id) {
                group_flags[12] = true;
                break;
            }
            if visited.len() > evaluations.len() {
                group_flags[12] = true;
                break;
            }
            current = evaluations
                .get(id)
                .and_then(|ordinal| obs[*ordinal].field("previousEvaluationId"))
                .filter(|previous| evaluations.contains_key(previous));
        }
    }
    let mut reached = BTreeSet::new();
    let mut tip = None;
    if roots.len() == 1 {
        let mut current = Some(roots[0]);
        while let Some(id) = current {
            if !reached.insert(id) || reached.len() > evaluations.len() {
                break;
            }
            tip = Some(id);
            current = successors
                .get(id)
                .filter(|next| next.len() == 1)
                .and_then(|next| next.first().copied());
        }
    }
    group_flags[13] =
        !evaluations.is_empty() && (roots.len() != 1 || reached.len() != evaluations.len());
    for ordinal in ordinals.iter().filter(|i| obs[**i].kind() == "evaluation") {
        for diagnostic in [11, 12, 13] {
            flags[*ordinal][diagnostic] |= group_flags[diagnostic];
        }
    }
    let valid = !core(&group_flags);
    let (state, tip, result) = if valid && evaluations.is_empty() {
        ("no_evaluation", None, None)
    } else if valid {
        let tip = tip.expect("nonempty complete single reported chain");
        (
            "single_chain_unverified",
            Some(tip.to_owned()),
            obs[evaluations[tip]].field("result").map(str::to_owned),
        )
    } else {
        ("unknown_or_conflict", None, None)
    };
    Ok(ReportedChain {
        request_id: request_id.to_owned(),
        engine_id: engine_id.to_owned(),
        request_event_ordinals: request_ordinals.to_vec(),
        evaluation_ids: evaluations.keys().map(|id| (*id).to_owned()).collect(),
        reported_chain_status: state,
        reported_chain_tip_id: tip,
        reported_result: result,
        diagnostics: codes(&group_flags),
    })
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    #[test]
    fn work_budgets_reserve_before_growth_and_fail_atomically() {
        let mut budget = Budget::default();
        budget
            .reserve(MAX_RELATIONSHIP_REFERENCES, MAX_RELATIONSHIP_MATCHES)
            .unwrap();
        assert_eq!(budget.reserve(1, 0), Err(RelationshipError::Limit));
        assert_eq!(budget.reserve(0, 1), Err(RelationshipError::Limit));
        assert_eq!(budget.reserve(usize::MAX, 0), Err(RelationshipError::Limit));
        assert_eq!(budget.references, MAX_RELATIONSHIP_REFERENCES);
        assert_eq!(budget.matches, MAX_RELATIONSHIP_MATCHES);
    }
    #[test]
    fn each_closed_core_diagnostic_blocks_candidate_selection() {
        for diagnostic in 0..15 {
            let mut flags = [false; 16];
            flags[diagnostic] = true;
            assert!(core(&flags));
        }
        let mut history = [false; 16];
        history[15] = true;
        assert!(!core(&history));
    }
}
