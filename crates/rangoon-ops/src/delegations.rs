//! Bounded inspection of reported aliases and explicit parent links, without admission.
use crate::delegation_types::*;
use crate::projection::bounded_json;
use crate::records::{Event, Window};
use crate::relationship_types::ReferenceLink;
use crate::{DecodedWindow, ProjectionError, project_window};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const CODES: [&str; 16] = [
    "structural_incomplete",
    "reference_missing",
    "reference_ambiguous",
    "reference_kind_mismatch",
    "subject_kind_conflict",
    "system_mismatch",
    "self_delegation",
    "self_expansion",
    "parent_actor_mismatch",
    "scope_incomplete",
    "scope_expansion",
    "expiry_incomplete",
    "expiry_expansion",
    "attenuation_missing",
    "delegation_cycle",
    "subject_cycle",
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
const SCOPE_LISTS: [&str; 4] = [
    "environmentIds",
    "operationClasses",
    "resourceIds",
    "policyRevisionIds",
];
type Flags = [bool; 16];
type RecordIndex<'a> = BTreeMap<&'a str, IndexedRecord<'a>>;

struct IndexedRecord<'a> {
    ordinals: Vec<usize>,
    first_ordinal: usize,
    kind: &'a str,
    ambiguous: bool,
}
type Selector<'a> = (&'static str, Option<usize>, &'a str, &'static str);

struct Observation<'a> {
    event: &'a Event,
    value: Value,
    bytes: Vec<u8>,
}
impl Observation<'_> {
    fn id(&self) -> &str {
        text(&self.value, "id")
    }
    fn kind(&self) -> &str {
        text(&self.value, "kind")
    }
    fn selected(&self) -> bool {
        matches!(self.kind(), "actor" | "delegation")
    }
}

#[derive(Default)]
struct Budget {
    references: usize,
    matches: usize,
    path_steps: usize,
}
impl Budget {
    fn reserve(
        &mut self,
        refs: usize,
        matches: usize,
        steps: usize,
    ) -> Result<(), DelegationError> {
        let counts = [self.references, self.matches, self.path_steps];
        let adds = [refs, matches, steps];
        let limits = [
            MAX_DELEGATION_REFERENCES,
            MAX_DELEGATION_MATCHES,
            MAX_DELEGATION_PATH_STEPS,
        ];
        let mut next = [0; 3];
        for i in 0..3 {
            next[i] = counts[i]
                .checked_add(adds[i])
                .ok_or(DelegationError::Limit)?;
            if next[i] > limits[i] {
                return Err(DelegationError::Limit);
            }
        }
        self.references = next[0];
        self.matches = next[1];
        self.path_steps = next[2];
        Ok(())
    }
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().expect("private validated selector")
}
fn optional_text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value[key].as_str()
}
fn union(into: &mut Flags, from: &Flags) {
    for (target, flag) in into.iter_mut().zip(from) {
        *target |= flag;
    }
}
fn codes(flags: &Flags) -> Vec<&'static str> {
    CODES
        .iter()
        .zip(flags)
        .filter_map(|(code, set)| set.then_some(*code))
        .collect()
}
fn assessment(flags: &Flags) -> &'static str {
    if [2, 4, 5, 6, 7, 8, 10, 12, 13, 14, 15]
        .iter()
        .any(|i| flags[*i])
    {
        "conflict"
    } else if [0, 1, 3, 9, 11].iter().any(|i| flags[*i]) {
        "incomplete"
    } else {
        "linked_unverified"
    }
}
fn status(id: &str, kind: &str, index: &RecordIndex<'_>) -> &'static str {
    let Some(record) = index.get(id) else {
        return "missing";
    };
    if record.ambiguous {
        "ambiguous"
    } else if record.kind != kind {
        "wrong_kind"
    } else {
        "unique_unverified"
    }
}
fn unique(id: &str, kind: &str, index: &RecordIndex<'_>) -> Option<usize> {
    (status(id, kind, index) == "unique_unverified").then(|| index[id].first_ordinal)
}
fn status_flag(flags: &mut Flags, state: &str) {
    match state {
        "missing" => flags[1] = true,
        "ambiguous" => flags[2] = true,
        "wrong_kind" => flags[3] = true,
        _ => (),
    }
}
fn tuple(value: &Value) -> SubjectTuple {
    SubjectTuple {
        system_id: text(value, "systemId").to_owned(),
        canonical_subject_id: text(value, "canonicalSubjectId").to_owned(),
    }
}
fn actor_tuple(id: &str, index: &RecordIndex<'_>, obs: &[Observation<'_>]) -> Option<SubjectTuple> {
    unique(id, "actor", index).map(|i| tuple(&obs[i].value))
}
fn selectors<'a>(item: &'a Observation<'_>) -> Vec<Selector<'a>> {
    let value = &item.value;
    let mut result = Vec::new();
    if item.kind() == "actor" {
        if let Some(id) = optional_text(value, "identityEvidenceId") {
            result.push(("identityEvidenceId", None, id, "evidence"));
        }
    } else {
        for field in ["delegatorId", "delegateId", "changedByActorId"] {
            result.push((field, None, text(value, field), "actor"));
        }
        if let Some(id) = optional_text(value, "parentDelegationId") {
            result.push(("parentDelegationId", None, id, "delegation"));
        }
        for (i, id) in value["evidenceIds"]
            .as_array()
            .expect("private list")
            .iter()
            .enumerate()
        {
            result.push((
                "evidenceIds",
                Some(i),
                id.as_str().expect("private identity"),
                "evidence",
            ));
        }
    }
    result
}
fn scope_set<'a>(value: &'a Value, key: &str) -> BTreeSet<&'a str> {
    value[key]
        .as_array()
        .expect("private scope list")
        .iter()
        .map(|id| id.as_str().expect("private scope member"))
        .collect()
}
fn finite_checks(child: &Value, parent: Option<&Value>, flags: &mut Flags) {
    let scope = &child["scope"];
    flags[9] |= SCOPE_LISTS
        .iter()
        .any(|key| scope_set(scope, key).is_empty());
    let expiry = child["expiresMs"].as_u64();
    flags[11] |= expiry.is_none();
    let Some(parent) = parent else {
        return;
    };
    let parent_scope = &parent["scope"];
    flags[5] |= scope["systemId"] != parent_scope["systemId"];
    let mut complete = true;
    let mut expansion = false;
    let mut attenuated = false;
    for key in SCOPE_LISTS {
        let child_set = scope_set(scope, key);
        let parent_set = scope_set(parent_scope, key);
        complete &= !child_set.is_empty() && !parent_set.is_empty();
        let subset = child_set.is_subset(&parent_set);
        expansion |= !subset;
        attenuated |= subset && child_set != parent_set;
    }
    flags[9] |= !complete;
    flags[10] |= expansion;
    let parent_expiry = parent["expiresMs"].as_u64();
    flags[11] |= parent_expiry.is_none();
    if let (Some(child_expiry), Some(parent_expiry)) = (expiry, parent_expiry) {
        flags[12] |= child_expiry > parent_expiry;
        attenuated |= child_expiry < parent_expiry;
        if complete && !expansion && child_expiry <= parent_expiry && !attenuated {
            flags[13] = true;
        }
    }
}

/// Inspects reported identity and explicit delegation links without issuing authority.
pub fn inspect_delegations(
    decoded: &DecodedWindow,
) -> Result<DelegationInspection, DelegationError> {
    let projection = project_window(decoded).map_err(|error| match error {
        ProjectionError::Limit => DelegationError::StructuralLimit,
        ProjectionError::SerializationFailed => DelegationError::SerializationFailed,
    })?;
    let window: Window = serde_json::from_slice(decoded.canonical_bytes())
        .map_err(|_| DelegationError::SerializationFailed)?;
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
                    .map_err(|_| DelegationError::SerializationFailed)?,
                bytes: serde_json::to_vec(&event.record)
                    .map_err(|_| DelegationError::SerializationFailed)?,
            })
        })
        .collect::<Result<_, DelegationError>>()?;
    let mut index = BTreeMap::new();
    for (i, item) in obs.iter().enumerate() {
        let record = index.entry(item.id()).or_insert_with(|| IndexedRecord {
            ordinals: Vec::new(),
            first_ordinal: i,
            kind: item.kind(),
            ambiguous: false,
        });
        record.ambiguous |= item.bytes != obs[record.first_ordinal].bytes;
        record.ordinals.push(i);
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
    let stream_bad = |i: usize| {
        let event = obs[i].event;
        global || tainted.contains(&(event.producer_id.as_str(), event.fence_id.as_str()))
    };
    let mut flags = vec![[false; 16]; obs.len()];
    let mut references: Vec<Vec<ReferenceLink>> = (0..obs.len()).map(|_| Vec::new()).collect();
    let mut budget = Budget::default();
    for (i, item) in obs.iter().enumerate().filter(|(_, item)| item.selected()) {
        flags[i][0] = stream_bad(i);
        status_flag(&mut flags[i], status(item.id(), item.kind(), &index));
        for (field, list_index, id, kind) in selectors(item) {
            let matches = index
                .get(id)
                .map(|record| record.ordinals.as_slice())
                .unwrap_or(&[]);
            budget.reserve(1, matches.len(), 0)?;
            let state = status(id, kind, &index);
            status_flag(&mut flags[i], state);
            flags[i][0] |= matches.iter().any(|ordinal| stream_bad(*ordinal));
            references[i].push(ReferenceLink {
                field,
                index: list_index,
                record_id: id.to_owned(),
                expected_kind: kind,
                status: state,
                matching_event_ordinals: matches.to_vec(),
            });
        }
    }
    // Aggregate all aliases before any delegation inspects a reported subject.
    let mut subject_groups: BTreeMap<SubjectTuple, Vec<usize>> = BTreeMap::new();
    for (i, item) in obs
        .iter()
        .enumerate()
        .filter(|(_, item)| item.kind() == "actor")
    {
        subject_groups
            .entry(tuple(&item.value))
            .or_default()
            .push(i);
    }
    let mut subject_flags = BTreeMap::new();
    let mut subjects = Vec::new();
    for (subject, ordinals) in &subject_groups {
        budget.reserve(0, ordinals.len(), 0)?;
        let mut aggregate = [false; 16];
        let mut ids = BTreeSet::new();
        let mut kinds = BTreeSet::new();
        for i in ordinals {
            union(&mut aggregate, &flags[*i]);
            ids.insert(obs[*i].id().to_owned());
            kinds.insert(text(&obs[*i].value, "actorKind").to_owned());
        }
        aggregate[4] = kinds.len() > 1;
        for i in ordinals {
            union(&mut flags[*i], &aggregate);
        }
        subjects.push(SubjectInspection {
            system_id: subject.system_id.clone(),
            canonical_subject_id: subject.canonical_subject_id.clone(),
            actor_record_ids: ids.into_iter().collect(),
            reported_actor_kinds: kinds.into_iter().collect(),
            matching_event_ordinals: ordinals.clone(),
            assessment: assessment(&aggregate),
            diagnostics: codes(&aggregate),
        });
        subject_flags.insert(subject.clone(), aggregate);
    }
    let mut bindings = vec![[None, None, None]; obs.len()];
    for (i, item) in obs
        .iter()
        .enumerate()
        .filter(|(_, item)| item.kind() == "delegation")
    {
        let actors = ["delegatorId", "delegateId", "changedByActorId"];
        for (slot, field) in actors.iter().enumerate() {
            let subject = actor_tuple(text(&item.value, field), &index, &obs);
            if let Some(subject) = &subject {
                union(&mut flags[i], &subject_flags[subject]);
                flags[i][5] |= subject.system_id != text(&item.value["scope"], "systemId");
            }
            bindings[i][slot] = subject;
        }
        if let (Some(delegator), Some(delegate)) = (&bindings[i][0], &bindings[i][1]) {
            flags[i][6] |= delegator == delegate;
        }
        if let (Some(changed_by), Some(delegate)) = (&bindings[i][2], &bindings[i][1]) {
            flags[i][7] |= changed_by == delegate;
        }
        finite_checks(&item.value, None, &mut flags[i]);
    }
    // Complete every local parent comparison before propagating ancestor diagnostics.
    for (i, item) in obs
        .iter()
        .enumerate()
        .filter(|(_, item)| item.kind() == "delegation")
    {
        if let Some(parent) = optional_text(&item.value, "parentDelegationId")
            .and_then(|id| unique(id, "delegation", &index))
        {
            finite_checks(&item.value, Some(&obs[parent].value), &mut flags[i]);
            if let (Some(delegator), Some(parent_delegate)) =
                (&bindings[i][0], &bindings[parent][1])
            {
                flags[i][8] |= delegator != parent_delegate;
            }
        }
    }
    let mut aggregate_delegations = BTreeMap::new();
    for (id, record) in &index {
        if unique(id, "delegation", &index).is_some() {
            let mut aggregate = [false; 16];
            for i in &record.ordinals {
                union(&mut aggregate, &flags[*i]);
            }
            aggregate_delegations.insert(*id, aggregate);
        }
    }
    let mut ancestors: Vec<Vec<String>> = (0..obs.len()).map(|_| Vec::new()).collect();
    for (i, item) in obs
        .iter()
        .enumerate()
        .filter(|(_, item)| item.kind() == "delegation")
    {
        let mut current = item.id();
        let mut seen_ids = BTreeSet::new();
        let mut seen_subjects = BTreeSet::new();
        if let Some(delegate) = &bindings[i][1] {
            seen_subjects.insert(delegate.clone());
        }
        loop {
            budget.reserve(0, 0, 1)?;
            ancestors[i].push(current.to_owned());
            if !seen_ids.insert(current) {
                flags[i][14] = true;
                break;
            }
            let Some(ordinal) = unique(current, "delegation", &index) else {
                break;
            };
            union(&mut flags[i], &aggregate_delegations[current]);
            if let Some(delegator) = &bindings[ordinal][0] {
                if !seen_subjects.insert(delegator.clone()) {
                    flags[i][15] = true;
                }
            }
            let Some(parent) = optional_text(&obs[ordinal].value, "parentDelegationId") else {
                break;
            };
            current = parent;
        }
    }
    let mut counts = [0; 16];
    for subject in &subjects {
        for code in &subject.diagnostics {
            counts[CODES
                .iter()
                .position(|candidate| candidate == code)
                .expect("closed diagnostic")] += 1;
        }
    }
    let event_assessments = obs
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let selected = item.selected();
            if selected {
                for (count, flag) in counts.iter_mut().zip(&flags[i]) {
                    *count += usize::from(*flag);
                }
            }
            DelegationEvent {
                input_ordinal: i,
                event_id: item.event.event_id.clone(),
                record_id: item.id().to_owned(),
                record_kind: item.kind().to_owned(),
                assessment: if selected {
                    assessment(&flags[i])
                } else {
                    "not_assessed"
                },
                reported_state: (item.kind() == "delegation")
                    .then(|| text(&item.value, "state").to_owned()),
                reported_verification: selected
                    .then(|| text(&item.value, "reportedVerification").to_owned()),
                reported_freshness: selected.then(|| text(&item.value, "freshness").to_owned()),
                subject: (item.kind() == "actor").then(|| tuple(&item.value)),
                delegator_subject: bindings[i][0].take(),
                delegate_subject: bindings[i][1].take(),
                changed_by_subject: bindings[i][2].take(),
                ancestor_record_ids: std::mem::take(&mut ancestors[i]),
                diagnostics: if selected {
                    codes(&flags[i])
                } else {
                    Vec::new()
                },
                references: std::mem::take(&mut references[i]),
            }
        })
        .collect();
    let document = DelegationDocument {
        schema_version: DELEGATION_SCHEMA,
        input_canonical_digest: decoded.canonical_digest().to_owned(),
        structural_projection_digest: projection.canonical_digest(),
        authority: "none",
        authenticity: "unverified",
        execution: "unavailable",
        semantic_assessment: "reported_delegation_links",
        authoritative_eligibility: "unknown",
        authoritative_effective_evaluation_id: None,
        subdelegation_admission: "unavailable",
        diagnostic_counts: DelegationCounts::from_array(counts),
        subjects,
        event_assessments,
    };
    let canonical = bounded_json(&document).map_err(|error| match error {
        ProjectionError::Limit => DelegationError::Limit,
        ProjectionError::SerializationFailed => DelegationError::SerializationFailed,
    })?;
    Ok(DelegationInspection {
        canonical,
        document,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delegation_counters_are_independent_atomic_and_checked() {
        for slot in 0..3 {
            let mut budget = Budget::default();
            let mut at = [0; 3];
            at[slot] = 8_192;
            budget.reserve(at[0], at[1], at[2]).unwrap();
            let mut over = [0; 3];
            over[slot] = 1;
            assert_eq!(
                budget.reserve(over[0], over[1], over[2]),
                Err(DelegationError::Limit)
            );
            assert_eq!([budget.references, budget.matches, budget.path_steps], at);
            assert_eq!(
                budget.reserve(usize::MAX, usize::MAX, usize::MAX),
                Err(DelegationError::Limit)
            );
            assert_eq!([budget.references, budget.matches, budget.path_steps], at);
        }
    }
}
