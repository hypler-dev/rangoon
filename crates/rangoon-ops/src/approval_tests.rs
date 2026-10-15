use super::*;
use serde_json::{Value, json};

fn oid(number: u16) -> String {
    format!("ops:{number:064x}")
}

fn digest(number: u16) -> String {
    format!("{number:064x}")
}

fn context(span: u16) -> Value {
    json!({
        "workspaceId":oid(2),"systemId":oid(3),"environmentId":oid(4),"engineId":oid(5),
        "requestId":oid(108),"traceId":oid(6),"spanId":oid(span),"policyRevisionId":oid(104)
    })
}

fn scope() -> Value {
    json!({
        "systemId":oid(3),"environmentIds":[oid(4)],"operationClasses":["deploy"],
        "resourceIds":[oid(101)],"policyRevisionIds":[oid(104)]
    })
}

fn correlation(record: &Value) -> Value {
    let mut value = record.get("context").cloned().unwrap_or_else(|| {
        json!({
            "workspaceId":oid(2),"systemId":oid(3),"environmentId":oid(4),"engineId":oid(5),
            "traceId":oid(6),"spanId":oid(7),"requestId":oid(108),"policyRevisionId":oid(104)
        })
    });
    value["evaluationId"] = match record["kind"].as_str().unwrap() {
        "evaluation" => record["id"].clone(),
        "approval_plan" | "approval_decision" | "authorization" => record["evaluationId"].clone(),
        _ => Value::Null,
    };
    value["attemptId"] = if record["kind"] == "attempt" {
        record["attemptId"].clone()
    } else {
        Value::Null
    };
    value
}

fn event(sequence: u64, event_id: u16, record: Value) -> Value {
    let uncorrelated = matches!(
        record["kind"].as_str().unwrap(),
        "actor" | "resource" | "tool" | "workflow" | "engine"
    );
    json!({
        "eventId":oid(event_id),"producerId":oid(11),"fenceId":oid(21),"sequence":sequence,
        "previousEventId":if sequence == 1 { Value::Null } else { json!(oid(event_id - 1)) },
        "observedMs":null,"receivedMs":null,
        "correlation":if uncorrelated { Value::Null } else { correlation(&record) },"record":record
    })
}

fn append(window: &mut Value, record: Value) {
    let events = window["tail"].as_array_mut().unwrap();
    let sequence = events.len() as u64 + 1;
    events.push(event(sequence, 500 + sequence as u16, record));
}

fn rebuild_stream(window: &mut Value, producer_id: u16, fence_id: u16) {
    let producer_id = oid(producer_id);
    let fence_id = oid(fence_id);
    let indices = window["tail"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            (event["producerId"] == producer_id && event["fenceId"] == fence_id).then_some(index)
        })
        .collect::<Vec<_>>();
    let event_ids = indices
        .iter()
        .map(|index| window["tail"][*index]["eventId"].clone())
        .collect::<Vec<_>>();

    for (offset, index) in indices.into_iter().enumerate() {
        let event = &mut window["tail"][index];
        event["sequence"] = json!((offset + 1) as u64);
        event["previousEventId"] = if offset == 0 {
            Value::Null
        } else {
            event_ids[offset - 1].clone()
        };
    }
}

fn actor(id: u16, system: u16, subject: u16, kind: &str) -> Value {
    json!({
        "kind":"actor","id":oid(id),"systemId":oid(system),"canonicalSubjectId":oid(subject),
        "actorKind":kind,"identitySourceRef":null,"identityEvidenceId":null,
        "reportedVerification":"reported_verified","observedMs":null,"freshness":"reported_current"
    })
}

fn plan(stages: Value) -> Value {
    json!({
        "kind":"approval_plan","id":oid(110),"context":context(73),"evaluationId":oid(109),
        "previousPlanId":null,"changedByActorId":oid(100),"expiresMs":100,
        "stages":stages,"evidenceIds":[]
    })
}

fn stage(id: u16, approvers: Value, mode: &str, quorum: Value) -> Value {
    json!({
        "id":oid(id),"dependsOnStageIds":[],"approverIds":approvers,"humanRequired":true,
        "mode":mode,"quorum":quorum,"expiresMs":100,"escalationActorIds":[]
    })
}

fn decision(id: u16, actor_id: u16, stage_id: u16, decision: &str, prior: Value) -> Value {
    json!({
        "kind":"approval_decision","id":oid(id),"context":context(74),"evaluationId":oid(109),
        "planId":oid(110),"stageId":oid(stage_id),"actorId":oid(actor_id),"delegationId":null,
        "decision":decision,"supersedesDecisionId":prior,"reason":"none","evidenceIds":[]
    })
}

fn base_window() -> Value {
    json!({
        "schemaVersion":"rangoon.ops.window.v1",
        "snapshot":{"snapshotId":oid(1),"reducerVersion":"rangoon.ops.reducer.v1",
        "scope":{"workspaceId":oid(2),"systemIds":[oid(3)],"environmentIds":[oid(4)],"engineIds":[oid(5)]},
        "topology":{"revisionId":oid(8),"nodes":[],"edges":[],"policyRevisionIds":[oid(104)],"coverageRevisionIds":[oid(118)]},
        "prefixes":[],"asOfMs":0},"tail":[]
    })
}

fn complete_window() -> Value {
    let mut window = base_window();
    let records = vec![
        actor(100, 3, 200, "human"),
        json!({"kind":"resource","id":oid(101),"systemId":oid(3),"resourceClass":"repository","externalRef":null}),
        json!({"kind":"tool","id":oid(102),"systemId":oid(3),"operationClass":"deploy","externalRef":null}),
        json!({"kind":"workflow","id":oid(103),"systemId":oid(3),"externalRef":{"systemId":oid(3),"contractVersion":"v1","externalId":"workflow"},"capabilityRefs":[]}),
        json!({"kind":"engine","id":oid(5),"systemId":oid(3),"externalRef":{"systemId":oid(3),"contractVersion":"v1","externalId":"engine"},"artifactDigest":digest(5)}),
        json!({"kind":"policy_revision","id":oid(104),"gatewayId":oid(105),"previousRevisionId":null,"artifactDigest":digest(6),"externalRef":null}),
        json!({"kind":"gateway_revision","id":oid(105),"gatewayId":oid(105),"previousRevisionId":null,"policyRevisionId":oid(104),"scope":scope(),"humanRequired":true,"changedByActorId":oid(100),"state":"reported_active","evidenceIds":[]}),
        json!({"kind":"assignment","id":oid(106),"actorId":oid(100),"gatewayRevisionId":oid(105),"policyRevisionId":oid(104),"delegationId":null,"state":"reported_active","changedByActorId":oid(100),"evidenceIds":[]}),
        json!({"kind":"delegation","id":oid(107),"delegatorId":oid(100),"delegateId":oid(100),"parentDelegationId":null,"scope":scope(),"expiresMs":100,"state":"reported_active","changedByActorId":oid(100),"externalRef":null,"reportedVerification":"reported_verified","observedMs":null,"freshness":"reported_current","evidenceIds":[]}),
        json!({"kind":"request","id":oid(108),"context":context(70),"actorId":oid(100),"gatewayRevisionId":oid(105),"operationClass":"deploy","actionDigest":digest(8),"contextDigest":digest(9),"resourceIds":[oid(101)],"workflowRef":null,"expiresMs":null}),
        json!({"kind":"evaluation","id":oid(109),"context":context(71),"evaluationKind":"initial","gatewayRevisionId":oid(105),"evaluatorId":oid(100),"evaluatorArtifactDigest":digest(10),"inputSnapshotId":oid(1),"inputCanonicalization":"json-v1","inputDigest":digest(11),"contextSnapshotId":oid(1),"contextDigest":digest(12),"previousEvaluationId":null,"expectedEffectiveEvaluationId":null,"lineageFenceId":oid(21),"result":"unknown","expiresMs":null,"reason":"conflict","externalRef":null,"evidenceIds":[]}),
        plan(json!([stage(210, json!([oid(100)]), "all", Value::Null)])),
        decision(111, 100, 210, "approve", Value::Null),
        json!({"kind":"authorization","id":oid(112),"context":context(75),"evaluationId":oid(109),"planId":null,"decisionIds":[oid(111)],"previousObservationId":null,"state":"reported_issued","scope":scope(),"expiresMs":null,"reason":"none","externalRef":null,"evidenceIds":[]}),
        json!({"kind":"attempt","id":oid(113),"context":context(76),"authorizationId":null,"executorId":oid(100),"previousObservationId":null,"attemptId":oid(113),"state":"outcome_unknown","reason":"unknown_outcome","evidenceIds":[]}),
        json!({"kind":"suspension","id":oid(114),"gatewayRevisionId":oid(105),"requestedByActorId":oid(100),"scope":scope(),"state":"reported_partial","previousObservationId":null,"reason":"conflict","evidenceIds":[]}),
        json!({"kind":"metric","id":oid(115),"context":context(77),"attemptId":null,"resourceIds":[oid(101)],"latencyMs":null,"cost":null,"reason":"unknown_outcome"}),
        json!({"kind":"evidence","id":oid(117),"producerId":oid(11),"externalRef":null,"artifactDigest":null,"state":"unverified","reason":"unknown_outcome"}),
        json!({"kind":"coverage","id":oid(118),"systemId":oid(3),"engineId":oid(5),"gatewayRevisionId":oid(105),"operationClass":"deploy","environmentId":oid(4),"targetOs":"linux","observedMs":null,"freshness":"reported_current","targetResourceIds":[oid(101)],"adapterArtifactDigest":digest(13),"engineArtifactDigest":digest(14),"reportedLevel":"fully_mediated","checks":{"policyDecision":"yes","approvalDuties":"yes","agentDelegation":"yes","expiryRevocation":"yes","executionConsumption":"yes","replayProtection":"yes","outcomeEvidence":"yes","reconciliation":"yes"},"expiresMs":null,"evidenceIds":[]}),
    ];
    for record in records {
        append(&mut window, record);
    }
    window
}

fn clean_chain_window() -> Value {
    let mut window = base_window();
    let records = vec![
        actor(100, 3, 200, "human"),
        actor(201, 3, 301, "human"),
        json!({"kind":"resource","id":oid(101),"systemId":oid(3),"resourceClass":"repository","externalRef":null}),
        json!({"kind":"engine","id":oid(5),"systemId":oid(3),"externalRef":{"systemId":oid(3),"contractVersion":"v1","externalId":"engine"},"artifactDigest":digest(5)}),
        json!({"kind":"policy_revision","id":oid(104),"gatewayId":oid(105),"previousRevisionId":null,"artifactDigest":digest(6),"externalRef":null}),
        json!({"kind":"gateway_revision","id":oid(105),"gatewayId":oid(105),"previousRevisionId":null,"policyRevisionId":oid(104),"scope":scope(),"humanRequired":true,"changedByActorId":oid(100),"state":"reported_active","evidenceIds":[]}),
        json!({"kind":"request","id":oid(108),"context":context(70),"actorId":oid(100),"gatewayRevisionId":oid(105),"operationClass":"deploy","actionDigest":digest(8),"contextDigest":digest(9),"resourceIds":[oid(101)],"workflowRef":null,"expiresMs":null}),
        json!({"kind":"evaluation","id":oid(109),"context":context(71),"evaluationKind":"initial","gatewayRevisionId":oid(105),"evaluatorId":oid(100),"evaluatorArtifactDigest":digest(10),"inputSnapshotId":oid(1),"inputCanonicalization":"json-v1","inputDigest":digest(11),"contextSnapshotId":oid(1),"contextDigest":digest(9),"previousEvaluationId":null,"expectedEffectiveEvaluationId":null,"lineageFenceId":oid(21),"result":"unknown","expiresMs":null,"reason":"conflict","externalRef":null,"evidenceIds":[]}),
        plan(json!([stage(210, json!([oid(201)]), "all", Value::Null)])),
        decision(111, 201, 210, "approve", Value::Null),
        decision(212, 201, 210, "deny", json!(oid(111))),
        decision(111, 201, 210, "approve", Value::Null),
    ];
    for record in records {
        append(&mut window, record);
    }
    window
}

fn decode(value: &Value) -> DecodedWindow {
    decode_window(&serde_json::to_vec(value).unwrap()).unwrap()
}

fn document(value: &Value) -> Value {
    let decoded = decode(value);
    serde_json::to_value(inspect_approvals(&decoded).unwrap().document()).unwrap()
}

fn event_row(document: &Value, record_id: u16) -> &Value {
    document["eventAssessments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["recordId"] == oid(record_id))
        .unwrap()
}

fn timeline_row<'a>(document: &'a Value, event_id: &Value) -> &'a Value {
    document["timeline"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["eventId"] == *event_id)
        .unwrap()
}

fn stage_row(document: &Value, plan_id: u16, index: usize) -> &Value {
    event_row(document, plan_id)["stages"]
        .as_array()
        .unwrap()
        .get(index)
        .unwrap()
}

fn codes(row: &Value) -> Vec<String> {
    row["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|code| code.as_str().unwrap().to_owned())
        .collect()
}

fn assert_code(row: &Value, expected: &str) {
    assert!(
        codes(row).contains(&expected.to_owned()),
        "{row:?} missing {expected}"
    );
}

fn diagnostic_count(document: &Value, code: &str) -> u64 {
    document["diagnosticCounts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["code"] == code)
        .unwrap()["count"]
        .as_u64()
        .unwrap()
}

fn assert_fixed_trust(document: &Value) {
    assert_eq!(document["authority"], "none");
    assert_eq!(document["authenticity"], "unverified");
    assert_eq!(document["execution"], "unavailable");
    assert_eq!(
        document["semanticAssessment"],
        "reported_approval_structure"
    );
    assert_eq!(document["authoritativeEligibility"], "unknown");
    assert_eq!(document["authoritativeEffectiveEvaluationId"], Value::Null);
    assert_eq!(document["authoritativeDutySatisfaction"], "unknown");
}

#[test]
fn empty_window_has_literal_approval_golden_and_private_debug() {
    const EMPTY_WINDOW: &[u8] = br#"{"schemaVersion":"rangoon.ops.window.v1","snapshot":{"snapshotId":"ops:1111111111111111111111111111111111111111111111111111111111111111","reducerVersion":"rangoon.ops.reducer.v1","scope":{"workspaceId":"ops:1111111111111111111111111111111111111111111111111111111111111111","systemIds":[],"environmentIds":[],"engineIds":[]},"topology":{"revisionId":"ops:1111111111111111111111111111111111111111111111111111111111111111","nodes":[],"edges":[],"policyRevisionIds":[],"coverageRevisionIds":[]},"prefixes":[],"asOfMs":0},"tail":[]}"#;
    const INPUT_SHA256: &str = "8ab43d0754e994dc00445661f74d1da62b6efec06f7c6276bee1b9e1fc25c0a7";
    const OUTPUT: &str = r#"{"schemaVersion":"rangoon.ops.approvals.v1","inputCanonicalDigest":"8ab43d0754e994dc00445661f74d1da62b6efec06f7c6276bee1b9e1fc25c0a7","structuralProjectionDigest":"5c8986e86e5cc7e23d6acfeed340d70cce710e8a7e71b117058c02296e928bba","relationshipDigest":"07ce0a123f67d402d177313ab4cfb810ef28adcc69dc234beb466ae1ae914ee7","delegationDigest":"ff66c6814182f517231c0224cda18e79c3b1a708b782d094762d28e49771b4fd","authority":"none","authenticity":"unverified","execution":"unavailable","semanticAssessment":"reported_approval_structure","authoritativeEligibility":"unknown","authoritativeEffectiveEvaluationId":null,"authoritativeDutySatisfaction":"unknown","diagnosticCounts":[{"code":"structural_incomplete","count":0},{"code":"reference_missing","count":0},{"code":"reference_ambiguous","count":0},{"code":"reference_kind_mismatch","count":0},{"code":"relationship_incomplete","count":0},{"code":"subject_incomplete","count":0},{"code":"stage_identity_conflict","count":0},{"code":"stage_dependency_missing","count":0},{"code":"stage_dependency_cycle","count":0},{"code":"approver_kind_unsupported","count":0},{"code":"approver_alias_duplicate","count":0},{"code":"quorum_invalid","count":0},{"code":"decision_binding_mismatch","count":0},{"code":"decision_actor_not_listed","count":0},{"code":"requester_self_approval","count":0},{"code":"agent_delegation_missing","count":0},{"code":"delegation_incomplete","count":0},{"code":"delegation_binding_mismatch","count":0},{"code":"decision_lineage_mismatch","count":0},{"code":"decision_sequence_mismatch","count":0},{"code":"decision_branch","count":0},{"code":"decision_cycle","count":0},{"code":"decision_incomplete_chain","count":0},{"code":"human_report_missing","count":0},{"code":"reported_vote_shortfall","count":0},{"code":"non_tip_decision","count":0},{"code":"dependency_report_shortfall","count":0},{"code":"stage_expiry_expansion","count":0},{"code":"expiry_incomplete","count":0},{"code":"delegation_report_inactive","count":0},{"code":"actor_report_inactive","count":0},{"code":"human_stage_missing","count":0}],"eventAssessments":[],"decisionChains":[]}"#;
    const OUTPUT_SHA256: &str = "51d3c7452147072efc346de9e0cc0f867fa492f62c033b7db85034a337f8caba";

    let decoded = decode_window(EMPTY_WINDOW).unwrap();
    assert_eq!(decoded.canonical_bytes(), EMPTY_WINDOW);
    assert_eq!(decoded.canonical_digest(), INPUT_SHA256);
    let inspection = inspect_approvals(&decoded).unwrap();
    assert_eq!(inspection.canonical_bytes(), OUTPUT.as_bytes());
    assert_eq!(inspection.canonical_digest(), OUTPUT_SHA256);
    assert_eq!(
        rangoon_domain::byte_digest(OUTPUT.as_bytes()),
        OUTPUT_SHA256
    );
    let debug = format!("{inspection:?}");
    assert!(!debug.contains("eventAssessments"));
    assert!(!debug.contains("schemaVersion"));
    assert_fixed_trust(&serde_json::to_value(inspection.document()).unwrap());
}

#[test]
fn complete_nineteen_record_fixture_retains_closed_rows_and_diagnostic_order() {
    let value = complete_window();
    let decoded = decode(&value);
    let before = decoded.canonical_bytes().to_vec();
    let inspection = inspect_approvals(&decoded).unwrap();
    let output = serde_json::to_value(inspection.document()).unwrap();
    assert_eq!(output["eventAssessments"].as_array().unwrap().len(), 19);
    assert_eq!(output["decisionChains"].as_array().unwrap().len(), 1);
    assert_eq!(
        output["eventAssessments"][11]["recordKind"],
        "approval_plan"
    );
    assert_eq!(
        output["eventAssessments"][12]["recordKind"],
        "approval_decision"
    );
    assert_eq!(output["eventAssessments"][0]["assessment"], "not_assessed");
    assert_eq!(output["eventAssessments"][0]["references"], json!([]));
    assert_eq!(output["eventAssessments"][0]["stages"], json!([]));
    assert_eq!(output["diagnosticCounts"].as_array().unwrap().len(), 32);
    assert_eq!(
        output["diagnosticCounts"][0]["code"],
        "structural_incomplete"
    );
    assert_eq!(
        output["diagnosticCounts"][31]["code"],
        "human_stage_missing"
    );
    assert_fixed_trust(&output);
    assert_eq!(decoded.canonical_bytes(), before.as_slice());
}

#[test]
fn stage_references_duplicates_and_dependency_walks_stay_literal() {
    let mut value = complete_window();
    let plan = &mut value["tail"][11]["record"];
    let mut first = stage(210, json!([oid(999)]), "all", Value::Null);
    first["dependsOnStageIds"] = json!([oid(998), oid(210), oid(210)]);
    let mut second = stage(210, json!([oid(100)]), "all", Value::Null);
    second["escalationActorIds"] = json!([oid(100)]);
    plan["stages"] = json!([first, second]);

    let output = document(&value);
    let first = stage_row(&output, 110, 0);
    let second = stage_row(&output, 110, 1);
    assert_eq!(first["stageInputIndex"], 0);
    assert_eq!(second["stageInputIndex"], 1);
    assert_eq!(first["stageId"], oid(210));
    assert_eq!(second["stageId"], oid(210));
    assert_code(first, "reference_missing");
    assert_code(first, "stage_identity_conflict");
    assert_code(second, "stage_identity_conflict");
    assert_code(first, "stage_dependency_missing");
    assert_eq!(first["dependencyReferences"][0]["status"], "missing");
    assert_eq!(first["dependencyReferences"][1]["status"], "ambiguous");
    assert_eq!(first["dependencyReferences"][2]["status"], "ambiguous");
    assert_eq!(
        second["reportedEscalationSubjects"],
        json!([{
            "systemId": oid(3), "canonicalSubjectId": oid(200)
        }])
    );
}

#[test]
fn transitive_diamond_and_dangling_dependencies_keep_each_stage_row() {
    let mut value = clean_chain_window();
    let mut left = stage(211, json!([oid(201)]), "all", Value::Null);
    left["dependsOnStageIds"] = json!([oid(210)]);
    let mut right = stage(212, json!([oid(201)]), "all", Value::Null);
    right["dependsOnStageIds"] = json!([oid(210)]);
    let mut tip = stage(213, json!([oid(201)]), "all", Value::Null);
    tip["dependsOnStageIds"] = json!([oid(211), oid(212)]);
    value["tail"][8]["record"]["stages"] = json!([
        stage(210, json!([oid(201)]), "all", Value::Null),
        left,
        right,
        tip
    ]);
    let output = document(&value);
    assert_eq!(
        event_row(&output, 110)["stages"].as_array().unwrap().len(),
        4
    );
    assert_code(stage_row(&output, 110, 1), "dependency_report_shortfall");
    assert_code(stage_row(&output, 110, 2), "dependency_report_shortfall");
    assert_code(stage_row(&output, 110, 3), "dependency_report_shortfall");

    let mut dangling = value;
    dangling["tail"][8]["record"]["stages"][3]["dependsOnStageIds"] = json!([oid(999)]);
    let output = document(&dangling);
    assert_code(stage_row(&output, 110, 3), "stage_dependency_missing");
}

#[test]
fn unique_stage_cycle_and_foreign_plan_stage_target_stay_local() {
    let mut cycle = clean_chain_window();
    let mut first = stage(210, json!([oid(201)]), "all", Value::Null);
    first["dependsOnStageIds"] = json!([oid(211)]);
    let mut second = stage(211, json!([oid(201)]), "all", Value::Null);
    second["dependsOnStageIds"] = json!([oid(210)]);
    cycle["tail"][8]["record"]["stages"] = json!([first, second]);
    let output = document(&cycle);
    assert_code(stage_row(&output, 110, 0), "stage_dependency_cycle");
    assert_code(stage_row(&output, 110, 1), "stage_dependency_cycle");

    let mut foreign = clean_chain_window();
    foreign["tail"][8]["record"]["stages"][0]["dependsOnStageIds"] = json!([oid(999)]);
    let mut other = plan(json!([stage(999, json!([oid(201)]), "all", Value::Null)]));
    other["id"] = json!(oid(310));
    append(&mut foreign, other);
    let output = document(&foreign);
    assert_code(stage_row(&output, 110, 0), "stage_dependency_missing");
    assert_eq!(
        stage_row(&output, 110, 0)["dependencyReferences"][0]["status"],
        "missing"
    );
}

#[test]
fn stream_conflicts_taint_owned_records_while_graph_only_noise_does_not() {
    let mut plan_conflict = clean_chain_window();
    let mut conflicting_plan = plan_conflict["tail"][8]["record"].clone();
    conflicting_plan["expiresMs"] = json!(101);
    append(&mut plan_conflict, conflicting_plan);
    let output = document(&plan_conflict);
    assert_code(event_row(&output, 110), "structural_incomplete");
    assert_eq!(stage_row(&output, 110, 0)["reportedVotes"], Value::Null);

    let mut decision_conflict = clean_chain_window();
    let mut conflicting_decision = decision_conflict["tail"][9]["record"].clone();
    conflicting_decision["decision"] = json!("deny");
    append(&mut decision_conflict, conflicting_decision);
    let output = document(&decision_conflict);
    assert_code(event_row(&output, 111), "structural_incomplete");
    assert_eq!(stage_row(&output, 110, 0)["reportedVotes"], Value::Null);

    let mut graph_noise = clean_chain_window();
    graph_noise["snapshot"]["topology"]["nodes"] =
        json!([{"id":oid(900),"kind":"actor","recordId":oid(999)}]);
    let output = document(&graph_noise);
    assert!(!codes(event_row(&output, 111)).contains(&"structural_incomplete".to_owned()));
    assert_eq!(
        stage_row(&output, 110, 0)["reportedVotes"]["denySubjects"],
        1
    );
}

#[test]
fn stage_cardinality_human_and_expiry_boundaries_do_not_establish_duty() {
    let mut value = complete_window();
    let plan = &mut value["tail"][11]["record"];
    let mut all_with_quorum = stage(210, json!([oid(100)]), "all", json!(1));
    all_with_quorum["humanRequired"] = false.into();
    all_with_quorum["expiresMs"] = json!(101);
    let mut zero_quorum = stage(211, json!([oid(100)]), "quorum", json!(0));
    zero_quorum["humanRequired"] = false.into();
    zero_quorum["expiresMs"] = Value::Null;
    let mut empty_all = stage(212, json!([]), "all", Value::Null);
    empty_all["humanRequired"] = false.into();
    plan["stages"] = json!([all_with_quorum, zero_quorum, empty_all]);

    let output = document(&value);
    for index in 0..3 {
        assert_code(stage_row(&output, 110, index), "quorum_invalid");
    }
    assert_code(event_row(&output, 110), "human_stage_missing");
    assert_code(stage_row(&output, 110, 0), "stage_expiry_expansion");
    assert_code(stage_row(&output, 110, 1), "expiry_incomplete");
    assert_eq!(
        stage_row(&output, 110, 0)["authoritativeDutySatisfaction"],
        "unknown"
    );
    assert_eq!(
        stage_row(&output, 110, 1)["authoritativeDutySatisfaction"],
        "unknown"
    );
    assert_eq!(
        stage_row(&output, 110, 2)["requiredReportedSubjectCount"],
        Value::Null
    );
}

#[test]
fn decision_succession_keeps_deny_and_marks_old_nodes_without_authority() {
    let value = clean_chain_window();

    let output = document(&value);
    assert_eq!(output["decisionChains"].as_array().unwrap().len(), 1);
    let chain = &output["decisionChains"][0];
    assert_eq!(
        chain["reportedChainStatus"], "single_chain_unverified",
        "{chain:?}"
    );
    assert_eq!(chain["reportedDecision"], "deny");
    assert_eq!(chain["reportedChainTipId"], oid(212));
    assert_eq!(chain["decisionRecordIds"], json!([oid(111), oid(212)]));
    assert_eq!(chain["matchingEventOrdinals"], json!([9, 10, 11]));
    assert_code(chain, "non_tip_decision");
    assert_code(event_row(&output, 111), "non_tip_decision");
    assert!(!codes(event_row(&output, 212)).contains(&"non_tip_decision".to_owned()));
    assert_code(stage_row(&output, 110, 0), "non_tip_decision");
    assert_code(event_row(&output, 110), "non_tip_decision");
    assert_eq!(diagnostic_count(&output, "non_tip_decision"), 5);
    assert_eq!(
        stage_row(&output, 110, 0)["authoritativeDutySatisfaction"],
        "unknown"
    );
}

#[test]
fn competing_roots_cycles_and_wrong_plan_predecessors_never_select_a_tip() {
    let mut branching = clean_chain_window();
    branching["tail"].as_array_mut().unwrap().truncate(10);
    append(&mut branching, decision(212, 201, 210, "deny", Value::Null));
    let output = document(&branching);
    assert_code(&output["decisionChains"][0], "decision_branch");
    assert_eq!(output["decisionChains"][0]["reportedDecision"], Value::Null);
    assert_eq!(
        output["decisionChains"][0]["reportedChainTipId"],
        Value::Null
    );

    let mut cycle = clean_chain_window();
    cycle["tail"].as_array_mut().unwrap().truncate(10);
    cycle["tail"][9]["record"]["supersedesDecisionId"] = json!(oid(111));
    let output = document(&cycle);
    assert_code(&output["decisionChains"][0], "decision_cycle");
    assert_eq!(output["decisionChains"][0]["reportedDecision"], Value::Null);

    let mut wrong_plan = clean_chain_window();
    wrong_plan["tail"].as_array_mut().unwrap().truncate(10);
    let mut record = decision(212, 201, 210, "deny", json!(oid(111)));
    record["planId"] = json!(oid(999));
    append(&mut wrong_plan, record);
    let output = document(&wrong_plan);
    assert_code(event_row(&output, 212), "reference_missing");
    assert_eq!(
        event_row(&output, 212)["reportedDecisionChainTipId"],
        Value::Null
    );

    let mut successors = clean_chain_window();
    successors["tail"].as_array_mut().unwrap().truncate(10);
    append(
        &mut successors,
        decision(212, 201, 210, "deny", json!(oid(111))),
    );
    append(
        &mut successors,
        decision(213, 201, 210, "abstain", json!(oid(111))),
    );
    let output = document(&successors);
    assert_code(&output["decisionChains"][0], "decision_branch");
    assert_eq!(
        output["decisionChains"][0]["reportedChainTipId"],
        Value::Null
    );
}

#[test]
fn decision_path_cap_is_own_work_and_keeps_input_immutable() {
    let mut value = clean_chain_window();
    value["tail"].as_array_mut().unwrap().truncate(9);
    for offset in 0..129_u16 {
        let prior = if offset == 0 {
            Value::Null
        } else {
            json!(oid(5000 + offset - 1))
        };
        append(
            &mut value,
            decision(5000 + offset, 201, 210, "approve", prior),
        );
    }
    let decision_count = value["tail"].as_array().unwrap().len() - 9;
    let declared_stage_count = value["tail"][8]["record"]["stages"]
        .as_array()
        .unwrap()
        .len();
    // Every node starts a predecessor walk, so this fixture has 1 + ... + N visits.
    let predecessor_walk_steps = decision_count * (decision_count + 1) / 2;
    assert!(predecessor_walk_steps > MAX_APPROVAL_PATH_STEPS);
    assert!(declared_stage_count < MAX_APPROVAL_STAGES);
    expect_limit(&value, "approval_limit");
}

#[test]
fn predecessor_variants_never_select_a_reported_tip() {
    let mut self_cycle = clean_chain_window();
    self_cycle["tail"].as_array_mut().unwrap().truncate(10);
    self_cycle["tail"][9]["record"]["supersedesDecisionId"] = json!(oid(111));
    let output = document(&self_cycle);
    assert_code(&output["decisionChains"][0], "decision_cycle");
    assert_eq!(
        output["decisionChains"][0]["reportedChainTipId"],
        Value::Null
    );

    let mut transitive = clean_chain_window();
    transitive["tail"].as_array_mut().unwrap().truncate(10);
    append(
        &mut transitive,
        decision(212, 201, 210, "deny", json!(oid(111))),
    );
    transitive["tail"][9]["record"]["supersedesDecisionId"] = json!(oid(212));
    let output = document(&transitive);
    assert_code(&output["decisionChains"][0], "decision_cycle");
    assert_eq!(
        output["decisionChains"][0]["reportedChainTipId"],
        Value::Null
    );

    for (predecessor, reference_code) in [
        (json!(oid(999)), "reference_missing"),
        (json!(oid(101)), "reference_kind_mismatch"),
    ] {
        let mut value = clean_chain_window();
        value["tail"].as_array_mut().unwrap().truncate(10);
        value["tail"][9]["record"]["supersedesDecisionId"] = predecessor;
        let output = document(&value);
        assert_code(event_row(&output, 111), reference_code);
        assert_code(event_row(&output, 111), "decision_incomplete_chain");
        assert_eq!(
            event_row(&output, 111)["reportedDecisionChainTipId"],
            Value::Null
        );
    }

    let mut cross_subject = clean_chain_window();
    cross_subject["tail"].as_array_mut().unwrap().truncate(10);
    append(
        &mut cross_subject,
        decision(212, 100, 210, "deny", json!(oid(111))),
    );
    let output = document(&cross_subject);
    assert_code(event_row(&output, 212), "decision_binding_mismatch");
    assert_eq!(
        event_row(&output, 212)["reportedDecisionChainTipId"],
        Value::Null
    );

    let mut ambiguous = clean_chain_window();
    ambiguous["tail"].as_array_mut().unwrap().truncate(10);
    append(&mut ambiguous, decision(111, 201, 210, "deny", Value::Null));
    ambiguous["tail"][9]["record"]["supersedesDecisionId"] = json!(oid(111));
    let output = document(&ambiguous);
    assert_code(event_row(&output, 111), "reference_ambiguous");
    assert_eq!(
        event_row(&output, 111)["reportedDecisionChainTipId"],
        Value::Null
    );
}

#[test]
fn decision_plan_evaluation_and_stage_selectors_are_exact_pins() {
    let mut wrong_evaluation = clean_chain_window();
    wrong_evaluation["tail"]
        .as_array_mut()
        .unwrap()
        .truncate(10);
    wrong_evaluation["tail"][9]["record"]["evaluationId"] = json!(oid(999));
    wrong_evaluation["tail"][9]["correlation"]["evaluationId"] = json!(oid(999));
    let output = document(&wrong_evaluation);
    assert_code(event_row(&output, 111), "reference_missing");
    assert_eq!(
        event_row(&output, 111)["reportedDecisionChainTipId"],
        Value::Null
    );

    let mut wrong_stage = clean_chain_window();
    wrong_stage["tail"].as_array_mut().unwrap().truncate(10);
    wrong_stage["tail"][9]["record"]["stageId"] = json!(oid(999));
    let output = document(&wrong_stage);
    assert_code(event_row(&output, 111), "decision_binding_mismatch");
    assert_code(event_row(&output, 111), "decision_incomplete_chain");
    assert_eq!(
        stage_row(&output, 110, 0)["reportedVotes"],
        json!({
            "approveSubjects":0,"denySubjects":0,"abstainSubjects":0,
            "humanApproveSubjects":0,"agentApproveSubjects":0
        })
    );
    assert_code(stage_row(&output, 110, 0), "human_report_missing");
    assert_code(stage_row(&output, 110, 0), "reported_vote_shortfall");
}

#[test]
fn every_flow_key_is_pinned_while_span_can_differ() {
    for key in [
        "workspaceId",
        "systemId",
        "environmentId",
        "engineId",
        "requestId",
        "traceId",
        "policyRevisionId",
    ] {
        let mut value = clean_chain_window();
        value["tail"].as_array_mut().unwrap().truncate(10);
        value["tail"][9]["record"]["context"][key] = json!(oid(999));
        value["tail"][9]["correlation"][key] = json!(oid(999));
        let output = document(&value);
        assert_code(event_row(&output, 111), "decision_binding_mismatch");
        assert_eq!(
            event_row(&output, 111)["reportedDecisionChainTipId"],
            Value::Null
        );
    }

    let mut span = clean_chain_window();
    span["tail"].as_array_mut().unwrap().truncate(10);
    span["tail"][9]["record"]["context"]["spanId"] = json!(oid(999));
    span["tail"][9]["correlation"]["spanId"] = json!(oid(999));
    let output = document(&span);
    assert_eq!(
        output["decisionChains"][0]["reportedChainStatus"],
        "single_chain_unverified"
    );
}

#[test]
fn reported_vote_counts_and_aliases_stay_descriptive_not_duty_grants() {
    let output = document(&clean_chain_window());
    let stage = stage_row(&output, 110, 0);
    assert_eq!(
        stage["reportedVotes"],
        json!({
            "approveSubjects":0,"denySubjects":1,"abstainSubjects":0,
            "humanApproveSubjects":0,"agentApproveSubjects":0
        })
    );
    assert_code(stage, "human_report_missing");
    assert_code(stage, "reported_vote_shortfall");
    assert_eq!(stage["authoritativeDutySatisfaction"], "unknown");

    let mut aliases = clean_chain_window();
    aliases["tail"][8]["record"]["stages"][0]["approverIds"] = json!([oid(201), oid(202)]);
    append(&mut aliases, actor(202, 3, 301, "human"));
    let output = document(&aliases);
    let stage = stage_row(&output, 110, 0);
    assert_code(stage, "approver_alias_duplicate");
    assert_eq!(stage["requiredReportedSubjectCount"], 1);
    assert_eq!(
        stage["reportedApproverSubjects"].as_array().unwrap().len(),
        1
    );
}

#[test]
fn unresolved_cardinality_roles_and_cross_systems_never_invent_counts() {
    let mut value = clean_chain_window();
    value["tail"][8]["record"]["stages"][0]["approverIds"] = json!([oid(202), oid(203)]);
    value["tail"][8]["record"]["stages"][0]["mode"] = json!("quorum");
    value["tail"][8]["record"]["stages"][0]["quorum"] = Value::Null;
    append(&mut value, actor(202, 3, 302, "service"));
    append(&mut value, actor(203, 33, 303, "human"));
    let output = document(&value);
    let stage = stage_row(&output, 110, 0);
    assert_code(stage, "approver_kind_unsupported");
    assert_code(stage, "quorum_invalid");
    assert_eq!(stage["requiredReportedSubjectCount"], Value::Null);
    assert!(!codes(stage).contains(&"human_report_missing".to_owned()));
    assert_eq!(
        stage["reportedApproverSubjects"].as_array().unwrap().len(),
        2
    );
}

#[test]
fn requester_self_approval_and_agent_delegation_scope_stay_explicit() {
    let mut self_approval = clean_chain_window();
    self_approval["tail"][8]["record"]["stages"][0]["approverIds"] = json!([oid(100)]);
    self_approval["tail"][9]["record"]["actorId"] = json!(oid(100));
    self_approval["tail"][10]["record"]["actorId"] = json!(oid(100));
    self_approval["tail"][11]["record"]["actorId"] = json!(oid(100));
    let output = document(&self_approval);
    assert_code(event_row(&output, 111), "requester_self_approval");
    assert_code(stage_row(&output, 110, 0), "requester_self_approval");

    let mut agent = clean_chain_window();
    agent["tail"][1]["record"]["actorKind"] = json!("agent");
    for index in [9, 10, 11] {
        agent["tail"][index]["record"]["delegationId"] = json!(oid(107));
    }
    append(
        &mut agent,
        json!({
            "kind":"delegation","id":oid(107),"delegatorId":oid(100),"delegateId":oid(201),
            "parentDelegationId":null,"scope":scope(),"expiresMs":100,"state":"reported_active",
            "changedByActorId":oid(100),"externalRef":null,"reportedVerification":"reported_verified",
            "observedMs":null,"freshness":"reported_current","evidenceIds":[]
        }),
    );
    let output = document(&agent);
    assert!(!codes(event_row(&output, 111)).contains(&"agent_delegation_missing".to_owned()));

    let mut wrong_scope = agent.clone();
    wrong_scope["tail"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap()["record"]["scope"]["resourceIds"] = json!([]);
    let output = document(&wrong_scope);
    assert_code(event_row(&output, 111), "delegation_binding_mismatch");

    for (field, value) in [
        ("systemId", json!(oid(33))),
        ("environmentIds", json!([])),
        ("operationClasses", json!([])),
        ("policyRevisionIds", json!([])),
    ] {
        let mut mismatch = agent.clone();
        mismatch["tail"].as_array_mut().unwrap().last_mut().unwrap()["record"]["scope"][field] =
            value;
        let output = document(&mismatch);
        assert_code(event_row(&output, 111), "delegation_binding_mismatch");
    }

    let mut empty_resources = agent.clone();
    empty_resources["tail"][6]["record"]["resourceIds"] = json!([]);
    let output = document(&empty_resources);
    assert_code(event_row(&output, 111), "delegation_incomplete");

    let mut inactive = agent;
    inactive["tail"].as_array_mut().unwrap().last_mut().unwrap()["record"]["state"] =
        json!("reported_revoked");
    let output = document(&inactive);
    assert_code(event_row(&output, 111), "delegation_report_inactive");
}

#[test]
fn mixed_human_and_delegated_agent_tips_have_exact_reported_counts() {
    let mut value = clean_chain_window();
    value["tail"][8]["record"]["stages"][0]["approverIds"] = json!([oid(201), oid(202)]);
    append(&mut value, actor(202, 3, 302, "agent"));
    append(
        &mut value,
        json!({
            "kind":"delegation","id":oid(107),"delegatorId":oid(100),"delegateId":oid(202),
            "parentDelegationId":null,"scope":scope(),"expiresMs":100,"state":"reported_active",
            "changedByActorId":oid(100),"externalRef":null,"reportedVerification":"reported_verified",
            "observedMs":null,"freshness":"reported_current","evidenceIds":[]
        }),
    );
    let mut agent_tip = decision(300, 202, 210, "abstain", Value::Null);
    agent_tip["delegationId"] = json!(oid(107));
    append(&mut value, agent_tip);
    let output = document(&value);
    assert_eq!(
        stage_row(&output, 110, 0)["reportedVotes"],
        json!({
            "approveSubjects":0,"denySubjects":1,"abstainSubjects":1,
            "humanApproveSubjects":0,"agentApproveSubjects":0
        })
    );
    assert_eq!(
        stage_row(&output, 110, 0)["authoritativeDutySatisfaction"],
        "unknown"
    );
    assert_eq!(output["authority"], "none");
}

#[test]
fn mixed_human_and_agent_approve_tips_count_both_reported_roles() {
    let mut value = clean_chain_window();
    value["tail"].as_array_mut().unwrap().truncate(10);
    value["tail"][8]["record"]["stages"][0]["approverIds"] = json!([oid(201), oid(202)]);
    append(&mut value, actor(202, 3, 302, "agent"));
    append(
        &mut value,
        json!({"kind":"delegation","id":oid(107),"delegatorId":oid(100),"delegateId":oid(202),"parentDelegationId":null,"scope":scope(),"expiresMs":100,"state":"reported_active","changedByActorId":oid(100),"externalRef":null,"reportedVerification":"reported_verified","observedMs":null,"freshness":"reported_current","evidenceIds":[]}),
    );
    let mut agent = decision(300, 202, 210, "approve", Value::Null);
    agent["delegationId"] = json!(oid(107));
    append(&mut value, agent);
    let output = document(&value);
    assert_eq!(
        stage_row(&output, 110, 0)["reportedVotes"],
        json!({"approveSubjects":2,"denySubjects":0,"abstainSubjects":0,"humanApproveSubjects":1,"agentApproveSubjects":1})
    );
}

#[test]
fn broken_decision_actor_and_predecessor_stay_unknown_and_explicit() {
    let mut value = complete_window();
    value["tail"][12]["record"] = decision(111, 999, 210, "approve", json!(oid(998)));
    let output = document(&value);
    let row = event_row(&output, 111);
    assert_code(row, "reference_missing");
    assert_eq!(row["reportedDecisionChainTipId"], Value::Null);
    assert_eq!(stage_row(&output, 110, 0)["reportedVotes"], Value::Null);
}

#[test]
fn tainted_request_or_evaluation_stream_forwards_without_tainting_own_plan_stream() {
    for (target_index, metric_id) in [(6, 701), (7, 702)] {
        let mut value = clean_chain_window();
        append(
            &mut value,
            json!({
                "kind":"metric","id":oid(metric_id),"context":context(78),"attemptId":null,
                "resourceIds":[oid(101)],"latencyMs":null,"cost":null,"reason":"unknown_outcome"
            }),
        );

        for index in [target_index, 12] {
            value["tail"][index]["producerId"] = json!(oid(12));
            value["tail"][index]["fenceId"] = json!(oid(22));
        }
        if target_index == 7 {
            value["tail"][7]["record"]["lineageFenceId"] = json!(oid(22));
        }
        value["tail"][12]["correlation"]["traceId"] = json!(oid(999));
        rebuild_stream(&mut value, 11, 21);
        rebuild_stream(&mut value, 12, 22);

        let projection =
            serde_json::to_value(project_window(&decode(&value)).unwrap().document()).unwrap();
        assert_code(
            timeline_row(&projection, &value["tail"][12]["eventId"]),
            "correlation_mismatch",
        );
        assert!(
            !codes(timeline_row(
                &projection,
                &value["tail"][target_index]["eventId"]
            ))
            .contains(&"correlation_mismatch".to_owned())
        );
        for index in [8, 9, 10, 11] {
            assert!(
                !codes(timeline_row(&projection, &value["tail"][index]["eventId"]))
                    .contains(&"correlation_mismatch".to_owned())
            );
        }

        let output = document(&value);
        assert_code(event_row(&output, 110), "structural_incomplete");
        assert_code(event_row(&output, 111), "structural_incomplete");
        assert_code(stage_row(&output, 110, 0), "structural_incomplete");
        assert_eq!(stage_row(&output, 110, 0)["reportedVotes"], Value::Null);
        assert_eq!(
            output["decisionChains"][0]["reportedChainTipId"],
            Value::Null
        );
    }
}

#[test]
fn decision_succession_rejects_cross_stream_and_reversed_sequence_links() {
    let mut cross_stream = clean_chain_window();
    cross_stream["tail"][10]["producerId"] = json!(oid(12));
    cross_stream["tail"][10]["fenceId"] = json!(oid(22));
    rebuild_stream(&mut cross_stream, 11, 21);
    rebuild_stream(&mut cross_stream, 12, 22);
    let output = document(&cross_stream);
    assert_code(&output["decisionChains"][0], "decision_lineage_mismatch");
    assert_eq!(
        output["decisionChains"][0]["reportedChainTipId"],
        Value::Null
    );

    let mut reversed = clean_chain_window();
    reversed["tail"].as_array_mut().unwrap().truncate(9);
    append(
        &mut reversed,
        decision(111, 201, 210, "approve", Value::Null),
    );
    append(
        &mut reversed,
        decision(212, 201, 210, "deny", json!(oid(111))),
    );
    reversed["tail"].as_array_mut().unwrap().swap(9, 10);
    rebuild_stream(&mut reversed, 11, 21);
    let output = document(&reversed);
    assert_code(&output["decisionChains"][0], "decision_sequence_mismatch");
    assert_eq!(
        output["decisionChains"][0]["reportedChainTipId"],
        Value::Null
    );
}

#[test]
fn provided_agent_delegation_failures_keep_reference_and_binding_diagnostics() {
    let mut ambiguous = clean_chain_window();
    ambiguous["tail"][1]["record"]["actorKind"] = json!("agent");
    for index in [9, 10, 11] {
        ambiguous["tail"][index]["record"]["delegationId"] = json!(oid(721));
    }
    let delegation = json!({
        "kind":"delegation","id":oid(721),"delegatorId":oid(100),"delegateId":oid(201),
        "parentDelegationId":null,"scope":scope(),"expiresMs":100,"state":"reported_active",
        "changedByActorId":oid(100),"externalRef":null,"reportedVerification":"reported_verified",
        "observedMs":null,"freshness":"reported_current","evidenceIds":[]
    });
    append(&mut ambiguous, delegation.clone());
    let mut conflicting = delegation;
    conflicting["expiresMs"] = json!(101);
    append(&mut ambiguous, conflicting);
    let output = document(&ambiguous);
    assert_code(event_row(&output, 111), "reference_ambiguous");
    assert_eq!(stage_row(&output, 110, 0)["reportedVotes"], Value::Null);

    let mut wrong_kind = clean_chain_window();
    wrong_kind["tail"][1]["record"]["actorKind"] = json!("agent");
    for index in [9, 10, 11] {
        wrong_kind["tail"][index]["record"]["delegationId"] = json!(oid(722));
    }
    append(&mut wrong_kind, actor(722, 3, 722, "agent"));
    let output = document(&wrong_kind);
    assert_code(event_row(&output, 111), "reference_kind_mismatch");
    assert_eq!(stage_row(&output, 110, 0)["reportedVotes"], Value::Null);

    let mut attenuation = clean_chain_window();
    attenuation["tail"][1]["record"]["actorKind"] = json!("agent");
    for index in [9, 10, 11] {
        attenuation["tail"][index]["record"]["delegationId"] = json!(oid(723));
    }
    append(&mut attenuation, actor(724, 3, 724, "human"));
    append(
        &mut attenuation,
        json!({
            "kind":"delegation","id":oid(725),"delegatorId":oid(724),"delegateId":oid(100),
            "parentDelegationId":null,"scope":scope(),"expiresMs":100,"state":"reported_active",
            "changedByActorId":oid(724),"externalRef":null,"reportedVerification":"reported_verified",
            "observedMs":null,"freshness":"reported_current","evidenceIds":[]
        }),
    );
    append(
        &mut attenuation,
        json!({
            "kind":"delegation","id":oid(723),"delegatorId":oid(100),"delegateId":oid(201),
            "parentDelegationId":oid(725),"scope":scope(),"expiresMs":100,"state":"reported_active",
            "changedByActorId":oid(100),"externalRef":null,"reportedVerification":"reported_verified",
            "observedMs":null,"freshness":"reported_current","evidenceIds":[]
        }),
    );
    let output = document(&attenuation);
    assert_code(event_row(&output, 111), "delegation_binding_mismatch");
    assert_eq!(stage_row(&output, 110, 0)["reportedVotes"], Value::Null);
}

fn expect_limit(value: &Value, expected: &str) {
    let decoded = decode(value);
    let before = decoded.canonical_bytes().to_vec();
    let error = inspect_approvals(&decoded).unwrap_err();
    assert_eq!(error.to_string(), expected);
    assert!(!format!("{error:?}").contains("ops:"));
    assert_eq!(decoded.canonical_bytes(), before.as_slice());
}

#[test]
fn dependency_walk_reentry_cycle_and_dangling_paths_fail_atomically() {
    let mut diamond = base_window();
    let mut diamond_stages = Vec::new();
    for index in 0..24_u16 {
        let mut row = stage(7500 + index, json!([]), "all", Value::Null);
        let mut dependencies = Vec::new();
        if index + 1 < 24 {
            dependencies.push(oid(7500 + index + 1));
        }
        if index + 2 < 24 {
            dependencies.push(oid(7500 + index + 2));
        }
        row["dependsOnStageIds"] = json!(dependencies);
        diamond_stages.push(row);
    }
    append(&mut diamond, plan(json!(diamond_stages)));
    let diamond_rows = diamond["tail"][0]["record"]["stages"].as_array().unwrap();
    let diamond_stage_count = diamond_rows.len();
    let diamond_dependency_count = diamond_rows
        .iter()
        .map(|row| row["dependsOnStageIds"].as_array().unwrap().len())
        .sum::<usize>();
    let mut diamond_descendant_steps = vec![0_usize; diamond_stage_count];
    for index in (0..diamond_stage_count).rev() {
        diamond_descendant_steps[index] = match diamond_rows[index]["dependsOnStageIds"]
            .as_array()
            .unwrap()
            .len()
        {
            0 => 0,
            1 => 1 + diamond_descendant_steps[index + 1],
            2 => 2 + diamond_descendant_steps[index + 1] + diamond_descendant_steps[index + 2],
            _ => unreachable!(),
        };
    }
    let diamond_walk_steps = diamond_descendant_steps.iter().sum::<usize>();
    let diamond_reference_rows =
        diamond_dependency_count + (diamond["tail"].as_array().unwrap().len() * 5);
    // Re-entering both sides of each diamond exceeds traversal work before any earlier cap.
    assert!(diamond_walk_steps > MAX_APPROVAL_PATH_STEPS);
    assert!(diamond_stage_count < MAX_APPROVAL_STAGES);
    assert!(diamond_reference_rows < MAX_APPROVAL_REFERENCES);
    assert!(diamond_dependency_count < MAX_APPROVAL_MATCHES);
    expect_limit(&diamond, "approval_limit");

    let mut cycles = base_window();
    for plan_offset in 0..33_u16 {
        let mut stages = Vec::new();
        for stage_offset in 0..16_u16 {
            let stage_id = 10000 + (plan_offset * 20) + stage_offset;
            let next_stage_id = 10000 + (plan_offset * 20) + ((stage_offset + 1) % 16);
            let mut row = stage(stage_id, json!([]), "all", Value::Null);
            row["dependsOnStageIds"] = json!([oid(next_stage_id)]);
            stages.push(row);
        }
        let mut record = plan(json!(stages));
        record["id"] = json!(oid(11000 + plan_offset));
        append(&mut cycles, record);
    }
    let cycle_plan_count = cycles["tail"].as_array().unwrap().len();
    let cycle_stage_count = cycles["tail"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["record"]["stages"].as_array().unwrap().len())
        .sum::<usize>();
    let cycle_length = cycles["tail"][0]["record"]["stages"]
        .as_array()
        .unwrap()
        .len();
    let cycle_dependency_count = cycles["tail"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|event| event["record"]["stages"].as_array().unwrap())
        .map(|row| row["dependsOnStageIds"].as_array().unwrap().len())
        .sum::<usize>();
    let cycle_reference_rows = cycle_dependency_count + (cycle_plan_count * 5);
    let cycle_walk_steps = cycle_stage_count * cycle_length;
    // Every stage closes its own 16-node cycle, including the closing visit.
    assert!(cycle_walk_steps > MAX_APPROVAL_PATH_STEPS);
    assert!(cycle_stage_count < MAX_APPROVAL_STAGES);
    assert!(cycle_reference_rows < MAX_APPROVAL_REFERENCES);
    assert!(cycle_dependency_count < MAX_APPROVAL_MATCHES);
    expect_limit(&cycles, "approval_limit");

    let mut dangling = base_window();
    for plan_offset in 0..6_u16 {
        let mut stages = Vec::new();
        for stage_offset in 0..32_u16 {
            let stage_id = 13000 + (plan_offset * 40) + stage_offset;
            let mut row = stage(stage_id, json!([]), "all", Value::Null);
            if stage_offset + 1 < 32 {
                row["dependsOnStageIds"] = json!([oid(stage_id + 1)]);
            } else {
                row["dependsOnStageIds"] = json!(
                    (0..32_u16)
                        .map(|missing| oid(14000 + (plan_offset * 32) + missing))
                        .collect::<Vec<_>>()
                );
            }
            stages.push(row);
        }
        let mut record = plan(json!(stages));
        record["id"] = json!(oid(12000 + plan_offset));
        append(&mut dangling, record);
    }
    let dangling_plan_count = dangling["tail"].as_array().unwrap().len();
    let dangling_stage_count = dangling["tail"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["record"]["stages"].as_array().unwrap().len())
        .sum::<usize>();
    let dangling_chain_length = dangling["tail"][0]["record"]["stages"]
        .as_array()
        .unwrap()
        .len();
    let dangling_terminal_count = dangling["tail"][0]["record"]["stages"]
        [dangling_chain_length - 1]["dependsOnStageIds"]
        .as_array()
        .unwrap()
        .len();
    let dangling_dependency_count = dangling["tail"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|event| event["record"]["stages"].as_array().unwrap())
        .map(|row| row["dependsOnStageIds"].as_array().unwrap().len())
        .sum::<usize>();
    let dangling_matching_count = dangling["tail"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|event| event["record"]["stages"].as_array().unwrap())
        .filter(|row| row["dependsOnStageIds"].as_array().unwrap().len() == 1)
        .count();
    let dangling_reference_rows = dangling_dependency_count + (dangling_plan_count * 5);
    let dangling_walk_steps = dangling_plan_count
        * ((dangling_chain_length * dangling_terminal_count)
            + ((dangling_chain_length - 1) * dangling_chain_length / 2));
    // Each chain reaches all terminal dangling IDs, so path work exceeds the cap.
    assert!(dangling_walk_steps > MAX_APPROVAL_PATH_STEPS);
    assert!(dangling_stage_count < MAX_APPROVAL_STAGES);
    assert!(dangling_reference_rows < MAX_APPROVAL_REFERENCES);
    assert!(dangling_matching_count < MAX_APPROVAL_MATCHES);
    expect_limit(&dangling, "approval_limit");
}

#[test]
fn stage_and_reference_caps_fail_closed_without_mutating_input() {
    let mut stages = base_window();
    for index in 0..33_u16 {
        let mut rows = Vec::new();
        for offset in 0..32_u16 {
            rows.push(stage(
                2100 + (index * 32) + offset,
                json!([oid(100)]),
                "all",
                Value::Null,
            ));
        }
        let mut record = plan(json!(rows));
        record["id"] = json!(oid(1100 + index));
        append(&mut stages, record);
    }
    expect_limit(&stages, "approval_limit");

    let mut references = base_window();
    for index in 0..257_u16 {
        let record = json!({
            "kind":"authorization","id":oid(1200 + index),"context":context(75),
            "evaluationId":oid(109),"planId":null,"decisionIds":vec![oid(900); 32],
            "previousObservationId":null,"state":"reported_issued","scope":scope(),
            "expiresMs":null,"reason":"none","externalRef":null,"evidenceIds":[]
        });
        append(&mut references, record);
    }
    expect_limit(&references, "approval_relationship_limit");
}

#[test]
fn nonempty_wrapper_debug_is_digest_and_size_only() {
    let decoded = decode(&clean_chain_window());
    let inspection = inspect_approvals(&decoded).unwrap();
    let debug = format!("{inspection:?}");
    assert!(debug.contains("ApprovalInspection"));
    assert!(debug.contains(&inspection.canonical_digest()));
    assert!(debug.contains(&inspection.canonical_bytes().len().to_string()));
    for forbidden in [
        "ops:",
        "schemaVersion",
        "eventAssessments",
        "decisionChains",
    ] {
        assert!(!debug.contains(forbidden), "debug exposed {forbidden}");
    }
}

#[test]
fn escalation_is_not_membership_and_inactive_or_agent_reports_stay_diagnostic() {
    let mut escalation = clean_chain_window();
    escalation["tail"][8]["record"]["stages"][0]["escalationActorIds"] = json!([oid(100)]);
    escalation["tail"][9]["record"]["actorId"] = json!(oid(100));
    escalation["tail"][10]["record"]["actorId"] = json!(oid(100));
    escalation["tail"][11]["record"]["actorId"] = json!(oid(100));
    let output = document(&escalation);
    assert_eq!(
        stage_row(&output, 110, 0)["reportedEscalationSubjects"],
        json!([{"systemId":oid(3),"canonicalSubjectId":oid(200)}])
    );
    assert_code(event_row(&output, 111), "decision_actor_not_listed");

    let mut inactive = clean_chain_window();
    inactive["tail"][1]["record"]["freshness"] = json!("reported_stale");
    let output = document(&inactive);
    assert_code(event_row(&output, 111), "actor_report_inactive");

    let mut agent = clean_chain_window();
    agent["tail"][1]["record"]["actorKind"] = json!("agent");
    let output = document(&agent);
    assert_code(event_row(&output, 111), "agent_delegation_missing");
    assert_eq!(stage_row(&output, 110, 0)["reportedVotes"], Value::Null);
}

#[test]
fn requester_alias_and_foreign_system_do_not_gain_or_hide_identity_diagnostics() {
    let self_approval = clean_chain_window();
    let output = document(&self_approval);
    assert!(!codes(event_row(&output, 111)).contains(&"requester_self_approval".to_owned()));

    let mut foreign = clean_chain_window();
    foreign["tail"][6]["record"]["actorId"] = json!(oid(301));
    append(&mut foreign, actor(301, 33, 401, "human"));
    let output = document(&foreign);
    assert_code(event_row(&output, 111), "subject_incomplete");
    assert!(!codes(event_row(&output, 111)).contains(&"requester_self_approval".to_owned()));
}

#[test]
fn invalid_quorum_and_missing_requester_tuples_do_not_invent_identity() {
    let mut quorum = clean_chain_window();
    quorum["tail"][8]["record"]["stages"][0]["mode"] = json!("quorum");
    quorum["tail"][8]["record"]["stages"][0]["quorum"] = json!(2);
    let output = document(&quorum);
    let stage = stage_row(&output, 110, 0);
    assert_code(stage, "quorum_invalid");
    assert_eq!(stage["requiredReportedSubjectCount"], Value::Null);

    for decision_actor_missing in [false, true] {
        let mut value = clean_chain_window();
        value["tail"][6]["record"]["actorId"] = json!(oid(999));
        if decision_actor_missing {
            for index in [9, 10, 11] {
                value["tail"][index]["record"]["actorId"] = json!(oid(999));
            }
        }
        let output = document(&value);
        let row = event_row(&output, 111);
        assert_code(row, "reference_missing");
        assert_code(row, "subject_incomplete");
        assert!(!codes(row).contains(&"requester_self_approval".to_owned()));
    }
}

#[test]
fn matching_walk_and_upstream_caps_have_fixed_atomic_errors() {
    let matching_reference_rows = 65 * (5 + 32);
    let matching_ordinals = 65 * 32 * 128;
    assert!(matching_reference_rows < MAX_APPROVAL_REFERENCES);
    assert!(matching_ordinals > MAX_APPROVAL_MATCHES);
    let mut matching = base_window();
    for _ in 0..128 {
        append(&mut matching, actor(100, 3, 200, "human"));
    }
    for index in 0..65_u16 {
        let record = json!({
            "kind":"authorization","id":oid(1400 + index),"context":context(75),
            "evaluationId":oid(109),"planId":null,"decisionIds":vec![oid(100); 32],
            "previousObservationId":null,"state":"reported_issued","scope":scope(),
            "expiresMs":null,"reason":"none","externalRef":null,"evidenceIds":[]
        });
        append(&mut matching, record);
    }
    expect_limit(&matching, "approval_relationship_limit");

    let mut own_matches = base_window();
    append(&mut own_matches, actor(100, 3, 200, "human"));
    for _ in 0..128 {
        append(&mut own_matches, actor(201, 3, 301, "human"));
    }
    for index in 0..65_u16 {
        let mut record = plan(json!([stage(
            3500 + index,
            json!(vec![oid(201); 32]),
            "all",
            Value::Null
        )]));
        record["id"] = json!(oid(1500 + index));
        append(&mut own_matches, record);
    }
    expect_limit(&own_matches, "approval_limit");

    let walk_stages = 20 * 32;
    let walk_steps = 20 * (1..32).sum::<usize>();
    assert!(walk_stages < MAX_APPROVAL_STAGES);
    assert!(walk_steps > MAX_APPROVAL_PATH_STEPS);
    let mut walks = base_window();
    for plan_offset in 0..20_u16 {
        let mut stages = Vec::new();
        for stage_offset in 0..32_u16 {
            let mut row = stage(
                4000 + (plan_offset * 32) + stage_offset,
                json!([]),
                "all",
                Value::Null,
            );
            if stage_offset > 0 {
                row["dependsOnStageIds"] =
                    json!([oid(4000 + (plan_offset * 32) + stage_offset - 1)]);
            }
            stages.push(row);
        }
        let mut record = plan(json!(stages));
        record["id"] = json!(oid(2000 + plan_offset));
        append(&mut walks, record);
    }
    expect_limit(&walks, "approval_limit");

    let wire_reference_rows = 200 * (5 + 32);
    assert!(wire_reference_rows < MAX_APPROVAL_REFERENCES);
    let mut wire = base_window();
    for index in 0..200_u16 {
        let mut record = plan(json!([stage(
            3000 + index,
            json!(vec![oid(900); 32]),
            "all",
            Value::Null
        )]));
        record["id"] = json!(oid(3000 + index));
        append(&mut wire, record);
    }
    let wire_input = serde_json::to_vec(&wire).unwrap();
    assert!(wire_input.len() < MAX_INPUT_BYTES);
    expect_limit(&wire, "approval_limit");

    let mut upstream = base_window();
    for index in 0..1024_u16 {
        append(&mut upstream, actor(100, 3, 200, "human"));
        upstream["tail"].as_array_mut().unwrap().last_mut().unwrap()["eventId"] =
            json!(oid(5000 + index));
    }
    upstream["snapshot"]["topology"]["nodes"] = json!(
        (0..128_u16)
            .map(|index| json!({"id":oid(6000 + index),"kind":"actor","recordId":oid(100)}))
            .collect::<Vec<_>>()
    );
    expect_limit(&upstream, "approval_structural_limit");
}

#[test]
fn upstream_relationship_and_delegation_limits_map_to_approval_errors() {
    let mut relationship_wire = clean_chain_window();
    relationship_wire["tail"]
        .as_array_mut()
        .unwrap()
        .truncate(6);
    let request = json!({
        "kind":"request","id":oid(108),"context":context(70),"actorId":oid(100),
        "gatewayRevisionId":oid(105),"operationClass":"deploy","actionDigest":digest(8),
        "contextDigest":digest(9),"resourceIds":vec![oid(300); 32],"workflowRef":null,"expiresMs":null
    });
    for _ in 0..200 {
        append(&mut relationship_wire, request.clone());
    }
    expect_limit(&relationship_wire, "approval_relationship_limit");

    let mut delegation_wire = base_window();
    for index in 0..1024_u16 {
        append(
            &mut delegation_wire,
            actor(2000 + index, 3, 4000 + index, "agent"),
        );
    }
    expect_limit(&delegation_wire, "approval_delegation_limit");
}
