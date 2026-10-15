//! Bounded, deterministic reported duties and ballots, without authentication or admission.
use crate::approval_types::*;
use crate::delegation_types::{DelegationDocument, SubjectTuple};
use crate::projection::bounded_json;
use crate::records::{Event, Window};
use crate::relationship_types::{ReferenceLink, RelationshipDocument};
use crate::{DecodedWindow, DelegationError, ProjectionError, RelationshipError};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const CODES: [&str; 32] = [
    "structural_incomplete",
    "reference_missing",
    "reference_ambiguous",
    "reference_kind_mismatch",
    "relationship_incomplete",
    "subject_incomplete",
    "stage_identity_conflict",
    "stage_dependency_missing",
    "stage_dependency_cycle",
    "approver_kind_unsupported",
    "approver_alias_duplicate",
    "quorum_invalid",
    "decision_binding_mismatch",
    "decision_actor_not_listed",
    "requester_self_approval",
    "agent_delegation_missing",
    "delegation_incomplete",
    "delegation_binding_mismatch",
    "decision_lineage_mismatch",
    "decision_sequence_mismatch",
    "decision_branch",
    "decision_cycle",
    "decision_incomplete_chain",
    "human_report_missing",
    "reported_vote_shortfall",
    "non_tip_decision",
    "dependency_report_shortfall",
    "stage_expiry_expansion",
    "expiry_incomplete",
    "delegation_report_inactive",
    "actor_report_inactive",
    "human_stage_missing",
];
const INCOMPLETE: [usize; 15] = [0, 1, 3, 4, 5, 7, 15, 16, 22, 23, 24, 26, 28, 29, 30];
const NONBLOCKING: [usize; 6] = [10, 23, 24, 25, 26, 28];
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
type Flags = [bool; 32];

#[derive(Default)]
struct Budget {
    stages: usize,
    references: usize,
    matches: usize,
    steps: usize,
}
impl Budget {
    fn reserve(
        &mut self,
        stages: usize,
        references: usize,
        matches: usize,
        steps: usize,
    ) -> Result<(), ApprovalError> {
        let old = [self.stages, self.references, self.matches, self.steps];
        let add = [stages, references, matches, steps];
        let caps = [
            MAX_APPROVAL_STAGES,
            MAX_APPROVAL_REFERENCES,
            MAX_APPROVAL_MATCHES,
            MAX_APPROVAL_PATH_STEPS,
        ];
        let mut next = [0; 4];
        for i in 0..4 {
            next[i] = old[i].checked_add(add[i]).ok_or(ApprovalError::Limit)?;
            if next[i] > caps[i] {
                return Err(ApprovalError::Limit);
            }
        }
        [self.stages, self.references, self.matches, self.steps] = next;
        Ok(())
    }
}
fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().expect("private validated selector")
}
fn list<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v[key].as_array().expect("private validated list")
}
fn union(a: &mut Flags, b: &Flags) {
    for (x, y) in a.iter_mut().zip(b) {
        *x |= y;
    }
}
fn codes(f: &Flags) -> Vec<&'static str> {
    CODES
        .iter()
        .zip(f)
        .filter_map(|(c, v)| v.then_some(*c))
        .collect()
}
fn blocked(f: &Flags) -> bool {
    f.iter()
        .enumerate()
        .any(|(i, v)| *v && !NONBLOCKING.contains(&i))
}
fn assessment(f: &Flags) -> &'static str {
    if f.iter()
        .enumerate()
        .any(|(i, v)| *v && i != 10 && i != 25 && !INCOMPLETE.contains(&i))
    {
        "conflict"
    } else if INCOMPLETE.iter().any(|i| f[*i]) {
        "incomplete"
    } else {
        "linked_unverified"
    }
}
fn status_flag(f: &mut Flags, status: &str) {
    match status {
        "missing" => f[1] = true,
        "ambiguous" => f[2] = true,
        "wrong_kind" => f[3] = true,
        _ => (),
    }
}
fn relationship_flags(f: &mut Flags, diagnostics: &[&str]) {
    for c in diagnostics {
        match *c {
            "structural_incomplete" => f[0] = true,
            "reference_missing" => f[1] = true,
            "reference_ambiguous" => f[2] = true,
            "reference_kind_mismatch" => f[3] = true,
            "non_tip_evaluation" => (),
            _ => f[4] = true,
        }
    }
}
fn delegation_flags(f: &mut Flags, diagnostics: &[&str], subject: bool) {
    for c in diagnostics {
        match *c {
            "structural_incomplete" => f[0] = true,
            "reference_missing" => f[1] = true,
            "reference_ambiguous" => f[2] = true,
            "reference_kind_mismatch" => f[3] = true,
            "subject_kind_conflict" => f[9] = true,
            _ if subject => f[5] = true,
            "scope_incomplete" | "expiry_incomplete" => f[16] = true,
            _ => f[17] = true,
        }
    }
}
fn same_flow(a: &Value, b: &Value) -> bool {
    FLOW_KEYS.iter().all(|k| a[k] == b[k])
}
fn tuple(v: &Value) -> SubjectTuple {
    SubjectTuple {
        system_id: text(v, "systemId").to_owned(),
        canonical_subject_id: text(v, "canonicalSubjectId").to_owned(),
    }
}
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
        matches!(self.kind(), "approval_plan" | "approval_decision")
    }
}
struct IndexedRecord {
    first: usize,
    kind: String,
    ambiguous: bool,
    ordinals: Vec<usize>,
}
struct SubjectMeta {
    flags: Flags,
    kind: Option<String>,
}
struct Inspector<'a> {
    observations: Vec<Observation<'a>>,
    index: BTreeMap<String, IndexedRecord>,
    tainted: Vec<bool>,
    subjects: BTreeMap<SubjectTuple, SubjectMeta>,
    relationships: &'a RelationshipDocument,
    delegations: &'a DelegationDocument,
}
impl Inspector<'_> {
    fn status(&self, id: &str, kind: &str) -> &'static str {
        match self.index.get(id) {
            None => "missing",
            Some(r) if r.ambiguous => "ambiguous",
            Some(r) if r.kind != kind => "wrong_kind",
            Some(_) => "unique_unverified",
        }
    }
    fn unique(&self, id: &str, kind: &str) -> Option<usize> {
        (self.status(id, kind) == "unique_unverified").then(|| self.index[id].first)
    }
    fn value(&self, id: &str, kind: &str) -> Option<&Value> {
        self.unique(id, kind).map(|i| &self.observations[i].value)
    }
    fn reference(
        &self,
        budget: &mut Budget,
        flags: &mut Flags,
        field: &'static str,
        index: Option<usize>,
        id: &str,
        kind: &'static str,
    ) -> Result<ReferenceLink, ApprovalError> {
        let ords = self
            .index
            .get(id)
            .map(|r| r.ordinals.as_slice())
            .unwrap_or(&[]);
        budget.reserve(0, 1, ords.len(), 0)?;
        let status = self.status(id, kind);
        status_flag(flags, status);
        flags[0] |= ords.iter().any(|i| self.tainted[*i]);
        Ok(ReferenceLink {
            field,
            index,
            record_id: id.to_owned(),
            expected_kind: kind,
            status,
            matching_event_ordinals: ords.to_vec(),
        })
    }
    fn actor(&self, id: &str, flags: &mut Flags) -> Option<SubjectTuple> {
        status_flag(flags, self.status(id, "actor"));
        let Some(i) = self.unique(id, "actor") else {
            flags[5] = true;
            return None;
        };
        let subject = tuple(&self.observations[i].value);
        union(flags, &self.subjects[&subject].flags);
        Some(subject)
    }
    fn related_flags(&self, id: &str, kind: &str, flags: &mut Flags) {
        status_flag(flags, self.status(id, kind));
        let Some(record) = self.index.get(id) else {
            return;
        };
        for i in &record.ordinals {
            flags[0] |= self.tainted[*i];
            relationship_flags(flags, &self.relationships.event_links[*i].diagnostics);
        }
    }
    fn header(
        &self,
        i: usize,
        budget: &mut Budget,
    ) -> Result<(Flags, Vec<ReferenceLink>), ApprovalError> {
        let o = &self.observations[i];
        let v = &o.value;
        let context = &v["context"];
        let mut flags = [false; 32];
        flags[0] = self.tainted[i];
        status_flag(&mut flags, self.status(o.id(), o.kind()));
        let inherited = &self.relationships.event_links[i];
        relationship_flags(&mut flags, &inherited.diagnostics);
        let mut references = Vec::new();
        for r in &inherited.references {
            budget.reserve(0, 1, r.matching_event_ordinals.len(), 0)?;
            status_flag(&mut flags, r.status);
            flags[0] |= r.matching_event_ordinals.iter().any(|j| self.tainted[*j]);
            references.push(r.clone());
        }
        let eval_id = text(v, "evaluationId");
        self.related_flags(eval_id, "evaluation", &mut flags);
        if let Some(eval) = self.value(eval_id, "evaluation") {
            references.push(self.reference(
                budget,
                &mut flags,
                "evaluation.gatewayRevisionId",
                None,
                text(eval, "gatewayRevisionId"),
                "gateway_revision",
            )?);
            if o.kind() == "approval_plan" {
                if let Some(gateway) =
                    self.value(text(eval, "gatewayRevisionId"), "gateway_revision")
                {
                    flags[31] = gateway["humanRequired"] == true
                        && !list(v, "stages").iter().any(|s| s["humanRequired"] == true);
                } else {
                    flags[4] = true;
                }
            }
        } else {
            flags[4] = true;
        }
        let req_id = text(context, "requestId");
        self.related_flags(req_id, "request", &mut flags);
        if let Some(req) = self.value(req_id, "request") {
            references.push(self.reference(
                budget,
                &mut flags,
                "request.actorId",
                None,
                text(req, "actorId"),
                "actor",
            )?);
            for (j, resource) in list(req, "resourceIds").iter().enumerate() {
                references.push(self.reference(
                    budget,
                    &mut flags,
                    "request.resourceIds",
                    Some(j),
                    resource.as_str().expect("private identity"),
                    "resource",
                )?);
            }
        } else {
            flags[4] = true;
        }
        if self.relationships.chains.iter().any(|c| {
            c.request_id == req_id
                && c.engine_id == text(context, "engineId")
                && c.reported_chain_status == "unknown_or_conflict"
        }) {
            flags[4] = true;
        }
        if o.kind() == "approval_decision" {
            if let Some(d) = v["delegationId"]
                .as_str()
                .and_then(|id| self.value(id, "delegation"))
            {
                references.push(self.reference(
                    budget,
                    &mut flags,
                    "delegation.delegateId",
                    None,
                    text(d, "delegateId"),
                    "actor",
                )?);
            }
        }
        Ok((flags, references))
    }
}

struct StageWork {
    declaration: Flags,
    local: Flags,
    final_flags: Flags,
    row: ApprovalStage,
}
fn stage_map(stages: &[Value]) -> BTreeMap<&str, Vec<usize>> {
    let mut map = BTreeMap::new();
    for (i, s) in stages.iter().enumerate() {
        map.entry(text(s, "id")).or_insert_with(Vec::new).push(i);
    }
    map
}
fn declarations(
    ctx: &Inspector<'_>,
    i: usize,
    header: Flags,
    budget: &mut Budget,
) -> Result<Vec<StageWork>, ApprovalError> {
    let plan = &ctx.observations[i].value;
    let stages = list(plan, "stages");
    let stage_ids = stage_map(stages);
    let mut output = Vec::new();
    for (j, stage) in stages.iter().enumerate() {
        budget.reserve(1, 0, 0, 0)?;
        let mut flags = header;
        flags[6] |= stage_ids[text(stage, "id")].len() > 1;
        let mut approvers = BTreeSet::new();
        let mut escalations = BTreeSet::new();
        let mut approver_refs = Vec::new();
        let mut escalation_refs = Vec::new();
        let mut known = true;
        for (field, members, refs, normal) in [
            ("approverIds", &mut approvers, &mut approver_refs, true),
            (
                "escalationActorIds",
                &mut escalations,
                &mut escalation_refs,
                false,
            ),
        ] {
            for (k, id) in list(stage, field).iter().enumerate() {
                let id = id.as_str().expect("private actor identity");
                refs.push(ctx.reference(budget, &mut flags, field, Some(k), id, "actor")?);
                if let Some(subject) = ctx.actor(id, &mut flags) {
                    flags[12] |= subject.system_id != text(&plan["context"], "systemId");
                    let meta = &ctx.subjects[&subject];
                    if normal {
                        known &= meta
                            .kind
                            .as_deref()
                            .is_some_and(|k| matches!(k, "human" | "agent"));
                        flags[9] |= meta
                            .kind
                            .as_deref()
                            .is_some_and(|k| !matches!(k, "human" | "agent"));
                        flags[10] |= members.contains(&subject);
                    }
                    members.insert(subject);
                } else if normal {
                    known = false;
                }
            }
        }
        let mode = text(stage, "mode");
        let quorum = stage["quorum"].as_u64();
        let required = if mode == "all" {
            flags[11] |= quorum.is_some() || list(stage, "approverIds").is_empty();
            (known && !flags[11]).then_some(approvers.len() as u64)
        } else {
            flags[11] |= quorum.is_none_or(|n| n == 0);
            if known {
                flags[11] |= quorum.is_some_and(|n| n > approvers.len() as u64);
            }
            (known && !flags[11]).then_some(quorum).flatten()
        };
        let expires = stage["expiresMs"].as_u64();
        let plan_expires = plan["expiresMs"].as_u64();
        flags[28] |= expires.is_none() || plan_expires.is_none();
        if let (Some(child), Some(parent)) = (expires, plan_expires) {
            flags[27] |= child > parent;
        }
        let mut dependency_refs = Vec::new();
        for (k, id) in list(stage, "dependsOnStageIds").iter().enumerate() {
            let id = id.as_str().expect("private stage identity");
            let matches = stage_ids.get(id).map(Vec::as_slice).unwrap_or(&[]);
            budget.reserve(0, 1, matches.len(), 0)?;
            let status = match matches.len() {
                0 => "missing",
                1 => "unique_unverified",
                _ => "ambiguous",
            };
            flags[7] |= status == "missing";
            flags[2] |= status == "ambiguous";
            dependency_refs.push(StageReference {
                index: k,
                stage_id: id.to_owned(),
                status,
                matching_stage_indices: matches.to_vec(),
            });
        }
        output.push(StageWork {
            declaration: flags,
            local: flags,
            final_flags: flags,
            row: ApprovalStage {
                stage_input_index: j,
                stage_id: text(stage, "id").to_owned(),
                mode: mode.to_owned(),
                reported_quorum: quorum,
                reported_human_required: stage["humanRequired"].as_bool().expect("private boolean"),
                reported_expires_ms: expires,
                required_reported_subject_count: required,
                reported_votes: None,
                authoritative_duty_satisfaction: "unknown",
                assessment: "linked_unverified",
                diagnostics: Vec::new(),
                reported_approver_subjects: approvers.into_iter().collect(),
                reported_escalation_subjects: escalations.into_iter().collect(),
                approver_references: approver_refs,
                escalation_references: escalation_refs,
                dependency_references: dependency_refs,
            },
        });
    }
    Ok(output)
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct GroupKey {
    plan: String,
    evaluation: String,
    stage: String,
    subject: SubjectTuple,
}
struct ChainWork {
    key: GroupKey,
    flags: Flags,
    row: DecisionChain,
}
fn decision_local(
    ctx: &Inspector<'_>,
    i: usize,
    headers: &[Flags],
    plans: &[Vec<StageWork>],
    flags: &mut Flags,
) -> Option<GroupKey> {
    let v = &ctx.observations[i].value;
    let context = &v["context"];
    let subject = ctx.actor(text(v, "actorId"), flags);
    let role = subject
        .as_ref()
        .and_then(|s| ctx.subjects[s].kind.as_deref());
    if let Some(subject) = &subject {
        flags[12] |= subject.system_id != text(context, "systemId");
    }
    flags[9] |= role.is_some_and(|k| !matches!(k, "human" | "agent"));
    let plan_id = text(v, "planId");
    let plan_i = ctx.unique(plan_id, "approval_plan");
    if let Some(p) = plan_i {
        union(flags, &headers[p]);
        let plan = &ctx.observations[p].value;
        flags[12] |=
            plan["evaluationId"] != v["evaluationId"] || !same_flow(&plan["context"], context);
        let matches: Vec<_> = plans[p]
            .iter()
            .filter(|s| s.row.stage_id == text(v, "stageId"))
            .collect();
        match matches.as_slice() {
            [] => {
                flags[12] = true;
                flags[22] = true;
            }
            [stage] => {
                union(flags, &stage.declaration);
                if let Some(subject) = &subject {
                    flags[13] |= !stage.row.reported_approver_subjects.contains(subject);
                }
            }
            _ => {
                flags[6] = true;
                flags[22] = true;
            }
        }
    } else {
        status_flag(flags, ctx.status(plan_id, "approval_plan"));
        flags[22] = true;
    }
    if ctx.unique(text(v, "evaluationId"), "evaluation").is_none() {
        flags[22] = true;
    }
    let request = ctx.value(text(context, "requestId"), "request");
    if let Some(req) = request {
        let requester = ctx.actor(text(req, "actorId"), flags);
        let requester = requester.filter(|s| {
            let bound = s.system_id == text(&req["context"], "systemId");
            flags[5] |= !bound;
            bound
        });
        if let (Some(actor), Some(requester)) = (&subject, &requester) {
            flags[14] |= v["decision"] == "approve" && actor == requester;
        }
    } else {
        flags[5] = true;
    }
    let delegation_id = v["delegationId"].as_str();
    if role == Some("agent") && delegation_id.is_none() {
        flags[15] = true;
    }
    if let Some(id) = delegation_id {
        if let Some(d_i) = ctx.unique(id, "delegation") {
            let d = &ctx.observations[d_i].value;
            // Inspect every identical observation, not merely the first clean copy.
            for ordinal in &ctx.index[id].ordinals {
                delegation_flags(
                    flags,
                    &ctx.delegations.event_assessments[*ordinal].diagnostics,
                    false,
                );
                flags[0] |= ctx.tainted[*ordinal];
            }
            let delegate = ctx.actor(text(d, "delegateId"), flags);
            if let (Some(actor), Some(delegate)) = (&subject, &delegate) {
                flags[17] |= actor != delegate;
            }
            if delegate.is_none() {
                flags[16] = true;
            }
            flags[29] |= text(d, "state") != "reported_active"
                || d["reportedVerification"] == "reported_rejected"
                || d["freshness"] == "reported_stale";
            if let Some(req) = request {
                let scope = &d["scope"];
                let resources = list(req, "resourceIds");
                flags[16] |= resources.is_empty();
                flags[17] |= scope["systemId"] != req["context"]["systemId"]
                    || !list(scope, "environmentIds").contains(&req["context"]["environmentId"])
                    || !list(scope, "operationClasses").contains(&req["operationClass"])
                    || !list(scope, "policyRevisionIds")
                        .contains(&req["context"]["policyRevisionId"])
                    || resources
                        .iter()
                        .any(|r| !list(scope, "resourceIds").contains(r));
            } else {
                flags[16] = true;
            }
        } else {
            status_flag(flags, ctx.status(id, "delegation"));
            flags[16] = true;
        }
    }
    subject.map(|subject| GroupKey {
        plan: plan_id.to_owned(),
        evaluation: text(v, "evaluationId").to_owned(),
        stage: text(v, "stageId").to_owned(),
        subject,
    })
}
fn aggregate_identical(ctx: &Inspector<'_>, flags: &mut [Flags]) {
    for record in ctx.index.values().filter(|r| !r.ambiguous) {
        let mut aggregate = [false; 32];
        for i in &record.ordinals {
            union(&mut aggregate, &flags[*i]);
        }
        for i in &record.ordinals {
            union(&mut flags[*i], &aggregate);
        }
    }
}
fn chain(
    ctx: &Inspector<'_>,
    key: GroupKey,
    ordinals: Vec<usize>,
    keys: &[Option<GroupKey>],
    event_flags: &mut [Flags],
    budget: &mut Budget,
) -> Result<ChainWork, ApprovalError> {
    budget.reserve(0, 0, ordinals.len(), 0)?;
    let mut flags = [false; 32];
    let mut nodes = BTreeMap::new();
    let mut streams = BTreeSet::new();
    for i in &ordinals {
        union(&mut flags, &event_flags[*i]);
        let o = &ctx.observations[*i];
        nodes.entry(o.id()).or_insert(*i);
        streams.insert((&o.event.producer_id, &o.event.fence_id));
    }
    flags[18] |= streams.len() != 1;
    let mut roots = Vec::new();
    let mut successors: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (id, i) in &nodes {
        let v = &ctx.observations[*i].value;
        if let Some(pred_id) = v["supersedesDecisionId"].as_str() {
            status_flag(&mut flags, ctx.status(pred_id, "approval_decision"));
            successors.entry(pred_id).or_default().insert(*id);
            if let Some(p) = ctx.unique(pred_id, "approval_decision") {
                let pred = &ctx.observations[p];
                let bound = keys[p].as_ref() == Some(&key)
                    && same_flow(&pred.value["context"], &v["context"]);
                flags[12] |= !bound;
                flags[22] |= !bound;
                let first_pred_seq = ctx.index[pred_id]
                    .ordinals
                    .iter()
                    .map(|j| ctx.observations[*j].event.sequence)
                    .min()
                    .expect("private nonempty index");
                let first_seq = ctx.index[*id]
                    .ordinals
                    .iter()
                    .map(|j| ctx.observations[*j].event.sequence)
                    .min()
                    .expect("private nonempty index");
                flags[19] |= first_seq <= first_pred_seq;
            } else {
                flags[22] = true;
            }
        } else {
            roots.push(*id);
        }
    }
    flags[20] |= roots.len() > 1 || successors.values().any(|v| v.len() > 1);
    flags[22] |= roots.len() != 1;
    // Required predecessor inspection charges each visit, including closing/dangling IDs.
    for id in nodes.keys() {
        let mut current = *id;
        let mut active = BTreeSet::new();
        loop {
            budget.reserve(0, 0, 0, 1)?;
            if !active.insert(current) {
                flags[21] = true;
                break;
            }
            let Some(i) = ctx.unique(current, "approval_decision") else {
                flags[22] = true;
                break;
            };
            if keys[i].as_ref() != Some(&key) {
                flags[12] = true;
                flags[22] = true;
                break;
            }
            let Some(parent) = ctx.observations[i].value["supersedesDecisionId"].as_str() else {
                break;
            };
            current = parent;
        }
    }
    let mut ids: Vec<String> = nodes.keys().map(|s| (*s).to_owned()).collect();
    let mut tip = None;
    let mut result = None;
    if !blocked(&flags) {
        let mut current = roots[0];
        let mut ordered = Vec::new();
        let mut seen = BTreeSet::new();
        loop {
            budget.reserve(0, 0, 0, 1)?;
            if !seen.insert(current) {
                flags[21] = true;
                break;
            }
            ordered.push(current.to_owned());
            let Some(next) = successors.get(current).and_then(|s| s.first()).copied() else {
                break;
            };
            current = next;
        }
        if ordered.len() != nodes.len() {
            flags[22] = true;
        }
        if !blocked(&flags) {
            tip = Some(current.to_owned());
            result = Some(text(&ctx.observations[nodes[current]].value, "decision").to_owned());
            ids = ordered;
            if ids.len() > 1 {
                flags[25] = true;
            }
        }
    }
    for i in &ordinals {
        union(&mut event_flags[*i], &flags);
        // The chain summary has the old-node information; only old event rows carry it.
        event_flags[*i][25] = tip
            .as_deref()
            .is_some_and(|id| id != ctx.observations[*i].id());
    }
    let kind = ctx.subjects[&key.subject].kind.clone();
    let row = DecisionChain {
        plan_id: key.plan.clone(),
        evaluation_id: key.evaluation.clone(),
        stage_id: key.stage.clone(),
        subject: key.subject.clone(),
        decision_record_ids: ids,
        matching_event_ordinals: ordinals,
        reported_chain_status: if tip.is_some() {
            "single_chain_unverified"
        } else {
            "unknown_or_conflict"
        },
        reported_chain_tip_id: tip,
        reported_decision: result,
        reported_actor_kind: kind,
        diagnostics: codes(&flags),
    };
    Ok(ChainWork { key, flags, row })
}
fn local_votes(
    ctx: &Inspector<'_>,
    plans: &mut [Vec<StageWork>],
    flags: &[Flags],
    chains: &[ChainWork],
) {
    for (i, stages) in plans.iter_mut().enumerate().filter(|(_, s)| !s.is_empty()) {
        let plan_id = ctx.observations[i].id();
        for stage in stages {
            let mut local = stage.declaration;
            for (j, o) in ctx
                .observations
                .iter()
                .enumerate()
                .filter(|(_, o)| o.kind() == "approval_decision")
            {
                if o.value["planId"] == plan_id && o.value["stageId"] == stage.row.stage_id {
                    union(&mut local, &flags[j]);
                }
            }
            let mut votes = ReportedVotes::default();
            for c in chains
                .iter()
                .filter(|c| c.key.plan == plan_id && c.key.stage == stage.row.stage_id)
            {
                union(&mut local, &c.flags);
                match c.row.reported_decision.as_deref() {
                    Some("approve") => {
                        votes.approve_subjects += 1;
                        votes.human_approve_subjects +=
                            usize::from(c.row.reported_actor_kind.as_deref() == Some("human"));
                        votes.agent_approve_subjects +=
                            usize::from(c.row.reported_actor_kind.as_deref() == Some("agent"));
                    }
                    Some("deny") => votes.deny_subjects += 1,
                    Some("abstain") => votes.abstain_subjects += 1,
                    _ => (),
                }
            }
            if !blocked(&local) {
                if let Some(required) = stage.row.required_reported_subject_count {
                    local[24] |= votes.approve_subjects < required as usize;
                    local[23] |=
                        stage.row.reported_human_required && votes.human_approve_subjects == 0;
                    stage.row.reported_votes = Some(votes);
                }
            }
            stage.local = local;
            stage.final_flags = local;
        }
    }
}
fn report_shortfall(stage: &StageWork) -> bool {
    match (
        &stage.row.reported_votes,
        stage.row.required_reported_subject_count,
    ) {
        (Some(v), Some(required)) => {
            v.deny_subjects > 0
                || v.approve_subjects < required as usize
                || (stage.row.reported_human_required && v.human_approve_subjects == 0)
        }
        _ => true,
    }
}
fn dependencies(stages: &mut [StageWork], budget: &mut Budget) -> Result<(), ApprovalError> {
    let mut index: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, s) in stages.iter().enumerate() {
        index.entry(s.row.stage_id.clone()).or_default().push(i);
    }
    enum Walk<'a> {
        Enter(&'a str),
        Exit(&'a str),
    }
    let mut final_flags = Vec::new();
    for stage in stages.iter() {
        let mut flags = stage.local;
        let mut active = BTreeSet::from([stage.row.stage_id.as_str()]);
        let mut stack = Vec::new();
        for reference in stage.row.dependency_references.iter().rev() {
            budget.reserve(0, 0, 0, 1)?;
            stack.push(Walk::Enter(reference.stage_id.as_str()));
        }
        while let Some(action) = stack.pop() {
            match action {
                Walk::Exit(id) => {
                    active.remove(id);
                }
                Walk::Enter(id) => {
                    let Some(matches) = index.get(id) else {
                        flags[7] = true;
                        continue;
                    };
                    if matches.len() != 1 {
                        flags[2] = true;
                        for i in matches {
                            union(&mut flags, &stages[*i].local);
                        }
                        continue;
                    }
                    let predecessor = &stages[matches[0]];
                    union(&mut flags, &predecessor.local);
                    flags[26] |= report_shortfall(predecessor);
                    if !active.insert(id) {
                        flags[8] = true;
                        continue;
                    }
                    stack.push(Walk::Exit(id));
                    for reference in predecessor.row.dependency_references.iter().rev() {
                        budget.reserve(0, 0, 0, 1)?;
                        stack.push(Walk::Enter(reference.stage_id.as_str()));
                    }
                }
            }
        }
        final_flags.push(flags);
    }
    for (stage, flags) in stages.iter_mut().zip(final_flags) {
        stage.final_flags = flags;
        if blocked(&flags) {
            stage.row.reported_votes = None;
        }
        stage.row.assessment = assessment(&flags);
        stage.row.diagnostics = codes(&flags);
    }
    Ok(())
}

/// Inspect reported approval structure from one immutable window without satisfying duties.
pub fn inspect_approvals(decoded: &DecodedWindow) -> Result<ApprovalInspection, ApprovalError> {
    let projection = crate::project_window(decoded).map_err(|e| match e {
        ProjectionError::Limit => ApprovalError::StructuralLimit,
        ProjectionError::SerializationFailed => ApprovalError::SerializationFailed,
    })?;
    let relationships = crate::relate_window(decoded).map_err(|e| match e {
        RelationshipError::StructuralLimit => ApprovalError::StructuralLimit,
        RelationshipError::Limit => ApprovalError::RelationshipLimit,
        RelationshipError::SerializationFailed => ApprovalError::SerializationFailed,
    })?;
    let delegations = crate::inspect_delegations(decoded).map_err(|e| match e {
        DelegationError::StructuralLimit => ApprovalError::StructuralLimit,
        DelegationError::Limit => ApprovalError::DelegationLimit,
        DelegationError::SerializationFailed => ApprovalError::SerializationFailed,
    })?;
    let window: Window = serde_json::from_slice(decoded.canonical_bytes())
        .map_err(|_| ApprovalError::SerializationFailed)?;
    let observations: Vec<Observation<'_>> = window
        .snapshot
        .prefixes
        .iter()
        .flat_map(|p| &p.events)
        .chain(&window.tail)
        .map(|event| {
            Ok(Observation {
                event,
                value: serde_json::to_value(&event.record)
                    .map_err(|_| ApprovalError::SerializationFailed)?,
                bytes: serde_json::to_vec(&event.record)
                    .map_err(|_| ApprovalError::SerializationFailed)?,
            })
        })
        .collect::<Result<_, ApprovalError>>()?;
    let mut index: BTreeMap<String, IndexedRecord> = BTreeMap::new();
    for (i, o) in observations.iter().enumerate() {
        let r = index
            .entry(o.id().to_owned())
            .or_insert_with(|| IndexedRecord {
                first: i,
                kind: o.kind().to_owned(),
                ambiguous: false,
                ordinals: Vec::new(),
            });
        r.ambiguous |= o.bytes != observations[r.first].bytes;
        r.ordinals.push(i);
    }
    let mut bad_streams = BTreeSet::new();
    for item in &projection.document().timeline {
        if item
            .diagnostics
            .iter()
            .any(|c| STREAM_TAINT.contains(&c.as_str()))
        {
            bad_streams.insert((&item.producer_id, &item.fence_id));
        }
    }
    let global = projection.document().snapshot_diagnostics.iter().any(|c| {
        matches!(
            c.as_str(),
            "prefix_watermark_mismatch" | "prefix_identity_conflict"
        )
    });
    let tainted = observations
        .iter()
        .map(|o| global || bad_streams.contains(&(&o.event.producer_id, &o.event.fence_id)))
        .collect();
    let mut subjects = BTreeMap::new();
    for s in &delegations.document().subjects {
        let mut flags = [false; 32];
        delegation_flags(&mut flags, &s.diagnostics, true);
        for i in &s.matching_event_ordinals {
            let v = &observations[*i].value;
            flags[30] |= v["reportedVerification"] == "reported_rejected"
                || v["freshness"] == "reported_stale";
        }
        subjects.insert(
            SubjectTuple {
                system_id: s.system_id.clone(),
                canonical_subject_id: s.canonical_subject_id.clone(),
            },
            SubjectMeta {
                flags,
                kind: (s.reported_actor_kinds.len() == 1)
                    .then(|| s.reported_actor_kinds[0].clone()),
            },
        );
    }
    let ctx = Inspector {
        observations,
        index,
        tainted,
        subjects,
        relationships: relationships.document(),
        delegations: delegations.document(),
    };
    let mut budget = Budget::default();
    let mut headers = vec![[false; 32]; ctx.observations.len()];
    let mut references: Vec<Vec<ReferenceLink>> =
        (0..ctx.observations.len()).map(|_| Vec::new()).collect();
    for (i, _) in ctx
        .observations
        .iter()
        .enumerate()
        .filter(|(_, o)| o.selected())
    {
        (headers[i], references[i]) = ctx.header(i, &mut budget)?;
    }
    aggregate_identical(&ctx, &mut headers);
    let mut plans: Vec<Vec<StageWork>> = (0..ctx.observations.len()).map(|_| Vec::new()).collect();
    for (i, _) in ctx
        .observations
        .iter()
        .enumerate()
        .filter(|(_, o)| o.kind() == "approval_plan")
    {
        plans[i] = declarations(&ctx, i, headers[i], &mut budget)?;
    }
    let mut event_flags = headers.clone();
    let mut keys = vec![None; ctx.observations.len()];
    for (i, _) in ctx
        .observations
        .iter()
        .enumerate()
        .filter(|(_, o)| o.kind() == "approval_decision")
    {
        keys[i] = decision_local(&ctx, i, &headers, &plans, &mut event_flags[i]);
    }
    aggregate_identical(&ctx, &mut event_flags);
    let mut groups: BTreeMap<GroupKey, Vec<usize>> = BTreeMap::new();
    for (i, key) in keys.iter().enumerate() {
        if let Some(key) = key {
            groups.entry(key.clone()).or_default().push(i);
        }
    }
    let mut chains = Vec::new();
    for (key, ordinals) in groups {
        chains.push(chain(
            &ctx,
            key,
            ordinals,
            &keys,
            &mut event_flags,
            &mut budget,
        )?);
    }
    local_votes(&ctx, &mut plans, &event_flags, &chains);
    for stages in &mut plans {
        dependencies(stages, &mut budget)?;
    }
    let mut counts = [0; 32];
    let mut event_assessments = Vec::new();
    for (i, o) in ctx.observations.iter().enumerate() {
        if o.kind() == "approval_plan" {
            for stage in &plans[i] {
                union(&mut event_flags[i], &stage.final_flags);
            }
        }
        let diagnostics = if o.selected() {
            codes(&event_flags[i])
        } else {
            Vec::new()
        };
        for c in &diagnostics {
            counts[CODES
                .iter()
                .position(|x| x == c)
                .expect("private closed code")] += 1;
        }
        let stages: Vec<_> = std::mem::take(&mut plans[i])
            .into_iter()
            .map(|s| {
                for c in &s.row.diagnostics {
                    counts[CODES
                        .iter()
                        .position(|x| x == c)
                        .expect("private closed code")] += 1;
                }
                s.row
            })
            .collect();
        let reported_tip = keys[i]
            .as_ref()
            .and_then(|key| chains.iter().find(|c| &c.key == key))
            .and_then(|c| c.row.reported_chain_tip_id.clone());
        event_assessments.push(ApprovalEvent {
            input_ordinal: i,
            event_id: o.event.event_id.clone(),
            record_id: o.id().to_owned(),
            record_kind: o.kind().to_owned(),
            assessment: if o.selected() {
                assessment(&event_flags[i])
            } else {
                "not_assessed"
            },
            reported_decision: if o.kind() == "approval_decision" {
                Some(text(&o.value, "decision").to_owned())
            } else {
                None
            },
            actor_subject: keys[i].as_ref().map(|key| key.subject.clone()),
            reported_decision_chain_tip_id: reported_tip,
            diagnostics,
            references: std::mem::take(&mut references[i]),
            stages,
        });
    }
    let decision_chains = chains
        .into_iter()
        .map(|c| {
            for code in &c.row.diagnostics {
                counts[CODES
                    .iter()
                    .position(|x| x == code)
                    .expect("private closed code")] += 1;
            }
            c.row
        })
        .collect();
    let document = ApprovalDocument {
        schema_version: APPROVAL_SCHEMA,
        input_canonical_digest: decoded.canonical_digest().to_owned(),
        structural_projection_digest: projection.canonical_digest(),
        relationship_digest: relationships.canonical_digest(),
        delegation_digest: delegations.canonical_digest(),
        authority: "none",
        authenticity: "unverified",
        execution: "unavailable",
        semantic_assessment: "reported_approval_structure",
        authoritative_eligibility: "unknown",
        authoritative_effective_evaluation_id: None,
        authoritative_duty_satisfaction: "unknown",
        diagnostic_counts: CODES
            .iter()
            .zip(counts)
            .map(|(code, count)| ApprovalCount { code, count })
            .collect(),
        event_assessments,
        decision_chains,
    };
    let canonical = bounded_json(&document).map_err(|e| match e {
        ProjectionError::Limit => ApprovalError::Limit,
        ProjectionError::SerializationFailed => ApprovalError::SerializationFailed,
    })?;
    Ok(ApprovalInspection {
        canonical,
        document,
    })
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    #[test]
    fn counters_allow_exact_ceiling_and_fail_atomically() {
        for dimension in 0..4 {
            let mut b = Budget::default();
            let mut adds = [0; 4];
            adds[dimension] = [
                MAX_APPROVAL_STAGES,
                MAX_APPROVAL_REFERENCES,
                MAX_APPROVAL_MATCHES,
                MAX_APPROVAL_PATH_STEPS,
            ][dimension];
            b.reserve(adds[0], adds[1], adds[2], adds[3]).unwrap();
            let before = [b.stages, b.references, b.matches, b.steps];
            adds = [0; 4];
            adds[dimension] = 1;
            assert_eq!(
                b.reserve(adds[0], adds[1], adds[2], adds[3]),
                Err(ApprovalError::Limit)
            );
            assert_eq!([b.stages, b.references, b.matches, b.steps], before);
        }
    }
}
