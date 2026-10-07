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

fn correlation(span: u16, evaluation: Value) -> Value {
    json!({
        "workspaceId":oid(2),"systemId":oid(3),"environmentId":oid(4),"engineId":oid(5),
        "traceId":oid(6),"spanId":oid(span),"requestId":oid(108),
        "evaluationId":evaluation,"attemptId":null,"policyRevisionId":oid(104)
    })
}

fn event(sequence: u64, id: u16, record: Value, correlation: Value) -> Value {
    json!({
        "eventId":oid(id),"producerId":oid(11),"fenceId":oid(21),"sequence":sequence,
        "previousEventId":if sequence == 1 { Value::Null } else { json!(oid(id - 1)) },
        "observedMs":null,"receivedMs":null,"correlation":correlation,"record":record
    })
}

fn actor() -> Value {
    json!({"kind":"actor","id":oid(100),"systemId":oid(3),"canonicalSubjectId":oid(200),"actorKind":"agent","identitySourceRef":null,"identityEvidenceId":null,"reportedVerification":"reported_verified","observedMs":null,"freshness":"reported_current"})
}

fn engine() -> Value {
    json!({"kind":"engine","id":oid(5),"systemId":oid(3),"externalRef":{"systemId":oid(3),"contractVersion":"v1","externalId":"engine"},"artifactDigest":digest(5)})
}

fn policy() -> Value {
    json!({"kind":"policy_revision","id":oid(104),"gatewayId":oid(105),"previousRevisionId":null,"artifactDigest":digest(6),"externalRef":null})
}

fn gateway() -> Value {
    json!({"kind":"gateway_revision","id":oid(105),"gatewayId":oid(105),"previousRevisionId":null,"policyRevisionId":oid(104),"scope":{"systemId":oid(3),"environmentIds":[oid(4)],"operationClasses":[],"resourceIds":[],"policyRevisionIds":[oid(104)]},"humanRequired":true,"changedByActorId":oid(100),"state":"reported_active","evidenceIds":[]})
}

fn request() -> Value {
    json!({"kind":"request","id":oid(108),"context":context(70),"actorId":oid(100),"gatewayRevisionId":oid(105),"operationClass":"deploy","actionDigest":digest(8),"contextDigest":digest(9),"resourceIds":[],"workflowRef":null,"expiresMs":null})
}

fn evaluation(id: u16, kind: &str, previous: Value, expected: Value, span: u16) -> Value {
    json!({"kind":"evaluation","id":oid(id),"context":context(span),"evaluationKind":kind,"gatewayRevisionId":oid(105),"evaluatorId":oid(100),"evaluatorArtifactDigest":digest(10),"inputSnapshotId":oid(1),"inputCanonicalization":"json-v1","inputDigest":digest(11),"contextSnapshotId":oid(1),"contextDigest":digest(9),"previousEvaluationId":previous,"expectedEffectiveEvaluationId":expected,"lineageFenceId":oid(21),"result":"deny","expiresMs":null,"reason":"policy_denied","externalRef":null,"evidenceIds":[]})
}

fn plan() -> Value {
    json!({"kind":"approval_plan","id":oid(110),"context":context(73),"evaluationId":oid(109),"previousPlanId":null,"changedByActorId":oid(100),"expiresMs":null,"stages":[{"id":oid(210),"dependsOnStageIds":[],"approverIds":[oid(100)],"humanRequired":true,"mode":"all","quorum":null,"expiresMs":null,"escalationActorIds":[]}],"evidenceIds":[]})
}

fn decision() -> Value {
    json!({"kind":"approval_decision","id":oid(111),"context":context(74),"evaluationId":oid(109),"planId":oid(110),"stageId":oid(210),"actorId":oid(100),"delegationId":null,"decision":"approve","supersedesDecisionId":null,"reason":"none","evidenceIds":[]})
}

fn authorization() -> Value {
    json!({"kind":"authorization","id":oid(112),"context":context(75),"evaluationId":oid(109),"planId":oid(110),"decisionIds":[oid(111)],"previousObservationId":null,"state":"reported_issued","scope":{"systemId":oid(3),"environmentIds":[oid(4)],"operationClasses":[],"resourceIds":[],"policyRevisionIds":[oid(104)]},"expiresMs":null,"reason":"none","externalRef":null,"evidenceIds":[]})
}

fn window() -> Value {
    json!({
        "schemaVersion":"rangoon.ops.window.v1",
        "snapshot":{"snapshotId":oid(1),"reducerVersion":"rangoon.ops.reducer.v1",
        "scope":{"workspaceId":oid(2),"systemIds":[oid(3)],"environmentIds":[oid(4)],"engineIds":[oid(5)]},
        "topology":{"revisionId":oid(8),"nodes":[],"edges":[],"policyRevisionIds":[oid(104)],"coverageRevisionIds":[]},"prefixes":[],"asOfMs":0},
        "tail":[
            event(1,501,actor(),Value::Null),event(2,502,engine(),Value::Null),
            event(3,503,policy(),correlation(69,Value::Null)),event(4,504,gateway(),correlation(69,Value::Null)),
            event(5,505,request(),correlation(70,Value::Null)),
            event(6,506,evaluation(109,"initial",Value::Null,Value::Null,71),correlation(71,json!(oid(109)))),
            event(7,507,plan(),correlation(73,json!(oid(109)))),
            event(8,508,decision(),correlation(74,json!(oid(109)))),
            event(9,509,authorization(),correlation(75,json!(oid(109))))
        ]
    })
}

fn document(value: &Value) -> Value {
    let decoded = decode_window(&serde_json::to_vec(value).unwrap()).unwrap();
    let related = relate_window(&decoded).unwrap();
    serde_json::to_value(related.document()).unwrap()
}

fn link_codes(document: &Value, record_id: &str) -> Vec<String> {
    document["eventLinks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["recordId"] == record_id)
        .unwrap()["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect()
}

fn link<'a>(document: &'a Value, record_id: &str) -> &'a Value {
    document["eventLinks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["recordId"] == record_id)
        .unwrap()
}

fn resource(id: u16) -> Value {
    json!({
        "kind":"resource","id":oid(id),"systemId":oid(3),"resourceClass":"service",
        "externalRef":null
    })
}

fn append(value: &mut Value, sequence: u64, event_id: u16, record: Value, correlation: Value) {
    value["tail"]
        .as_array_mut()
        .unwrap()
        .push(event(sequence, event_id, record, correlation));
}

fn assert_code(document: &Value, record_id: u16, expected: &str) {
    assert!(
        link_codes(document, &oid(record_id)).contains(&expected.to_owned()),
        "{} missing {expected}",
        oid(record_id)
    );
}

#[test]
fn coherent_chain_with_different_spans_retains_deny_and_fixed_trust() {
    let document = document(&window());
    assert_eq!(document["authority"], "none");
    assert_eq!(document["authenticity"], "unverified");
    assert_eq!(document["execution"], "unavailable");
    assert_eq!(document["semanticAssessment"], "reported_links_only");
    assert_eq!(document["authoritativeEligibility"], "unknown");
    assert_eq!(document["authoritativeEffectiveEvaluationId"], Value::Null);
    assert_eq!(document["eventLinks"].as_array().unwrap().len(), 9);
    assert_eq!(document["chains"][0]["reportedResult"], "deny");
    assert_eq!(
        document["chains"][0]["reportedChainStatus"],
        "single_chain_unverified"
    );
    assert_eq!(document["chains"][0]["reportedChainTipId"], oid(109));
}

#[test]
fn eight_initial_successor_selector_combinations_do_not_silently_select_invalid_chain() {
    for (kind, previous, expected, valid) in [
        ("initial", Value::Null, Value::Null, true),
        ("initial", json!(oid(109)), Value::Null, false),
        ("initial", Value::Null, json!(oid(109)), false),
        ("initial", json!(oid(109)), json!(oid(109)), false),
        ("successor", Value::Null, Value::Null, false),
        ("successor", json!(oid(109)), Value::Null, false),
        ("successor", Value::Null, json!(oid(109)), false),
        ("successor", json!(oid(109)), json!(oid(109)), true),
    ] {
        let mut value = window();
        value["tail"][5]["record"] = evaluation(109, kind, previous, expected, 71);
        let document = document(&value);
        let codes = link_codes(&document, &oid(109));
        assert_eq!(
            !codes.contains(&"evaluation_selector_mismatch".to_owned()),
            valid,
            "{kind}"
        );
    }
}

#[test]
fn missing_wrong_kind_ambiguous_and_exact_repeat_references_remain_explicit() {
    let mut missing = window();
    missing["tail"][4]["record"]["actorId"] = json!(oid(999));
    assert!(link_codes(&document(&missing), &oid(108)).contains(&"reference_missing".to_owned()));
    let mut wrong = window();
    wrong["tail"][4]["record"]["actorId"] = json!(oid(5));
    assert!(
        link_codes(&document(&wrong), &oid(108)).contains(&"reference_kind_mismatch".to_owned())
    );
    let mut ambiguous = window();
    ambiguous["tail"].as_array_mut().unwrap().push(event(10,510,json!({"kind":"actor","id":oid(100),"systemId":oid(3),"canonicalSubjectId":oid(201),"actorKind":"agent","identitySourceRef":null,"identityEvidenceId":null,"reportedVerification":"unknown","observedMs":null,"freshness":"unknown"}),Value::Null));
    assert!(
        link_codes(&document(&ambiguous), &oid(108)).contains(&"reference_ambiguous".to_owned())
    );
    let mut repeated = window();
    let duplicate = repeated["tail"][0].clone();
    repeated["tail"].as_array_mut().unwrap().push(duplicate);
    let repeated_document = document(&repeated);
    assert!(!link_codes(&repeated_document, &oid(108)).contains(&"reference_ambiguous".to_owned()));
    let actor_reference = link(&repeated_document, &oid(108))["references"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["field"] == "actorId")
        .unwrap();
    assert_eq!(actor_reference["status"], "unique_unverified");
    assert_eq!(actor_reference["matchingEventOrdinals"], json!([0, 9]));
}

#[test]
fn branching_cycle_disconnected_and_lineage_fence_observations_never_pick_a_tip() {
    let mut branch = window();
    branch["tail"].as_array_mut().unwrap().push(event(
        10,
        510,
        evaluation(110, "successor", json!(oid(109)), json!(oid(109)), 76),
        correlation(76, json!(oid(110))),
    ));
    branch["tail"].as_array_mut().unwrap().push(event(
        11,
        511,
        evaluation(111, "successor", json!(oid(109)), json!(oid(109)), 77),
        correlation(77, json!(oid(111))),
    ));
    let branch_doc = document(&branch);
    assert_eq!(
        branch_doc["chains"][0]["reportedChainStatus"],
        "unknown_or_conflict"
    );
    assert_eq!(branch_doc["chains"][0]["reportedChainTipId"], Value::Null);
    assert!(
        branch_doc["diagnosticCounts"]["evaluation_branch"]
            .as_u64()
            .unwrap()
            > 0
    );

    let mut cycle = window();
    cycle["tail"][5]["record"] = evaluation(109, "successor", json!(oid(109)), json!(oid(109)), 71);
    let cycle_doc = document(&cycle);
    assert_eq!(cycle_doc["chains"][0]["reportedChainTipId"], Value::Null);
    assert!(
        cycle_doc["diagnosticCounts"]["evaluation_cycle"]
            .as_u64()
            .unwrap()
            > 0
    );

    let mut roots = window();
    roots["tail"].as_array_mut().unwrap().push(event(
        10,
        510,
        evaluation(113, "initial", Value::Null, Value::Null, 76),
        correlation(76, json!(oid(113))),
    ));
    let roots_doc = document(&roots);
    assert_eq!(
        roots_doc["chains"][0]["reportedChainStatus"],
        "unknown_or_conflict"
    );
    assert!(
        roots_doc["diagnosticCounts"]["evaluation_branch"]
            .as_u64()
            .unwrap()
            > 0
    );

    let mut disconnected = window();
    append(
        &mut disconnected,
        10,
        510,
        evaluation(113, "successor", json!(oid(114)), json!(oid(114)), 76),
        correlation(76, json!(oid(113))),
    );
    append(
        &mut disconnected,
        11,
        511,
        evaluation(114, "successor", json!(oid(113)), json!(oid(113)), 77),
        correlation(77, json!(oid(114))),
    );
    let disconnected_doc = document(&disconnected);
    assert_eq!(
        disconnected_doc["chains"][0]["reportedChainStatus"],
        "unknown_or_conflict"
    );
    assert!(
        disconnected_doc["diagnosticCounts"]["evaluation_incomplete_chain"]
            .as_u64()
            .unwrap()
            > 0
    );

    let mut fence = window();
    fence["tail"][5]["fenceId"] = json!(oid(22));
    let fence_doc = document(&fence);
    assert!(link_codes(&fence_doc, &oid(109)).contains(&"lineage_fence_mismatch".to_owned()));
}

#[test]
fn request_only_chain_is_reported_without_an_evaluation() {
    let mut value = window();
    value["tail"].as_array_mut().unwrap().truncate(5);

    let document = document(&value);
    assert_eq!(document["chains"].as_array().unwrap().len(), 1);
    assert_eq!(
        document["chains"][0]["reportedChainStatus"],
        "no_evaluation"
    );
    assert_eq!(document["chains"][0]["reportedChainTipId"], Value::Null);
    assert_eq!(document["chains"][0]["reportedResult"], Value::Null);
    assert_eq!(
        link(&document, &oid(108))["assessment"],
        "linked_unverified"
    );
}

#[test]
fn every_flow_key_selector_mismatch_is_explicit_while_spans_remain_distinct() {
    for (offset, field) in [
        "workspaceId",
        "systemId",
        "environmentId",
        "engineId",
        "requestId",
        "traceId",
        "policyRevisionId",
    ]
    .iter()
    .enumerate()
    {
        let mut value = window();
        value["tail"][6]["record"]["context"][*field] = json!(oid(300 + offset as u16));
        let document = document(&value);
        assert_code(&document, 110, "context_mismatch");
    }

    let document = document(&window());
    for record_id in [108, 109, 110, 111, 112] {
        assert!(
            !link_codes(&document, &oid(record_id)).contains(&"context_mismatch".to_owned()),
            "different per-step span must stay legitimate: {}",
            oid(record_id)
        );
    }
}

#[test]
fn gateway_policy_context_digest_and_lineage_mismatches_are_never_linked() {
    let mut gateway = window();
    gateway["tail"][5]["record"]["gatewayRevisionId"] = json!(oid(106));
    let mut alternate = gateway["tail"][3]["record"].clone();
    alternate["id"] = json!(oid(106));
    append(
        &mut gateway,
        10,
        510,
        alternate,
        correlation(69, Value::Null),
    );
    assert_code(&document(&gateway), 109, "gateway_mismatch");

    let mut policy = window();
    policy["tail"][3]["record"]["gatewayId"] = json!(oid(106));
    assert_code(&document(&policy), 108, "gateway_policy_mismatch");

    let mut pinned_policy = window();
    pinned_policy["tail"][4]["record"]["context"]["policyRevisionId"] = json!(oid(106));
    assert_code(&document(&pinned_policy), 108, "gateway_policy_mismatch");

    let mut digest_mismatch = window();
    digest_mismatch["tail"][5]["record"]["contextDigest"] = json!(digest(99));
    assert_code(&document(&digest_mismatch), 109, "context_digest_mismatch");

    let mut declared_fence = window();
    declared_fence["tail"][5]["record"]["lineageFenceId"] = json!(oid(22));
    assert_code(&document(&declared_fence), 109, "lineage_fence_mismatch");
}

#[test]
fn predecessor_producer_and_sequence_mismatches_block_successor_selection() {
    let mut producer = window();
    let successor = evaluation(113, "successor", json!(oid(109)), json!(oid(109)), 76);
    let mut observed = event(1, 510, successor, correlation(76, json!(oid(113))));
    observed["producerId"] = json!(oid(12));
    observed["previousEventId"] = Value::Null;
    producer["tail"].as_array_mut().unwrap().push(observed);
    let producer_document = document(&producer);
    assert_code(&producer_document, 113, "lineage_fence_mismatch");
    assert_eq!(
        producer_document["chains"][0]["reportedChainStatus"],
        "unknown_or_conflict"
    );

    let mut sequence = window();
    sequence["tail"][5]["sequence"] = json!(10);
    sequence["tail"][5]["previousEventId"] = json!(oid(509));
    append(
        &mut sequence,
        7,
        510,
        evaluation(113, "successor", json!(oid(109)), json!(oid(109)), 76),
        correlation(76, json!(oid(113))),
    );
    assert_code(&document(&sequence), 113, "evaluation_sequence_mismatch");
}

#[test]
fn structural_stream_and_envelope_taints_are_conservative_but_graph_only_noise_is_not() {
    let mut correlation_mismatch = window();
    correlation_mismatch["tail"][5]["correlation"]["spanId"] = json!(oid(999));
    assert_code(
        &document(&correlation_mismatch),
        109,
        "structural_incomplete",
    );

    let mut stream_gap = window();
    stream_gap["tail"][6]["sequence"] = json!(10);
    stream_gap["tail"][6]["previousEventId"] = json!(oid(506));
    assert_code(&document(&stream_gap), 110, "structural_incomplete");

    let mut prefix_taint = window();
    prefix_taint["snapshot"]["prefixes"] = json!([{
        "producerId":oid(11),"fenceId":oid(21),
        "watermark":{"sequence":999,"eventId":oid(999)},"events":[]
    }]);
    assert_code(&document(&prefix_taint), 108, "structural_incomplete");

    let mut graph_only = window();
    graph_only["snapshot"]["topology"]["nodes"] = json!([{
        "id":oid(900),"kind":"tool","recordId":oid(999)
    }]);
    let graph_document = document(&graph_only);
    assert!(!link_codes(&graph_document, &oid(108)).contains(&"structural_incomplete".to_owned()));
}

#[test]
fn decision_and_authorization_bindings_require_the_same_plan_evaluation_and_stage() {
    let mut stage = window();
    stage["tail"][7]["record"]["stageId"] = json!(oid(211));
    assert_code(&document(&stage), 111, "binding_mismatch");

    let mut decision_evaluation = window();
    append(
        &mut decision_evaluation,
        10,
        510,
        evaluation(113, "successor", json!(oid(109)), json!(oid(109)), 76),
        correlation(76, json!(oid(113))),
    );
    decision_evaluation["tail"][7]["record"]["evaluationId"] = json!(oid(113));
    assert_code(&document(&decision_evaluation), 111, "binding_mismatch");

    let mut decision_plan = window();
    append(
        &mut decision_plan,
        10,
        510,
        evaluation(113, "successor", json!(oid(109)), json!(oid(109)), 76),
        correlation(76, json!(oid(113))),
    );
    let mut later_plan = plan();
    later_plan["id"] = json!(oid(120));
    later_plan["context"] = context(80);
    later_plan["evaluationId"] = json!(oid(113));
    append(
        &mut decision_plan,
        11,
        511,
        later_plan,
        correlation(80, json!(oid(113))),
    );
    decision_plan["tail"][7]["record"]["planId"] = json!(oid(120));
    assert_code(&document(&decision_plan), 111, "binding_mismatch");

    let mut authorization = window();
    append(
        &mut authorization,
        10,
        510,
        evaluation(113, "successor", json!(oid(109)), json!(oid(109)), 76),
        correlation(76, json!(oid(113))),
    );
    let mut alternate_plan = plan();
    alternate_plan["id"] = json!(oid(120));
    alternate_plan["context"] = context(80);
    alternate_plan["evaluationId"] = json!(oid(113));
    append(
        &mut authorization,
        11,
        511,
        alternate_plan,
        correlation(80, json!(oid(113))),
    );
    let mut alternate_decision = decision();
    alternate_decision["id"] = json!(oid(121));
    alternate_decision["context"] = context(81);
    alternate_decision["evaluationId"] = json!(oid(113));
    alternate_decision["planId"] = json!(oid(120));
    append(
        &mut authorization,
        12,
        512,
        alternate_decision,
        correlation(81, json!(oid(113))),
    );
    authorization["tail"][8]["record"]["decisionIds"] = json!([oid(121)]);
    assert_code(&document(&authorization), 112, "binding_mismatch");

    let mut authorization_plan = window();
    append(
        &mut authorization_plan,
        10,
        510,
        evaluation(113, "successor", json!(oid(109)), json!(oid(109)), 76),
        correlation(76, json!(oid(113))),
    );
    let mut alternate_plan = plan();
    alternate_plan["id"] = json!(oid(120));
    alternate_plan["context"] = context(80);
    alternate_plan["evaluationId"] = json!(oid(113));
    append(
        &mut authorization_plan,
        11,
        511,
        alternate_plan,
        correlation(80, json!(oid(113))),
    );
    authorization_plan["tail"][8]["record"]["planId"] = json!(oid(120));
    assert_code(&document(&authorization_plan), 112, "binding_mismatch");
}

#[test]
fn late_historical_approval_records_stay_on_their_old_evaluation() {
    let mut value = window();
    let mut successor = evaluation(113, "successor", json!(oid(109)), json!(oid(109)), 76);
    successor["result"] = json!("allow");
    append(
        &mut value,
        10,
        510,
        successor,
        correlation(76, json!(oid(113))),
    );

    let mut old_plan = plan();
    old_plan["id"] = json!(oid(120));
    old_plan["context"] = context(80);
    old_plan["previousPlanId"] = json!(oid(110));
    append(
        &mut value,
        11,
        511,
        old_plan,
        correlation(80, json!(oid(109))),
    );
    let mut old_decision = decision();
    old_decision["id"] = json!(oid(121));
    old_decision["context"] = context(81);
    old_decision["planId"] = json!(oid(120));
    old_decision["supersedesDecisionId"] = json!(oid(111));
    append(
        &mut value,
        12,
        512,
        old_decision,
        correlation(81, json!(oid(109))),
    );
    let mut old_authorization = authorization();
    old_authorization["id"] = json!(oid(122));
    old_authorization["context"] = context(82);
    old_authorization["planId"] = json!(oid(120));
    old_authorization["decisionIds"] = json!([oid(121)]);
    old_authorization["previousObservationId"] = json!(oid(112));
    append(
        &mut value,
        13,
        513,
        old_authorization,
        correlation(82, json!(oid(109))),
    );

    let document = document(&value);
    assert_eq!(document["chains"][0]["reportedChainTipId"], oid(113));
    assert_eq!(document["chains"][0]["reportedResult"], "allow");
    for record_id in [120, 121, 122] {
        assert_code(&document, record_id, "non_tip_evaluation");
        assert_eq!(
            link(&document, &oid(record_id))["assessment"],
            "linked_unverified"
        );
    }
    assert_eq!(document["authoritativeEligibility"], "unknown");
    assert_eq!(document["authoritativeEffectiveEvaluationId"], Value::Null);
}

#[test]
fn resources_and_unassessed_events_are_retained_with_every_reference_observation() {
    let mut value = window();
    value["tail"][4]["record"]["resourceIds"] = json!([oid(300), oid(300)]);
    append(&mut value, 10, 510, resource(300), Value::Null);
    append(
        &mut value,
        11,
        511,
        json!({
            "kind":"tool","id":oid(301),"systemId":oid(3),"operationClass":"deploy",
            "externalRef":null
        }),
        Value::Null,
    );

    let document = document(&value);
    assert_eq!(document["eventLinks"].as_array().unwrap().len(), 11);
    assert_eq!(link(&document, &oid(301))["assessment"], "not_assessed");
    let references = link(&document, &oid(108))["references"].as_array().unwrap();
    let resource_rows: Vec<&Value> = references
        .iter()
        .filter(|row| row["field"] == "resourceIds")
        .collect();
    assert_eq!(resource_rows.len(), 2);
    for (index, row) in resource_rows.into_iter().enumerate() {
        assert_eq!(row["index"], index);
        assert_eq!(row["recordId"], oid(300));
        assert_eq!(row["status"], "unique_unverified");
        assert_eq!(row["matchingEventOrdinals"], json!([9]));
    }
}

#[test]
fn empty_window_has_independent_literal_canonical_bytes_and_sha() {
    const EMPTY_CANONICAL_WINDOW: &[u8] = br#"{"schemaVersion":"rangoon.ops.window.v1","snapshot":{"snapshotId":"ops:1111111111111111111111111111111111111111111111111111111111111111","reducerVersion":"rangoon.ops.reducer.v1","scope":{"workspaceId":"ops:1111111111111111111111111111111111111111111111111111111111111111","systemIds":[],"environmentIds":[],"engineIds":[]},"topology":{"revisionId":"ops:1111111111111111111111111111111111111111111111111111111111111111","nodes":[],"edges":[],"policyRevisionIds":[],"coverageRevisionIds":[]},"prefixes":[],"asOfMs":0},"tail":[]}"#;
    const EMPTY_SHA256: &str = "8ab43d0754e994dc00445661f74d1da62b6efec06f7c6276bee1b9e1fc25c0a7";
    const EMPTY_RELATIONSHIPS: &str = r#"{"schemaVersion":"rangoon.ops.relationships.v1","inputCanonicalDigest":"8ab43d0754e994dc00445661f74d1da62b6efec06f7c6276bee1b9e1fc25c0a7","structuralProjectionDigest":"5c8986e86e5cc7e23d6acfeed340d70cce710e8a7e71b117058c02296e928bba","authority":"none","authenticity":"unverified","execution":"unavailable","semanticAssessment":"reported_links_only","authoritativeEligibility":"unknown","authoritativeEffectiveEvaluationId":null,"diagnosticCounts":{"structural_incomplete":0,"reference_missing":0,"reference_ambiguous":0,"reference_kind_mismatch":0,"context_mismatch":0,"gateway_mismatch":0,"context_digest_mismatch":0,"gateway_policy_mismatch":0,"evaluation_selector_mismatch":0,"lineage_fence_mismatch":0,"evaluation_sequence_mismatch":0,"evaluation_branch":0,"evaluation_cycle":0,"evaluation_incomplete_chain":0,"binding_mismatch":0,"non_tip_evaluation":0},"eventLinks":[],"chains":[]}"#;
    const EMPTY_RELATIONSHIPS_SHA256: &str =
        "07ce0a123f67d402d177313ab4cfb810ef28adcc69dc234beb466ae1ae914ee7";

    let decoded = decode_window(EMPTY_CANONICAL_WINDOW).unwrap();
    assert_eq!(decoded.canonical_bytes(), EMPTY_CANONICAL_WINDOW);
    assert_eq!(decoded.canonical_digest(), EMPTY_SHA256);

    let relationships = relate_window(&decoded).unwrap();
    assert_eq!(
        rangoon_domain::byte_digest(EMPTY_RELATIONSHIPS.as_bytes()),
        EMPTY_RELATIONSHIPS_SHA256
    );
    assert_eq!(
        relationships.canonical_bytes(),
        EMPTY_RELATIONSHIPS.as_bytes()
    );
    assert_eq!(relationships.canonical_digest(), EMPTY_RELATIONSHIPS_SHA256);
    let debug = format!("{relationships:?}");
    assert!(!debug.is_empty());
    assert!(!debug.contains("eventLinks"));
    assert!(!debug.contains("schemaVersion"));
    let document = serde_json::to_value(relationships.document()).unwrap();
    assert_eq!(document["inputCanonicalDigest"], EMPTY_SHA256);
    assert_eq!(document["eventLinks"], json!([]));
    assert_eq!(document["chains"], json!([]));
    assert_eq!(document["authority"], "none");
    assert_eq!(document["authenticity"], "unverified");
    assert_eq!(document["execution"], "unavailable");
}

#[test]
fn relationship_output_cap_fails_atomically_without_mutating_decoded_input() {
    let reference_rows = 200 * 36;
    let matching_event_ordinals = (200 * 4) + 200;
    assert!(reference_rows < 8_192);
    assert!(matching_event_ordinals < 8_192);
    let mut value = window();
    let mut repeated_request = request();
    repeated_request["resourceIds"] = json!(vec![oid(300); 32]);
    let mut tail = value["tail"].as_array().unwrap()[..4].to_vec();
    for index in 0..200_u64 {
        tail.push(event(
            index + 5,
            505 + index as u16,
            repeated_request.clone(),
            correlation(70, Value::Null),
        ));
    }
    value["tail"] = json!(tail);

    let decoded = decode_window(&serde_json::to_vec(&value).unwrap()).unwrap();
    let before = decoded.canonical_bytes().to_vec();
    let error = relate_window(&decoded).unwrap_err();
    assert_eq!(error.to_string(), "relationship_limit");
    assert!(!format!("{error:?}").contains(&oid(300)));
    assert_eq!(decoded.canonical_bytes(), before.as_slice());
}

#[test]
fn matching_event_amplification_hits_the_shared_limit_before_row_limit() {
    let reference_rows = 65 * 4;
    let actor_matching_event_ordinals = 65 * 128;
    assert!(reference_rows < 8_192);
    assert!(actor_matching_event_ordinals > 8_192);
    let mut value = window();
    let mut tail = value["tail"].as_array().unwrap()[..4].to_vec();
    for index in 0..127_u64 {
        tail.push(event(index + 5, 505 + index as u16, actor(), Value::Null));
    }
    for index in 0..65_u64 {
        tail.push(event(
            index + 132,
            632 + index as u16,
            request(),
            correlation(70, Value::Null),
        ));
    }
    value["tail"] = json!(tail);

    let decoded = decode_window(&serde_json::to_vec(&value).unwrap()).unwrap();
    let before = decoded.canonical_bytes().to_vec();
    let error = relate_window(&decoded).unwrap_err();
    assert_eq!(error.to_string(), "relationship_limit");
    assert!(!format!("{error:?}").contains(&oid(100)));
    assert_eq!(decoded.canonical_bytes(), before.as_slice());
}

#[test]
fn relationship_reference_row_cap_fails_atomically_without_mutating_decoded_input() {
    let reference_rows = 260 * 36;
    assert!(reference_rows > 8_192);
    let mut value = window();
    let mut repeated_request = request();
    repeated_request["resourceIds"] = json!(vec![oid(300); 32]);
    let mut tail = value["tail"].as_array().unwrap()[..4].to_vec();
    for index in 0..260_u64 {
        tail.push(event(
            index + 5,
            505 + index as u16,
            repeated_request.clone(),
            correlation(70, Value::Null),
        ));
    }
    value["tail"] = json!(tail);

    let decoded = decode_window(&serde_json::to_vec(&value).unwrap()).unwrap();
    let before = decoded.canonical_bytes().to_vec();
    let error = relate_window(&decoded).unwrap_err();
    assert_eq!(error.to_string(), "relationship_limit");
    assert!(!format!("{error:?}").contains(&oid(300)));
    assert_eq!(decoded.canonical_bytes(), before.as_slice());
}

#[test]
fn all_1024_unrelated_inventory_events_are_retained_without_topology_amplification() {
    let mut value = window();
    value["tail"] = json!(
        (0..1024_u64)
            .map(|index| event(index + 1, 500 + index as u16, resource(300), Value::Null))
            .collect::<Vec<_>>()
    );

    let document = document(&value);
    let links = document["eventLinks"].as_array().unwrap();
    assert_eq!(links.len(), 1024);
    assert_eq!(links[0]["inputOrdinal"], 0);
    assert_eq!(links[1023]["inputOrdinal"], 1023);
    assert!(
        links
            .iter()
            .all(|link| link["assessment"] == "not_assessed")
    );
    assert_eq!(document["chains"], json!([]));
}

#[test]
fn upstream_projection_cap_maps_to_the_fixed_atomic_relationship_error() {
    let mut value = window();
    value["tail"] = json!(
        (0..1024_u64)
            .map(|index| event(index + 1, 500 + index as u16, actor(), Value::Null))
            .collect::<Vec<_>>()
    );
    value["snapshot"]["topology"]["nodes"] = json!(
        (0..128_u16)
            .map(|index| json!({"id":oid(900 + index),"kind":"actor","recordId":oid(100)}))
            .collect::<Vec<_>>()
    );

    let decoded = decode_window(&serde_json::to_vec(&value).unwrap()).unwrap();
    let before = decoded.canonical_bytes().to_vec();
    let error = relate_window(&decoded).unwrap_err();
    assert_eq!(error.to_string(), "relationship_structural_limit");
    assert!(!format!("{error:?}").contains(&oid(100)));
    assert_eq!(decoded.canonical_bytes(), before.as_slice());
}
