use super::*;
use serde_json::{Value, json};

fn oid(number: u16) -> String {
    format!("ops:{number:064x}")
}

fn digest(number: u16) -> String {
    format!("{number:064x}")
}

fn actor(id: u16, freshness: &str) -> Value {
    json!({
        "kind":"actor","id":oid(id),"systemId":oid(3),"canonicalSubjectId":oid(200 + id),
        "actorKind":"agent","identitySourceRef":null,"identityEvidenceId":null,
        "reportedVerification":"reported_verified","observedMs":null,"freshness":freshness
    })
}

fn base_window() -> Value {
    json!({
        "schemaVersion":"rangoon.ops.window.v1",
        "snapshot":{
            "snapshotId":oid(1),"reducerVersion":"rangoon.ops.reducer.v1",
            "scope":{"workspaceId":oid(2),"systemIds":[oid(3),oid(33)],"environmentIds":[oid(4),oid(44)],"engineIds":[oid(5),oid(55)]},
            "topology":{"revisionId":oid(8),"nodes":[],"edges":[],"policyRevisionIds":[oid(104),oid(140)],"coverageRevisionIds":[]},
            "prefixes":[],"asOfMs":0
        },"tail":[]
    })
}

fn event(
    number: u64,
    producer: u16,
    fence: u16,
    event_id: u16,
    previous: Value,
    record: Value,
    correlation: Value,
) -> Value {
    json!({
        "eventId":oid(event_id),"producerId":oid(producer),"fenceId":oid(fence),
        "sequence":number,"previousEventId":previous,"observedMs":null,"receivedMs":null,
        "correlation":correlation,"record":record
    })
}

fn flow(record_id: u16, policy: u16) -> Value {
    json!({
        "workspaceId":oid(2),"systemId":oid(3),"environmentId":oid(4),"engineId":oid(5),
        "requestId":oid(record_id),"traceId":oid(6),"spanId":oid(7),"policyRevisionId":oid(policy)
    })
}

fn correlation(record_id: u16, policy: u16) -> Value {
    let mut value = flow(record_id, policy);
    value["evaluationId"] = Value::Null;
    value["attemptId"] = Value::Null;
    value
}

fn request(id: u16, policy: u16) -> Value {
    json!({
        "kind":"request","id":oid(id),"context":flow(id, policy),"actorId":oid(100),
        "gatewayRevisionId":oid(105),"operationClass":"deploy","actionDigest":digest(8),
        "contextDigest":digest(9),"resourceIds":[],"workflowRef":null,"expiresMs":null
    })
}

fn action_scope(system: u16, environment: u16, policy: u16) -> Value {
    json!({
        "systemId":oid(system),"environmentIds":[oid(environment)],"operationClasses":[],
        "resourceIds":[],"policyRevisionIds":[oid(policy)]
    })
}

fn gateway_revision(policy: u16, scope: Value) -> Value {
    json!({
        "kind":"gateway_revision","id":oid(105),"gatewayId":oid(105),"previousRevisionId":null,
        "policyRevisionId":oid(policy),"scope":scope,"humanRequired":true,
        "changedByActorId":oid(100),"state":"reported_active","evidenceIds":[]
    })
}

fn assignment(policy: u16) -> Value {
    json!({
        "kind":"assignment","id":oid(106),"actorId":oid(100),"gatewayRevisionId":oid(105),
        "policyRevisionId":oid(policy),"delegationId":null,"state":"reported_active",
        "changedByActorId":oid(100),"evidenceIds":[]
    })
}

fn coverage(system: u16, environment: u16, engine: u16) -> Value {
    json!({
        "kind":"coverage","id":oid(118),"systemId":oid(system),"engineId":oid(engine),
        "gatewayRevisionId":oid(105),"operationClass":"deploy","environmentId":oid(environment),
        "targetOs":"linux","observedMs":null,"freshness":"unknown","targetResourceIds":[],
        "adapterArtifactDigest":digest(13),"engineArtifactDigest":digest(14),"reportedLevel":"unknown",
        "checks":{"policyDecision":"unknown","approvalDuties":"unknown","agentDelegation":"unknown","expiryRevocation":"unknown","executionConsumption":"unknown","replayProtection":"unknown","outcomeEvidence":"unknown","reconciliation":"unknown"},
        "expiresMs":null,"evidenceIds":[]
    })
}

fn evidence(producer: u16) -> Value {
    json!({
        "kind":"evidence","id":oid(117),"producerId":oid(producer),"externalRef":null,
        "artifactDigest":null,"state":"unverified","reason":"none"
    })
}

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

fn document(value: &Value) -> Value {
    let decoded = decode_window(&bytes(value)).unwrap();
    let projection = project_window(&decoded).unwrap();
    serde_json::to_value(projection.document()).unwrap()
}

fn all_codes(document: &Value) -> Vec<String> {
    let mut codes = Vec::new();
    codes.extend(
        document["snapshotDiagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().to_owned()),
    );
    for field in ["timeline", "nodes", "edges"] {
        for entry in document[field].as_array().unwrap() {
            codes.extend(
                entry["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_str().unwrap().to_owned()),
            );
        }
    }
    codes
}

#[test]
fn empty_projection_has_manually_authored_golden_shape_and_fixed_trust() {
    let value = json!({
        "schemaVersion":"rangoon.ops.window.v1",
        "snapshot":{
            "snapshotId":"ops:1111111111111111111111111111111111111111111111111111111111111111",
            "reducerVersion":"rangoon.ops.reducer.v1",
            "scope":{"workspaceId":"ops:1111111111111111111111111111111111111111111111111111111111111111","systemIds":[],"environmentIds":[],"engineIds":[]},
            "topology":{"revisionId":"ops:1111111111111111111111111111111111111111111111111111111111111111","nodes":[],"edges":[],"policyRevisionIds":[],"coverageRevisionIds":[]},
            "prefixes":[],"asOfMs":0
        },"tail":[]
    });
    let decoded = decode_window(&bytes(&value)).unwrap();
    let projection = project_window(&decoded).unwrap();
    let expected = concat!(
        r#"{"schemaVersion":"rangoon.ops.projection.v1","inventory":{"schemaVersion":"rangoon.ops.inventory.v1","canonicalDigest":"8ab43d0754e994dc00445661f74d1da62b6efec06f7c6276bee1b9e1fc25c0a7","canonicalBytes":527,"producerCount":0,"eventCount":0,"topologyNodeCount":0,"topologyEdgeCount":0,"recordCounts":{"actor":0,"approval_decision":0,"approval_plan":0,"assignment":0,"attempt":0,"authorization":0,"coverage":0,"delegation":0,"engine":0,"evaluation":0,"evidence":0,"gateway_revision":0,"metric":0,"policy_revision":0,"request":0,"resource":0,"suspension":0,"tool":0,"workflow":0},"authority":"none","authenticity":"unverified","execution":"unavailable"},"replay":{"canonicalDigest":"8ab43d0754e994dc00445661f74d1da62b6efec06f7c6276bee1b9e1fc25c0a7","snapshotId":"ops:1111111111111111111111111111111111111111111111111111111111111111","reducerVersion":"rangoon.ops.reducer.v1","asOfMs":0,"scope":{"workspaceId":"ops:1111111111111111111111111111111111111111111111111111111111111111","systemIds":[],"environmentIds":[],"engineIds":[]},"topologyRevisionId":"ops:1111111111111111111111111111111111111111111111111111111111111111","policyRevisionIds":[],"coverageRevisionIds":[],"prefixes":[]},"authority":"none","authenticity":"unverified","execution":"unavailable","semanticAssessment":"unavailable","authoritativeEligibility":"unknown","authoritativeEffectiveEvaluationId":null,"structuralState":"structural_unverified","snapshotDiagnostics":[],"diagnosticCounts":{"duplicate_observation":0,"event_identity_conflict":0,"sequence_conflict":0,"record_identity_conflict":0,"sequence_gap":0,"predecessor_missing":0,"predecessor_conflict":0,"prefix_fence_mismatch":0,"prefix_watermark_mismatch":0,"prefix_identity_conflict":0,"scope_mismatch":0,"correlation_mismatch":0,"duplicate_node":0,"node_record_missing":0,"node_record_kind_mismatch":0,"duplicate_edge":0,"edge_endpoint_missing":0,"edge_endpoint_ambiguous":0},"timeline":[],"nodes":[],"edges":[]}"#
    );
    assert_eq!(
        rangoon_domain::byte_digest(expected.as_bytes()),
        "5c8986e86e5cc7e23d6acfeed340d70cce710e8a7e71b117058c02296e928bba"
    );
    assert_eq!(projection.canonical_bytes(), expected.as_bytes());
    let document = serde_json::to_value(projection.document()).unwrap();
    assert_eq!(document["schemaVersion"], "rangoon.ops.projection.v1");
    assert_eq!(
        document["replay"]["canonicalDigest"],
        decoded.canonical_digest()
    );
    assert_eq!(document["authority"], "none");
    assert_eq!(document["authenticity"], "unverified");
    assert_eq!(document["execution"], "unavailable");
    assert_eq!(document["semanticAssessment"], "unavailable");
    assert_eq!(document["authoritativeEligibility"], "unknown");
    assert_eq!(document["authoritativeEffectiveEvaluationId"], Value::Null);
    assert_eq!(document["structuralState"], "structural_unverified");
    assert_eq!(document["timeline"], json!([]));
    assert_eq!(document["nodes"], json!([]));
    assert_eq!(document["edges"], json!([]));
    assert_eq!(document["diagnosticCounts"].as_object().unwrap().len(), 18);
}

#[test]
fn diagnostic_counts_serialize_in_the_contract_table_order() {
    let decoded = decode_window(&bytes(&base_window())).unwrap();
    let projection = project_window(&decoded).unwrap();
    let raw = std::str::from_utf8(projection.canonical_bytes()).unwrap();
    let codes = [
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
    let positions = codes
        .iter()
        .map(|code| raw.find(&format!(r#""{code}":"#)).unwrap())
        .collect::<Vec<_>>();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn timeline_keeps_locations_ordinals_and_sorted_stream_order() {
    let mut value = base_window();
    value["snapshot"]["prefixes"] = json!([
        {"producerId":oid(12),"fenceId":oid(22),"watermark":{"sequence":1,"eventId":oid(502)},"events":[event(1,12,22,502,Value::Null,actor(102,"unknown"),Value::Null)]},
        {"producerId":oid(11),"fenceId":oid(21),"watermark":{"sequence":1,"eventId":oid(501)},"events":[event(1,11,21,501,Value::Null,actor(101,"unknown"),Value::Null)]}
    ]);
    value["tail"] = json!([event(
        1,
        13,
        23,
        503,
        Value::Null,
        actor(103, "unknown"),
        Value::Null
    )]);
    let doc = document(&value);
    assert_eq!(doc["timeline"].as_array().unwrap().len(), 3);
    assert_eq!(doc["timeline"][0]["eventId"], oid(501));
    assert_eq!(doc["timeline"][1]["eventId"], oid(502));
    assert_eq!(doc["timeline"][2]["eventId"], oid(503));
    assert_eq!(doc["timeline"][0]["inputOrdinal"], 1);
    assert_eq!(
        doc["timeline"][0]["location"],
        json!({"kind":"prefix","prefixIndex":1,"eventIndex":0})
    );
    assert_eq!(doc["timeline"][2]["inputOrdinal"], 2);
    assert_eq!(
        doc["timeline"][2]["location"],
        json!({"kind":"tail","eventIndex":0})
    );
}

#[test]
fn graph_preserves_declared_order_and_matching_ordinals() {
    let mut value = base_window();
    value["tail"] = json!([
        event(
            1,
            11,
            21,
            501,
            Value::Null,
            actor(101, "unknown"),
            Value::Null
        ),
        event(
            2,
            11,
            21,
            502,
            json!(oid(501)),
            actor(102, "unknown"),
            Value::Null
        )
    ]);
    value["snapshot"]["topology"]["nodes"] = json!([
        {"id":oid(402),"kind":"actor","recordId":oid(102)},
        {"id":oid(401),"kind":"actor","recordId":oid(101)}
    ]);
    value["snapshot"]["topology"]["edges"] = json!([
        {"id":oid(501),"kind":"declared_dependency","from":oid(402),"to":oid(401)}
    ]);
    let doc = document(&value);
    assert_eq!(doc["nodes"][0]["id"], oid(402));
    assert_eq!(doc["nodes"][0]["inputNodeIndex"], 0);
    assert_eq!(doc["nodes"][0]["matchingEventOrdinals"], json!([1]));
    assert_eq!(doc["nodes"][1]["matchingEventOrdinals"], json!([0]));
    assert_eq!(doc["edges"][0]["inputEdgeIndex"], 0);
}

#[test]
fn stream_and_identity_diagnostics_cover_exact_contract_codes() {
    let mut duplicate = base_window();
    let same = event(
        1,
        11,
        21,
        501,
        Value::Null,
        actor(101, "unknown"),
        Value::Null,
    );
    duplicate["tail"] = json!([same.clone(), same]);
    let duplicate_doc = document(&duplicate);
    assert!(all_codes(&duplicate_doc).contains(&"duplicate_observation".to_owned()));
    assert_eq!(
        duplicate_doc["diagnosticCounts"]["duplicate_observation"],
        2
    );

    let mut identity = base_window();
    identity["tail"] = json!([
        event(
            1,
            11,
            21,
            501,
            Value::Null,
            actor(101, "unknown"),
            Value::Null
        ),
        event(
            2,
            11,
            21,
            501,
            json!(oid(501)),
            actor(101, "reported_stale"),
            Value::Null
        )
    ]);
    let identity_doc = document(&identity);
    let codes = all_codes(&identity_doc);
    for code in ["event_identity_conflict", "record_identity_conflict"] {
        assert!(codes.contains(&code.to_owned()), "{code}");
        assert!(identity_doc["diagnosticCounts"][code].as_u64().unwrap() > 0);
    }

    let mut sequence = base_window();
    sequence["tail"] = json!([
        event(
            1,
            11,
            21,
            501,
            Value::Null,
            actor(101, "unknown"),
            Value::Null
        ),
        event(
            1,
            11,
            21,
            502,
            Value::Null,
            actor(102, "unknown"),
            Value::Null
        ),
        event(
            2,
            11,
            21,
            504,
            Value::Null,
            actor(104, "unknown"),
            Value::Null
        ),
        event(
            4,
            11,
            21,
            503,
            json!(oid(502)),
            actor(103, "unknown"),
            Value::Null
        )
    ]);
    let sequence_doc = document(&sequence);
    for code in [
        "sequence_conflict",
        "sequence_gap",
        "predecessor_missing",
        "predecessor_conflict",
    ] {
        assert!(
            all_codes(&sequence_doc).contains(&code.to_owned()),
            "{code}"
        );
        assert!(sequence_doc["diagnosticCounts"][code].as_u64().unwrap() > 0);
    }
}

#[test]
fn wrong_and_cross_fence_predecessors_are_conflicts_without_event_loss() {
    let mut value = base_window();
    value["tail"] = json!([
        event(
            1,
            11,
            21,
            501,
            Value::Null,
            actor(101, "unknown"),
            Value::Null
        ),
        event(
            2,
            12,
            22,
            502,
            json!(oid(501)),
            actor(102, "unknown"),
            Value::Null
        )
    ]);
    let doc = document(&value);
    assert_eq!(doc["timeline"].as_array().unwrap().len(), 2);
    assert!(all_codes(&doc).contains(&"predecessor_conflict".to_owned()));
    assert_eq!(doc["structuralState"], "conflict");
}

#[test]
fn prefix_scope_correlation_and_trust_are_only_structural_observations() {
    let mut value = base_window();
    value["snapshot"]["prefixes"] = json!([
        {"producerId":oid(11),"fenceId":oid(21),"watermark":null,"events":[event(1,12,22,501,Value::Null,actor(101,"unknown"),Value::Null)]},
        {"producerId":oid(11),"fenceId":oid(21),"watermark":null,"events":[]}
    ]);
    value["tail"] = json!([event(
        1,
        13,
        23,
        502,
        Value::Null,
        request(108, 104),
        correlation(108, 104)
    )]);
    let doc = document(&value);
    for code in [
        "prefix_fence_mismatch",
        "prefix_watermark_mismatch",
        "prefix_identity_conflict",
    ] {
        assert!(all_codes(&doc).contains(&code.to_owned()), "{code}");
    }
    assert_eq!(doc["diagnosticCounts"]["prefix_fence_mismatch"], 1);
    assert_eq!(doc["diagnosticCounts"]["prefix_watermark_mismatch"], 2);
    assert_eq!(doc["diagnosticCounts"]["prefix_identity_conflict"], 2);

    let mut unequal = base_window();
    let mut wrong_correlation = correlation(108, 104);
    wrong_correlation["systemId"] = json!(oid(33));
    unequal["tail"] = json!([event(
        1,
        11,
        21,
        501,
        Value::Null,
        request(108, 104),
        wrong_correlation
    )]);
    let unequal_doc = document(&unequal);
    assert!(all_codes(&unequal_doc).contains(&"correlation_mismatch".to_owned()));
    assert!(!all_codes(&unequal_doc).contains(&"scope_mismatch".to_owned()));

    let mut out_of_scope = base_window();
    let mut context = flow(108, 140);
    context["environmentId"] = json!(oid(99));
    let mut request = request(108, 140);
    request["context"] = context.clone();
    out_of_scope["tail"] = json!([event(
        1,
        11,
        21,
        501,
        Value::Null,
        request,
        correlation(108, 140)
    )]);
    let scope_doc = document(&out_of_scope);
    assert!(all_codes(&scope_doc).contains(&"scope_mismatch".to_owned()));
}

#[test]
fn prefix_watermark_accepts_exact_greatest_event_and_flags_empty_or_tied_prefixes() {
    let mut good = base_window();
    good["snapshot"]["prefixes"] = json!([{
        "producerId":oid(11),"fenceId":oid(21),
        "watermark":{"sequence":1,"eventId":oid(501)},
        "events":[event(1,11,21,501,Value::Null,actor(101,"unknown"),Value::Null)]
    }]);
    let good_doc = document(&good);
    assert!(!all_codes(&good_doc).contains(&"prefix_watermark_mismatch".to_owned()));

    let mut empty = base_window();
    empty["snapshot"]["prefixes"] = json!([{
        "producerId":oid(11),"fenceId":oid(21),
        "watermark":{"sequence":1,"eventId":oid(501)},"events":[]
    }]);
    let empty_doc = document(&empty);
    assert_eq!(
        empty_doc["diagnosticCounts"]["prefix_watermark_mismatch"],
        1
    );

    let mut tied = base_window();
    tied["snapshot"]["prefixes"] = json!([{
        "producerId":oid(11),"fenceId":oid(21),
        "watermark":{"sequence":1,"eventId":oid(501)},
        "events":[
            event(1,11,21,501,Value::Null,actor(101,"unknown"),Value::Null),
            event(1,11,21,502,Value::Null,actor(102,"unknown"),Value::Null)
        ]
    }]);
    let tied_doc = document(&tied);
    assert_eq!(tied_doc["diagnosticCounts"]["prefix_watermark_mismatch"], 3);
}

#[test]
fn sequence_one_with_predecessor_is_a_conflict() {
    let mut value = base_window();
    value["tail"] = json!([event(
        1,
        11,
        21,
        501,
        json!(oid(500)),
        actor(101, "unknown"),
        Value::Null
    )]);
    let doc = document(&value);
    assert_eq!(doc["diagnosticCounts"]["predecessor_conflict"], 1);
}

#[test]
fn topology_diagnostics_retain_declared_nodes_and_edges() {
    let mut value = base_window();
    value["tail"] = json!([event(
        1,
        11,
        21,
        501,
        Value::Null,
        actor(101, "unknown"),
        Value::Null
    )]);
    value["snapshot"]["topology"]["nodes"] = json!([
        {"id":oid(401),"kind":"actor","recordId":oid(101)},
        {"id":oid(401),"kind":"workflow","recordId":oid(101)},
        {"id":oid(403),"kind":"actor","recordId":oid(999)}
    ]);
    value["snapshot"]["topology"]["edges"] = json!([
        {"id":oid(501),"kind":"declared_dependency","from":oid(401),"to":oid(999)},
        {"id":oid(501),"kind":"declared_dependency","from":oid(401),"to":oid(401)}
    ]);
    let doc = document(&value);
    for code in [
        "duplicate_node",
        "node_record_missing",
        "node_record_kind_mismatch",
        "duplicate_edge",
        "edge_endpoint_missing",
        "edge_endpoint_ambiguous",
    ] {
        assert!(all_codes(&doc).contains(&code.to_owned()), "{code}");
        assert!(doc["diagnosticCounts"][code].as_u64().unwrap() > 0);
    }
    assert_eq!(doc["nodes"].as_array().unwrap().len(), 3);
    assert_eq!(doc["edges"].as_array().unwrap().len(), 2);
    assert_eq!(doc["diagnosticCounts"]["duplicate_node"], 2);
    assert_eq!(doc["diagnosticCounts"]["node_record_missing"], 1);
    assert_eq!(doc["diagnosticCounts"]["node_record_kind_mismatch"], 1);
    assert_eq!(doc["diagnosticCounts"]["duplicate_edge"], 2);
    assert_eq!(doc["diagnosticCounts"]["edge_endpoint_missing"], 1);
    assert_eq!(doc["diagnosticCounts"]["edge_endpoint_ambiguous"], 2);
}

#[test]
fn late_arrival_creates_new_replay_without_mutating_old_projection() {
    let mut before = base_window();
    before["tail"] = json!([event(
        1,
        11,
        21,
        501,
        Value::Null,
        actor(101, "unknown"),
        Value::Null
    )]);
    let decoded_before = decode_window(&bytes(&before)).unwrap();
    let projection_before = project_window(&decoded_before).unwrap();
    let before_bytes = projection_before.canonical_bytes().to_vec();
    let before_digest = projection_before.canonical_digest().to_owned();

    let mut late = before.clone();
    late["tail"] = json!([
        event(
            1,
            11,
            21,
            501,
            Value::Null,
            actor(101, "unknown"),
            Value::Null
        ),
        event(
            2,
            11,
            21,
            502,
            json!(oid(501)),
            actor(102, "unknown"),
            Value::Null
        )
    ]);
    let decoded_late = decode_window(&bytes(&late)).unwrap();
    let projection_late = project_window(&decoded_late).unwrap();
    assert_ne!(projection_late.canonical_digest(), before_digest);
    assert_eq!(projection_before.canonical_bytes(), before_bytes);
    assert_eq!(projection_before.canonical_digest(), before_digest);
}

#[test]
fn retains_all_1024_bounded_events() {
    let mut value = base_window();
    let events = (0..1_024)
        .map(|index| {
            event(
                index + 1,
                11,
                21,
                1_000 + index as u16,
                if index == 0 {
                    Value::Null
                } else {
                    json!(oid(999 + index as u16))
                },
                actor(101, "unknown"),
                Value::Null,
            )
        })
        .collect::<Vec<_>>();
    value["tail"] = json!(events);
    let doc = document(&value);
    assert_eq!(doc["timeline"].as_array().unwrap().len(), 1_024);
    assert_eq!(doc["timeline"][0]["inputOrdinal"], 0);
    assert_eq!(doc["timeline"][1_023]["inputOrdinal"], 1_023);
}

#[test]
fn node_match_amplification_exceeding_one_mebibyte_fails_atomically() {
    let mut value = base_window();
    value["tail"] = json!(
        (0..1_024)
            .map(|index| event(
                index + 1,
                11,
                21,
                2_000 + index as u16,
                if index == 0 {
                    Value::Null
                } else {
                    json!(oid(1_999 + index as u16))
                },
                actor(101, "unknown"),
                Value::Null
            ))
            .collect::<Vec<_>>()
    );
    value["snapshot"]["topology"]["nodes"] = json!(
        (0..128)
            .map(|index| json!({"id":oid(4_000 + index),"kind":"actor","recordId":oid(101)}))
            .collect::<Vec<_>>()
    );
    let decoded = decode_window(&bytes(&value)).unwrap();
    let input = decoded.canonical_bytes().to_vec();
    let error = project_window(&decoded).unwrap_err();
    assert_eq!(error.to_string(), "projection_limit");
    assert_eq!(decoded.canonical_bytes(), input);
}

#[test]
fn reported_allow_approval_issue_verification_and_coverage_never_raise_trust() {
    let mut value = base_window();
    let mut evaluation_correlation = correlation(108, 104);
    evaluation_correlation["evaluationId"] = json!(oid(109));
    let approval_correlation = evaluation_correlation.clone();
    let authorization_correlation = evaluation_correlation.clone();
    let coverage_correlation = evaluation_correlation.clone();
    value["tail"] = json!([
        event(
            1,
            11,
            21,
            501,
            Value::Null,
            actor(100, "reported_current"),
            Value::Null
        ),
        event(
            2,
            11,
            21,
            502,
            json!(oid(501)),
            json!({
                "kind":"evaluation","id":oid(109),"context":flow(108,104),"evaluationKind":"initial",
                "gatewayRevisionId":oid(105),"evaluatorId":oid(100),"evaluatorArtifactDigest":digest(10),
                "inputSnapshotId":oid(1),"inputCanonicalization":"json-v1","inputDigest":digest(11),
                "contextSnapshotId":oid(1),"contextDigest":digest(12),"previousEvaluationId":null,
                "expectedEffectiveEvaluationId":null,"lineageFenceId":oid(21),"result":"allow",
                "expiresMs":null,"reason":"none","externalRef":null,"evidenceIds":[]
            }),
            evaluation_correlation
        ),
        event(
            3,
            11,
            21,
            503,
            json!(oid(502)),
            json!({
                "kind":"approval_decision","id":oid(111),"context":flow(108,104),"evaluationId":oid(109),
                "planId":oid(110),"stageId":oid(210),"actorId":oid(100),"delegationId":null,
                "decision":"approve","supersedesDecisionId":null,"reason":"none","evidenceIds":[]
            }),
            approval_correlation
        ),
        event(
            4,
            11,
            21,
            504,
            json!(oid(503)),
            json!({
                "kind":"authorization","id":oid(112),"context":flow(108,104),"evaluationId":oid(109),
                "planId":null,"decisionIds":[oid(111)],"previousObservationId":null,
                "state":"reported_issued","scope":{"systemId":oid(3),"environmentIds":[oid(4)],"operationClasses":[],"resourceIds":[],"policyRevisionIds":[oid(104)]},
                "expiresMs":null,"reason":"none","externalRef":null,"evidenceIds":[]
            }),
            authorization_correlation
        ),
        event(
            5,
            11,
            21,
            505,
            json!(oid(504)),
            json!({
                "kind":"coverage","id":oid(118),"systemId":oid(3),"engineId":oid(5),
                "gatewayRevisionId":oid(105),"operationClass":"deploy","environmentId":oid(4),
                "targetOs":"linux","observedMs":null,"freshness":"reported_current","targetResourceIds":[],
                "adapterArtifactDigest":digest(13),"engineArtifactDigest":digest(14),
                "reportedLevel":"fully_mediated",
                "checks":{"policyDecision":"yes","approvalDuties":"yes","agentDelegation":"yes","expiryRevocation":"yes","executionConsumption":"yes","replayProtection":"yes","outcomeEvidence":"yes","reconciliation":"yes"},
                "expiresMs":null,"evidenceIds":[]
            }),
            coverage_correlation
        )
    ]);
    let doc = document(&value);
    assert_eq!(doc["authority"], "none");
    assert_eq!(doc["authenticity"], "unverified");
    assert_eq!(doc["execution"], "unavailable");
    assert_eq!(doc["semanticAssessment"], "unavailable");
    assert_eq!(doc["authoritativeEligibility"], "unknown");
    assert_eq!(doc["authoritativeEffectiveEvaluationId"], Value::Null);
}

#[test]
fn direct_selector_matrix_flags_in_scope_disagreement_without_trust_inference() {
    let mut correlation = correlation(108, 104);
    correlation["evaluationId"] = json!(oid(109));
    let mut value = base_window();
    value["tail"] = json!([
        event(
            1,
            11,
            21,
            501,
            Value::Null,
            json!({
                "kind":"request","id":oid(109),"context":flow(108,104),"actorId":oid(100),
                "gatewayRevisionId":oid(105),"operationClass":"deploy","actionDigest":digest(8),
                "contextDigest":digest(9),"resourceIds":[],"workflowRef":null,"expiresMs":null
            }),
            correlation.clone()
        ),
        event(
            2,
            11,
            21,
            502,
            json!(oid(501)),
            json!({
                "kind":"gateway_revision","id":oid(105),"gatewayId":oid(105),"previousRevisionId":null,
                "policyRevisionId":oid(140),"scope":{"systemId":oid(33),"environmentIds":[oid(44)],"operationClasses":[],"resourceIds":[],"policyRevisionIds":[oid(140)]},
                "humanRequired":true,"changedByActorId":oid(100),"state":"reported_active","evidenceIds":[]
            }),
            correlation.clone()
        ),
        event(
            3,
            11,
            21,
            503,
            json!(oid(502)),
            json!({
                "kind":"assignment","id":oid(106),"actorId":oid(100),"gatewayRevisionId":oid(105),
                "policyRevisionId":oid(140),"delegationId":null,"state":"reported_active",
                "changedByActorId":oid(100),"evidenceIds":[]
            }),
            correlation.clone()
        ),
        event(
            4,
            11,
            21,
            504,
            json!(oid(503)),
            json!({
                "kind":"evidence","id":oid(117),"producerId":oid(12),"externalRef":null,
                "artifactDigest":null,"state":"unverified","reason":"none"
            }),
            correlation.clone()
        ),
        event(
            5,
            11,
            21,
            505,
            json!(oid(504)),
            json!({
                "kind":"coverage","id":oid(118),"systemId":oid(33),"engineId":oid(55),
                "gatewayRevisionId":oid(105),"operationClass":"deploy","environmentId":oid(44),
                "targetOs":"linux","observedMs":null,"freshness":"unknown","targetResourceIds":[],
                "adapterArtifactDigest":digest(13),"engineArtifactDigest":digest(14),"reportedLevel":"unknown",
                "checks":{"policyDecision":"unknown","approvalDuties":"unknown","agentDelegation":"unknown","expiryRevocation":"unknown","executionConsumption":"unknown","replayProtection":"unknown","outcomeEvidence":"unknown","reconciliation":"unknown"},
                "expiresMs":null,"evidenceIds":[]
            }),
            correlation
        )
    ]);
    let doc = document(&value);
    assert!(all_codes(&doc).contains(&"correlation_mismatch".to_owned()));
    assert_eq!(doc["authority"], "none");
}

#[test]
fn every_direct_selector_mismatch_isolated_from_scope_membership() {
    let correlation = correlation(108, 104);
    let cases = [
        ("coverage system", coverage(33, 4, 5)),
        ("coverage environment", coverage(3, 44, 5)),
        ("coverage engine", coverage(3, 4, 55)),
        (
            "gateway policy",
            gateway_revision(140, action_scope(3, 4, 104)),
        ),
        (
            "scope system",
            gateway_revision(104, action_scope(33, 4, 104)),
        ),
        (
            "scope environment",
            gateway_revision(104, action_scope(3, 44, 104)),
        ),
        (
            "scope policy",
            gateway_revision(104, action_scope(3, 4, 140)),
        ),
        ("assignment policy", assignment(140)),
        ("request identity", request(109, 104)),
        ("evidence producer", evidence(12)),
    ];
    for (name, record) in cases {
        let mut window = base_window();
        window["tail"] = json!([event(
            1,
            11,
            21,
            501,
            Value::Null,
            record,
            correlation.clone()
        )]);
        let doc = document(&window);
        let entry = &doc["timeline"][0]["diagnostics"];
        assert!(
            entry
                .as_array()
                .unwrap()
                .iter()
                .any(|code| code == "correlation_mismatch"),
            "{name}"
        );
        assert!(
            !entry
                .as_array()
                .unwrap()
                .iter()
                .any(|code| code == "scope_mismatch"),
            "{name}"
        );
    }
}
