use super::{SourceReferenceProbeOutcome, probe_source_references};
use crate::decode_window;
use crate::source_ref_probe::is_opaque_token;
use serde_json::{Value, json};

const HOST_VECTORS: &str = include_str!("../../../docs/rusl-d0a-host-mapping-vectors.json");

fn case(id: &str) -> Value {
    let vectors: Value = serde_json::from_str(HOST_VECTORS).expect("host vectors parse");
    vectors["cases"]
        .as_array()
        .expect("cases array")
        .iter()
        .find(|case| case["id"] == id)
        .expect("named case")
        .clone()
}

fn decode_case(id: &str) -> crate::DecodedWindow {
    let source = case(id)["source"].clone();
    decode_window(&serde_json::to_vec(&source).expect("fixture serializes"))
        .expect("fixture decodes")
}

fn expected_outcome(result: &str) -> SourceReferenceProbeOutcome {
    match result {
        "candidate_unverified_source_shape" => {
            SourceReferenceProbeOutcome::CandidateUnverifiedSourceShape
        }
        "refuse_no_same_record_flow_context" => {
            SourceReferenceProbeOutcome::RefuseNoSameRecordFlowContext
        }
        "refuse_unsupported_record_kind" => {
            SourceReferenceProbeOutcome::RefuseUnsupportedRecordKind
        }
        "refuse_missing_external_reference" => {
            SourceReferenceProbeOutcome::RefuseMissingExternalReference
        }
        "refuse_system_mismatch" => SourceReferenceProbeOutcome::RefuseSystemMismatch,
        "refuse_invalid_reference" => SourceReferenceProbeOutcome::RefuseInvalidReference,
        other => panic!("unexpected expected result: {other}"),
    }
}

fn assert_case(id: &str) {
    let fixture = case(id);
    let decoded = decode_case(id);
    let canonical_before = decoded.canonical_bytes().to_vec();
    let probe = probe_source_references(&decoded).expect("probe succeeds");
    assert_eq!(decoded.canonical_bytes(), canonical_before, "{id}");
    let expected_rows = fixture["expected"]["probeRows"]
        .as_array()
        .expect("expected rows");

    assert_eq!(probe.rows().len(), expected_rows.len(), "{id}");
    assert_eq!(
        probe.rows().len(),
        decoded.inventory().event_count(),
        "{id}"
    );
    for (row, expected) in probe.rows().iter().zip(expected_rows) {
        assert_eq!(serde_json::to_value(row).unwrap(), expected.clone(), "{id}");
        assert_eq!(
            row.event_id(),
            expected["eventId"].as_str().unwrap(),
            "{id}"
        );
        assert_eq!(
            row.record_id(),
            expected["recordId"].as_str().unwrap(),
            "{id}"
        );
        assert_eq!(
            row.outcome(),
            expected_outcome(expected["result"].as_str().unwrap()),
            "{id}"
        );
        match (&expected["reference"], row.reference()) {
            (Value::Null, None) => {}
            (Value::Null, Some(_)) => panic!("{id}: unexpected reference"),
            (reference, Some(actual)) => {
                assert_eq!(
                    actual.system_id(),
                    reference["systemId"].as_str().unwrap(),
                    "{id}"
                );
                assert_eq!(
                    actual.environment_id(),
                    reference["environmentId"].as_str().unwrap(),
                    "{id}"
                );
                assert_eq!(
                    actual.engine_id(),
                    reference["engineId"].as_str().unwrap(),
                    "{id}"
                );
                assert_eq!(
                    actual.contract_version(),
                    reference["contractVersion"].as_str().unwrap(),
                    "{id}"
                );
                assert_eq!(
                    actual.native_id(),
                    reference["nativeId"].as_str().unwrap(),
                    "{id}"
                );
            }
            (_, None) => panic!("{id}: missing expected reference"),
        }
    }
}

#[test]
fn host_vectors_fix_candidate_and_refusal_rows() {
    for id in ["D0A-H01", "D0A-H02", "D0A-H04", "D0A-H05", "D0A-H07"] {
        assert_case(id);
    }
}

#[test]
fn h06_prefix_variant_preserves_candidate_without_a_d0a_observation() {
    let mut source = case("D0A-H01")["source"].clone();
    let event = source["tail"].as_array_mut().unwrap().remove(0);
    source["snapshot"]["prefixes"] = json!([{
        "producerId": event["producerId"],
        "fenceId": event["fenceId"],
        "watermark": null,
        "events": [event],
    }]);
    source["tail"] = json!([]);

    let decoded = decode_window(&serde_json::to_vec(&source).unwrap()).unwrap();
    let probe = probe_source_references(&decoded).unwrap();
    let vectors: Value = serde_json::from_str(HOST_VECTORS).unwrap();
    let expected = &vectors["derivedCases"][0]["expected"]["probeRows"];
    assert_eq!(
        serde_json::to_value(probe.rows()).unwrap(),
        expected.clone(),
        "H06 exact derived row"
    );
}

#[test]
fn empty_o1_window_has_no_rows() {
    let empty = br#"{"schemaVersion":"rangoon.ops.window.v1","snapshot":{"snapshotId":"ops:1111111111111111111111111111111111111111111111111111111111111111","reducerVersion":"rangoon.ops.reducer.v1","scope":{"workspaceId":"ops:1111111111111111111111111111111111111111111111111111111111111111","systemIds":[],"environmentIds":[],"engineIds":[]},"topology":{"revisionId":"ops:1111111111111111111111111111111111111111111111111111111111111111","nodes":[],"edges":[],"policyRevisionIds":[],"coverageRevisionIds":[]},"prefixes":[],"asOfMs":0},"tail":[]}"#;
    let decoded = decode_window(empty).unwrap();
    assert!(probe_source_references(&decoded).unwrap().rows().is_empty());
}

#[test]
fn non_evaluation_context_records_refuse_unsupported_and_tool_refuses_missing_context() {
    let mut source = case("D0A-H01")["source"].clone();
    let base = source["tail"][0].clone();

    let mut request = base.clone();
    request["record"] = json!({
        "kind":"request", "id":"ops:000000000000000000000000000000000000000000000000000000000000006e",
        "context":base["record"]["context"], "actorId":"ops:0000000000000000000000000000000000000000000000000000000000000064",
        "gatewayRevisionId":"ops:0000000000000000000000000000000000000000000000000000000000000069",
        "operationClass":"inspect", "actionDigest":"000000000000000000000000000000000000000000000000000000000000000b",
        "contextDigest":"000000000000000000000000000000000000000000000000000000000000000c",
        "resourceIds":[], "workflowRef":null, "expiresMs":null
    });
    let mut authorization = base.clone();
    authorization["eventId"] =
        json!("ops:000000000000000000000000000000000000000000000000000000000000012f");
    authorization["sequence"] = json!(2);
    authorization["previousEventId"] = request["eventId"].clone();
    authorization["record"] = json!({
        "kind":"authorization", "id":"ops:000000000000000000000000000000000000000000000000000000000000006f",
        "context":base["record"]["context"], "evaluationId":"ops:000000000000000000000000000000000000000000000000000000000000006d",
        "planId":null, "decisionIds":[], "previousObservationId":null, "state":"reported_issued",
        "scope":{"systemId":"ops:0000000000000000000000000000000000000000000000000000000000000003","environmentIds":[],"operationClasses":[],"resourceIds":[],"policyRevisionIds":[]},
        "expiresMs":null, "reason":"none",
        "externalRef":{"systemId":"ops:0000000000000000000000000000000000000000000000000000000000000003","contractVersion":"v1","externalId":"authorization-1"},
        "evidenceIds":[]
    });
    let mut tool = base;
    tool["eventId"] = json!("ops:0000000000000000000000000000000000000000000000000000000000000130");
    tool["sequence"] = json!(3);
    tool["previousEventId"] = authorization["eventId"].clone();
    tool["record"] = json!({
        "kind":"tool", "id":"ops:000000000000000000000000000000000000000000000000000000000000006f",
        "systemId":"ops:0000000000000000000000000000000000000000000000000000000000000003",
        "operationClass":"inspect", "externalRef":{"systemId":"ops:0000000000000000000000000000000000000000000000000000000000000003","contractVersion":"v1","externalId":"tool-1"}
    });
    request["eventId"] =
        json!("ops:000000000000000000000000000000000000000000000000000000000000012e");
    request["sequence"] = json!(1);
    request["previousEventId"] = Value::Null;
    source["tail"] = json!([request, authorization, tool]);

    let decoded = decode_window(&serde_json::to_vec(&source).unwrap()).unwrap();
    let rows = probe_source_references(&decoded).unwrap();
    assert_eq!(
        rows.rows()[0].outcome(),
        SourceReferenceProbeOutcome::RefuseUnsupportedRecordKind
    );
    assert_eq!(
        rows.rows()[1].outcome(),
        SourceReferenceProbeOutcome::RefuseUnsupportedRecordKind
    );
    assert_eq!(
        rows.rows()[2].outcome(),
        SourceReferenceProbeOutcome::RefuseNoSameRecordFlowContext
    );
}

#[test]
fn null_reference_times_and_correlation_cannot_fill_candidate_gaps() {
    let mut source = case("D0A-H01")["source"].clone();
    let event = &mut source["tail"][0];
    event["record"]["externalRef"] = Value::Null;
    event["observedMs"] = Value::Null;
    event["receivedMs"] = json!(9_007_199_254_740_991u64);
    let decoded = decode_window(&serde_json::to_vec(&source).unwrap()).unwrap();
    let probe = probe_source_references(&decoded).unwrap();
    let row = &probe.rows()[0];
    assert_eq!(
        row.outcome(),
        SourceReferenceProbeOutcome::RefuseMissingExternalReference
    );
    assert!(row.reference().is_none());
}

#[test]
fn null_and_large_times_do_not_change_a_candidate_reference() {
    for (observed, received) in [
        (Value::Null, Value::Null),
        (
            json!(9_007_199_254_740_991u64),
            json!(9_007_199_254_740_991u64),
        ),
    ] {
        let mut source = case("D0A-H01")["source"].clone();
        source["tail"][0]["observedMs"] = observed;
        source["tail"][0]["receivedMs"] = received;
        let decoded = decode_window(&serde_json::to_vec(&source).unwrap()).unwrap();
        let probe = probe_source_references(&decoded).unwrap();
        let row = &probe.rows()[0];
        assert_eq!(
            row.outcome(),
            SourceReferenceProbeOutcome::CandidateUnverifiedSourceShape
        );
        assert_eq!(row.reference().unwrap().native_id(), "decision-fixture-1");
    }
}

#[test]
fn token_checker_rejects_d0a_invalid_native_ids_without_normalization() {
    for invalid in ["", "has space", "a/b", "café", "x\n", &"a".repeat(129)] {
        assert!(!is_opaque_token(invalid), "{invalid:?}");
    }
    assert!(is_opaque_token("provider:decision-v1.2_3"));
}

#[test]
fn malformed_source_cannot_reach_probe() {
    let mut source = case("D0A-H01")["source"].clone();
    source["tail"][0]["record"]["externalRef"]["externalId"] = json!("not valid/path");
    assert!(decode_window(&serde_json::to_vec(&source).unwrap()).is_err());
}
