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
        "workspaceId": oid(2), "systemId": oid(3), "environmentId": oid(4),
        "engineId": oid(5), "requestId": oid(108), "traceId": oid(6),
        "spanId": oid(span), "policyRevisionId": oid(104)
    })
}

fn scope() -> Value {
    json!({
        "systemId": oid(3), "environmentIds": [oid(4)], "operationClasses": ["deploy"],
        "resourceIds": [oid(101)], "policyRevisionIds": [oid(104)]
    })
}

fn correlation(record: &Value) -> Value {
    let mut value = record.get("context").cloned().unwrap_or_else(|| {
        json!({
            "workspaceId": oid(2), "systemId": oid(3), "environmentId": oid(4),
            "engineId": oid(5), "requestId": oid(108), "traceId": oid(6),
            "spanId": oid(7), "policyRevisionId": oid(104)
        })
    });
    value["evaluationId"] = match record["kind"].as_str().unwrap() {
        "evaluation" => record["id"].clone(),
        "approval_plan" | "approval_decision" | "authorization" => record["evaluationId"].clone(),
        _ => Value::Null,
    };
    value["attemptId"] = if matches!(record["kind"].as_str().unwrap(), "attempt" | "metric") {
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
        "eventId": oid(event_id), "producerId": oid(11), "fenceId": oid(21),
        "sequence": sequence,
        "previousEventId": if sequence == 1 { Value::Null } else { json!(oid(event_id - 1)) },
        "observedMs": null, "receivedMs": null,
        "correlation": if uncorrelated { Value::Null } else { correlation(&record) },
        "record": record
    })
}

fn append(window: &mut Value, record: Value) {
    let tail = window["tail"].as_array_mut().unwrap();
    let sequence = tail.len() as u64 + 1;
    tail.push(event(sequence, 500 + sequence as u16, record));
}

fn base_window() -> Value {
    json!({
        "schemaVersion": "rangoon.ops.window.v1",
        "snapshot": {
            "snapshotId": oid(1), "reducerVersion": "rangoon.ops.reducer.v1",
            "scope": {"workspaceId": oid(2), "systemIds": [oid(3)],
                "environmentIds": [oid(4)], "engineIds": [oid(5)]},
            "topology": {"revisionId": oid(8), "nodes": [], "edges": [],
                "policyRevisionIds": [oid(104)], "coverageRevisionIds": [oid(118)]},
            "prefixes": [], "asOfMs": 50
        },
        "tail": []
    })
}

fn actor(id: u16) -> Value {
    json!({
        "kind": "actor", "id": oid(id), "systemId": oid(3),
        "canonicalSubjectId": oid(id + 100), "actorKind": "human",
        "identitySourceRef": null, "identityEvidenceId": null,
        "reportedVerification": "reported_verified", "observedMs": null,
        "freshness": "reported_current"
    })
}

fn stage(id: u16) -> Value {
    json!({
        "id": oid(id), "dependsOnStageIds": [], "approverIds": [oid(100)],
        "humanRequired": true, "mode": "all", "quorum": null, "expiresMs": 100,
        "escalationActorIds": []
    })
}

fn authorization(id: u16, state: &str, prior: Value) -> Value {
    json!({
        "kind": "authorization", "id": oid(id), "context": context(id),
        "evaluationId": oid(109), "planId": null, "decisionIds": [oid(111)],
        "previousObservationId": prior, "state": state, "scope": scope(),
        "expiresMs": 100, "reason": "none", "externalRef": null, "evidenceIds": []
    })
}

fn attempt(id: u16, attempt_id: u16, state: &str, prior: Value, evidence_ids: Value) -> Value {
    json!({
        "kind": "attempt", "id": oid(id), "context": context(id),
        "authorizationId": oid(112), "executorId": oid(100),
        "previousObservationId": prior, "attemptId": oid(attempt_id), "state": state,
        "reason": "unknown_outcome", "evidenceIds": evidence_ids
    })
}

fn metric(id: u16, attempt_id: u16) -> Value {
    json!({
        "kind": "metric", "id": oid(id), "context": context(id),
        "attemptId": oid(attempt_id), "resourceIds": [], "latencyMs": null,
        "cost": null, "reason": "unknown_outcome"
    })
}

fn suspension(id: u16, state: &str, prior: Value) -> Value {
    json!({
        "kind": "suspension", "id": oid(id), "gatewayRevisionId": oid(105),
        "requestedByActorId": oid(100), "scope": scope(), "state": state,
        "previousObservationId": prior, "reason": "conflict", "evidenceIds": []
    })
}

fn coverage(id: u16, freshness: &str, expires_ms: Value, evidence_ids: Value) -> Value {
    json!({
        "kind": "coverage", "id": oid(id), "systemId": oid(3), "engineId": oid(5),
        "gatewayRevisionId": oid(105), "operationClass": "deploy", "environmentId": oid(4),
        "targetOs": "linux", "observedMs": 10, "freshness": freshness,
        "targetResourceIds": [oid(101)], "adapterArtifactDigest": digest(13),
        "engineArtifactDigest": digest(14), "reportedLevel": "fully_mediated",
        "checks": {"policyDecision": "yes", "approvalDuties": "yes",
            "agentDelegation": "yes", "expiryRevocation": "yes",
            "executionConsumption": "yes", "replayProtection": "yes",
            "outcomeEvidence": "yes", "reconciliation": "yes"},
        "expiresMs": expires_ms, "evidenceIds": evidence_ids
    })
}

fn full_window() -> Value {
    let mut window = base_window();
    let records = vec![
        actor(100),
        json!({"kind":"resource","id":oid(101),"systemId":oid(3),"resourceClass":"repository","externalRef":null}),
        json!({"kind":"tool","id":oid(102),"systemId":oid(3),"operationClass":"deploy","externalRef":null}),
        json!({"kind":"workflow","id":oid(103),"systemId":oid(3),"externalRef":{"systemId":oid(3),"contractVersion":"v1","externalId":"workflow"},"capabilityRefs":[]}),
        json!({"kind":"engine","id":oid(5),"systemId":oid(3),"externalRef":{"systemId":oid(3),"contractVersion":"v1","externalId":"engine"},"artifactDigest":digest(5)}),
        json!({"kind":"policy_revision","id":oid(104),"gatewayId":oid(105),"previousRevisionId":null,"artifactDigest":digest(6),"externalRef":null}),
        json!({"kind":"gateway_revision","id":oid(105),"gatewayId":oid(105),"previousRevisionId":null,"policyRevisionId":oid(104),"scope":scope(),"humanRequired":true,"changedByActorId":oid(100),"state":"reported_active","evidenceIds":[]}),
        json!({"kind":"assignment","id":oid(106),"actorId":oid(100),"gatewayRevisionId":oid(105),"policyRevisionId":oid(104),"delegationId":null,"state":"reported_active","changedByActorId":oid(100),"evidenceIds":[]}),
        json!({"kind":"delegation","id":oid(107),"delegatorId":oid(100),"delegateId":oid(100),"parentDelegationId":null,"scope":scope(),"expiresMs":100,"state":"reported_active","changedByActorId":oid(100),"externalRef":null,"reportedVerification":"reported_verified","observedMs":null,"freshness":"reported_current","evidenceIds":[]}),
        json!({"kind":"request","id":oid(108),"context":context(70),"actorId":oid(100),"gatewayRevisionId":oid(105),"operationClass":"deploy","actionDigest":digest(8),"contextDigest":digest(9),"resourceIds":[oid(101)],"workflowRef":null,"expiresMs":null}),
        json!({"kind":"evaluation","id":oid(109),"context":context(71),"evaluationKind":"initial","gatewayRevisionId":oid(105),"evaluatorId":oid(100),"evaluatorArtifactDigest":digest(10),"inputSnapshotId":oid(1),"inputCanonicalization":"json-v1","inputDigest":digest(11),"contextSnapshotId":oid(1),"contextDigest":digest(9),"previousEvaluationId":null,"expectedEffectiveEvaluationId":null,"lineageFenceId":oid(21),"result":"unknown","expiresMs":null,"reason":"conflict","externalRef":null,"evidenceIds":[]}),
        json!({"kind":"approval_plan","id":oid(110),"context":context(73),"evaluationId":oid(109),"previousPlanId":null,"changedByActorId":oid(100),"expiresMs":100,"stages":[stage(210)],"evidenceIds":[]}),
        json!({"kind":"approval_decision","id":oid(111),"context":context(74),"evaluationId":oid(109),"planId":oid(110),"stageId":oid(210),"actorId":oid(100),"delegationId":null,"decision":"approve","supersedesDecisionId":null,"reason":"none","evidenceIds":[]}),
        authorization(112, "reported_issued", Value::Null),
        attempt(113, 900, "outcome_unknown", Value::Null, json!([])),
        suspension(114, "requested", Value::Null),
        json!({"kind":"metric","id":oid(115),"context":context(77),"attemptId":oid(900),"resourceIds":[oid(101)],"latencyMs":null,"cost":null,"reason":"unknown_outcome"}),
        json!({"kind":"evidence","id":oid(117),"producerId":oid(11),"externalRef":null,"artifactDigest":null,"state":"unverified","reason":"unknown_outcome"}),
        coverage(118, "reported_current", json!(100), json!([oid(117)])),
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
    serde_json::to_value(inspect_lifecycle(&decoded).unwrap().document()).unwrap()
}

fn event_row(document: &Value, record_id: u16) -> &Value {
    document["eventAssessments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["recordId"] == oid(record_id))
        .unwrap()
}

fn code(row: &Value, expected: &str) {
    assert!(
        row["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == expected),
        "{row:?} missing {expected}"
    );
}

fn diagnostic_count(document: &Value, expected: &str) -> u64 {
    document["diagnosticCounts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["code"] == expected)
        .unwrap()["count"]
        .as_u64()
        .unwrap()
}

fn fixed_trust(document: &Value) {
    assert_eq!(document["authority"], "none");
    assert_eq!(document["authenticity"], "unverified");
    assert_eq!(document["execution"], "unavailable");
    assert_eq!(document["authoritativeEligibility"], "unknown");
    assert_eq!(document["authoritativeOutcome"], "unknown");
    assert_eq!(document["qualifiedCoverage"], "unknown");
}

#[test]
fn empty_window_has_literal_lifecycle_shape_fixed_trust_and_private_debug() {
    const EMPTY: &[u8] = br#"{"schemaVersion":"rangoon.ops.window.v1","snapshot":{"snapshotId":"ops:1111111111111111111111111111111111111111111111111111111111111111","reducerVersion":"rangoon.ops.reducer.v1","scope":{"workspaceId":"ops:1111111111111111111111111111111111111111111111111111111111111111","systemIds":[],"environmentIds":[],"engineIds":[]},"topology":{"revisionId":"ops:1111111111111111111111111111111111111111111111111111111111111111","nodes":[],"edges":[],"policyRevisionIds":[],"coverageRevisionIds":[]},"prefixes":[],"asOfMs":0},"tail":[]}"#;
    let decoded = decode_window(EMPTY).unwrap();
    assert_eq!(decoded.canonical_bytes(), EMPTY);
    let inspection = inspect_lifecycle(&decoded).unwrap();
    let output = serde_json::to_value(inspection.document()).unwrap();
    let canonical = std::str::from_utf8(inspection.canonical_bytes()).unwrap();
    let mut previous = 0;
    for field in [
        "schemaVersion",
        "inputCanonicalDigest",
        "structuralProjectionDigest",
        "relationshipDigest",
        "delegationDigest",
        "approvalDigest",
        "authority",
        "authenticity",
        "execution",
        "semanticAssessment",
        "authoritativeEligibility",
        "authoritativeOutcome",
        "qualifiedCoverage",
        "diagnosticCounts",
        "eventAssessments",
        "reportedChains",
        "coverageAssessments",
    ] {
        let position = canonical.find(&format!("\"{field}\"")).unwrap();
        assert!(position >= previous, "{field} serialized out of order");
        previous = position;
    }
    assert_eq!(
        inspection.canonical_digest(),
        rangoon_domain::byte_digest(inspection.canonical_bytes())
    );
    assert_eq!(output["schemaVersion"], "rangoon.ops.lifecycle.v1");
    assert_eq!(output["semanticAssessment"], "linked_unverified");
    assert_eq!(output["eventAssessments"], json!([]));
    assert_eq!(output["reportedChains"], json!([]));
    assert_eq!(output["coverageAssessments"], json!([]));
    fixed_trust(&output);
    let counts = output["diagnosticCounts"].as_array().unwrap();
    assert_eq!(counts.len(), 22);
    assert!(counts.iter().all(|row| row["count"] == 0));
    let debug = format!("{inspection:?}");
    assert!(debug.contains("LifecycleInspection"));
    for private in [
        "schemaVersion",
        "eventAssessments",
        "reportedChains",
        "ops:",
    ] {
        assert!(!debug.contains(private), "Debug leaked {private}: {debug}");
    }
}

#[test]
fn all_nineteen_kinds_retain_input_order_and_only_lifecycle_kinds_are_assessed() {
    let value = full_window();
    let decoded = decode(&value);
    let input = decoded.canonical_bytes().to_vec();
    let output = serde_json::to_value(inspect_lifecycle(&decoded).unwrap().document()).unwrap();
    let rows = output["eventAssessments"].as_array().unwrap();
    assert_eq!(rows.len(), 19);
    for (ordinal, row) in rows.iter().enumerate() {
        assert_eq!(row["inputOrdinal"], ordinal);
    }
    for row in &rows[..13] {
        assert_eq!(row["assessment"], "not_assessed");
        assert_eq!(row["reportedState"], Value::Null);
        assert_eq!(row["reportedAttemptId"], Value::Null);
        assert_eq!(row["diagnostics"], json!([]));
        assert_eq!(row["references"], json!([]));
    }
    assert_eq!(rows[13]["reportedState"], "reported_issued");
    assert_eq!(rows[14]["reportedAttemptId"], oid(900));
    assert_eq!(rows[16]["reportedState"], Value::Null);
    assert_eq!(rows[17]["reportedState"], "unverified");
    assert_eq!(rows[18]["recordKind"], "coverage");
    assert_eq!(decoded.canonical_bytes(), input.as_slice());
    fixed_trust(&output);
}

#[test]
fn one_bad_metric_correlation_taints_only_its_event_in_a_shared_stream() {
    let mut value = full_window();
    value["tail"][16]["correlation"]["attemptId"] = Value::Null;
    let output = document(&value);

    let metric = event_row(&output, 115);
    assert_eq!(metric["assessment"], "incomplete");
    code(metric, "structural_incomplete");
    assert_eq!(diagnostic_count(&output, "structural_incomplete"), 1);

    for record_id in [112, 114, 117, 118] {
        let row = event_row(&output, record_id);
        assert_eq!(row["assessment"], "linked_unverified", "{row:?}");
        assert!(
            !row["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|diagnostic| diagnostic == "structural_incomplete"),
            "unrelated record {record_id} was tainted: {row:?}"
        );
    }
}

#[test]
fn coherent_reported_chains_keep_root_to_tip_rows_without_granting_outcome() {
    let mut value = full_window();
    value["tail"][14]["record"]["state"] = json!("reported_accepted");
    append(
        &mut value,
        authorization(119, "reported_revoked", json!(oid(112))),
    );
    append(
        &mut value,
        attempt(120, 900, "reported_active", json!(oid(113)), json!([])),
    );
    append(
        &mut value,
        attempt(121, 900, "reported_completed", json!(oid(120)), json!([])),
    );
    append(
        &mut value,
        suspension(122, "reported_partial", json!(oid(114))),
    );
    append(
        &mut value,
        suspension(123, "reported_resumed", json!(oid(122))),
    );
    let output = document(&value);
    let chains = output["reportedChains"].as_array().unwrap();
    assert_eq!(chains.len(), 3);
    assert_eq!(chains[0]["kind"], "authorization");
    assert_eq!(chains[1]["kind"], "attempt");
    assert_eq!(chains[2]["kind"], "suspension");
    assert_eq!(
        chains[0]["reportedChainStatus"], "single_chain_unverified",
        "{chains:?}"
    );
    assert_eq!(
        chains[1]["reportedChainStatus"], "single_chain_unverified",
        "{chains:?}"
    );
    assert_eq!(
        chains[2]["reportedChainStatus"], "single_chain_unverified",
        "{chains:?}"
    );
    assert_eq!(chains[1]["groupId"], oid(900));
    assert_eq!(
        chains[1]["recordIds"],
        json!([oid(113), oid(120), oid(121)])
    );
    assert_eq!(chains[1]["reportedTipRecordId"], oid(121));
    assert_eq!(chains[1]["reportedTipState"], "reported_completed");
    fixed_trust(&output);
}

#[test]
fn metric_attempt_selector_uses_attempt_id_not_attempt_record_id_and_keeps_nullable_values() {
    let output = document(&full_window());
    let metric = event_row(&output, 115);
    assert_eq!(metric["reportedAttemptId"], oid(900));
    let reference = metric["references"].as_array().unwrap().first().unwrap();
    assert_eq!(reference["field"], "attemptId");
    assert_eq!(reference["recordId"], oid(900));
    assert_eq!(reference["expectedKind"], "attempt");
    assert_eq!(reference["status"], "unique_unverified");
    assert_eq!(reference["matchingEventOrdinals"], json!([14]));
}

#[test]
fn coherent_multi_observation_attempt_has_one_metric_target_with_all_ordinals() {
    let mut value = full_window();
    value["tail"][14]["record"]["state"] = json!("reported_accepted");
    append(
        &mut value,
        attempt(120, 900, "reported_active", json!(oid(113)), json!([])),
    );
    append(
        &mut value,
        attempt(121, 900, "reported_completed", json!(oid(120)), json!([])),
    );
    let output = document(&value);
    let metric = event_row(&output, 115);
    let reference = metric["references"].as_array().unwrap().first().unwrap();
    assert_eq!(reference["recordId"], oid(900));
    assert_eq!(reference["expectedKind"], "attempt");
    assert_eq!(reference["status"], "unique_unverified");
    assert_eq!(reference["matchingEventOrdinals"], json!([14, 19, 20]));
}

#[test]
fn metric_selector_refuses_an_attempt_group_with_unresolved_evidence() {
    let mut value = full_window();
    value["tail"][14]["record"]["state"] = json!("reported_accepted");
    value["tail"][14]["record"]["evidenceIds"] = json!([oid(777)]);
    let output = document(&value);
    let attempt = event_row(&output, 113);
    code(attempt, "reference_missing");
    let metric = event_row(&output, 115);
    assert_eq!(metric["references"][0]["status"], "ambiguous");
    code(metric, "reference_ambiguous");
}

#[test]
fn repeated_id_uses_earliest_stream_sequence_not_first_input_ordinal() {
    let mut value = full_window();
    value["tail"][14]["record"]["state"] = json!("reported_accepted");
    append(
        &mut value,
        attempt(120, 900, "reported_active", json!(oid(113)), json!([])),
    );
    append(
        &mut value,
        attempt(113, 900, "reported_accepted", Value::Null, json!([])),
    );
    value["tail"].as_array_mut().unwrap().swap(14, 19);

    let output = document(&value);
    let chain = output["reportedChains"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["kind"] == "attempt")
        .unwrap();
    assert_eq!(chain["matchingEventOrdinals"], json!([14, 19, 20]));
    assert_eq!(chain["recordIds"], json!([oid(113), oid(120)]));
    assert_eq!(chain["reportedChainStatus"], "single_chain_unverified");
    assert_eq!(chain["reportedTipRecordId"], oid(120));
}

#[test]
fn terminal_attempt_and_non_requested_suspension_roots_are_incomplete() {
    let mut terminal_attempt = full_window();
    terminal_attempt["tail"][14]["record"]["state"] = json!("reported_completed");
    let terminal_output = document(&terminal_attempt);
    assert_eq!(event_row(&terminal_output, 113)["assessment"], "incomplete");

    let mut partial_suspension = full_window();
    partial_suspension["tail"][15]["record"]["state"] = json!("reported_partial");
    let suspension_output = document(&partial_suspension);
    assert_eq!(
        event_row(&suspension_output, 114)["assessment"],
        "incomplete"
    );
}

#[test]
fn unresolved_wrong_kind_and_ambiguous_references_never_choose_a_first_match() {
    let mut missing = full_window();
    missing["tail"][14]["record"]["authorizationId"] = json!(oid(777));
    code(event_row(&document(&missing), 113), "reference_missing");

    let mut wrong_kind = full_window();
    wrong_kind["tail"][14]["record"]["authorizationId"] = json!(oid(101));
    code(
        event_row(&document(&wrong_kind), 113),
        "reference_kind_mismatch",
    );

    let mut ambiguous = full_window();
    append(
        &mut ambiguous,
        attempt(130, 900, "reported_accepted", Value::Null, json!([])),
    );
    let output = document(&ambiguous);
    let metric = event_row(&output, 115);
    let reference = metric["references"].as_array().unwrap().first().unwrap();
    assert_eq!(reference["status"], "ambiguous");
    code(metric, "reference_ambiguous");
}

#[test]
fn metric_selector_missing_and_cross_stream_repeated_attempt_are_never_resolved_by_position() {
    let mut missing_selector = full_window();
    missing_selector["tail"][16]["record"]["attemptId"] = json!(oid(999));
    missing_selector["tail"][16]["correlation"]["attemptId"] = json!(oid(999));
    let missing_output = document(&missing_selector);
    let metric = event_row(&missing_output, 115);
    assert_eq!(metric["assessment"], "incomplete");
    assert_eq!(metric["references"][0]["status"], "missing");
    code(metric, "reference_missing");

    let mut cross_stream = full_window();
    append(
        &mut cross_stream,
        attempt(113, 900, "outcome_unknown", Value::Null, json!([])),
    );
    let repeated = cross_stream["tail"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap();
    repeated["producerId"] = json!(oid(12));
    repeated["fenceId"] = json!(oid(22));
    repeated["sequence"] = json!(1);
    repeated["previousEventId"] = Value::Null;
    let output = document(&cross_stream);
    let chain = output["reportedChains"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["kind"] == "attempt")
        .unwrap();
    assert_eq!(chain["reportedChainStatus"], "unknown_or_conflict");
    code(chain, "stream_mismatch");
}

#[test]
fn repeated_identical_changed_same_id_and_uncertain_states_stay_visible_as_conflicts_or_incomplete()
{
    let mut repeated = full_window();
    append(
        &mut repeated,
        attempt(113, 900, "outcome_unknown", Value::Null, json!([])),
    );
    let repeated_output = document(&repeated);
    let attempts = repeated_output["reportedChains"].as_array().unwrap();
    let attempt_chain = attempts
        .iter()
        .find(|row| row["kind"] == "attempt")
        .unwrap();
    assert_eq!(attempt_chain["matchingEventOrdinals"], json!([14, 19]));
    code(event_row(&repeated_output, 113), "effect_unknown");

    let mut changed = full_window();
    append(
        &mut changed,
        attempt(113, 900, "reported_completed", Value::Null, json!([])),
    );
    let changed_output = document(&changed);
    let chain = changed_output["reportedChains"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["kind"] == "attempt")
        .unwrap();
    assert_eq!(chain["reportedChainStatus"], "unknown_or_conflict");
    assert_eq!(chain["reportedTipRecordId"], Value::Null);

    let mut denied = full_window();
    denied["tail"][13]["record"] = authorization(112, "reported_refused", Value::Null);
    let denied_output = document(&denied);
    assert_eq!(
        event_row(&denied_output, 112)["reportedState"],
        "reported_refused"
    );
    fixed_trust(&denied_output);
}

#[test]
fn reconciliation_requires_evidence_and_cancelled_or_unknown_effects_remain_incomplete() {
    let mut missing_evidence = full_window();
    append(
        &mut missing_evidence,
        attempt(
            120,
            900,
            "reported_reconciled_completed",
            json!(oid(113)),
            json!([]),
        ),
    );
    let output = document(&missing_evidence);
    code(event_row(&output, 120), "reconciliation_evidence_missing");
    code(event_row(&output, 113), "effect_unknown");

    let mut with_evidence = full_window();
    with_evidence["tail"][14]["record"]["evidenceIds"] = json!([oid(117)]);
    append(
        &mut with_evidence,
        attempt(
            120,
            900,
            "reported_reconciled_completed",
            json!(oid(113)),
            json!([oid(117)]),
        ),
    );
    let evidence_output = document(&with_evidence);
    assert_eq!(
        event_row(&evidence_output, 120)["reportedState"],
        "reported_reconciled_completed"
    );

    let mut cancelled = full_window();
    cancelled["tail"][14]["record"]["state"] = json!("reported_cancelled");
    code(event_row(&document(&cancelled), 113), "effect_unknown");
}

#[test]
fn reconciliation_with_one_unique_and_one_missing_evidence_keeps_chain_and_metric_selector_unique()
{
    let mut value = full_window();
    append(
        &mut value,
        attempt(
            120,
            900,
            "reported_reconciled_completed",
            json!(oid(113)),
            json!([oid(117), oid(777)]),
        ),
    );
    let output = document(&value);
    let chain = output["reportedChains"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["kind"] == "attempt")
        .unwrap();
    assert_eq!(chain["reportedChainStatus"], "single_chain_unverified");
    assert_eq!(chain["reportedTipRecordId"], oid(120));
    code(event_row(&output, 120), "reference_missing");
    let metric = event_row(&output, 115);
    assert_eq!(metric["references"][0]["status"], "unique_unverified");
}

#[test]
fn suspension_and_coverage_rows_report_partial_resume_stale_expired_missing_and_conflict_without_qualification()
 {
    let mut value = full_window();
    append(
        &mut value,
        suspension(120, "reported_partial", json!(oid(114))),
    );
    append(
        &mut value,
        suspension(121, "reported_resumed", json!(oid(120))),
    );
    value["tail"][18]["record"] = coverage(118, "reported_stale", json!(40), json!([]));
    let mut conflicting = coverage(119, "reported_current", json!(100), json!([oid(117)]));
    conflicting["checks"]["reconciliation"] = json!("unknown");
    append(&mut value, conflicting);
    let output = document(&value);
    let suspension_chain = output["reportedChains"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["kind"] == "suspension")
        .unwrap();
    assert_eq!(suspension_chain["reportedTipState"], "reported_resumed");
    let coverage = output["coverageAssessments"].as_array().unwrap();
    assert_eq!(coverage.len(), 2);
    code(&coverage[0], "coverage_stale");
    code(&coverage[0], "coverage_expired");
    code(&coverage[0], "coverage_evidence_missing");
    code(&coverage[1], "coverage_conflict");
    let coverage_event = event_row(&output, 118);
    code(coverage_event, "coverage_stale");
    code(coverage_event, "coverage_expired");
    code(coverage_event, "coverage_evidence_missing");
    code(event_row(&output, 119), "coverage_conflict");
    assert!(
        coverage
            .iter()
            .all(|row| row["qualifiedLevel"] == "unknown")
    );
    fixed_trust(&output);
}

#[test]
fn lifecycle_reference_ceiling_fails_atomically_with_a_payload_free_error() {
    let mut value = base_window();
    let evidence_ids = json!(
        (0..32)
            .map(|offset| oid(7_000 + offset))
            .collect::<Vec<_>>()
    );
    for offset in 0..257_u16 {
        append(
            &mut value,
            attempt(
                1_000 + offset,
                8_000 + offset,
                "outcome_unknown",
                Value::Null,
                evidence_ids.clone(),
            ),
        );
    }
    let decoded = decode(&value);
    let before = decoded.canonical_bytes().to_vec();
    let error = inspect_lifecycle(&decoded).unwrap_err();
    assert_eq!(error.to_string(), "lifecycle_limit");
    assert!(!format!("{error:?}").contains("ops:"));
    assert_eq!(decoded.canonical_bytes(), before.as_slice());
}

#[test]
fn lifecycle_writer_cap_fails_before_reference_cap_without_mutating_input() {
    let mut value = base_window();
    let evidence_ids = json!(
        (0..32)
            .map(|offset| oid(7_000 + offset))
            .collect::<Vec<_>>()
    );
    for offset in 0..200_u16 {
        let mut record = attempt(
            1_000 + offset,
            8_000 + offset,
            "outcome_unknown",
            Value::Null,
            evidence_ids.clone(),
        );
        record["authorizationId"] = Value::Null;
        append(&mut value, record);
    }
    let decoded = decode(&value);
    let before = decoded.canonical_bytes().to_vec();
    let error = inspect_lifecycle(&decoded).unwrap_err();
    assert_eq!(error, LifecycleError::Limit);
    assert_eq!(error.to_string(), "lifecycle_limit");
    assert!(!format!("{error:?}").contains("ops:"));
    assert_eq!(decoded.canonical_bytes(), before.as_slice());
}

fn matching_budget_window(metric_count: u16) -> Value {
    let mut value = base_window();
    for _ in 0..128 {
        let mut record = attempt(113, 900, "reported_accepted", Value::Null, json!([]));
        record["authorizationId"] = Value::Null;
        append(&mut value, record);
    }
    for offset in 0..metric_count {
        append(&mut value, metric(2_000 + offset, 900));
    }
    value
}

#[test]
fn lifecycle_matching_ordinal_cap_permits_8192_and_rejects_8193_atomically() {
    let at_limit = decode(&matching_budget_window(64));
    assert!(inspect_lifecycle(&at_limit).is_ok());

    let over_limit = decode(&matching_budget_window(65));
    let before = over_limit.canonical_bytes().to_vec();
    let error = inspect_lifecycle(&over_limit).unwrap_err();
    assert_eq!(error.to_string(), "lifecycle_limit");
    assert!(!format!("{error:?}").contains("ops:"));
    assert_eq!(over_limit.canonical_bytes(), before.as_slice());
}

fn append_invalid_predecessor_chain(value: &mut Value, first_id: u16, nodes: u16, attempt_id: u16) {
    for offset in 0..nodes {
        let id = first_id + offset;
        let prior = if offset == 0 {
            Value::Null
        } else {
            json!(oid(id - 1))
        };
        let state = if offset == 0 {
            "reported_accepted"
        } else {
            "reported_active"
        };
        append(value, attempt(id, attempt_id, state, prior, json!([])));
    }
}

fn predecessor_walk_window(over_limit: bool) -> Value {
    let mut value = base_window();
    append_invalid_predecessor_chain(&mut value, 2_000, 127, 9_000);
    append_invalid_predecessor_chain(&mut value, 3_000, 10, 9_001);
    append_invalid_predecessor_chain(&mut value, 4_000, 3, 9_002);
    for offset in 0..if over_limit { 4 } else { 3 } {
        append(
            &mut value,
            attempt(
                5_000 + offset,
                9_003,
                "outcome_unknown",
                Value::Null,
                json!([]),
            ),
        );
    }
    value
}

#[test]
fn lifecycle_predecessor_walk_cap_permits_8192_and_rejects_step_8193_atomically() {
    let at_limit = decode(&predecessor_walk_window(false));
    assert!(inspect_lifecycle(&at_limit).is_ok());

    let over_limit = decode(&predecessor_walk_window(true));
    let before = over_limit.canonical_bytes().to_vec();
    let error = inspect_lifecycle(&over_limit).unwrap_err();
    assert_eq!(error.to_string(), "lifecycle_limit");
    assert!(!format!("{error:?}").contains("ops:"));
    assert_eq!(over_limit.canonical_bytes(), before.as_slice());
}
