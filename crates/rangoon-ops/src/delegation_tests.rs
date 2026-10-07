use super::*;
use serde_json::{Value, json};

fn oid(number: u16) -> String {
    format!("ops:{number:064x}")
}

fn digest(number: u16) -> String {
    format!("{number:064x}")
}

fn scope(system: u16) -> Value {
    json!({
        "systemId":oid(system),"environmentIds":[oid(4),oid(44)],
        "operationClasses":["deploy","read"],"resourceIds":[oid(300),oid(301)],
        "policyRevisionIds":[oid(104),oid(140)]
    })
}

fn actor(id: u16, system: u16, subject: u16, kind: &str) -> Value {
    json!({
        "kind":"actor","id":oid(id),"systemId":oid(system),"canonicalSubjectId":oid(subject),
        "actorKind":kind,"identitySourceRef":null,"identityEvidenceId":null,
        "reportedVerification":"reported_verified","observedMs":null,"freshness":"reported_current"
    })
}

fn delegation(id: u16, delegator: u16, delegate: u16, changed_by: u16) -> Value {
    json!({
        "kind":"delegation","id":oid(id),"delegatorId":oid(delegator),"delegateId":oid(delegate),
        "parentDelegationId":null,"scope":scope(3),"expiresMs":100,"state":"reported_active",
        "changedByActorId":oid(changed_by),"externalRef":null,"reportedVerification":"reported_verified",
        "observedMs":null,"freshness":"reported_current","evidenceIds":[]
    })
}

fn base_window() -> Value {
    json!({
        "schemaVersion":"rangoon.ops.window.v1",
        "snapshot":{"snapshotId":oid(1),"reducerVersion":"rangoon.ops.reducer.v1",
        "scope":{"workspaceId":oid(2),"systemIds":[oid(3),oid(33)],"environmentIds":[oid(4),oid(44)],"engineIds":[oid(5)]},
        "topology":{"revisionId":oid(8),"nodes":[],"edges":[],"policyRevisionIds":[oid(104),oid(140)],"coverageRevisionIds":[]},
        "prefixes":[],"asOfMs":0},"tail":[]
    })
}

fn correlation(record: &Value) -> Value {
    let mut value = record.get("context").cloned().unwrap_or_else(|| {
        json!({
            "workspaceId":oid(2),"systemId":oid(3),"environmentId":oid(4),"engineId":oid(5),
            "traceId":oid(6),"spanId":oid(7),"requestId":oid(108),"policyRevisionId":oid(104)
        })
    });
    value["evaluationId"] = if record["kind"] == "evaluation" {
        record["id"].clone()
    } else {
        Value::Null
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
        "observedMs":null,"receivedMs":null,"correlation":if uncorrelated { Value::Null } else { correlation(&record) },"record":record
    })
}

fn append(value: &mut Value, record: Value) {
    let events = value["tail"].as_array_mut().unwrap();
    let sequence = events.len() as u64 + 1;
    events.push(event(sequence, 500 + sequence as u16, record));
}

fn decode(value: &Value) -> DecodedWindow {
    decode_window(&serde_json::to_vec(value).unwrap()).unwrap()
}

fn document(value: &Value) -> Value {
    let decoded = decode(value);
    serde_json::to_value(inspect_delegations(&decoded).unwrap().document()).unwrap()
}

fn event_row(document: &Value, record_id: u16) -> &Value {
    document["eventAssessments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["recordId"] == oid(record_id))
        .unwrap()
}

fn subject_row(document: &Value, system: u16, subject: u16) -> &Value {
    document["subjects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["systemId"] == oid(system) && row["canonicalSubjectId"] == oid(subject))
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

fn assert_fixed_trust(document: &Value) {
    assert_eq!(document["authority"], "none");
    assert_eq!(document["authenticity"], "unverified");
    assert_eq!(document["execution"], "unavailable");
    assert_eq!(document["semanticAssessment"], "reported_delegation_links");
    assert_eq!(document["authoritativeEligibility"], "unknown");
    assert_eq!(document["authoritativeEffectiveEvaluationId"], Value::Null);
    assert_eq!(document["subdelegationAdmission"], "unavailable");
}

#[test]
fn empty_window_has_independent_literal_output_golden_and_fixed_trust() {
    const EMPTY_WINDOW: &[u8] = br#"{"schemaVersion":"rangoon.ops.window.v1","snapshot":{"snapshotId":"ops:1111111111111111111111111111111111111111111111111111111111111111","reducerVersion":"rangoon.ops.reducer.v1","scope":{"workspaceId":"ops:1111111111111111111111111111111111111111111111111111111111111111","systemIds":[],"environmentIds":[],"engineIds":[]},"topology":{"revisionId":"ops:1111111111111111111111111111111111111111111111111111111111111111","nodes":[],"edges":[],"policyRevisionIds":[],"coverageRevisionIds":[]},"prefixes":[],"asOfMs":0},"tail":[]}"#;
    const INPUT_SHA256: &str = "8ab43d0754e994dc00445661f74d1da62b6efec06f7c6276bee1b9e1fc25c0a7";
    const OUTPUT: &str = r#"{"schemaVersion":"rangoon.ops.delegations.v1","inputCanonicalDigest":"8ab43d0754e994dc00445661f74d1da62b6efec06f7c6276bee1b9e1fc25c0a7","structuralProjectionDigest":"5c8986e86e5cc7e23d6acfeed340d70cce710e8a7e71b117058c02296e928bba","authority":"none","authenticity":"unverified","execution":"unavailable","semanticAssessment":"reported_delegation_links","authoritativeEligibility":"unknown","authoritativeEffectiveEvaluationId":null,"subdelegationAdmission":"unavailable","diagnosticCounts":{"structural_incomplete":0,"reference_missing":0,"reference_ambiguous":0,"reference_kind_mismatch":0,"subject_kind_conflict":0,"system_mismatch":0,"self_delegation":0,"self_expansion":0,"parent_actor_mismatch":0,"scope_incomplete":0,"scope_expansion":0,"expiry_incomplete":0,"expiry_expansion":0,"attenuation_missing":0,"delegation_cycle":0,"subject_cycle":0},"subjects":[],"eventAssessments":[]}"#;
    const OUTPUT_SHA256: &str = "ff66c6814182f517231c0224cda18e79c3b1a708b782d094762d28e49771b4fd";

    let decoded = decode_window(EMPTY_WINDOW).unwrap();
    assert_eq!(decoded.canonical_bytes(), EMPTY_WINDOW);
    assert_eq!(decoded.canonical_digest(), INPUT_SHA256);
    let inspection = inspect_delegations(&decoded).unwrap();
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
fn nonempty_inspection_debug_reveals_only_wrapper_digest_and_byte_count() {
    let mut value = base_window();
    append(&mut value, actor(100, 3, 200, "agent"));
    append(&mut value, actor(101, 3, 201, "agent"));
    append(&mut value, delegation(120, 100, 101, 100));
    let decoded = decode(&value);
    let inspection = inspect_delegations(&decoded).unwrap();
    let debug = format!("{inspection:?}");
    assert!(debug.contains("DelegationInspection"));
    assert!(debug.contains(&inspection.canonical_digest()));
    assert!(debug.contains(&inspection.canonical_bytes().len().to_string()));
    for forbidden in [
        "ops:",
        "schemaVersion",
        "eventAssessments",
        "subjects",
        "reportedState",
    ] {
        assert!(
            !debug.contains(forbidden),
            "Debug leaked {forbidden}: {debug}"
        );
    }
}

fn unassessed_record(kind: &str, id: u16) -> Value {
    let flow = json!({"workspaceId":oid(2),"systemId":oid(3),"environmentId":oid(4),"engineId":oid(5),"requestId":oid(108),"traceId":oid(6),"spanId":oid(7),"policyRevisionId":oid(104)});
    let external = json!({"systemId":oid(3),"contractVersion":"v1","externalId":"external"});
    match kind {
        "resource" => {
            json!({"kind":kind,"id":oid(id),"systemId":oid(3),"resourceClass":"service","externalRef":null})
        }
        "tool" => {
            json!({"kind":kind,"id":oid(id),"systemId":oid(3),"operationClass":"deploy","externalRef":null})
        }
        "workflow" => {
            json!({"kind":kind,"id":oid(id),"systemId":oid(3),"externalRef":external,"capabilityRefs":[]})
        }
        "engine" => {
            json!({"kind":kind,"id":oid(id),"systemId":oid(3),"externalRef":external,"artifactDigest":digest(id)})
        }
        "policy_revision" => {
            json!({"kind":kind,"id":oid(id),"gatewayId":oid(105),"previousRevisionId":null,"artifactDigest":digest(id),"externalRef":null})
        }
        "gateway_revision" => {
            json!({"kind":kind,"id":oid(id),"gatewayId":oid(105),"previousRevisionId":null,"policyRevisionId":oid(104),"scope":scope(3),"humanRequired":true,"changedByActorId":oid(100),"state":"reported_active","evidenceIds":[]})
        }
        "assignment" => {
            json!({"kind":kind,"id":oid(id),"actorId":oid(100),"gatewayRevisionId":oid(105),"policyRevisionId":oid(104),"delegationId":null,"state":"reported_active","changedByActorId":oid(100),"evidenceIds":[]})
        }
        "request" => {
            json!({"kind":kind,"id":oid(id),"context":flow,"actorId":oid(100),"gatewayRevisionId":oid(105),"operationClass":"deploy","actionDigest":digest(8),"contextDigest":digest(9),"resourceIds":[],"workflowRef":null,"expiresMs":null})
        }
        "evaluation" => {
            json!({"kind":kind,"id":oid(id),"context":flow,"evaluationKind":"initial","gatewayRevisionId":oid(105),"evaluatorId":oid(100),"evaluatorArtifactDigest":digest(10),"inputSnapshotId":oid(1),"inputCanonicalization":"json-v1","inputDigest":digest(11),"contextSnapshotId":oid(1),"contextDigest":digest(9),"previousEvaluationId":null,"expectedEffectiveEvaluationId":null,"lineageFenceId":oid(21),"result":"deny","expiresMs":null,"reason":"none","externalRef":null,"evidenceIds":[]})
        }
        "approval_plan" => {
            json!({"kind":kind,"id":oid(id),"context":flow,"evaluationId":oid(109),"previousPlanId":null,"changedByActorId":oid(100),"expiresMs":null,"stages":[],"evidenceIds":[]})
        }
        "approval_decision" => {
            json!({"kind":kind,"id":oid(id),"context":flow,"evaluationId":oid(109),"planId":oid(110),"stageId":oid(210),"actorId":oid(100),"delegationId":null,"decision":"approve","supersedesDecisionId":null,"reason":"none","evidenceIds":[]})
        }
        "authorization" => {
            json!({"kind":kind,"id":oid(id),"context":flow,"evaluationId":oid(109),"planId":null,"decisionIds":[],"previousObservationId":null,"state":"reported_issued","scope":scope(3),"expiresMs":null,"reason":"none","externalRef":null,"evidenceIds":[]})
        }
        "attempt" => {
            json!({"kind":kind,"id":oid(id),"context":flow,"authorizationId":null,"executorId":oid(100),"previousObservationId":null,"attemptId":oid(116),"state":"reported_completed","reason":"none","evidenceIds":[]})
        }
        "suspension" => {
            json!({"kind":kind,"id":oid(id),"gatewayRevisionId":oid(105),"requestedByActorId":oid(100),"scope":scope(3),"state":"reported_acknowledged","previousObservationId":null,"reason":"none","evidenceIds":[]})
        }
        "metric" => {
            json!({"kind":kind,"id":oid(id),"context":flow,"attemptId":null,"resourceIds":[],"latencyMs":null,"cost":null,"reason":"none"})
        }
        "evidence" => {
            json!({"kind":kind,"id":oid(id),"producerId":oid(11),"externalRef":null,"artifactDigest":null,"state":"unverified","reason":"none"})
        }
        "coverage" => {
            json!({"kind":kind,"id":oid(id),"systemId":oid(3),"engineId":oid(5),"gatewayRevisionId":oid(105),"operationClass":"deploy","environmentId":oid(4),"targetOs":"linux","observedMs":null,"freshness":"unknown","targetResourceIds":[],"adapterArtifactDigest":digest(13),"engineArtifactDigest":digest(14),"reportedLevel":"unknown","checks":{"policyDecision":"unknown","approvalDuties":"unknown","agentDelegation":"unknown","expiryRevocation":"unknown","executionConsumption":"unknown","replayProtection":"unknown","outcomeEvidence":"unknown","reconciliation":"unknown"},"expiresMs":null,"evidenceIds":[]})
        }
        _ => unreachable!(),
    }
}

#[test]
fn retains_all_nineteen_kinds_and_only_copies_report_labels_for_assessed_rows() {
    let mut value = base_window();
    append(&mut value, actor(100, 3, 200, "agent"));
    append(&mut value, delegation(120, 100, 100, 100));
    let unassessed = [
        "resource",
        "tool",
        "workflow",
        "engine",
        "policy_revision",
        "gateway_revision",
        "assignment",
        "request",
        "evaluation",
        "approval_plan",
        "approval_decision",
        "authorization",
        "attempt",
        "suspension",
        "metric",
        "evidence",
        "coverage",
    ];
    for (offset, kind) in unassessed.iter().enumerate() {
        append(&mut value, unassessed_record(kind, 300 + offset as u16));
    }

    let inspected = document(&value);
    assert_fixed_trust(&inspected);
    assert_eq!(inspected["eventAssessments"].as_array().unwrap().len(), 19);
    for (ordinal, row) in inspected["eventAssessments"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        assert_eq!(row["inputOrdinal"], ordinal);
        if row["recordKind"] != "actor" && row["recordKind"] != "delegation" {
            assert_eq!(row["assessment"], "not_assessed");
            assert_eq!(row["reportedState"], Value::Null);
            assert_eq!(row["reportedVerification"], Value::Null);
            assert_eq!(row["reportedFreshness"], Value::Null);
            assert_eq!(row["subject"], Value::Null);
            assert_eq!(row["ancestorRecordIds"], json!([]));
            assert_eq!(row["references"], json!([]));
        }
    }
    let actor_row = event_row(&inspected, 100);
    assert_eq!(actor_row["reportedState"], Value::Null);
    assert_eq!(actor_row["reportedVerification"], "reported_verified");
    assert_eq!(actor_row["reportedFreshness"], "reported_current");
    let delegation_row = event_row(&inspected, 120);
    assert_eq!(delegation_row["reportedState"], "reported_active");
    assert_eq!(delegation_row["reportedVerification"], "reported_verified");
    assert_eq!(delegation_row["reportedFreshness"], "reported_current");
}

#[test]
fn aliases_cross_systems_role_conflicts_and_exact_duplicate_ordinals_remain_literal() {
    let mut value = base_window();
    append(&mut value, actor(100, 3, 200, "agent"));
    append(&mut value, actor(101, 3, 200, "human"));
    append(&mut value, actor(102, 33, 200, "agent"));
    append(&mut value, actor(103, 3, 201, "agent"));
    append(&mut value, delegation(120, 100, 103, 100));
    let duplicate = value["tail"][0].clone();
    value["tail"].as_array_mut().unwrap().push(duplicate);

    let inspected = document(&value);
    assert_eq!(inspected["subjects"].as_array().unwrap().len(), 3);
    let alias = subject_row(&inspected, 3, 200);
    assert_eq!(alias["actorRecordIds"], json!([oid(100), oid(101)]));
    assert_eq!(alias["reportedActorKinds"], json!(["agent", "human"]));
    assert_eq!(alias["matchingEventOrdinals"], json!([0, 1, 5]));
    assert_code(alias, "subject_kind_conflict");
    assert_eq!(
        subject_row(&inspected, 33, 200)["actorRecordIds"],
        json!([oid(102)])
    );
    assert_eq!(event_row(&inspected, 120)["assessment"], "conflict");
}

#[test]
fn missing_ambiguous_wrong_kind_evidence_actor_and_parent_references_stay_explicit() {
    let mut value = base_window();
    append(&mut value, actor(100, 3, 200, "agent"));
    append(&mut value, actor(101, 3, 201, "agent"));
    append(&mut value, unassessed_record("resource", 300));
    let mut missing = delegation(120, 999, 101, 100);
    missing["evidenceIds"] = json!([oid(998)]);
    missing["parentDelegationId"] = json!(oid(997));
    append(&mut value, missing);
    let mut wrong = delegation(121, 300, 101, 100);
    wrong["evidenceIds"] = json!([oid(300)]);
    wrong["parentDelegationId"] = json!(oid(300));
    append(&mut value, wrong);
    append(&mut value, actor(100, 3, 299, "agent"));
    append(&mut value, actor(102, 3, 202, "agent"));
    let ambiguous_parent = delegation(122, 100, 101, 100);
    append(&mut value, ambiguous_parent.clone());
    let mut incompatible_parent = ambiguous_parent;
    incompatible_parent["state"] = json!("reported_revoked");
    append(&mut value, incompatible_parent);
    let mut ambiguous_child = delegation(123, 101, 102, 101);
    ambiguous_child["parentDelegationId"] = json!(oid(122));
    ambiguous_child["scope"]["resourceIds"] = json!([oid(300)]);
    append(&mut value, ambiguous_child);

    let inspected = document(&value);
    let missing_row = event_row(&inspected, 120);
    assert_code(missing_row, "reference_missing");
    assert_eq!(missing_row["delegatorSubject"], Value::Null);
    assert_eq!(
        missing_row["ancestorRecordIds"],
        json!([oid(120), oid(997)])
    );
    assert!(
        missing_row["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["field"] == "delegatorId" && r["status"] == "missing")
    );
    assert!(
        missing_row["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["field"] == "parentDelegationId" && r["status"] == "missing")
    );
    let wrong_row = event_row(&inspected, 121);
    assert_code(wrong_row, "reference_kind_mismatch");
    assert!(
        wrong_row["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["field"] == "delegatorId" && r["status"] == "wrong_kind")
    );
    assert!(
        wrong_row["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["field"] == "evidenceIds" && r["status"] == "wrong_kind")
    );
    assert!(
        wrong_row["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["field"] == "parentDelegationId" && r["status"] == "wrong_kind")
    );
    assert!(
        event_row(&inspected, 123)["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["field"] == "parentDelegationId" && r["status"] == "ambiguous")
    );
    assert_code(event_row(&inspected, 120), "reference_ambiguous");
}

#[test]
fn actor_identity_evidence_missing_wrong_kind_and_ambiguous_remains_reference_data() {
    let mut value = base_window();
    let mut missing = actor(100, 3, 200, "agent");
    missing["identityEvidenceId"] = json!(oid(998));
    let mut wrong = actor(101, 3, 201, "agent");
    wrong["identityEvidenceId"] = json!(oid(300));
    let mut ambiguous_one = actor(102, 3, 202, "agent");
    ambiguous_one["identityEvidenceId"] = json!(oid(901));
    let ambiguous_two = unassessed_record("evidence", 901);
    let mut resource = unassessed_record("resource", 300);
    resource["id"] = json!(oid(300));
    append(&mut value, missing);
    append(&mut value, wrong);
    append(&mut value, ambiguous_one);
    append(&mut value, ambiguous_two);
    let mut incompatible = unassessed_record("evidence", 901);
    incompatible["reason"] = json!("conflict");
    append(&mut value, incompatible);
    append(&mut value, resource);
    let inspected = document(&value);
    assert_code(event_row(&inspected, 100), "reference_missing");
    assert_code(event_row(&inspected, 101), "reference_kind_mismatch");
    assert_code(event_row(&inspected, 102), "reference_ambiguous");
    assert!(
        event_row(&inspected, 100)["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["field"] == "identityEvidenceId" && row["status"] == "missing")
    );
    assert!(
        event_row(&inspected, 101)["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["field"] == "identityEvidenceId" && row["status"] == "wrong_kind")
    );
    assert!(
        event_row(&inspected, 102)["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["field"] == "identityEvidenceId" && row["status"] == "ambiguous")
    );
}

#[test]
fn stream_taint_propagates_to_subject_and_dependencies() {
    let mut value = base_window();
    append(&mut value, actor(100, 3, 200, "agent"));
    append(&mut value, actor(101, 3, 201, "agent"));
    append(&mut value, delegation(120, 100, 101, 100));
    value["tail"][1]["sequence"] = json!(9);
    let inspected = document(&value);
    assert_code(subject_row(&inspected, 3, 201), "structural_incomplete");
    assert_code(event_row(&inspected, 120), "structural_incomplete");
}

#[test]
fn graph_only_noise_does_not_taint_clean_actor_or_delegation_rows() {
    let mut value = base_window();
    append(&mut value, actor(100, 3, 200, "agent"));
    append(&mut value, actor(101, 3, 201, "agent"));
    append(&mut value, delegation(120, 100, 101, 100));
    value["snapshot"]["topology"]["nodes"] =
        json!([{"id":oid(900),"kind":"actor","recordId":oid(999)}]);
    let inspected = document(&value);
    assert!(!codes(subject_row(&inspected, 3, 200)).contains(&"structural_incomplete".to_owned()));
    assert!(!codes(event_row(&inspected, 120)).contains(&"structural_incomplete".to_owned()));
    assert!(!codes(event_row(&inspected, 120)).contains(&"reference_missing".to_owned()));
}

#[test]
fn alias_self_checks_system_checks_and_null_actor_comparisons_follow_exact_tuples() {
    let mut value = base_window();
    append(&mut value, actor(100, 3, 200, "agent"));
    append(&mut value, actor(101, 3, 200, "agent"));
    append(&mut value, actor(102, 33, 201, "agent"));
    let mut self_delegate = delegation(120, 100, 101, 100);
    self_delegate["changedByActorId"] = json!(oid(101));
    append(&mut value, self_delegate);
    append(&mut value, delegation(121, 100, 102, 100));
    let mut missing = delegation(122, 999, 101, 999);
    missing["scope"]["systemId"] = json!(oid(33));
    append(&mut value, missing);
    let inspected = document(&value);
    assert_code(event_row(&inspected, 120), "self_delegation");
    assert_code(event_row(&inspected, 120), "self_expansion");
    assert_code(event_row(&inspected, 121), "system_mismatch");
    assert_code(event_row(&inspected, 122), "reference_missing");
    assert!(!codes(event_row(&inspected, 122)).contains(&"self_delegation".to_owned()));
    assert!(!codes(event_row(&inspected, 122)).contains(&"self_expansion".to_owned()));
}

#[test]
fn unresolved_parent_actor_pairs_do_not_create_parent_actor_mismatch() {
    let mut both_null = base_window();
    append(&mut both_null, actor(100, 3, 200, "agent"));
    append(&mut both_null, actor(101, 3, 201, "agent"));
    let parent = delegation(120, 100, 999, 100);
    let mut child = delegation(121, 998, 101, 101);
    child["parentDelegationId"] = json!(oid(120));
    child["scope"]["resourceIds"] = json!([oid(300)]);
    append(&mut both_null, parent);
    append(&mut both_null, child);
    let inspected = document(&both_null);
    let row = event_row(&inspected, 121);
    assert_code(row, "reference_missing");
    assert!(!codes(row).contains(&"parent_actor_mismatch".to_owned()));

    let mut one_null = base_window();
    append(&mut one_null, actor(100, 3, 200, "agent"));
    append(&mut one_null, actor(101, 3, 201, "agent"));
    let parent = delegation(120, 100, 101, 100);
    let mut child = delegation(121, 999, 100, 100);
    child["parentDelegationId"] = json!(oid(120));
    child["scope"]["resourceIds"] = json!([oid(300)]);
    append(&mut one_null, parent);
    append(&mut one_null, child);
    let inspected = document(&one_null);
    let row = event_row(&inspected, 121);
    assert_code(row, "reference_missing");
    assert!(!codes(row).contains(&"parent_actor_mismatch".to_owned()));
}

#[test]
fn finite_scope_dimensions_empty_sets_and_expansion_are_checked_without_wildcards() {
    for dimension in [
        "environmentIds",
        "operationClasses",
        "resourceIds",
        "policyRevisionIds",
    ] {
        let mut value = base_window();
        append(&mut value, actor(100, 3, 200, "agent"));
        append(&mut value, actor(101, 3, 201, "agent"));
        append(&mut value, actor(102, 3, 202, "agent"));
        let parent = delegation(120, 100, 101, 100);
        let mut child = delegation(121, 101, 102, 101);
        child["parentDelegationId"] = json!(oid(120));
        child["scope"][dimension] = match dimension {
            "environmentIds" => json!([oid(4), oid(45)]),
            "operationClasses" => json!(["deploy", "other"]),
            "resourceIds" => json!([oid(300), oid(302)]),
            "policyRevisionIds" => json!([oid(104), oid(141)]),
            _ => unreachable!(),
        };
        append(&mut value, parent);
        append(&mut value, child);
        assert_code(event_row(&document(&value), 121), "scope_expansion");
    }
    for dimension in [
        "environmentIds",
        "operationClasses",
        "resourceIds",
        "policyRevisionIds",
    ] {
        let mut value = base_window();
        append(&mut value, actor(100, 3, 200, "agent"));
        append(&mut value, actor(101, 3, 201, "agent"));
        append(&mut value, actor(102, 3, 202, "agent"));
        let parent = delegation(120, 100, 101, 100);
        let mut child = delegation(121, 101, 102, 101);
        child["parentDelegationId"] = json!(oid(120));
        child["scope"][dimension] = match dimension {
            "environmentIds" => json!([oid(4)]),
            "operationClasses" => json!(["deploy"]),
            "resourceIds" => json!([oid(300)]),
            "policyRevisionIds" => json!([oid(104)]),
            _ => unreachable!(),
        };
        append(&mut value, parent);
        append(&mut value, child);
        let inspected = document(&value);
        let row = event_row(&inspected, 121);
        assert_eq!(row["assessment"], "linked_unverified", "{dimension}");
        assert!(!codes(row).contains(&"attenuation_missing".to_owned()));
    }
    let mut value = base_window();
    append(&mut value, actor(100, 3, 200, "agent"));
    append(&mut value, actor(101, 3, 201, "agent"));
    let mut empty = delegation(120, 100, 101, 100);
    empty["scope"]["environmentIds"] = json!([]);
    append(&mut value, empty);
    assert_code(event_row(&document(&value), 120), "scope_incomplete");
}

#[test]
fn scope_subsets_expiry_and_parent_subject_bindings_have_exact_assessments() {
    let mut value = base_window();
    append(&mut value, actor(100, 3, 200, "agent"));
    append(&mut value, actor(101, 3, 201, "agent"));
    append(&mut value, actor(102, 3, 202, "agent"));
    append(&mut value, actor(103, 3, 299, "agent"));
    let parent = delegation(120, 100, 101, 100);
    let mut subset = delegation(121, 101, 102, 101);
    subset["parentDelegationId"] = json!(oid(120));
    subset["scope"]["environmentIds"] = json!([oid(4)]);
    let mut short_expiry = delegation(126, 101, 102, 101);
    short_expiry["parentDelegationId"] = json!(oid(120));
    short_expiry["expiresMs"] = json!(99);
    let mut equal = delegation(122, 101, 102, 101);
    equal["parentDelegationId"] = json!(oid(120));
    let mut later = delegation(123, 101, 102, 101);
    later["parentDelegationId"] = json!(oid(120));
    later["expiresMs"] = json!(101);
    let mut nullable = delegation(124, 101, 102, 101);
    nullable["parentDelegationId"] = json!(oid(120));
    nullable["expiresMs"] = Value::Null;
    let mut mismatch = delegation(125, 103, 102, 103);
    mismatch["parentDelegationId"] = json!(oid(120));
    let mut null_parent = delegation(127, 100, 101, 100);
    null_parent["expiresMs"] = Value::Null;
    let mut finite_child = delegation(128, 101, 102, 101);
    finite_child["parentDelegationId"] = json!(oid(127));
    finite_child["scope"]["resourceIds"] = json!([oid(300)]);
    append(&mut value, parent);
    append(&mut value, subset);
    append(&mut value, short_expiry);
    append(&mut value, equal);
    append(&mut value, later);
    append(&mut value, nullable);
    append(&mut value, mismatch);
    append(&mut value, null_parent);
    append(&mut value, finite_child);
    let inspected = document(&value);
    assert_eq!(
        event_row(&inspected, 121)["assessment"],
        "linked_unverified"
    );
    assert_eq!(
        event_row(&inspected, 126)["assessment"],
        "linked_unverified"
    );
    assert_code(event_row(&inspected, 122), "attenuation_missing");
    assert_code(event_row(&inspected, 123), "expiry_expansion");
    assert_code(event_row(&inspected, 124), "expiry_incomplete");
    assert_code(event_row(&inspected, 125), "parent_actor_mismatch");
    assert_code(event_row(&inspected, 128), "expiry_incomplete");
}

#[test]
fn direct_transitive_id_and_subject_cycles_and_ancestor_diagnostics_are_visible() {
    let mut value = base_window();
    append(&mut value, actor(100, 3, 200, "agent"));
    append(&mut value, actor(101, 3, 201, "agent"));
    append(&mut value, actor(102, 3, 202, "agent"));
    let mut first = delegation(120, 100, 101, 100);
    first["parentDelegationId"] = json!(oid(122));
    let mut second = delegation(121, 101, 102, 101);
    second["parentDelegationId"] = json!(oid(120));
    let mut third = delegation(122, 102, 100, 102);
    third["parentDelegationId"] = json!(oid(121));
    append(&mut value, first);
    append(&mut value, second);
    append(&mut value, third);
    let inspected = document(&value);
    let row = event_row(&inspected, 120);
    assert_eq!(
        row["ancestorRecordIds"],
        json!([oid(120), oid(122), oid(121), oid(120)])
    );
    assert_code(row, "delegation_cycle");
    assert_code(row, "subject_cycle");
    assert_code(event_row(&inspected, 121), "delegation_cycle");
}

#[test]
fn direct_self_parent_closes_once_and_flags_id_cycle() {
    let mut value = base_window();
    append(&mut value, actor(100, 3, 200, "agent"));
    append(&mut value, actor(101, 3, 201, "agent"));
    let mut self_parent = delegation(120, 100, 101, 100);
    self_parent["parentDelegationId"] = json!(oid(120));
    append(&mut value, self_parent);
    let inspected = document(&value);
    let row = event_row(&inspected, 120);
    assert_eq!(row["ancestorRecordIds"], json!([oid(120), oid(120)]));
    assert_code(row, "delegation_cycle");
}

#[test]
fn acyclic_parent_ids_can_still_have_subject_cycle() {
    let mut value = base_window();
    append(&mut value, actor(100, 3, 200, "agent"));
    append(&mut value, actor(101, 3, 201, "agent"));
    let parent = delegation(120, 100, 101, 100);
    let mut child = delegation(121, 101, 100, 101);
    child["parentDelegationId"] = json!(oid(120));
    child["scope"]["resourceIds"] = json!([oid(300)]);
    append(&mut value, parent);
    append(&mut value, child);
    let inspected = document(&value);
    let row = event_row(&inspected, 121);
    assert_eq!(row["ancestorRecordIds"], json!([oid(121), oid(120)]));
    assert_code(row, "subject_cycle");
    assert!(!codes(row).contains(&"delegation_cycle".to_owned()));
}

#[test]
fn aggregate_taint_from_repeated_parent_observation_reaches_leaf() {
    let mut value = base_window();
    append(&mut value, actor(100, 3, 200, "agent"));
    append(&mut value, actor(101, 3, 201, "agent"));
    append(&mut value, actor(102, 3, 202, "agent"));
    append(&mut value, actor(103, 3, 203, "agent"));
    let parent = delegation(120, 100, 101, 100);
    let mut child = delegation(121, 101, 102, 101);
    child["parentDelegationId"] = json!(oid(120));
    child["scope"]["resourceIds"] = json!([oid(300)]);
    let mut grandchild = delegation(122, 102, 103, 102);
    grandchild["parentDelegationId"] = json!(oid(121));
    grandchild["scope"]["environmentIds"] = json!([oid(4)]);
    append(&mut value, parent.clone());
    append(&mut value, child);
    append(&mut value, grandchild);
    let mut tainted_repeat = event(2, 800, parent);
    tainted_repeat["producerId"] = json!(oid(12));
    tainted_repeat["fenceId"] = json!(oid(22));
    value["tail"].as_array_mut().unwrap().push(tainted_repeat);
    let inspected = document(&value);
    let child_row = event_row(&inspected, 121);
    let parent_reference = child_row["references"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["field"] == "parentDelegationId")
        .unwrap();
    assert_eq!(parent_reference["status"], "unique_unverified");
    assert_eq!(parent_reference["matchingEventOrdinals"], json!([4, 7]));
    assert_code(child_row, "structural_incomplete");
    assert_code(event_row(&inspected, 122), "structural_incomplete");
}

fn expect_limit(value: &Value, expected: &str) {
    let decoded = decode(value);
    assert!(project_window(&decoded).is_ok());
    let before = decoded.canonical_bytes().to_vec();
    let error = inspect_delegations(&decoded).unwrap_err();
    assert_eq!(error.to_string(), expected);
    assert!(!format!("{error:?}").contains("ops:"));
    assert_eq!(decoded.canonical_bytes(), before.as_slice());
}

#[test]
fn reference_row_limit_is_atomic_after_valid_structural_projection() {
    let mut rows = base_window();
    append(&mut rows, actor(100, 3, 200, "agent"));
    append(&mut rows, actor(101, 3, 201, "agent"));
    for index in 0..235_u16 {
        let mut record = delegation(1200 + index, 100, 101, 100);
        record["evidenceIds"] = json!(vec![oid(900); 32]);
        append(&mut rows, record);
    }
    expect_limit(&rows, "delegation_limit");
}

#[test]
fn matching_ordinal_limit_is_atomic_after_valid_structural_projection() {
    let mut matches = base_window();
    for _ in 0..128 {
        append(&mut matches, actor(100, 3, 200, "agent"));
    }
    append(&mut matches, actor(101, 3, 201, "agent"));
    for index in 0..65_u16 {
        append(&mut matches, delegation(1300 + index, 100, 101, 100));
    }
    expect_limit(&matches, "delegation_limit");
}

#[test]
fn ancestry_path_limit_is_atomic_after_valid_structural_projection() {
    let mut paths = base_window();
    append(&mut paths, actor(100, 3, 200, "agent"));
    append(&mut paths, actor(101, 3, 201, "agent"));
    for index in 0..100_u16 {
        let mut record = delegation(1400 + index, 100, 101, 100);
        if index > 0 {
            record["parentDelegationId"] = json!(oid(1399 + index));
        }
        append(&mut paths, record);
    }
    let leaf = paths["tail"].as_array().unwrap().last().unwrap().clone();
    for _ in 0..100 {
        append(&mut paths, leaf["record"].clone());
    }
    expect_limit(&paths, "delegation_limit");
}

#[test]
fn wire_output_limit_is_atomic_after_valid_structural_projection() {
    let mut wire = base_window();
    for index in 0..1024_u16 {
        append(&mut wire, actor(2000 + index, 3, 4000 + index, "agent"));
    }
    expect_limit(&wire, "delegation_limit");
}

#[test]
fn upstream_projection_limit_maps_to_fixed_atomic_error() {
    let mut upstream = base_window();
    for index in 0..1024_u16 {
        append(&mut upstream, actor(100, 3, 200, "agent"));
        upstream["tail"].as_array_mut().unwrap().last_mut().unwrap()["eventId"] =
            json!(oid(3000 + index));
    }
    upstream["snapshot"]["topology"]["nodes"] = json!(
        (0..128_u16)
            .map(|index| json!({"id":oid(4000 + index),"kind":"actor","recordId":oid(100)}))
            .collect::<Vec<_>>()
    );
    let decoded = decode(&upstream);
    assert_eq!(
        project_window(&decoded).unwrap_err().to_string(),
        "projection_limit"
    );
    let before = decoded.canonical_bytes().to_vec();
    let error = inspect_delegations(&decoded).unwrap_err();
    assert_eq!(error.to_string(), "delegation_structural_limit");
    assert!(!format!("{error:?}").contains("ops:"));
    assert_eq!(decoded.canonical_bytes(), before.as_slice());
}
