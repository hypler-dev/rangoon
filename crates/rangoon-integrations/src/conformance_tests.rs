use crate::{InspectionError, candidate_inventory, fixture_profile, inspect_fixture};
use rangoon_domain::byte_digest;
use serde_json::{Value, json};

const FIXTURE_SCHEMA: &str = "rangoon.integrations.fixture.v1";
const INSPECTION_SCHEMA: &str = "rangoon.integrations.inspection.v1";
const ADAPTER_DIGEST: &str = "a63ba7cb9f219dccb2635f3f75fd973ed2ea53062a908a1e8094ab9906ac76af";

fn profile() -> Value {
    json!({
        "instanceId": "offline-fixture",
        "providerFamily": "synthetic",
        "productTarget": "rangoon-offline-fixture",
        "productVersion": "1",
        "protocol": "rangoon.fixture.evidence.v1",
        "adapterId": "rangoon.offline-fixture",
        "adapterVersion": "1",
        "adapterDigest": ADAPTER_DIGEST,
    })
}

fn canonical_bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("fixture JSON serializes")
}

fn digest_json(value: &Value) -> String {
    byte_digest(&canonical_bytes(value))
}

fn digest_text(value: &str) -> String {
    byte_digest(value.as_bytes())
}

fn request(operation: &str, input: Value, configuration: Option<&str>) -> Value {
    let input_digest = digest_json(&input);
    let configuration_digest = configuration.map(digest_text);
    json!({
        "id": "request:fixture-1",
        "operation": operation,
        "input": input,
        "inputDigest": input_digest,
        "configuration": configuration,
        "configurationDigest": configuration_digest,
    })
}

struct EvidenceSpec<'a> {
    kind: Option<&'a str>,
    redaction: &'a str,
    payload: Value,
    original_digest: Option<&'a str>,
    reported_outcome: Option<&'a str>,
}

fn evidence<'a>(
    kind: Option<&'a str>,
    redaction: &'a str,
    payload: Value,
    original_digest: Option<&'a str>,
    reported_outcome: Option<&'a str>,
) -> EvidenceSpec<'a> {
    EvidenceSpec {
        kind,
        redaction,
        payload,
        original_digest,
        reported_outcome,
    }
}

fn observation(request: &Value, state: &str, reason: &str, evidence: EvidenceSpec<'_>) -> Value {
    let inspected = state == "inspected";
    let payload_digest = if inspected && evidence.redaction != "omitted" {
        Value::String(digest_json(&evidence.payload))
    } else {
        Value::Null
    };
    json!({
        "requestId": request["id"],
        "operation": request["operation"],
        "inputDigest": request["inputDigest"],
        "configurationDigest": request["configurationDigest"],
        "state": state,
        "reason": reason,
        "kind": evidence.kind,
        "reportedOutcome": evidence.reported_outcome,
        "evidenceReference": if inspected { json!("evidence:fixture-1") } else { Value::Null },
        "policyRevision": if inspected { json!("policy:1") } else { Value::Null },
        "traceId": if inspected { json!("trace:1") } else { Value::Null },
        "resourceReference": if inspected { json!("resource:fixture") } else { Value::Null },
        "observedProductVersion": if inspected { json!("1") } else { Value::Null },
        "observedProtocol": if inspected { json!("rangoon.fixture.evidence.v1") } else { Value::Null },
        "observedAt": if inspected { json!("reported:1") } else { Value::Null },
        "receivedAt": if inspected { json!("received:1") } else { Value::Null },
        "redaction": evidence.redaction,
        "originalDigest": evidence.original_digest,
        "payload": evidence.payload,
        "payloadDigest": payload_digest,
    })
}

fn envelope(request: Value, observation: Value) -> Value {
    json!({
        "schemaVersion": FIXTURE_SCHEMA,
        "profile": profile(),
        "request": request,
        "observation": observation,
    })
}

fn inspected_record(
    operation: &str,
    input: Value,
    configuration: Option<&str>,
    evidence: EvidenceSpec<'_>,
) -> Value {
    let request = request(operation, input, configuration);
    let observation = observation(&request, "inspected", "observation_only", evidence);
    envelope(request, observation)
}

fn non_inspected_record(operation: &str, state: &str, reason: &str) -> Value {
    let request = request(
        operation,
        if operation == "connection_check" {
            Value::Null
        } else {
            json!({"source": "offline"})
        },
        None,
    );
    let observation = observation(
        &request,
        state,
        reason,
        evidence(None, "omitted", Value::Null, None, None),
    );
    envelope(request, observation)
}

fn inspect(value: &Value) -> Result<crate::Inspection, InspectionError> {
    inspect_fixture(&canonical_bytes(value))
}

fn assert_error(bytes: &[u8], expected: &str) {
    let error = inspect_fixture(bytes).expect_err("fixture must fail");
    assert_eq!(error.code(), expected);
    assert_eq!(error.to_string(), expected);
}

fn assert_fixed_output(inspection: &crate::Inspection, validation: &str) {
    let output: Value =
        serde_json::from_slice(inspection.canonical_bytes()).expect("inspection emits JSON");
    assert_eq!(output["schemaVersion"], INSPECTION_SCHEMA);
    assert_eq!(output["source"], "offline_fixture");
    assert_eq!(output["authority"], "none");
    assert_eq!(output["execution"], "unavailable");
    assert_eq!(output["authenticity"], "unverified");
    assert_eq!(output["compatibility"], "unknown");
    assert_eq!(output["connection"], "unknown");
    assert_eq!(output["networkAttempted"], false);
    assert_eq!(output["configurationValidation"], validation);
    assert_eq!(inspection.state(), "inspected");
    assert_eq!(
        inspection.canonical_digest(),
        byte_digest(inspection.canonical_bytes())
    );
}

fn value_count(value: &Value) -> usize {
    match value {
        Value::Array(items) => 1 + items.iter().map(value_count).sum::<usize>(),
        Value::Object(fields) => 1 + fields.values().map(value_count).sum::<usize>(),
        _ => 1,
    }
}

#[test]
fn fixture_metadata_is_exact_and_candidate_inventory_stays_unavailable() {
    assert_eq!(
        fixture_profile(),
        json!({
            "instanceId": "offline-fixture",
            "providerFamily": "synthetic",
            "productTarget": "rangoon-offline-fixture",
            "productVersion": "1",
            "protocol": "rangoon.fixture.evidence.v1",
            "adapterId": "rangoon.offline-fixture",
            "adapterVersion": "1",
            "adapterDigest": ADAPTER_DIGEST,
        })
    );

    let inventory = candidate_inventory();
    let entries = inventory.as_array().expect("inventory array");
    assert_eq!(entries.len(), 8);
    let expected_targets = [
        "agentgateway standalone",
        "Cerbos PDP",
        "Permit authorization",
        "Permit FastMCP middleware",
        "Permit MCP Gateway",
        "Invariant local",
        "Invariant remote",
        "LNSAT",
    ];
    let expected_operations = [
        "configuration_preview",
        "connection_check",
        "evidence_read",
        "authorization_evaluation",
        "guardrail_evaluation",
    ];
    for (entry, target) in entries.iter().zip(expected_targets) {
        assert_eq!(entry.as_object().expect("candidate object").len(), 5);
        assert_eq!(entry["productTarget"], target);
        assert_eq!(entry["state"], "unavailable");
        assert_eq!(entry["reason"], "adapter_not_implemented");
        assert_eq!(entry["observedVersion"], Value::Null);
        let capabilities = entry["capabilities"]
            .as_array()
            .expect("capabilities array");
        assert_eq!(capabilities.len(), expected_operations.len());
        for (capability, operation) in capabilities.iter().zip(expected_operations) {
            assert_eq!(
                capability,
                &json!({"operation": operation, "state": "unavailable"})
            );
        }
    }
}

#[test]
fn configuration_preview_has_exact_canonical_output_and_digest() {
    let configuration = r#"{"rules":[{"effect":"allow","name":"sample"}]}"#;
    let fixture = inspected_record(
        "configuration_preview",
        json!({"resource": "demo", "action": "inspect"}),
        Some(configuration),
        evidence(
            Some("provider_configuration"),
            "none",
            serde_json::from_str(configuration).expect("literal configuration JSON"),
            None,
            None,
        ),
    );
    let inspection = inspect(&fixture).expect("valid configuration preview");
    assert_fixed_output(&inspection, "local_shape");

    const EXPECTED: &str = r#"{"authenticity":"unverified","authority":"none","compatibility":"unknown","configurationValidation":"local_shape","connection":"unknown","execution":"unavailable","networkAttempted":false,"observation":{"configurationDigest":"d9424af24eb13b8adb947f8129336e8808472837504ba2bf0df073a2b51359f3","evidenceReference":"evidence:fixture-1","inputDigest":"b0fda473bf8059230aac16d870e63eff140cbd1b804a2c4307e94fefc02fee6c","kind":"provider_configuration","observedAt":"reported:1","observedProductVersion":"1","observedProtocol":"rangoon.fixture.evidence.v1","operation":"configuration_preview","originalDigest":null,"payload":{"rules":[{"effect":"allow","name":"sample"}]},"payloadDigest":"d9424af24eb13b8adb947f8129336e8808472837504ba2bf0df073a2b51359f3","policyRevision":"policy:1","reason":"observation_only","receivedAt":"received:1","redaction":"none","reportedOutcome":null,"requestId":"request:fixture-1","resourceReference":"resource:fixture","state":"inspected","traceId":"trace:1"},"profile":{"adapterDigest":"a63ba7cb9f219dccb2635f3f75fd973ed2ea53062a908a1e8094ab9906ac76af","adapterId":"rangoon.offline-fixture","adapterVersion":"1","instanceId":"offline-fixture","productTarget":"rangoon-offline-fixture","productVersion":"1","protocol":"rangoon.fixture.evidence.v1","providerFamily":"synthetic"},"request":{"configuration":"{\"rules\":[{\"effect\":\"allow\",\"name\":\"sample\"}]}","configurationDigest":"d9424af24eb13b8adb947f8129336e8808472837504ba2bf0df073a2b51359f3","id":"request:fixture-1","input":{"action":"inspect","resource":"demo"},"inputDigest":"b0fda473bf8059230aac16d870e63eff140cbd1b804a2c4307e94fefc02fee6c","operation":"configuration_preview"},"schemaVersion":"rangoon.integrations.inspection.v1","source":"offline_fixture"}"#;
    const EXPECTED_DIGEST: &str =
        "ffe70526b0657bf28dd0c088b1667cbc1de1ae97f842deee897b025db92412ca";
    assert_eq!(
        std::str::from_utf8(inspection.canonical_bytes()).expect("canonical output UTF-8"),
        EXPECTED
    );
    assert_eq!(inspection.canonical_digest(), EXPECTED_DIGEST);
}

#[test]
fn object_reordering_preserves_canonical_identity() {
    let canonical = inspected_record(
        "evidence_read",
        json!({"a": 2, "z": 1}),
        None,
        evidence(
            Some("provider_decision"),
            "none",
            json!({"a": 2, "z": 1}),
            None,
            None,
        ),
    );
    let first = canonical_bytes(&canonical);
    let second = format!(
        r#"{{"observation":{},"request":{},"profile":{},"schemaVersion":"{}"}}"#,
        canonical["observation"], canonical["request"], canonical["profile"], FIXTURE_SCHEMA
    );
    let one = inspect_fixture(&first).expect("first input valid");
    let two = inspect_fixture(second.as_bytes()).expect("reordered input valid");
    assert_eq!(one.canonical_bytes(), two.canonical_bytes());
    assert_eq!(one.canonical_digest(), two.canonical_digest());
}

#[test]
fn five_operations_and_all_states_obey_closed_contract() {
    let configuration = r#"{"policy":{"effect":"allow"}}"#;
    let cases = [
        (
            "configuration_preview",
            json!({"target": "fixture"}),
            Some(configuration),
            "provider_configuration",
            serde_json::from_str(configuration).expect("configuration JSON"),
            "local_shape",
        ),
        (
            "evidence_read",
            json!({"target": "fixture"}),
            None,
            "local_content_review",
            json!({"summary": "offline"}),
            "unavailable",
        ),
        (
            "authorization_evaluation",
            json!({"action": "read"}),
            None,
            "provider_decision",
            json!({"effect": "allow"}),
            "unavailable",
        ),
        (
            "guardrail_evaluation",
            json!({"content": "fixture"}),
            None,
            "provider_guardrail_result",
            json!({"violations": []}),
            "unavailable",
        ),
    ];
    for (operation, input, configuration, kind, payload, validation) in cases {
        let record = inspected_record(
            operation,
            input,
            configuration,
            evidence(Some(kind), "none", payload, None, None),
        );
        let inspection = inspect(&record).expect("closed operation accepted");
        assert_fixed_output(&inspection, validation);
    }

    for (state, reason) in [
        ("unavailable", "adapter_unavailable"),
        ("unsupported", "operation_unsupported"),
        ("disconnected", "not_connected"),
        ("error", "inspection_failed"),
    ] {
        let record = non_inspected_record("connection_check", state, reason);
        let inspection = inspect(&record).expect("non-inspected connection state accepted");
        let output: Value =
            serde_json::from_slice(inspection.canonical_bytes()).expect("output JSON");
        assert_eq!(inspection.state(), state);
        assert_eq!(output["configurationValidation"], "unavailable");
        assert_eq!(output["networkAttempted"], false);
    }
}

#[test]
fn provenance_and_redaction_remain_explicit_reported_data() {
    let kinds = [
        "provider_configuration",
        "provider_decision",
        "provider_guardrail_result",
        "provider_approval",
        "provider_execution_outcome",
        "local_content_review",
    ];
    for kind in kinds {
        let record = inspected_record(
            "evidence_read",
            json!({"kind": kind}),
            None,
            evidence(
                Some(kind),
                "none",
                json!({"kind": kind}),
                None,
                Some("provider.reported.allow"),
            ),
        );
        assert_fixed_output(
            &inspect(&record).expect("evidence kind accepted"),
            "unavailable",
        );
    }

    let original = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    for redaction in ["redacted", "omitted"] {
        let payload = if redaction == "omitted" {
            Value::Null
        } else {
            json!({"visible": "redacted-copy"})
        };
        let record = inspected_record(
            "evidence_read",
            json!({"ref": redaction}),
            None,
            evidence(
                Some("provider_decision"),
                redaction,
                payload,
                Some(original),
                None,
            ),
        );
        assert_fixed_output(
            &inspect(&record).expect("redaction accepted"),
            "unavailable",
        );
    }
}

#[test]
fn reported_allow_approval_and_execution_never_create_authority() {
    for (kind, outcome) in [
        ("provider_decision", "allow"),
        ("provider_guardrail_result", "no_violations"),
        ("local_content_review", "reviewed"),
        ("provider_approval", "approved"),
        ("provider_execution_outcome", "execution_success"),
    ] {
        let record = inspected_record(
            "evidence_read",
            json!({"outcome": outcome}),
            None,
            evidence(
                Some(kind),
                "none",
                json!({"reported": outcome}),
                None,
                Some(outcome),
            ),
        );
        assert_fixed_output(
            &inspect(&record).expect("reported outcome accepted"),
            "unavailable",
        );
    }
}

#[test]
fn connection_check_is_source_free_and_never_inspected() {
    let accepted = non_inspected_record("connection_check", "disconnected", "not_connected");
    let inspection = inspect(&accepted).expect("source-free connection record accepted");
    let output: Value = serde_json::from_slice(inspection.canonical_bytes()).expect("output JSON");
    assert_eq!(output["networkAttempted"], false);
    assert_eq!(output["authority"], "none");

    let mut with_input = accepted.clone();
    with_input["request"]["input"] = json!({"destination": "https://forbidden.example"});
    with_input["request"]["inputDigest"] = json!(digest_json(&with_input["request"]["input"]));
    assert_error(&canonical_bytes(&with_input), "invalid_record");

    let mut inspected = accepted;
    inspected["observation"]["state"] = json!("inspected");
    inspected["observation"]["reason"] = json!("observation_only");
    inspected["observation"]["kind"] = json!("provider_decision");
    inspected["observation"]["redaction"] = json!("none");
    inspected["observation"]["payload"] = json!({"result": "reachable"});
    inspected["observation"]["payloadDigest"] =
        json!(digest_json(&inspected["observation"]["payload"]));
    assert_error(&canonical_bytes(&inspected), "invalid_record");
}

#[test]
fn unknown_duplicate_and_unsupported_profiles_fail_without_fallback() {
    let valid = inspected_record(
        "evidence_read",
        json!({"source": "fixture"}),
        None,
        evidence(
            Some("provider_decision"),
            "none",
            json!({"effect": "allow"}),
            None,
            None,
        ),
    );
    let mut unknown = valid.clone();
    unknown["request"]["unexpected"] = json!(true);
    assert_error(&canonical_bytes(&unknown), "invalid_record");

    let mut unavailable_profile = valid;
    unavailable_profile["profile"]["productTarget"] = json!("Cerbos PDP");
    assert_error(
        &canonical_bytes(&unavailable_profile),
        "unsupported_profile",
    );

    let mut unknown_operation = inspected_record(
        "evidence_read",
        json!({"source": "fixture"}),
        None,
        evidence(
            Some("provider_decision"),
            "none",
            json!({"effect": "allow"}),
            None,
            None,
        ),
    );
    unknown_operation["request"]["operation"] = json!("provider_fallback");
    unknown_operation["observation"]["operation"] = json!("provider_fallback");
    assert_error(&canonical_bytes(&unknown_operation), "invalid_record");

    let duplicate_escaped = b"{\"schemaVersion\":\"rangoon.integrations.fixture.v1\",\"profile\":{},\"request\":{\"id\":\"a\",\"\\u0069d\":\"b\"},\"observation\":{}}";
    assert_error(duplicate_escaped, "duplicate_field");

    let duplicate_source = inspected_record(
        "evidence_read",
        json!({"source": "fixture"}),
        None,
        evidence(
            Some("provider_decision"),
            "none",
            json!({"effect": "allow"}),
            None,
            None,
        ),
    );
    let duplicate_input = String::from_utf8(canonical_bytes(&duplicate_source))
        .expect("fixture bytes UTF-8")
        .replace(
            r#""input":{"source":"fixture"}"#,
            r#""input":{"source":"first","\u0073ource":"second"}"#,
        );
    assert_error(duplicate_input.as_bytes(), "duplicate_field");
    let duplicate_payload = String::from_utf8(canonical_bytes(&duplicate_source))
        .expect("fixture bytes UTF-8")
        .replace(
            r#""payload":{"effect":"allow"}"#,
            r#""payload":{"effect":"first","\u0065ffect":"second"}"#,
        );
    assert_error(duplicate_payload.as_bytes(), "duplicate_field");
}

#[test]
fn malformed_encoding_json_and_numbers_use_fixed_errors() {
    assert_error(&[0xff], "invalid_encoding");
    assert_error(b"{\"schemaVersion\":", "invalid_json");

    let mut decimal =
        non_inspected_record("connection_check", "unavailable", "adapter_unavailable");
    decimal["request"]["input"] = json!(1.5);
    decimal["request"]["inputDigest"] = json!(digest_json(&decimal["request"]["input"]));
    assert_error(&canonical_bytes(&decimal), "invalid_record");

    let exponent = b"{\"schemaVersion\":\"rangoon.integrations.fixture.v1\",\"profile\":{},\"request\":{\"id\":\"a\",\"operation\":\"evidence_read\",\"input\":1e3,\"inputDigest\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"configuration\":null,\"configurationDigest\":null},\"observation\":{}}";
    assert_error(exponent, "invalid_record");

    let out_of_range = b"{\"schemaVersion\":\"rangoon.integrations.fixture.v1\",\"profile\":{},\"request\":{\"id\":\"a\",\"operation\":\"evidence_read\",\"input\":9007199254740992,\"inputDigest\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"configuration\":null,\"configurationDigest\":null},\"observation\":{}}";
    assert_error(out_of_range, "invalid_record");
}

#[test]
fn pin_context_and_digest_drift_have_exact_codes() {
    let valid = inspected_record(
        "evidence_read",
        json!({"source": "fixture"}),
        None,
        evidence(
            Some("provider_decision"),
            "none",
            json!({"effect": "allow"}),
            None,
            None,
        ),
    );
    let mut context = valid.clone();
    context["observation"]["requestId"] = json!("request:other");
    assert_error(&canonical_bytes(&context), "context_mismatch");

    let mut compatibility = valid.clone();
    compatibility["observation"]["observedProductVersion"] = json!("2");
    assert_error(&canonical_bytes(&compatibility), "compatibility_mismatch");

    let mut payload_digest = valid.clone();
    payload_digest["observation"]["payloadDigest"] =
        json!("0000000000000000000000000000000000000000000000000000000000000000");
    assert_error(&canonical_bytes(&payload_digest), "digest_mismatch");

    let mut request_digest = valid;
    request_digest["request"]["inputDigest"] =
        json!("0000000000000000000000000000000000000000000000000000000000000000");
    request_digest["observation"]["inputDigest"] = request_digest["request"]["inputDigest"].clone();
    assert_error(&canonical_bytes(&request_digest), "digest_mismatch");
}

#[test]
fn configuration_uses_exact_bytes_and_nested_parser_contract() {
    let configuration = "{\n  \"policy\": { \"effect\": \"allow\" }\n}";
    let payload = json!({"policy": {"effect": "allow"}});
    let record = inspected_record(
        "configuration_preview",
        json!({"selected": "one"}),
        Some(configuration),
        evidence(
            Some("provider_configuration"),
            "none",
            payload.clone(),
            None,
            None,
        ),
    );
    let inspection = inspect(&record).expect("exact configuration bytes valid");
    let output: Value = serde_json::from_slice(inspection.canonical_bytes()).expect("output JSON");
    assert_eq!(output["request"]["configuration"], configuration);
    assert_eq!(
        output["request"]["configurationDigest"],
        digest_text(configuration)
    );
    assert_ne!(
        output["request"]["configurationDigest"],
        digest_json(&payload)
    );

    let mut unequal = record.clone();
    unequal["observation"]["payload"] = json!({"policy": {"effect": "deny"}});
    unequal["observation"]["payloadDigest"] =
        json!(digest_json(&unequal["observation"]["payload"]));
    assert_error(&canonical_bytes(&unequal), "context_mismatch");

    let duplicate_configuration = r#"{"name":"one","\u006eame":"two"}"#;
    let duplicate_payload = json!({"name": "two"});
    let duplicate = inspected_record(
        "configuration_preview",
        json!({"selected": "duplicate"}),
        Some(duplicate_configuration),
        evidence(
            Some("provider_configuration"),
            "none",
            duplicate_payload,
            None,
            None,
        ),
    );
    assert_error(&canonical_bytes(&duplicate), "duplicate_field");
}

#[test]
fn contract_resource_limits_are_checked_before_partial_output() {
    assert_error(&vec![b' '; 262_145], "input_limit");

    let mut too_many_fields = inspected_record(
        "evidence_read",
        json!({"limits": "fields"}),
        None,
        evidence(Some("provider_decision"), "none", json!({}), None, None),
    );
    let fields = too_many_fields["observation"]["payload"]
        .as_object_mut()
        .expect("payload object");
    for index in 0..33 {
        fields.insert(format!("field{index}"), json!(index));
    }
    too_many_fields["observation"]["payloadDigest"] =
        json!(digest_json(&too_many_fields["observation"]["payload"]));
    assert_error(&canonical_bytes(&too_many_fields), "collection_limit");

    let mut too_many_items = inspected_record(
        "evidence_read",
        json!({"limits": "array"}),
        None,
        evidence(Some("provider_decision"), "none", json!([]), None, None),
    );
    too_many_items["observation"]["payload"] = json!((0..129).collect::<Vec<_>>());
    too_many_items["observation"]["payloadDigest"] =
        json!(digest_json(&too_many_items["observation"]["payload"]));
    assert_error(&canonical_bytes(&too_many_items), "collection_limit");

    let mut too_many_values = serde_json::Map::new();
    for object_index in 0..32 {
        too_many_values.insert(
            format!("object{object_index:02}"),
            json!((0..128).collect::<Vec<_>>()),
        );
    }
    let values_record = inspected_record(
        "evidence_read",
        json!({"limits": "values"}),
        None,
        evidence(
            Some("provider_decision"),
            "none",
            Value::Object(too_many_values),
            None,
            None,
        ),
    );
    assert_error(&canonical_bytes(&values_record), "value_limit");

    let mut deep = json!(0);
    for _ in 0..14 {
        deep = json!([deep]);
    }
    let depth_record = inspected_record(
        "evidence_read",
        json!({"limits": "depth"}),
        None,
        evidence(Some("provider_decision"), "none", deep, None, None),
    );
    assert_error(&canonical_bytes(&depth_record), "depth_limit");

    let mut long_id =
        non_inspected_record("connection_check", "unavailable", "adapter_unavailable");
    long_id["request"]["id"] = json!("a".repeat(129));
    long_id["observation"]["requestId"] = long_id["request"]["id"].clone();
    assert_error(&canonical_bytes(&long_id), "invalid_record");

    let mut byte_overflow = inspected_record(
        "evidence_read",
        Value::String(String::new()),
        None,
        evidence(
            Some("provider_decision"),
            "none",
            json!({"effect": "allow"}),
            None,
            None,
        ),
    );
    let fixed_bytes = canonical_bytes(&byte_overflow).len();
    byte_overflow["request"]["input"] = json!("a".repeat(262_144 - fixed_bytes));
    byte_overflow["request"]["inputDigest"] =
        json!(digest_json(&byte_overflow["request"]["input"]));
    byte_overflow["observation"]["inputDigest"] = byte_overflow["request"]["inputDigest"].clone();
    assert_eq!(canonical_bytes(&byte_overflow).len(), 262_144);
    assert_error(&canonical_bytes(&byte_overflow), "output_limit");

    let mut structural_overflow =
        non_inspected_record("evidence_read", "unavailable", "adapter_unavailable");
    let baseline_values = value_count(&structural_overflow);
    let replaced_input_values = value_count(&structural_overflow["request"]["input"]);
    let required_input_values = 4_096 - baseline_values + replaced_input_values;
    let mut scalar_values = required_input_values - 33;
    let mut arrays = Vec::new();
    for _ in 0..32 {
        let take = scalar_values.min(128);
        arrays.push(Value::Array(vec![Value::Null; take]));
        scalar_values -= take;
    }
    assert_eq!(scalar_values, 0);
    structural_overflow["request"]["input"] = Value::Array(arrays);
    structural_overflow["request"]["inputDigest"] =
        json!(digest_json(&structural_overflow["request"]["input"]));
    structural_overflow["observation"]["inputDigest"] =
        structural_overflow["request"]["inputDigest"].clone();
    assert_eq!(value_count(&structural_overflow), 4_096);
    assert_error(&canonical_bytes(&structural_overflow), "output_limit");
}
