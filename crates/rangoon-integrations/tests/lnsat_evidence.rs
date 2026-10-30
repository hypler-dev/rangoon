use rangoon_integrations::lnsat_evidence::{
    LNSAT_EVIDENCE_COMMIT, LNSAT_EVIDENCE_CONTRACT, LnsatEvidenceError, LnsatEvidenceInput,
    MAX_LNSAT_EVIDENCE_BYTES, decode_lnsat_evidence,
};
use serde_json::Value;

const FIXTURE: &str =
    include_str!("../../../fixtures/lnsat-evidence/phase8-public-main-f669181a.json");

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).expect("synthetic LNSAT fixture")
}

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("fixture JSON")
}

fn template<'a>(fixture: &'a Value, name: &str) -> &'a Value {
    &fixture["templates"][name]
}

fn apply_patch(value: &mut Value, patch: &Value) {
    if let (Some(value), Some(patch)) = (value.as_object_mut(), patch.as_object()) {
        for (key, patch_value) in patch {
            if let Some(existing) = value.get_mut(key) {
                apply_patch(existing, patch_value);
            } else {
                value.insert(key.clone(), patch_value.clone());
            }
        }
    } else {
        *value = patch.clone();
    }
}

fn case_body(
    fixture: &Value,
    input: &Value,
    response: &str,
    patch: &str,
    raw: &str,
    oversized: &str,
) -> Option<Vec<u8>> {
    if let Some(raw) = input.get(raw).and_then(Value::as_str) {
        return Some(raw.as_bytes().to_vec());
    }
    if let Some(length) = input.get(oversized).and_then(Value::as_u64) {
        return Some(vec![b' '; length as usize]);
    }
    let name = input.get(response).and_then(Value::as_str)?;
    let mut value = template(fixture, name).clone();
    if let Some(patch) = input.get(patch) {
        apply_patch(&mut value, patch);
    }
    Some(bytes(&value))
}

fn expected_text<'a>(expected: &'a Value, key: &str) -> Option<&'a str> {
    expected
        .get(key)
        .unwrap_or_else(|| panic!("fixture expected {key}"))
        .as_str()
}

fn expected_bool(expected: &Value, key: &str) -> bool {
    expected
        .get(key)
        .unwrap_or_else(|| panic!("fixture expected {key}"))
        .as_bool()
        .unwrap_or_else(|| panic!("fixture expected bool {key}"))
}

fn bounded_value_tree(depth: usize) -> String {
    if depth == 0 {
        "0".into()
    } else {
        format!(
            "[{}]",
            (0..16)
                .map(|_| bounded_value_tree(depth - 1))
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

fn input<'a>(
    authorization: Option<&'a [u8]>,
    operation: Option<&'a [u8]>,
    attempt: Option<&'a [u8]>,
    authorization_id: Option<&'a str>,
    operation_id: Option<&'a str>,
    attempt_id: Option<&'a str>,
) -> LnsatEvidenceInput<'a> {
    LnsatEvidenceInput {
        installation_id: "installation:fixture",
        environment: "offline:fixture",
        selected_authorization_id: authorization_id,
        selected_operation_id: operation_id,
        selected_operation_attempt_id: attempt_id,
        authorization,
        operation,
        attempt,
    }
}

#[test]
fn synthetic_fixture_is_pinned_and_secret_free() {
    let fixture = fixture();
    assert_eq!(fixture["fixtureKind"], "synthetic_pinned_lnsat_envelopes");
    assert_eq!(fixture["capturedNativeResponse"], false);
    assert_eq!(fixture["releaseResearchCommit"], LNSAT_EVIDENCE_COMMIT);
    assert_eq!(fixture["contract"], LNSAT_EVIDENCE_CONTRACT);
    assert!(FIXTURE.len() <= MAX_LNSAT_EVIDENCE_BYTES);
}

#[test]
fn replays_fixture_cases_without_provider_access() {
    let fixture = fixture();
    for case in fixture["cases"].as_array().expect("fixture cases") {
        let values = &case["input"];
        let authorization = case_body(
            &fixture,
            values,
            "authorization",
            "authorizationPatch",
            "rawAuthorization",
            "oversizedAuthorizationBytes",
        );
        let operation = case_body(
            &fixture,
            values,
            "operation",
            "operationPatch",
            "rawOperation",
            "oversizedOperationBytes",
        );
        let attempt = case_body(
            &fixture,
            values,
            "attempt",
            "attemptPatch",
            "rawAttempt",
            "oversizedAttemptBytes",
        );
        let result = decode_lnsat_evidence(input(
            authorization.as_deref(),
            operation.as_deref(),
            attempt.as_deref(),
            values.get("authorizationId").and_then(Value::as_str),
            values.get("operationId").and_then(Value::as_str),
            values.get("attemptId").and_then(Value::as_str),
        ));
        let expected = &case["expected"];
        match expected_text(expected, "outcome") {
            Some("error") => assert_eq!(
                result.expect_err("fixture refusal").code(),
                expected_text(expected, "code").expect("fixture refusal code"),
                "{}",
                case["id"]
            ),
            Some("success") => {
                let output = result.unwrap_or_else(|error| panic!("{}: {error}", case["id"]));
                assert_eq!(
                    output.availability,
                    expected_text(expected, "availability").expect("availability expected"),
                    "{}",
                    case["id"]
                );
                assert_eq!(
                    output
                        .authorization
                        .as_ref()
                        .map(|value| value.native_state.as_str()),
                    expected_text(expected, "authorizationState"),
                    "{}",
                    case["id"]
                );
                assert_eq!(
                    output
                        .operation
                        .as_ref()
                        .map(|value| value.native_state.as_str()),
                    expected_text(expected, "operationState"),
                    "{}",
                    case["id"]
                );
                assert_eq!(
                    output
                        .attempt
                        .as_ref()
                        .map(|value| value.native_state.as_str()),
                    expected_text(expected, "attemptState"),
                    "{}",
                    case["id"]
                );
                assert_eq!(
                    output.attempt.as_ref().map(|value| value.state_projection),
                    expected_text(expected, "attemptProjection"),
                    "{}",
                    case["id"]
                );
                assert_eq!(
                    output
                        .operation
                        .as_ref()
                        .and_then(|value| value.receipt.as_ref())
                        .is_some(),
                    expected_bool(expected, "receipt"),
                    "{}",
                    case["id"]
                );
                assert_eq!(
                    output
                        .operation
                        .as_ref()
                        .and_then(|value| value.reconciliation.as_ref())
                        .is_some(),
                    expected_bool(expected, "reconciliation"),
                    "{}",
                    case["id"]
                );
                assert_eq!(
                    output.read_rejections.first().map(|value| value.state),
                    expected_text(expected, "readRejection"),
                    "{}",
                    case["id"]
                );
                assert_eq!(
                    output.cross_engine_join,
                    expected_text(expected, "crossEngineJoin").expect("cross engine expected"),
                    "{}",
                    case["id"]
                );
                assert_eq!(output.rangoon_authority, "none");
                assert_eq!(output.approval, "not_inferred");
                assert_eq!(output.execution, "not_inferred");
                assert_eq!(output.verified_effect, "not_observed");
                let mut expected_losses = vec![
                    "caller_declared_scope_unverified",
                    "source_authenticity_unverified",
                    "approval_identity_not_observed",
                    "approval_chain_not_observed",
                    "independent_effect_not_observed",
                    "denial_history_unsupported_by_selected_read_surface",
                    "cross_engine_join_not_observed",
                ];
                if expected_text(expected, "readRejection").is_some() {
                    expected_losses.push("read_rejection_cause_unclassified");
                }
                assert_eq!(output.losses, expected_losses, "{}", case["id"]);
                if let Some(authorization) = output.authorization {
                    assert_eq!(authorization.project_ref, "project:one");
                    assert_eq!(authorization.resource_ref, "resource:one");
                    assert_eq!(
                        authorization.packet_sha256,
                        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    );
                    assert_eq!(
                        authorization.binding_digest,
                        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    );
                    assert_eq!(
                        authorization.action_digest,
                        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                    );
                    assert_eq!(
                        authorization.target_digest,
                        "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
                    );
                    assert_eq!(
                        authorization.configuration_digest,
                        "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
                    );
                    assert_eq!(
                        authorization.omitted_fields,
                        [
                            "requester_ref",
                            "requester_session_ref",
                            "approver_ref",
                            "approver_session_ref"
                        ]
                    );
                    match authorization.native_state.as_str() {
                        "active" => assert_eq!(
                            (
                                authorization.operation_id.as_str(),
                                authorization.operation_audit_binding_id.as_str(),
                                authorization.operation_idempotency_key.as_str(),
                                authorization.operation_request_digest.as_str(),
                            ),
                            (
                                "opn_pending",
                                "audit:operation-pending",
                                "idempotency:pending",
                                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                            )
                        ),
                        "consumed" => assert_eq!(
                            (
                                authorization.operation_id.as_str(),
                                authorization.operation_audit_binding_id.as_str(),
                                authorization.operation_idempotency_key.as_str(),
                                authorization.operation_request_digest.as_str(),
                            ),
                            (
                                "opn_one",
                                "audit:operation",
                                "idempotency:one",
                                "sha256:2222222222222222222222222222222222222222222222222222222222222222",
                            )
                        ),
                        _ => panic!("fixture authorization state"),
                    }
                }
                if let Some(operation) = output.operation {
                    assert_eq!(operation.project_ref, "project:one");
                    assert_eq!(operation.resource_ref, "resource:one");
                    assert_eq!(operation.authorization_id, "xau_one");
                    if let Some(receipt) = operation.receipt {
                        assert_eq!(receipt.receipt_id, "receipt:one");
                        assert_eq!(receipt.received_at.as_deref(), Some("2026-10-01T00:02:00Z"));
                        assert_eq!(
                            receipt.effect,
                            "reported_receipt_not_independent_effect_proof"
                        );
                    }
                    if let Some(reconciliation) = operation.reconciliation {
                        assert_eq!(reconciliation.reconciliation_id, "reconciliation:one");
                        assert_eq!(reconciliation.status.as_deref(), Some("recorded"));
                        assert_eq!(
                            reconciliation.recorded_at.as_deref(),
                            Some("2026-10-01T00:03:00Z")
                        );
                        assert_eq!(
                            reconciliation.meaning,
                            "already_recorded_not_invoked_by_rangoon"
                        );
                    }
                }
                if let Some(attempt) = output.attempt {
                    assert_eq!(attempt.operation_id, "opn_one");
                    assert_eq!(attempt.project_ref, "project:one");
                    assert_eq!(attempt.resource_ref, "resource:one");
                    assert_eq!(
                        attempt.tool_arguments_digest,
                        "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                    );
                }
            }
            _ => panic!("fixture outcome"),
        }
    }
}

#[test]
fn selected_ids_are_checked_for_each_lone_native_envelope() {
    let fixture = fixture();
    let mut authorization = template(&fixture, "authorization").clone();
    authorization["authorization"]["authorization_id"] = Value::String("xau_other".into());
    assert_eq!(
        decode_lnsat_evidence(input(
            Some(&bytes(&authorization)),
            None,
            None,
            Some("xau_one"),
            None,
            None
        ))
        .unwrap_err(),
        LnsatEvidenceError::IdMismatch
    );

    let mut operation = template(&fixture, "operation").clone();
    operation["operation"]["operation_id"] = Value::String("opn_other".into());
    assert_eq!(
        decode_lnsat_evidence(input(
            None,
            Some(&bytes(&operation)),
            None,
            Some("xau_one"),
            Some("opn_one"),
            None
        ))
        .unwrap_err(),
        LnsatEvidenceError::IdMismatch
    );

    let mut attempt = template(&fixture, "attempt").clone();
    attempt["attempt"]["operation_attempt_id"] = Value::String("opa_other".into());
    assert_eq!(
        decode_lnsat_evidence(input(
            None,
            None,
            Some(&bytes(&attempt)),
            Some("xau_one"),
            Some("opn_one"),
            Some("opa_one")
        ))
        .unwrap_err(),
        LnsatEvidenceError::IdMismatch
    );
}

#[test]
fn refuses_cross_envelope_conflicts_and_preserves_unsupported_native_state() {
    let fixture = fixture();
    let mut authorization = template(&fixture, "authorization").clone();
    authorization["authorization"]["operation_id"] = Value::String("opn_one".into());
    authorization["authorization"]["operation_audit_binding_id"] =
        Value::String("audit:operation".into());
    let authorization = bytes(&authorization);
    let mut operation = template(&fixture, "operation").clone();
    operation["operation"]["project_ref"] = Value::String("project:other".into());
    assert_eq!(
        decode_lnsat_evidence(input(
            Some(&authorization),
            Some(&bytes(&operation)),
            None,
            Some("xau_one"),
            Some("opn_one"),
            None
        ))
        .unwrap_err(),
        LnsatEvidenceError::ScopeMismatch
    );

    let mut unknown = template(&fixture, "attempt").clone();
    unknown["attempt"]["state"] = Value::String("future_native_state".into());
    let unknown = bytes(&unknown);
    let output = decode_lnsat_evidence(input(
        None,
        None,
        Some(&unknown),
        Some("xau_one"),
        Some("opn_one"),
        Some("opa_one"),
    ))
    .expect("unknown native state remains observable");
    assert_eq!(
        output.attempt.expect("attempt").state_projection,
        "unsupported_native_state"
    );
}

#[test]
fn recognizes_pinned_native_states_without_inferring_authority() {
    let fixture = fixture();
    for state in ["active", "consumed", "cancelled", "expired", "revoked"] {
        let mut authorization = template(&fixture, "authorization").clone();
        authorization["authorization"]["state"] = Value::String(state.into());
        authorization["authorization"]["active"] = Value::Bool(state == "active");
        let encoded = bytes(&authorization);
        let output = decode_lnsat_evidence(input(
            Some(&encoded),
            None,
            None,
            Some("xau_one"),
            None,
            None,
        ))
        .expect("pinned authorization state remains observable");
        let observed = output.authorization.expect("authorization");
        assert_eq!(observed.native_state, state);
        assert_eq!(
            observed.state_projection, "reported_native_state",
            "{state}"
        );
        assert_eq!(output.rangoon_authority, "none");
        assert_eq!(output.approval, "not_inferred");
        assert_eq!(output.execution, "not_inferred");
    }
    for state in [
        "prepared",
        "dispatching",
        "outcome_unknown",
        "completed",
        "failed",
    ] {
        let mut operation = template(&fixture, "operation").clone();
        operation["operation"]["state"] = Value::String(state.into());
        if state == "prepared" {
            operation["operation"]["consumption_id"] = Value::Null;
        }
        let encoded = bytes(&operation);
        let output = decode_lnsat_evidence(input(
            None,
            Some(&encoded),
            None,
            Some("xau_one"),
            Some("opn_one"),
            None,
        ))
        .expect("pinned operation state remains observable");
        let observed = output.operation.expect("operation");
        assert_eq!(observed.native_state, state);
        assert_eq!(
            observed.state_projection, "reported_native_state",
            "{state}"
        );
        assert_eq!(output.rangoon_authority, "none");
        assert_eq!(output.approval, "not_inferred");
        assert_eq!(output.execution, "not_inferred");
    }
}

#[test]
fn authorization_and_operation_attempt_must_name_the_same_adapter() {
    let fixture = fixture();
    let mut authorization = template(&fixture, "authorization").clone();
    authorization["authorization"]["operation_id"] = Value::String("opn_one".into());
    authorization["authorization"]["operation_audit_binding_id"] =
        Value::String("audit:operation".into());
    let authorization = bytes(&authorization);
    for (with_operation, with_nested, with_standalone) in [
        (false, false, true),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        for matching_adapter in [true, false] {
            let mut attempt = template(&fixture, "attempt").clone();
            if !matching_adapter {
                attempt["attempt"]["adapter_ref"] = Value::String("adapter:other".into());
            }
            let mut operation = template(&fixture, "operation").clone();
            if with_nested {
                operation["operation"]["attempt"] = attempt["attempt"].clone();
            }
            let operation = with_operation.then(|| bytes(&operation));
            let attempt = with_standalone.then(|| bytes(&attempt));
            let decoded = decode_lnsat_evidence(input(
                Some(&authorization),
                operation.as_deref(),
                attempt.as_deref(),
                Some("xau_one"),
                Some("opn_one"),
                Some("opa_one"),
            ));
            if matching_adapter {
                let output = decoded.expect("matching adapter preserves the existing join");
                assert_eq!(output.attempt.expect("attempt").adapter_ref, "adapter:one");
                assert_eq!(output.rangoon_authority, "none");
                assert_eq!(output.execution, "not_inferred");
            } else {
                let error = decoded.expect_err("contradictory adapter must refuse atomically");
                assert_eq!(error, LnsatEvidenceError::ReferenceMismatch);
                assert_eq!(error.to_string(), "lnsat_evidence_reference_mismatch");
                assert!(!error.to_string().contains("adapter:other"));
            }
        }
    }
}

#[test]
fn non_native_state_labels_remain_explicitly_unsupported() {
    let fixture = fixture();
    let mut operation = template(&fixture, "operation").clone();
    operation["operation"]["state"] = Value::String("created".into());
    let encoded = bytes(&operation);
    let output = decode_lnsat_evidence(input(
        None,
        Some(&encoded),
        None,
        Some("xau_one"),
        Some("opn_one"),
        None,
    ))
    .expect("unrecognized state is retained without becoming supported");
    let observed = output.operation.expect("operation");
    assert_eq!(observed.native_state, "created");
    assert_eq!(observed.state_projection, "unsupported_native_state");
    assert_eq!(output.rangoon_authority, "none");
    assert_eq!(output.execution, "not_inferred");

    let mut authorization = template(&fixture, "authorization").clone();
    authorization["authorization"]["state"] = Value::String("issued".into());
    let encoded = bytes(&authorization);
    let output = decode_lnsat_evidence(input(
        Some(&encoded),
        None,
        None,
        Some("xau_one"),
        None,
        None,
    ))
    .expect("non-native authorization state is retained without becoming supported");
    let observed = output.authorization.expect("authorization");
    assert_eq!(observed.native_state, "issued");
    assert_eq!(observed.state_projection, "unsupported_native_state");
    assert_eq!(output.rangoon_authority, "none");
    assert_eq!(output.execution, "not_inferred");
}

#[test]
fn refuses_unmatched_authorization_operation_references_without_projection() {
    let fixture = fixture();
    let operation = bytes(template(&fixture, "operation"));
    let mut id_mismatch = template(&fixture, "authorization").clone();
    id_mismatch["authorization"]["operation_id"] = Value::String("opn_other".into());
    id_mismatch["authorization"]["operation_audit_binding_id"] =
        Value::String("audit:operation".into());
    let id_mismatch = bytes(&id_mismatch);
    let error = decode_lnsat_evidence(input(
        Some(&id_mismatch),
        Some(&operation),
        None,
        Some("xau_one"),
        Some("opn_one"),
        None,
    ))
    .expect_err("unmatched operation ID must refuse before projection");
    assert_eq!(error, LnsatEvidenceError::ReferenceMismatch);
    assert_eq!(error.to_string(), "lnsat_evidence_reference_mismatch");
    assert!(!error.to_string().contains("opn_other"));

    let mut audit_mismatch = template(&fixture, "authorization").clone();
    audit_mismatch["authorization"]["operation_id"] = Value::String("opn_one".into());
    audit_mismatch["authorization"]["operation_audit_binding_id"] =
        Value::String("audit:operation-other".into());
    let audit_mismatch = bytes(&audit_mismatch);
    assert_eq!(
        decode_lnsat_evidence(input(
            Some(&audit_mismatch),
            Some(&operation),
            None,
            Some("xau_one"),
            Some("opn_one"),
            None,
        ))
        .unwrap_err(),
        LnsatEvidenceError::ReferenceMismatch
    );

    let authorization = bytes(template(&fixture, "authorization"));
    let attempt = bytes(template(&fixture, "attempt"));
    assert_eq!(
        decode_lnsat_evidence(input(
            Some(&authorization),
            None,
            Some(&attempt),
            Some("xau_one"),
            Some("opn_one"),
            Some("opa_one"),
        ))
        .unwrap_err(),
        LnsatEvidenceError::ReferenceMismatch
    );

    let mut scope_mismatch = template(&fixture, "authorization").clone();
    scope_mismatch["authorization"]["operation_id"] = Value::String("opn_one".into());
    scope_mismatch["authorization"]["resource_ref"] = Value::String("resource:other".into());
    let scope_mismatch = bytes(&scope_mismatch);
    let error = decode_lnsat_evidence(input(
        Some(&scope_mismatch),
        None,
        Some(&attempt),
        Some("xau_one"),
        Some("opn_one"),
        Some("opa_one"),
    ))
    .expect_err("standalone attempt scope conflict must refuse before projection");
    assert_eq!(error, LnsatEvidenceError::ScopeMismatch);
    assert_eq!(error.to_string(), "lnsat_evidence_scope_mismatch");
    assert!(!error.to_string().contains("resource:other"));
}

#[test]
fn retains_bounded_utf8_native_source_values() {
    let fixture = fixture();
    let mut authorization = template(&fixture, "authorization").clone();
    authorization["authorization"]["audience"] = Value::String("監査 audience".into());
    let authorization = bytes(&authorization);
    let output = decode_lnsat_evidence(input(
        Some(&authorization),
        None,
        None,
        Some("xau_one"),
        None,
        None,
    ))
    .expect("bounded UTF-8 native field remains observable");
    assert_eq!(
        output.authorization.expect("authorization").audience,
        "監査 audience"
    );
}

#[test]
fn refuses_nested_attempt_disagreement_and_operation_authorization_mismatch() {
    let fixture = fixture();
    let mut operation = template(&fixture, "operation").clone();
    operation["operation"]["attempt"] = template(&fixture, "attempt")["attempt"].clone();
    let mut standalone = template(&fixture, "attempt").clone();
    standalone["attempt"]["attempt_sequence"] = Value::from(2_u64);
    assert_eq!(
        decode_lnsat_evidence(input(
            None,
            Some(&bytes(&operation)),
            Some(&bytes(&standalone)),
            Some("xau_one"),
            Some("opn_one"),
            Some("opa_one")
        ))
        .unwrap_err(),
        LnsatEvidenceError::AttemptMismatch
    );

    let mut mismatched = template(&fixture, "operation").clone();
    mismatched["operation"]["authorization_id"] = Value::String("xau_other".into());
    assert_eq!(
        decode_lnsat_evidence(input(
            None,
            Some(&bytes(&mismatched)),
            None,
            Some("xau_one"),
            Some("opn_one"),
            None
        ))
        .unwrap_err(),
        LnsatEvidenceError::IdMismatch
    );
}

#[test]
fn malformed_duplicate_unknown_and_oversized_input_refuse_without_output() {
    assert_eq!(
        decode_lnsat_evidence(input(
            Some(br#"{"contract":"x","contract":"x"}"#),
            None,
            None,
            Some("xau_one"),
            None,
            None
        ))
        .unwrap_err(),
        LnsatEvidenceError::DuplicateField
    );
    let oversized = vec![b' '; MAX_LNSAT_EVIDENCE_BYTES + 1];
    assert_eq!(
        decode_lnsat_evidence(input(
            Some(&oversized),
            None,
            None,
            Some("xau_one"),
            None,
            None
        ))
        .unwrap_err(),
        LnsatEvidenceError::InputLimit
    );
    let fixture = fixture();
    let mut unknown = template(&fixture, "attempt").clone();
    unknown["attempt"]["untrusted_extra"] = Value::Bool(true);
    assert_eq!(
        decode_lnsat_evidence(input(
            None,
            None,
            Some(&bytes(&unknown)),
            Some("xau_one"),
            Some("opn_one"),
            Some("opa_one")
        ))
        .unwrap_err(),
        LnsatEvidenceError::UnknownField
    );
}

#[test]
fn decoder_refuses_encoding_json_and_structural_limits() {
    assert_eq!(
        decode_lnsat_evidence(input(Some(b"{"), None, None, Some("xau_one"), None, None))
            .unwrap_err(),
        LnsatEvidenceError::InvalidJson
    );
    assert_eq!(
        decode_lnsat_evidence(input(
            Some(&[0xff]),
            None,
            None,
            Some("xau_one"),
            None,
            None
        ))
        .unwrap_err(),
        LnsatEvidenceError::InvalidEncoding
    );
    let deep = format!("{}0{}", "[".repeat(16), "]".repeat(16));
    assert_eq!(
        decode_lnsat_evidence(input(
            Some(deep.as_bytes()),
            None,
            None,
            Some("xau_one"),
            None,
            None
        ))
        .unwrap_err(),
        LnsatEvidenceError::DepthLimit
    );
    let values = bounded_value_tree(3);
    assert_eq!(
        decode_lnsat_evidence(input(
            Some(values.as_bytes()),
            None,
            None,
            Some("xau_one"),
            None,
            None
        ))
        .unwrap_err(),
        LnsatEvidenceError::ValueLimit
    );
    let fields = format!(
        "{{{}}}",
        (0..49)
            .map(|index| format!("\"field{index}\":0"))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(
        decode_lnsat_evidence(input(
            Some(fields.as_bytes()),
            None,
            None,
            Some("xau_one"),
            None,
            None
        ))
        .unwrap_err(),
        LnsatEvidenceError::CollectionLimit
    );
    let items = format!("[{}]", vec!["0"; 129].join(","));
    assert_eq!(
        decode_lnsat_evidence(input(
            Some(items.as_bytes()),
            None,
            None,
            Some("xau_one"),
            None,
            None
        ))
        .unwrap_err(),
        LnsatEvidenceError::CollectionLimit
    );
    let long_string = format!("\"{}\"", "x".repeat(4_097));
    assert_eq!(
        decode_lnsat_evidence(input(
            Some(long_string.as_bytes()),
            None,
            None,
            Some("xau_one"),
            None,
            None
        ))
        .unwrap_err(),
        LnsatEvidenceError::ValueLimit
    );
}

#[test]
fn decoder_refuses_non_native_digests_and_out_of_range_sequences() {
    let fixture = fixture();
    let mut authorization = template(&fixture, "authorization").clone();
    authorization["authorization"]["packet_sha256"] = Value::String("b".repeat(64));
    let authorization = bytes(&authorization);
    assert_eq!(
        decode_lnsat_evidence(input(
            Some(&authorization),
            None,
            None,
            Some("xau_one"),
            None,
            None
        ))
        .unwrap_err(),
        LnsatEvidenceError::InvalidEnvelope
    );
    let mut uppercase = template(&fixture, "attempt").clone();
    uppercase["attempt"]["tool_arguments_digest"] = Value::String(
        "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into(),
    );
    let uppercase = bytes(&uppercase);
    assert_eq!(
        decode_lnsat_evidence(input(
            None,
            None,
            Some(&uppercase),
            None,
            Some("opn_one"),
            Some("opa_one")
        ))
        .unwrap_err(),
        LnsatEvidenceError::InvalidEnvelope
    );
    let mut out_of_range = template(&fixture, "attempt").clone();
    out_of_range["attempt"]["attempt_sequence"] = Value::from(u64::from(u32::MAX) + 1);
    let out_of_range = bytes(&out_of_range);
    assert_eq!(
        decode_lnsat_evidence(input(
            None,
            None,
            Some(&out_of_range),
            None,
            Some("opn_one"),
            Some("opa_one")
        ))
        .unwrap_err(),
        LnsatEvidenceError::InvalidEnvelope
    );
    let mut state_out_of_range = template(&fixture, "attempt").clone();
    state_out_of_range["attempt"]["state_sequence"] = Value::from(u64::from(u32::MAX) + 1);
    let state_out_of_range = bytes(&state_out_of_range);
    assert_eq!(
        decode_lnsat_evidence(input(
            None,
            None,
            Some(&state_out_of_range),
            None,
            Some("opn_one"),
            Some("opa_one")
        ))
        .unwrap_err(),
        LnsatEvidenceError::InvalidEnvelope
    );
}

#[test]
fn redacted_identity_and_session_fields_are_validated_before_omission() {
    let fixture = fixture();
    let mut authorization = template(&fixture, "authorization").clone();
    authorization["authorization"]["requester_session_ref"] = Value::Null;
    let authorization = bytes(&authorization);
    let error = decode_lnsat_evidence(input(
        Some(&authorization),
        None,
        None,
        Some("xau_one"),
        None,
        None,
    ))
    .expect_err("non-string redacted native field must refuse before projection");
    assert_eq!(error, LnsatEvidenceError::InvalidEnvelope);
    assert_eq!(error.to_string(), "lnsat_evidence_invalid_envelope");
    assert!(!error.to_string().contains("requester_session_ref"));
}
