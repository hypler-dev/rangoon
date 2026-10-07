use super::*;
use serde_json::{Value, json};

fn oid(number: u16) -> String {
    format!("ops:{number:064x}")
}

fn digest(number: u16) -> String {
    format!("{number:064x}")
}

fn external(number: u16) -> Value {
    json!({
        "systemId": oid(number),
        "contractVersion": "v1",
        "externalId": format!("external-{number}"),
    })
}

fn flow_context() -> Value {
    json!({
        "workspaceId": oid(2), "systemId": oid(3), "environmentId": oid(4),
        "engineId": oid(5), "requestId": oid(108), "traceId": oid(6),
        "spanId": oid(7), "policyRevisionId": oid(104),
    })
}

fn action_scope() -> Value {
    json!({
        "systemId": oid(3), "environmentIds": [oid(4)], "operationClasses": ["deploy"],
        "resourceIds": [oid(101)], "policyRevisionIds": [oid(104)],
    })
}

fn correlation() -> Value {
    json!({
        "workspaceId": oid(2), "systemId": oid(3), "environmentId": oid(4),
        "engineId": oid(5), "traceId": oid(6), "spanId": oid(7),
        "requestId": oid(108), "evaluationId": oid(109), "attemptId": oid(113),
        "policyRevisionId": oid(104),
    })
}

fn event(number: u64, producer: u16, record: Value, correlation: Value) -> Value {
    json!({
        "eventId": oid(300 + number as u16), "producerId": oid(producer),
        "fenceId": oid(20 + producer), "sequence": number, "previousEventId": null,
        "observedMs": null, "receivedMs": null, "correlation": correlation, "record": record,
    })
}

fn rich_window() -> Value {
    let actor = json!({
        "kind":"actor", "id":oid(100), "systemId":oid(3), "canonicalSubjectId":oid(200),
        "actorKind":"agent", "identitySourceRef":null, "identityEvidenceId":null,
        "reportedVerification":"reported_verified", "observedMs":null, "freshness":"reported_stale",
    });
    let records = vec![
        json!({"kind":"resource","id":oid(101),"systemId":oid(3),"resourceClass":"repository","externalRef":null}),
        json!({"kind":"tool","id":oid(102),"systemId":oid(3),"operationClass":"deploy","externalRef":external(30)}),
        json!({"kind":"workflow","id":oid(103),"systemId":oid(3),"externalRef":external(31),"capabilityRefs":[external(32)]}),
        json!({"kind":"engine","id":oid(5),"systemId":oid(3),"externalRef":external(33),"artifactDigest":digest(5)}),
        json!({"kind":"policy_revision","id":oid(104),"gatewayId":oid(105),"previousRevisionId":null,"artifactDigest":digest(6),"externalRef":null}),
        json!({"kind":"gateway_revision","id":oid(105),"gatewayId":oid(105),"previousRevisionId":null,"policyRevisionId":oid(104),"scope":action_scope(),"humanRequired":true,"changedByActorId":oid(100),"state":"reported_active","evidenceIds":[oid(117)]}),
        json!({"kind":"assignment","id":oid(106),"actorId":oid(100),"gatewayRevisionId":oid(105),"policyRevisionId":oid(104),"delegationId":null,"state":"reported_active","changedByActorId":oid(100),"evidenceIds":[oid(117)]}),
        json!({"kind":"delegation","id":oid(107),"delegatorId":oid(100),"delegateId":oid(100),"parentDelegationId":null,"scope":action_scope(),"expiresMs":null,"state":"reported_active","changedByActorId":oid(100),"externalRef":null,"reportedVerification":"reported_verified","observedMs":null,"freshness":"reported_current","evidenceIds":[oid(117)]}),
        json!({"kind":"request","id":oid(108),"context":flow_context(),"actorId":oid(100),"gatewayRevisionId":oid(105),"operationClass":"deploy","actionDigest":digest(8),"contextDigest":digest(9),"resourceIds":[oid(101)],"workflowRef":null,"expiresMs":null}),
        json!({"kind":"evaluation","id":oid(109),"context":flow_context(),"evaluationKind":"successor","gatewayRevisionId":oid(105),"evaluatorId":oid(100),"evaluatorArtifactDigest":digest(10),"inputSnapshotId":oid(1),"inputCanonicalization":"json-v1","inputDigest":digest(11),"contextSnapshotId":oid(1),"contextDigest":digest(12),"previousEvaluationId":null,"expectedEffectiveEvaluationId":null,"lineageFenceId":oid(21),"result":"unknown","expiresMs":null,"reason":"conflict","externalRef":null,"evidenceIds":[oid(117)]}),
        json!({"kind":"approval_plan","id":oid(110),"context":flow_context(),"evaluationId":oid(109),"previousPlanId":null,"changedByActorId":oid(100),"expiresMs":null,"stages":[{"id":oid(210),"dependsOnStageIds":[oid(210)],"approverIds":[oid(100)],"humanRequired":true,"mode":"quorum","quorum":0,"expiresMs":null,"escalationActorIds":[]}],"evidenceIds":[oid(117)]}),
        json!({"kind":"approval_decision","id":oid(111),"context":flow_context(),"evaluationId":oid(109),"planId":oid(110),"stageId":oid(210),"actorId":oid(100),"delegationId":null,"decision":"abstain","supersedesDecisionId":null,"reason":"human_required","evidenceIds":[oid(117)]}),
        json!({"kind":"authorization","id":oid(112),"context":flow_context(),"evaluationId":oid(109),"planId":null,"decisionIds":[oid(111)],"previousObservationId":null,"state":"reported_issued","scope":action_scope(),"expiresMs":null,"reason":"none","externalRef":null,"evidenceIds":[oid(117)]}),
        json!({"kind":"attempt","id":oid(113),"context":flow_context(),"authorizationId":null,"executorId":oid(100),"previousObservationId":null,"attemptId":oid(113),"state":"outcome_unknown","reason":"unknown_outcome","evidenceIds":[oid(117)]}),
        json!({"kind":"suspension","id":oid(114),"gatewayRevisionId":oid(105),"requestedByActorId":oid(100),"scope":action_scope(),"state":"reported_partial","previousObservationId":null,"reason":"conflict","evidenceIds":[oid(117)]}),
        json!({"kind":"metric","id":oid(115),"context":flow_context(),"attemptId":null,"resourceIds":[oid(101)],"latencyMs":null,"cost":null,"reason":"unknown_outcome"}),
        json!({"kind":"evidence","id":oid(117),"producerId":oid(11),"externalRef":null,"artifactDigest":null,"state":"unverified","reason":"unknown_outcome"}),
        json!({"kind":"coverage","id":oid(118),"systemId":oid(3),"engineId":oid(5),"gatewayRevisionId":oid(105),"operationClass":"deploy","environmentId":oid(4),"targetOs":"linux","observedMs":null,"freshness":"reported_current","targetResourceIds":[oid(101)],"adapterArtifactDigest":digest(13),"engineArtifactDigest":digest(14),"reportedLevel":"fully_mediated","checks":{"policyDecision":"yes","approvalDuties":"yes","agentDelegation":"yes","expiryRevocation":"yes","executionConsumption":"yes","replayProtection":"yes","outcomeEvidence":"yes","reconciliation":"yes"},"expiresMs":null,"evidenceIds":[oid(117)]}),
    ];
    let mut tail = Vec::new();
    for (offset, record) in records.into_iter().enumerate() {
        tail.push(event((offset + 2) as u64, 12, record, correlation()));
    }
    json!({
        "schemaVersion":"rangoon.ops.window.v1",
        "snapshot":{
            "snapshotId":oid(1), "reducerVersion":"rangoon.ops.reducer.v1",
            "scope":{"workspaceId":oid(2),"systemIds":[oid(3)],"environmentIds":[oid(4)],"engineIds":[oid(5)]},
            "topology":{
                "revisionId":oid(8),
                "nodes":[{"id":oid(400),"kind":"gateway","recordId":oid(105)}],
                "edges":[{"id":oid(401),"kind":"declared_dependency","from":oid(400),"to":oid(400)}],
                "policyRevisionIds":[oid(104)],"coverageRevisionIds":[oid(118)]
            },
            "prefixes":[{"producerId":oid(11),"fenceId":oid(21),"watermark":null,"events":[event(1,11,actor,Value::Null)]}],
            "asOfMs":0
        },
        "tail":tail
    })
}

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

fn rich_event_mut(window: &mut Value, index: usize) -> &mut serde_json::Map<String, Value> {
    window["tail"][index].as_object_mut().unwrap()
}

fn rich_record_mut(window: &mut Value, index: usize) -> &mut serde_json::Map<String, Value> {
    rich_event_mut(window, index)["record"]
        .as_object_mut()
        .unwrap()
}

#[test]
fn empty_window_has_stable_inventory() {
    let golden = br#"{"schemaVersion":"rangoon.ops.window.v1","snapshot":{"snapshotId":"ops:1111111111111111111111111111111111111111111111111111111111111111","reducerVersion":"rangoon.ops.reducer.v1","scope":{"workspaceId":"ops:1111111111111111111111111111111111111111111111111111111111111111","systemIds":[],"environmentIds":[],"engineIds":[]},"topology":{"revisionId":"ops:1111111111111111111111111111111111111111111111111111111111111111","nodes":[],"edges":[],"policyRevisionIds":[],"coverageRevisionIds":[]},"prefixes":[],"asOfMs":0},"tail":[]}"#;
    let window = decode_window(golden).unwrap();
    assert_eq!(window.canonical_bytes(), golden);
    assert_eq!(
        window.canonical_digest(),
        "8ab43d0754e994dc00445661f74d1da62b6efec06f7c6276bee1b9e1fc25c0a7"
    );
    assert_eq!(window.inventory().event_count(), 0);
    assert_eq!(window.inventory().authority(), "none");
    assert_eq!(window.inventory().authenticity(), "unverified");
    assert_eq!(window.inventory().execution(), "unavailable");
    assert_eq!(window.inventory().record_counts().len(), 19);
}

#[test]
fn rich_window_covers_vocabulary_without_authority_inference() {
    let window = decode_window(&bytes(&rich_window())).unwrap();
    let inventory = window.inventory();
    assert_eq!(inventory.producer_count(), 2);
    assert_eq!(inventory.event_count(), 19);
    assert_eq!(inventory.topology_node_count(), 1);
    assert_eq!(inventory.topology_edge_count(), 1);
    for kind in [
        "actor",
        "resource",
        "tool",
        "workflow",
        "engine",
        "policy_revision",
        "gateway_revision",
        "assignment",
        "delegation",
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
    ] {
        assert_eq!(inventory.record_counts()[kind], 1, "{kind}");
    }
    assert_eq!(inventory.authority(), "none");
    assert_eq!(inventory.authenticity(), "unverified");
    assert_eq!(inventory.execution(), "unavailable");
    let canonical: Value = serde_json::from_slice(window.canonical_bytes()).unwrap();
    assert_eq!(
        canonical["snapshot"]["prefixes"][0]["watermark"],
        Value::Null
    );
    assert_eq!(
        canonical["tail"][9]["record"]["evaluationKind"],
        "successor"
    );
    assert_eq!(
        canonical["tail"][9]["record"]["previousEvaluationId"],
        Value::Null
    );
    assert_eq!(
        canonical["tail"][17]["record"]["reportedLevel"],
        "fully_mediated"
    );
    assert_eq!(
        canonical["tail"][17]["record"]["checks"]["reconciliation"],
        "yes"
    );
    assert_eq!(canonical["tail"][0]["record"]["externalRef"], Value::Null);
    assert_eq!(
        canonical["tail"][10]["record"]["stages"][0]["dependsOnStageIds"],
        json!([oid(210)])
    );
}

#[test]
fn each_contract_record_decodes_in_an_independent_window() {
    let fixture = rich_window();
    let kinds = [
        "resource",
        "tool",
        "workflow",
        "engine",
        "policy_revision",
        "gateway_revision",
        "assignment",
        "delegation",
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
    let mut actor = fixture.clone();
    actor["tail"] = json!([]);
    assert!(decode_window(&bytes(&actor)).is_ok(), "actor");
    for (index, kind) in kinds.into_iter().enumerate() {
        let mut window = fixture.clone();
        let event = window["tail"][index].take();
        window["snapshot"]["prefixes"] = json!([]);
        window["tail"] = json!([event]);
        assert!(decode_window(&bytes(&window)).is_ok(), "{kind}");
    }
}

#[test]
fn canonicalization_is_key_stable_and_preserves_values() {
    let first = rich_window();
    let mut reordered = rich_window();
    let object = reordered.as_object_mut().unwrap();
    let tail = object.remove("tail").unwrap();
    let schema = object.remove("schemaVersion").unwrap();
    let snapshot = object.remove("snapshot").unwrap();
    *object = [
        ("tail".to_owned(), tail),
        ("snapshot".to_owned(), snapshot),
        ("schemaVersion".to_owned(), schema),
    ]
    .into_iter()
    .collect();
    let one = decode_window(&bytes(&first)).unwrap();
    let two = decode_window(&bytes(&reordered)).unwrap();
    assert_eq!(one.canonical_bytes(), two.canonical_bytes());
    assert_eq!(one.canonical_digest(), two.canonical_digest());
    let canonical: Value = serde_json::from_slice(one.canonical_bytes()).unwrap();
    assert_eq!(canonical["tail"][1]["record"]["operationClass"], "deploy");
    assert_eq!(
        canonical["tail"][2]["record"]["capabilityRefs"],
        json!([external(32)])
    );
    assert_eq!(canonical["tail"][15]["record"]["cost"], Value::Null);
}

#[test]
fn retains_initial_and_successor_and_unverified_reported_observations() {
    for kind in ["initial", "successor"] {
        for previous in [Value::Null, json!(oid(109))] {
            for expected in [Value::Null, json!(oid(999))] {
                let mut input = rich_window();
                let evaluation = rich_record_mut(&mut input, 9);
                evaluation.insert("evaluationKind".into(), json!(kind));
                evaluation.insert("previousEvaluationId".into(), previous.clone());
                evaluation.insert("expectedEffectiveEvaluationId".into(), expected.clone());
                let window = decode_window(&bytes(&input)).unwrap();
                let canonical: Value = serde_json::from_slice(window.canonical_bytes()).unwrap();
                assert_eq!(canonical["tail"][9]["record"]["evaluationKind"], kind);
                assert_eq!(
                    canonical["tail"][9]["record"]["previousEvaluationId"],
                    previous
                );
                assert_eq!(
                    canonical["tail"][9]["record"]["expectedEffectiveEvaluationId"],
                    expected
                );
                assert_eq!(window.inventory().authenticity(), "unverified");
            }
        }
    }
}

#[test]
fn rejects_escaped_duplicate_keys_missing_nullable_and_extra_tagged_fields() {
    let mut duplicate = bytes(&rich_window());
    duplicate.truncate(duplicate.len() - 1);
    duplicate.extend_from_slice(br#","ta\u0069l":[]}"#);
    assert_eq!(
        decode_window(&duplicate).unwrap_err().code(),
        "duplicate_field"
    );
    let mut missing = rich_window();
    rich_record_mut(&mut missing, 0).remove("externalRef");
    assert_eq!(
        decode_window(&bytes(&missing)).unwrap_err().code(),
        "invalid_record"
    );
    let mut extra = rich_window();
    rich_record_mut(&mut extra, 0).insert("extra".into(), Value::Null);
    assert_eq!(
        decode_window(&bytes(&extra)).unwrap_err().code(),
        "invalid_record"
    );
}

#[test]
fn rejects_escaped_duplicates_inside_nested_and_tagged_objects() {
    let source = String::from_utf8(bytes(&rich_window())).unwrap();
    let workspace = oid(2);
    let nested_needle = format!(r#""workspaceId":"{workspace}""#);
    let nested_replacement =
        format!(r#""workspaceId":"{workspace}","\u0077orkspaceId":"{workspace}""#);
    let nested = source.replacen(&nested_needle, &nested_replacement, 1);
    assert_ne!(nested, source);
    let nested_error = decode_window(nested.as_bytes()).unwrap_err();
    assert_eq!(nested_error.code(), "duplicate_field");
    assert!(!nested_error.to_string().contains("workspaceId"));

    let tagged = source.replacen(
        r#""kind":"resource""#,
        r#""kind":"resource","\u006bind":"resource""#,
        1,
    );
    assert_ne!(tagged, source);
    let tagged_error = decode_window(tagged.as_bytes()).unwrap_err();
    assert_eq!(tagged_error.code(), "duplicate_field");
    assert!(!tagged_error.to_string().contains("resource"));
}

#[test]
fn every_required_nullable_field_must_be_present() {
    let record_nullable = [
        (0, "externalRef"),
        (4, "previousRevisionId"),
        (4, "externalRef"),
        (5, "previousRevisionId"),
        (6, "delegationId"),
        (7, "parentDelegationId"),
        (7, "expiresMs"),
        (7, "externalRef"),
        (7, "observedMs"),
        (8, "workflowRef"),
        (8, "expiresMs"),
        (9, "previousEvaluationId"),
        (9, "expectedEffectiveEvaluationId"),
        (9, "expiresMs"),
        (9, "externalRef"),
        (10, "previousPlanId"),
        (10, "expiresMs"),
        (11, "delegationId"),
        (11, "supersedesDecisionId"),
        (12, "planId"),
        (12, "previousObservationId"),
        (12, "expiresMs"),
        (12, "externalRef"),
        (13, "authorizationId"),
        (13, "previousObservationId"),
        (14, "previousObservationId"),
        (15, "attemptId"),
        (15, "latencyMs"),
        (15, "cost"),
        (16, "externalRef"),
        (16, "artifactDigest"),
        (17, "observedMs"),
        (17, "expiresMs"),
    ];
    for (record, field) in record_nullable {
        let mut window = rich_window();
        rich_record_mut(&mut window, record).remove(field);
        assert_eq!(
            decode_window(&bytes(&window)).unwrap_err().code(),
            "invalid_record",
            "tail {record} {field}"
        );
    }
    for field in ["identitySourceRef", "identityEvidenceId", "observedMs"] {
        let mut window = rich_window();
        window["snapshot"]["prefixes"][0]["events"][0]["record"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(
            decode_window(&bytes(&window)).unwrap_err().code(),
            "invalid_record",
            "actor {field}"
        );
    }
    let mut watermark = rich_window();
    watermark["snapshot"]["prefixes"][0]
        .as_object_mut()
        .unwrap()
        .remove("watermark");
    assert_eq!(
        decode_window(&bytes(&watermark)).unwrap_err().code(),
        "invalid_record"
    );
    for field in ["previousEventId", "observedMs", "receivedMs", "correlation"] {
        let mut window = rich_window();
        window["snapshot"]["prefixes"][0]["events"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(
            decode_window(&bytes(&window)).unwrap_err().code(),
            "invalid_record",
            "event {field}"
        );
    }
    for field in ["requestId", "evaluationId", "attemptId", "policyRevisionId"] {
        let mut window = rich_window();
        rich_event_mut(&mut window, 8)["correlation"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(
            decode_window(&bytes(&window)).unwrap_err().code(),
            "invalid_record",
            "correlation {field}"
        );
    }
    for field in ["quorum", "expiresMs"] {
        let mut window = rich_window();
        rich_record_mut(&mut window, 10)["stages"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(
            decode_window(&bytes(&window)).unwrap_err().code(),
            "invalid_record",
            "stage {field}"
        );
    }
    let mut money = rich_window();
    rich_record_mut(&mut money, 15).insert(
        "cost".into(),
        json!({"kind":"final","currency":"USD","amountMicros":0,"sourceEvidenceId":null}),
    );
    rich_record_mut(&mut money, 15)["cost"]
        .as_object_mut()
        .unwrap()
        .remove("sourceEvidenceId");
    assert_eq!(
        decode_window(&bytes(&money)).unwrap_err().code(),
        "invalid_record"
    );
}

#[test]
fn rejects_schema_kind_tag_boolean_id_digest_token_currency_and_wrong_types() {
    let mut cases: Vec<(&str, Value)> = Vec::new();
    let mut schema = rich_window();
    schema["schemaVersion"] = json!("rangoon.ops.window.v2");
    cases.push(("schema", schema));
    let mut kind = rich_window();
    rich_record_mut(&mut kind, 0).insert("kind".into(), json!("unknown"));
    cases.push(("kind", kind));
    let mut tag = rich_window();
    rich_record_mut(&mut tag, 5).insert("state".into(), json!("active"));
    cases.push(("tag", tag));
    let mut boolean = rich_window();
    rich_record_mut(&mut boolean, 5).insert("humanRequired".into(), json!("true"));
    cases.push(("boolean", boolean));
    let mut id = rich_window();
    rich_record_mut(&mut id, 0).insert("id".into(), json!("not-an-id"));
    cases.push(("id", id));
    let mut subject = rich_window();
    subject["snapshot"]["prefixes"][0]["events"][0]["record"]["canonicalSubjectId"] =
        json!("subject-100");
    cases.push(("subject", subject));
    let mut digest_case = rich_window();
    rich_record_mut(&mut digest_case, 3).insert("artifactDigest".into(), json!("ABC"));
    cases.push(("digest", digest_case));
    let mut token = rich_window();
    rich_record_mut(&mut token, 1).insert("operationClass".into(), json!("bad/token"));
    cases.push(("token", token));
    let mut type_case = rich_window();
    rich_event_mut(&mut type_case, 0).insert("sequence".into(), json!("2"));
    cases.push(("type", type_case));
    let mut currency = rich_window();
    rich_record_mut(&mut currency, 15).insert(
        "cost".into(),
        json!({"kind":"observed","currency":"usd","amountMicros":0,"sourceEvidenceId":null}),
    );
    cases.push(("currency", currency));
    let mut reason = rich_window();
    rich_record_mut(&mut reason, 16).insert("reason".into(), json!("unknown"));
    cases.push(("reason", reason));
    for (name, value) in cases {
        let expected = if name == "schema" {
            "unsupported_schema"
        } else {
            "invalid_record"
        };
        assert_eq!(
            decode_window(&bytes(&value)).unwrap_err().code(),
            expected,
            "{name}"
        );
    }
}

#[test]
fn rejects_invalid_encoding_json_trailing_and_disallowed_correlation() {
    assert_eq!(
        decode_window(&[0xff]).unwrap_err().code(),
        "invalid_encoding"
    );
    assert_eq!(
        decode_window(br#"{"schemaVersion":false}"#)
            .unwrap_err()
            .code(),
        "invalid_record"
    );
    let mut trailing = bytes(&rich_window());
    trailing.extend_from_slice(b" false");
    assert_eq!(decode_window(&trailing).unwrap_err().code(), "invalid_json");
    let mut correlation = rich_window();
    rich_event_mut(&mut correlation, 8).insert("correlation".into(), Value::Null);
    assert_eq!(
        decode_window(&bytes(&correlation)).unwrap_err().code(),
        "invalid_record"
    );
}

#[test]
fn numeric_boundaries_reject_zero_negative_fractional_and_overflow() {
    for value in [
        json!(0),
        json!(-1),
        json!(1.5),
        json!(9_007_199_254_740_992u64),
    ] {
        let mut window = rich_window();
        rich_event_mut(&mut window, 0).insert("sequence".into(), value);
        assert_eq!(
            decode_window(&bytes(&window)).unwrap_err().code(),
            "invalid_record"
        );
    }
    for (record, field) in [(15, "latencyMs"), (17, "observedMs")] {
        for value in [json!(-1), json!(1.5), json!(9_007_199_254_740_992u64)] {
            let mut window = rich_window();
            rich_record_mut(&mut window, record).insert(field.into(), value);
            assert_eq!(
                decode_window(&bytes(&window)).unwrap_err().code(),
                "invalid_record",
                "{field}"
            );
        }
    }
    for value in [json!(-1), json!(1.5), json!(9_007_199_254_740_992u64)] {
        let mut cost = rich_window();
        rich_record_mut(&mut cost, 15).insert(
            "cost".into(),
            json!({"kind":"final","currency":"USD","amountMicros":value,"sourceEvidenceId":null}),
        );
        assert_eq!(
            decode_window(&bytes(&cost)).unwrap_err().code(),
            "invalid_record"
        );
        let mut quorum = rich_window();
        rich_record_mut(&mut quorum, 10)["stages"][0]
            .as_object_mut()
            .unwrap()
            .insert("quorum".into(), value);
        assert_eq!(
            decode_window(&bytes(&quorum)).unwrap_err().code(),
            "invalid_record"
        );
    }
    for value in [json!(-1), json!(1.5), json!(9_007_199_254_740_992u64)] {
        let mut window = rich_window();
        window["snapshot"]["asOfMs"] = value;
        assert_eq!(
            decode_window(&bytes(&window)).unwrap_err().code(),
            "invalid_record"
        );
    }
    for value in [
        json!(0),
        json!(-1),
        json!(1.5),
        json!(9_007_199_254_740_992u64),
    ] {
        let mut window = rich_window();
        window["snapshot"]["prefixes"][0]["watermark"] =
            json!({"sequence":value,"eventId":oid(301)});
        assert_eq!(
            decode_window(&bytes(&window)).unwrap_err().code(),
            "invalid_record"
        );
    }
    for value in [json!(-1), json!(1.5), json!(9_007_199_254_740_992u64)] {
        let mut window = rich_window();
        rich_record_mut(&mut window, 9).insert("expiresMs".into(), value);
        assert_eq!(
            decode_window(&bytes(&window)).unwrap_err().code(),
            "invalid_record"
        );
    }
}

#[test]
fn raw_shape_limits_run_before_typed_shape_checks() {
    let object = format!(
        "{{{}}}",
        (0..65)
            .map(|n| format!("\"f{n}\":0"))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(
        decode_window(object.as_bytes()).unwrap_err().code(),
        "collection_limit"
    );
    let array = format!("[{}]", vec!["0"; 1_025].join(","));
    assert_eq!(
        decode_window(array.as_bytes()).unwrap_err().code(),
        "collection_limit"
    );
    let values = format!(
        "[{}]",
        vec![format!("[{}]", vec!["0"; 1_024].join(",")); 32].join(",")
    );
    assert_eq!(
        decode_window(values.as_bytes()).unwrap_err().code(),
        "value_limit"
    );
    let deep = format!("{}0{}", "[".repeat(33), "]".repeat(33));
    assert_eq!(
        decode_window(deep.as_bytes()).unwrap_err().code(),
        "depth_limit"
    );
}

#[test]
fn accepts_exact_input_byte_ceiling_and_rejects_one_byte_more() {
    let base = br#"{"schemaVersion":"rangoon.ops.window.v1","snapshot":{"snapshotId":"ops:1111111111111111111111111111111111111111111111111111111111111111","reducerVersion":"rangoon.ops.reducer.v1","scope":{"workspaceId":"ops:1111111111111111111111111111111111111111111111111111111111111111","systemIds":[],"environmentIds":[],"engineIds":[]},"topology":{"revisionId":"ops:1111111111111111111111111111111111111111111111111111111111111111","nodes":[],"edges":[],"policyRevisionIds":[],"coverageRevisionIds":[]},"prefixes":[],"asOfMs":0},"tail":[]}"#;
    let mut exact = base.to_vec();
    exact.resize(2_097_152, b' ');
    assert_eq!(exact.len(), 2_097_152);
    assert!(decode_window(&exact).is_ok());
    exact.push(b' ');
    assert_eq!(decode_window(&exact).unwrap_err().code(), "input_limit");
}

#[test]
fn accepts_nonnull_nullable_contract_values_at_zero_and_maximum() {
    let maximum = 9_007_199_254_740_991u64;
    let mut window = rich_window();
    window["snapshot"]["asOfMs"] = json!(maximum);
    window["snapshot"]["prefixes"][0]["watermark"] = json!({"sequence":maximum,"eventId":oid(301)});
    let prefix_event = &mut window["snapshot"]["prefixes"][0]["events"][0];
    prefix_event["previousEventId"] = json!(oid(300));
    prefix_event["observedMs"] = json!(0);
    prefix_event["receivedMs"] = json!(maximum);
    let actor = prefix_event["record"].as_object_mut().unwrap();
    actor.insert("identitySourceRef".into(), external(31));
    actor.insert("identityEvidenceId".into(), json!(oid(117)));
    actor.insert("observedMs".into(), json!(maximum));
    rich_record_mut(&mut window, 0).insert("externalRef".into(), external(32));
    rich_record_mut(&mut window, 9).insert("expiresMs".into(), json!(maximum));
    rich_record_mut(&mut window, 10).insert("previousPlanId".into(), json!(oid(110)));
    rich_record_mut(&mut window, 10).insert("expiresMs".into(), json!(0));
    let stage = rich_record_mut(&mut window, 10)["stages"][0]
        .as_object_mut()
        .unwrap();
    stage.insert("quorum".into(), json!(maximum));
    stage.insert("expiresMs".into(), json!(maximum));
    rich_record_mut(&mut window, 15).insert("attemptId".into(), json!(oid(113)));
    rich_record_mut(&mut window, 15).insert("latencyMs".into(), json!(maximum));
    rich_record_mut(&mut window, 15).insert(
        "cost".into(),
        json!({"kind":"final","currency":"USD","amountMicros":0,"sourceEvidenceId":oid(117)}),
    );
    let decoded = decode_window(&bytes(&window)).unwrap();
    let canonical: Value = serde_json::from_slice(decoded.canonical_bytes()).unwrap();
    assert_eq!(canonical["snapshot"]["asOfMs"], maximum);
    assert_eq!(
        canonical["snapshot"]["prefixes"][0]["watermark"]["sequence"],
        maximum
    );
    assert_eq!(
        canonical["tail"][15]["record"]["cost"]["sourceEvidenceId"],
        oid(117)
    );
    assert_eq!(
        canonical["tail"][10]["record"]["stages"][0]["expiresMs"],
        maximum
    );
}

#[test]
fn retains_prefix_event_producer_mismatch_for_later_relationship_validation() {
    let actor = rich_window()["snapshot"]["prefixes"][0]["events"][0]["record"].clone();
    let mut window = rich_window();
    window["tail"] = json!([]);
    window["snapshot"]["prefixes"] = json!([{
        "producerId":oid(9_600), "fenceId":oid(9_601), "watermark":null,
        "events":[event(1, 9_602, actor, Value::Null)]
    }]);
    assert!(decode_window(&bytes(&window)).is_ok());
}

#[test]
fn accepts_every_collection_at_its_contract_boundary() {
    let ids = |start: u16, count: u16| -> Vec<Value> {
        (start..start + count)
            .map(|number| json!(oid(number)))
            .collect()
    };
    let mut lists = rich_window();
    lists["snapshot"]["scope"]["systemIds"] = json!(ids(500, 32));
    lists["snapshot"]["scope"]["environmentIds"] = json!(ids(600, 32));
    lists["snapshot"]["scope"]["engineIds"] = json!(ids(700, 32));
    lists["snapshot"]["topology"]["policyRevisionIds"] = json!(ids(800, 32));
    lists["snapshot"]["topology"]["coverageRevisionIds"] = json!(ids(900, 32));
    rich_record_mut(&mut lists, 2).insert(
        "capabilityRefs".into(),
        json!(
            (0..32)
                .map(|number| external(1000 + number))
                .collect::<Vec<_>>()
        ),
    );
    for record in [5, 6, 7, 9, 10, 11, 12, 13, 14, 17] {
        if rich_record_mut(&mut lists, record).contains_key("evidenceIds") {
            rich_record_mut(&mut lists, record).insert("evidenceIds".into(), json!(ids(1100, 32)));
        }
    }
    for record in [5, 7, 12, 14] {
        let scope = &mut rich_record_mut(&mut lists, record)["scope"];
        scope["environmentIds"] = json!(ids(1200, 32));
        scope["operationClasses"] = json!(
            (0..32)
                .map(|number| format!("operation-{number}"))
                .collect::<Vec<_>>()
        );
        scope["resourceIds"] = json!(ids(1300, 32));
        scope["policyRevisionIds"] = json!(ids(1400, 32));
    }
    rich_record_mut(&mut lists, 8).insert("resourceIds".into(), json!(ids(1500, 32)));
    rich_record_mut(&mut lists, 12).insert("decisionIds".into(), json!(ids(1600, 32)));
    rich_record_mut(&mut lists, 15).insert("resourceIds".into(), json!(ids(1700, 32)));
    rich_record_mut(&mut lists, 17).insert("targetResourceIds".into(), json!(ids(1800, 32)));
    let stages = (0..32)
        .map(|number| {
            json!({
                "id":oid(1900 + number), "dependsOnStageIds":ids(2000, 32),
                "approverIds":ids(2100, 32), "humanRequired":true, "mode":"quorum",
                "quorum":0, "expiresMs":null, "escalationActorIds":ids(2200, 32),
            })
        })
        .collect::<Vec<_>>();
    rich_record_mut(&mut lists, 10).insert("stages".into(), json!(stages));
    assert!(decode_window(&bytes(&lists)).is_ok(), "typed lists at 32");

    let actor = lists["snapshot"]["prefixes"][0]["events"][0]["record"].clone();
    let mut events = rich_window();
    events["tail"] = json!([]);
    let prefixes = (0..32)
        .map(|prefix| {
            let producer = 2300 + prefix;
            let prefix_events = (0..32)
                .map(|sequence| event(sequence + 1, producer, actor.clone(), Value::Null))
                .collect::<Vec<_>>();
            json!({"producerId":oid(producer),"fenceId":oid(2400 + prefix),"watermark":null,"events":prefix_events})
        })
        .collect::<Vec<_>>();
    events["snapshot"]["prefixes"] = json!(prefixes);
    events["snapshot"]["topology"]["nodes"] = json!(
        (0..128)
            .map(|number| json!({"id":oid(3000 + number),"kind":"gateway","recordId":oid(105)}))
            .collect::<Vec<_>>()
    );
    events["snapshot"]["topology"]["edges"] = json!((0..512).map(|number| json!({"id":oid(4000 + number),"kind":"declared_dependency","from":oid(3000),"to":oid(3000)})).collect::<Vec<_>>());
    let window = decode_window(&bytes(&events)).unwrap();
    assert_eq!(window.inventory().producer_count(), 32);
    assert_eq!(window.inventory().event_count(), 1_024);
    assert_eq!(window.inventory().topology_node_count(), 128);
    assert_eq!(window.inventory().topology_edge_count(), 512);
}

#[test]
fn rejects_collection_limits_above_each_shared_ceiling() {
    let mut typed = rich_window();
    rich_record_mut(&mut typed, 2).insert(
        "capabilityRefs".into(),
        json!(
            (0..33)
                .map(|number| external(5000 + number))
                .collect::<Vec<_>>()
        ),
    );
    assert_eq!(
        decode_window(&bytes(&typed)).unwrap_err().code(),
        "collection_limit"
    );

    let mut nodes = rich_window();
    nodes["snapshot"]["topology"]["nodes"] = json!(
        (0..129)
            .map(|number| json!({"id":oid(6000 + number),"kind":"gateway","recordId":oid(105)}))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        decode_window(&bytes(&nodes)).unwrap_err().code(),
        "collection_limit"
    );
    let mut edges = rich_window();
    edges["snapshot"]["topology"]["edges"] = json!((0..513).map(|number| json!({"id":oid(7000 + number),"kind":"declared_dependency","from":oid(400),"to":oid(400)})).collect::<Vec<_>>());
    assert_eq!(
        decode_window(&bytes(&edges)).unwrap_err().code(),
        "collection_limit"
    );

    let actor = rich_window()["snapshot"]["prefixes"][0]["events"][0]["record"].clone();
    let mut events = rich_window();
    events["tail"] = json!(
        (0..1_025)
            .map(|sequence| event(sequence + 1, 11, actor.clone(), Value::Null))
            .collect::<Vec<_>>()
    );
    events["snapshot"]["prefixes"] = json!([]);
    assert_eq!(
        decode_window(&bytes(&events)).unwrap_err().code(),
        "collection_limit"
    );

    let ids = |start: u16| -> Vec<Value> { (start..start + 33).map(|id| json!(oid(id))).collect() };
    let mut scope = rich_window();
    scope["snapshot"]["scope"]["systemIds"] = json!(ids(8_000));
    assert_eq!(
        decode_window(&bytes(&scope)).unwrap_err().code(),
        "collection_limit"
    );
    let mut pins = rich_window();
    pins["snapshot"]["topology"]["policyRevisionIds"] = json!(ids(8_100));
    assert_eq!(
        decode_window(&bytes(&pins)).unwrap_err().code(),
        "collection_limit"
    );
    let mut coverage_pins = rich_window();
    coverage_pins["snapshot"]["topology"]["coverageRevisionIds"] = json!(ids(8_200));
    assert_eq!(
        decode_window(&bytes(&coverage_pins)).unwrap_err().code(),
        "collection_limit"
    );
    let mut evidence = rich_window();
    rich_record_mut(&mut evidence, 5).insert("evidenceIds".into(), json!(ids(8_300)));
    assert_eq!(
        decode_window(&bytes(&evidence)).unwrap_err().code(),
        "collection_limit"
    );
    let mut operations = rich_window();
    rich_record_mut(&mut operations, 5)["scope"]["operationClasses"] = json!(
        (0..33)
            .map(|number| format!("operation-{number}"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        decode_window(&bytes(&operations)).unwrap_err().code(),
        "collection_limit"
    );
    let mut resources = rich_window();
    rich_record_mut(&mut resources, 5)["scope"]["resourceIds"] = json!(ids(8_400));
    assert_eq!(
        decode_window(&bytes(&resources)).unwrap_err().code(),
        "collection_limit"
    );
    let mut policies = rich_window();
    rich_record_mut(&mut policies, 5)["scope"]["policyRevisionIds"] = json!(ids(8_500));
    assert_eq!(
        decode_window(&bytes(&policies)).unwrap_err().code(),
        "collection_limit"
    );
    let mut decisions = rich_window();
    rich_record_mut(&mut decisions, 12).insert("decisionIds".into(), json!(ids(8_600)));
    assert_eq!(
        decode_window(&bytes(&decisions)).unwrap_err().code(),
        "collection_limit"
    );
    let mut stages = rich_window();
    rich_record_mut(&mut stages, 10).insert(
        "stages".into(),
        json!((0..33).map(|number| json!({"id":oid(8_700 + number),"dependsOnStageIds":[],"approverIds":[],"humanRequired":true,"mode":"all","quorum":null,"expiresMs":null,"escalationActorIds":[]})).collect::<Vec<_>>()),
    );
    assert_eq!(
        decode_window(&bytes(&stages)).unwrap_err().code(),
        "collection_limit"
    );
    for (field, start) in [
        ("dependsOnStageIds", 8_800),
        ("approverIds", 8_900),
        ("escalationActorIds", 9_000),
    ] {
        let mut stage = rich_window();
        rich_record_mut(&mut stage, 10)["stages"][0][field] = json!(ids(start));
        assert_eq!(
            decode_window(&bytes(&stage)).unwrap_err().code(),
            "collection_limit",
            "{field}"
        );
    }
    let mut coverage_resources = rich_window();
    rich_record_mut(&mut coverage_resources, 17)
        .insert("targetResourceIds".into(), json!(ids(9_100)));
    assert_eq!(
        decode_window(&bytes(&coverage_resources))
            .unwrap_err()
            .code(),
        "collection_limit"
    );

    let actor = rich_window()["snapshot"]["prefixes"][0]["events"][0]["record"].clone();
    let mut tail_producers = rich_window();
    tail_producers["snapshot"]["prefixes"] = json!([]);
    tail_producers["tail"] = json!(
        (0..33)
            .map(|index| event(index + 1, 9_200 + index as u16, actor.clone(), Value::Null))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        decode_window(&bytes(&tail_producers)).unwrap_err().code(),
        "collection_limit"
    );
    let mut prefix_count = rich_window();
    prefix_count["tail"] = json!([]);
    prefix_count["snapshot"]["prefixes"] = json!((0..33).map(|index| json!({"producerId":oid(9_300 + index),"fenceId":oid(9_400 + index),"watermark":null,"events":[]})).collect::<Vec<_>>());
    assert_eq!(
        decode_window(&bytes(&prefix_count)).unwrap_err().code(),
        "collection_limit"
    );
    let mut combined = rich_window();
    combined["snapshot"]["prefixes"] = json!([{
        "producerId":oid(9_500),"fenceId":oid(9_501),"watermark":null,
        "events":(0..512).map(|index| event(index + 1, 9_502, actor.clone(), Value::Null)).collect::<Vec<_>>()
    }]);
    combined["tail"] = json!(
        (0..513)
            .map(|index| event(index + 1, 9_503, actor.clone(), Value::Null))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        decode_window(&bytes(&combined)).unwrap_err().code(),
        "collection_limit"
    );
}

#[test]
fn errors_never_echo_input() {
    let error = decode_window(br#"{"secret-value":true}"#).unwrap_err();
    assert_eq!(error.code(), "invalid_record");
    assert!(!error.to_string().contains("secret-value"));
}
