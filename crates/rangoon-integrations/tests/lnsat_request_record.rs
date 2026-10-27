//! Bounded synthetic conformance groups S01--S36 for the request-record profile.
//! These fixtures are caller-owned bytes.  They never contact LNSAT or mutate state.

use rangoon_domain::byte_digest;
use rangoon_integrations::lnsat_request_record::{
    ReadUnavailable, RequestRecordBody, RequestRecordError, RequestRecordInput,
    inspect_request_record,
};
use serde_json::{Value, json};

const REQUEST: &str = "arq_0000000000000000000000000000000000000000000000000000000000000001";
const PROJECT: &str = "project:synthetic";
const RESOURCE: &str = "resource:synthetic";
const REQUESTER_SENTINEL: &str = "requester-private-sentinel";

fn id(prefix: &str, suffix: u8) -> String {
    format!("{prefix}{suffix:064x}")
}

fn digest(suffix: u8) -> String {
    format!("sha256:{suffix:064x}")
}

fn at(second: u8) -> String {
    format!("2026-10-26T12:00:{second:02}.000Z")
}

// Keep the digest recipe deliberately local to the tests.  The production decoder
// receives only the final commitment and must not supply this test oracle.
fn public_commitment(domain: &str, parts: Vec<Value>) -> String {
    let body = serde_json::to_vec(&Value::Array(parts)).expect("vector serializes");
    let mut message = Vec::with_capacity(domain.len() + 1 + body.len());
    message.extend_from_slice(domain.as_bytes());
    message.push(b'\n');
    message.extend_from_slice(&body);
    format!("sha256:{}", byte_digest(&message))
}

fn root() -> Value {
    json!({
        "request_id": REQUEST,
        "requester_ref": REQUESTER_SENTINEL,
        "project_ref": PROJECT,
        "resource_ref": RESOURCE,
        "packet_id": "packet:synthetic",
        "packet_sha256": digest(1),
        "action_digest": digest(2),
        "target_digest": digest(3),
        "configuration_digest": digest(4),
        "created_at": at(0),
        "evidence_digest": digest(5),
    })
}

fn policy(decision: &str) -> Value {
    json!({
        "decision_id": id("pol_", 1),
        "evaluated_at": at(0),
        "expires_at": at(59),
        "decision": decision,
        "requires_approval": if decision == "approval_required" { 1 } else { 0 },
    })
}

fn evaluation(status: &str, policy_id: Option<String>, reason: &str) -> Value {
    let r = root();
    let packet_id = r["packet_id"].clone();
    let packet_sha = r["packet_sha256"].clone();
    let digest = public_commitment(
        "lnsat.action_request.evaluation.v1",
        vec![
            json!(REQUEST),
            packet_id,
            packet_sha,
            json!(PROJECT),
            json!(RESOURCE),
            json!(status),
            policy_id.clone().map(Value::String).unwrap_or(Value::Null),
            json!(reason),
            json!(at(0)),
        ],
    );
    json!({
        "status": status,
        "policy_decision_id": policy_id,
        "reason_code": reason,
        "recorded_at": at(0),
        "evidence_digest": digest,
    })
}

fn observation(kind: &str, submitted: String, second: u8) -> Value {
    let proof = public_commitment(
        "lnsat.action_request.observation.v1",
        vec![
            json!(REQUEST),
            json!(kind),
            json!(submitted),
            json!(at(second)),
        ],
    );
    json!({
        "kind": kind,
        "submitted_digest": submitted,
        "observed_at": at(second),
        "evidence_digest": proof,
    })
}

fn evidence(decision: &str) -> Value {
    json!({
        "schema": "lnsat.action_request_linked_evidence.v1",
        "policy": policy(decision),
        "approvals": [],
        "approval_decisions": [],
        "authorization_attempts": [],
        "authorizations": [],
        "operations": [],
        "consumptions": [],
        "operation_attempts": [],
        "receipts": [],
        "reconciliations": [],
        "states": [],
    })
}

fn complete(decision: &str) -> Value {
    let policy_id = id("pol_", 1);
    json!({
        "schema": "lnsat.action_request_record.v1",
        "status": "complete",
        "request_id": REQUEST,
        "evaluation_state": "policy_recorded",
        "request": root(),
        "evaluation": evaluation("recorded", Some(policy_id), "policy_recorded"),
        "observations": [],
        "evidence": evidence(decision),
        "security_activity_touched": false,
    })
}

fn failed(reason: &str) -> Value {
    json!({
        "schema": "lnsat.action_request_record.v1",
        "status": "complete",
        "request_id": REQUEST,
        "evaluation_state": "evaluation_failed",
        "request": root(),
        "evaluation": evaluation("failed", None, reason),
        "observations": [],
        "evidence": null,
        "security_activity_touched": false,
    })
}

fn unknown() -> Value {
    json!({
        "schema": "lnsat.action_request_record.v1",
        "status": "complete",
        "request_id": REQUEST,
        "evaluation_state": "evaluation_unknown",
        "request": root(),
        "evaluation": null,
        "observations": [],
        "evidence": null,
        "security_activity_touched": false,
    })
}

fn degraded() -> Value {
    json!({
        "schema": "lnsat.action_request_record.v1",
        "status": "degraded",
        "request_id": REQUEST,
        "evaluation_state": "evaluation_unknown",
        "request": null,
        "evaluation": null,
        "observations": [],
        "evidence": null,
        "security_activity_touched": true,
    })
}

fn input(body: RequestRecordBody) -> RequestRecordInput {
    RequestRecordInput {
        installation_id: "synthetic-installation".into(),
        environment: "test".into(),
        selected_request_id: REQUEST.into(),
        expected_project_ref: PROJECT.into(),
        expected_resource_ref: RESOURCE.into(),
        body,
        read_at: Some(at(58)),
    }
}

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("fixture JSON serializes")
}

fn reverse_top_level_bytes(value: &Value) -> Vec<u8> {
    let members = value.as_object().expect("fixture object");
    let mut encoded = Vec::new();
    encoded.push(b'{');
    for (index, (key, member)) in members.iter().rev().enumerate() {
        if index > 0 {
            encoded.push(b',');
        }
        encoded.extend_from_slice(serde_json::to_string(key).unwrap().as_bytes());
        encoded.push(b':');
        encoded.extend_from_slice(serde_json::to_string(member).unwrap().as_bytes());
    }
    encoded.push(b'}');
    encoded
}

fn inspect(
    value: &Value,
) -> Result<rangoon_integrations::lnsat_request_record::RequestRecordInspection, RequestRecordError>
{
    inspect_request_record(input(RequestRecordBody::Body200(bytes(value))))
}

fn host(value: &Value) -> Value {
    let result = inspect(value).expect("valid synthetic record");
    serde_json::from_slice(result.host_json()).expect("decoder host JSON")
}

fn summary(value: &Value) -> Value {
    let result = inspect(value).expect("valid synthetic record");
    serde_json::from_slice(result.summary_json()).expect("decoder summary JSON")
}

fn err(value: &Value) -> RequestRecordError {
    match inspect(value) {
        Ok(_) => panic!("invalid synthetic record must be refused"),
        Err(error) => error,
    }
}

fn assert_error_code(error: RequestRecordError, expected: &str) {
    assert_eq!(error.code(), expected, "fixed non-echoing error code");
    let bytes = error.json_bytes();
    assert_eq!(
        bytes,
        reference_bytes(
            &json!({"schemaVersion":"rangoon.integrations.lnsat-request-error.v1","code":expected,"truth":reference_truth()})
        ),
        "exact independent error bytes"
    );
    key_order(&bytes, &["schemaVersion", "code", "truth"]);
    assert!(bytes.len() <= 1_024);
    let value: Value = serde_json::from_slice(&bytes).expect("fixed error JSON");
    assert_exact_keys(&value, &["schemaVersion", "code", "truth"]);
    assert_eq!(
        value["schemaVersion"],
        "rangoon.integrations.lnsat-request-error.v1"
    );
    assert_eq!(value["code"], expected);
    assert_truth(&value["truth"]);
}

fn key_order(bytes: &[u8], keys: &[&str]) {
    let text = std::str::from_utf8(bytes).expect("output is UTF-8");
    assert!(!text.starts_with('\u{feff}'));
    assert!(!text.ends_with('\n'));
    let mut previous = 0;
    for key in keys {
        let at = text
            .find(&format!("\"{key}\":"))
            .expect("required serialized key");
        assert!(at >= previous, "{key} must serialize after prior key");
        previous = at;
    }
}

fn assert_truth(value: &Value) {
    assert_eq!(value["authenticity"], "unverified");
    assert_eq!(value["authority"], "none");
    assert_eq!(value["execution"], "unavailable");
    assert_eq!(value["authoritativeOutcome"], "unknown");
    assert_eq!(value["independentEffect"], "not_observed");
    assert_eq!(value["currentness"], "unverified");
}

fn object_keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect()
}

fn assert_exact_keys(value: &Value, expected: &[&str]) {
    let mut actual = object_keys(value);
    let mut wanted = expected.to_vec();
    actual.sort_unstable();
    wanted.sort_unstable();
    assert_eq!(actual, wanted, "closed output object key set");
}

fn assert_omissions(value: &Value) {
    let rows = value.as_array().expect("omission array");
    assert_eq!(rows.len(), 2);
    assert_exact_keys(&rows[0], &["path", "reason", "disposition"]);
    assert_exact_keys(&rows[1], &["path", "reason", "disposition"]);
    assert_eq!(
        rows[0],
        json!({
            "path": "request.requester_ref",
            "reason": "privacy_projection",
            "disposition": "policy_omission_not_presence_claim",
        })
    );
    assert_eq!(
        rows[1],
        json!({
            "path": "evidence.approval_decisions[*].reason",
            "reason": "privacy_projection",
            "disposition": "policy_omission_not_presence_claim",
        })
    );
}

fn assert_raw_host_oracle(bytes: &[u8]) {
    key_order(
        bytes,
        &[
            "schemaVersion",
            "profileVersion",
            "source",
            "selection",
            "recordAvailability",
            "reportedEvaluation",
            "reportedPolicy",
            "securityActivityTouched",
            "inputDigest",
            "native",
            "readFailure",
            "truth",
            "omissions",
        ],
    );
    let host: Value = serde_json::from_slice(bytes).expect("host JSON parses");
    assert_exact_keys(
        &host,
        &[
            "schemaVersion",
            "profileVersion",
            "source",
            "selection",
            "recordAvailability",
            "reportedEvaluation",
            "reportedPolicy",
            "securityActivityTouched",
            "inputDigest",
            "native",
            "readFailure",
            "truth",
            "omissions",
        ],
    );
    assert_eq!(
        host["schemaVersion"],
        "rangoon.integrations.lnsat-request-record.v1"
    );
    assert_eq!(host["profileVersion"], 1);
    assert_truth(&host["truth"]);
    assert_omissions(&host["omissions"]);
}

fn assert_host_contract(
    result: &rangoon_integrations::lnsat_request_record::RequestRecordInspection,
) {
    key_order(
        result.host_json(),
        &[
            "schemaVersion",
            "profileVersion",
            "source",
            "selection",
            "recordAvailability",
            "reportedEvaluation",
            "reportedPolicy",
            "securityActivityTouched",
            "inputDigest",
            "native",
            "readFailure",
            "truth",
            "omissions",
        ],
    );
    key_order(
        result.summary_json(),
        &[
            "schemaVersion",
            "recordAvailability",
            "reportedEvaluation",
            "reportedPolicy",
            "readFailure",
            "securityActivityTouched",
            "familyCounts",
            "truth",
            "omissions",
        ],
    );
    assert_raw_host_oracle(result.host_json());
    let host: Value =
        serde_json::from_slice(result.host_json()).expect("host parses independently");
    let summary: Value =
        serde_json::from_slice(result.summary_json()).expect("summary parses independently");
    assert_exact_keys(
        &host,
        &[
            "schemaVersion",
            "profileVersion",
            "source",
            "selection",
            "recordAvailability",
            "reportedEvaluation",
            "reportedPolicy",
            "securityActivityTouched",
            "inputDigest",
            "native",
            "readFailure",
            "truth",
            "omissions",
        ],
    );
    assert_exact_keys(
        &summary,
        &[
            "schemaVersion",
            "recordAvailability",
            "reportedEvaluation",
            "reportedPolicy",
            "readFailure",
            "securityActivityTouched",
            "familyCounts",
            "truth",
            "omissions",
        ],
    );
    assert_exact_keys(
        &host["source"],
        &[
            "repository",
            "commit",
            "wireContract",
            "compatibility",
            "transportAuthentication",
        ],
    );
    assert_exact_keys(
        &host["selection"],
        &[
            "installationId",
            "environment",
            "requestId",
            "expectedProjectRef",
            "expectedResourceRef",
            "readAt",
        ],
    );
    assert_exact_keys(
        &host["truth"],
        &[
            "authenticity",
            "authority",
            "execution",
            "authoritativeOutcome",
            "independentEffect",
            "currentness",
        ],
    );
    assert_eq!(
        host["schemaVersion"],
        "rangoon.integrations.lnsat-request-record.v1"
    );
    assert_eq!(host["profileVersion"], 1);
    assert_eq!(host["source"]["repository"], "hypler-dev/LNSAT");
    assert_eq!(
        host["source"]["commit"],
        "35a41963ad698934c7ee00852c71355e329c34fa"
    );
    assert_eq!(
        host["source"]["wireContract"],
        "lnsat.action_request_record.v1"
    );
    assert_eq!(
        host["source"]["compatibility"],
        "source_pinned_not_runtime_qualified"
    );
    assert_eq!(host["source"]["transportAuthentication"], "unverified");
    assert_truth(&host["truth"]);
    assert_truth(&summary["truth"]);
    assert_eq!(host["omissions"], summary["omissions"]);
    assert_omissions(&host["omissions"]);
    assert!(result.host_json().len() <= 131_072);
    assert!(result.summary_json().len() <= 8_192);
}

fn state(
    event: u8,
    target_id: String,
    kind: &str,
    sequence: u8,
    prior: Option<(String, u8)>,
    name: &str,
    second: u8,
) -> Value {
    json!({
        "state_event_id": id("ste_", event),
        "target_entity_id": target_id,
        "target_entity_kind": kind,
        "state_sequence": sequence,
        "state": name,
        "prior_state_event_id": prior.as_ref().map(|p| p.0.clone()),
        "prior_state_sequence": prior.map(|p| p.1),
        "effective_at": at(second),
        "state_digest": digest(sequence.saturating_add(60)),
    })
}

fn add_minimal_execution(document: &mut Value, terminal: &str, reconcile: Option<&str>) {
    let policy_id = id("pol_", 1);
    let approval_id = id("apr_", 1);
    let decision_id = id("apd_", 1);
    let attempt_id = id("aat_", 1);
    let authorization_id = id("xau_", 1);
    let operation_id = id("opn_", 1);
    let consumption_id = id("cpc_", 1);
    let operation_attempt_id = id("opa_", 1);
    let receipt_id = id("rcp_", 1);
    let evidence = document["evidence"]
        .as_object_mut()
        .expect("recorded evidence");
    evidence.insert("policy".into(), policy("approval_required"));
    evidence.insert(
        "approvals".into(),
        json!([{
            "approval_request_id": approval_id,
            "policy_decision_id": policy_id,
            "status": "requested",
            "requested_at": at(1),
            "expires_at": at(59),
        }]),
    );
    evidence.insert(
        "approval_decisions".into(),
        json!([{
            "approval_decision_id": decision_id,
            "approval_request_id": approval_id,
            "policy_decision_id": policy_id,
            "decision": "approved",
            "reason": "approval.operator_approved",
            "decided_at": at(2),
            "expires_at": at(59),
            "approval_gate_satisfied": 1,
        }]),
    );
    evidence.insert(
        "authorization_attempts".into(),
        json!([{
            "authorization_attempt_id": attempt_id,
            "approval_decision_id": decision_id,
            "approval_request_id": approval_id,
            "policy_decision_id": policy_id,
            "result_status": "persistence_prepared",
            "requested_at": at(3),
            "expires_at": at(59),
        }]),
    );
    evidence.insert(
        "authorizations".into(),
        json!([{
            "authorization_id": authorization_id,
            "authorization_attempt_id": attempt_id,
            "approval_decision_id": decision_id,
            "policy_decision_id": policy_id,
            "issued_at": at(4),
            "expires_at": at(59),
        }]),
    );
    evidence.insert(
        "operations".into(),
        json!([{
            "operation_id": operation_id,
            "authorization_id": authorization_id,
            "created_at": at(5),
            "expires_at": at(59),
        }]),
    );
    evidence.insert(
        "consumptions".into(),
        json!([{
            "consumption_id": consumption_id,
            "authorization_id": authorization_id,
            "operation_id": operation_id,
            "consumed_at": at(5),
        }]),
    );
    evidence.insert(
        "operation_attempts".into(),
        json!([{
            "operation_attempt_id": operation_attempt_id,
            "operation_id": operation_id,
            "attempt_sequence": 1,
            "created_at": at(6),
        }]),
    );
    if terminal == "completed" {
        evidence.insert(
            "receipts".into(),
            json!([{
                "receipt_id": receipt_id,
                "operation_id": operation_id,
                "operation_attempt_id": operation_attempt_id,
                "authorization_id": authorization_id,
                "consumption_id": consumption_id,
                "verification_status": "accepted",
                "result_digest": digest(42),
                "received_at": at(8),
            }]),
        );
    }
    let mut states = vec![
        state(
            1,
            authorization_id.clone(),
            "execution_authorization",
            1,
            None,
            "active",
            4,
        ),
        state(
            2,
            authorization_id.clone(),
            "execution_authorization",
            2,
            Some((id("ste_", 1), 1)),
            "consumed",
            5,
        ),
        state(3, operation_id.clone(), "operation", 1, None, "prepared", 5),
        state(
            4,
            operation_id.clone(),
            "operation",
            2,
            Some((id("ste_", 3), 1)),
            "dispatching",
            6,
        ),
        state(
            5,
            operation_attempt_id.clone(),
            "operation_attempt",
            1,
            None,
            "dispatching",
            6,
        ),
    ];
    if terminal != "dispatching" {
        states.push(state(
            6,
            operation_id.clone(),
            "operation",
            3,
            Some((id("ste_", 4), 2)),
            terminal,
            7,
        ));
        states.push(state(
            7,
            operation_attempt_id.clone(),
            "operation_attempt",
            2,
            Some((id("ste_", 5), 1)),
            terminal,
            7,
        ));
    }
    evidence.insert("states".into(), Value::Array(states));
    if let Some(status) = reconcile {
        let (linked_receipt, observed) = match status {
            "pending" | "unresolved" => (Value::Null, Value::Null),
            "matched" => (json!(receipt_id), json!(digest(42))),
            "mismatch" => (json!(receipt_id), json!(digest(43))),
            _ => unreachable!("fixture status"),
        };
        evidence.insert(
            "reconciliations".into(),
            json!([{
                "reconciliation_id": id("rec_", 1),
                "operation_id": operation_id,
                "operation_attempt_id": operation_attempt_id,
                "receipt_id": linked_receipt,
                "status": status,
                "observed_result_digest": observed,
                "recorded_at": at(9),
            }]),
        );
    }
}

fn approval_rows(count: u8) -> Value {
    Value::Array(
        (0..count)
            .map(|n| {
                json!({
                    "approval_request_id": id("apr_", n.saturating_add(1)),
                    "policy_decision_id": id("pol_", 1),
                    "status": "requested",
                    "requested_at": at(1),
                    "expires_at": at(59),
                })
            })
            .collect(),
    )
}

const HOST_KEYS: &[&str] = &[
    "schemaVersion",
    "profileVersion",
    "source",
    "selection",
    "recordAvailability",
    "reportedEvaluation",
    "reportedPolicy",
    "securityActivityTouched",
    "inputDigest",
    "native",
    "readFailure",
    "truth",
    "omissions",
];
const SUMMARY_KEYS: &[&str] = &[
    "schemaVersion",
    "recordAvailability",
    "reportedEvaluation",
    "reportedPolicy",
    "readFailure",
    "securityActivityTouched",
    "familyCounts",
    "truth",
    "omissions",
];
const ERROR_KEYS: &[&str] = &["schemaVersion", "code", "truth"];
const SOURCE_KEYS: &[&str] = &[
    "repository",
    "commit",
    "wireContract",
    "compatibility",
    "transportAuthentication",
];
const SELECTION_KEYS: &[&str] = &[
    "installationId",
    "environment",
    "requestId",
    "expectedProjectRef",
    "expectedResourceRef",
    "readAt",
];
const TRUTH_KEYS: &[&str] = &[
    "authenticity",
    "authority",
    "execution",
    "authoritativeOutcome",
    "independentEffect",
    "currentness",
];
const OMISSION_KEYS: &[&str] = &["path", "reason", "disposition"];
const NATIVE_KEYS: &[&str] = &["request", "evaluation", "observations", "evidence"];
const REQUEST_KEYS: &[&str] = &[
    "request_id",
    "project_ref",
    "resource_ref",
    "packet_id",
    "packet_sha256",
    "action_digest",
    "target_digest",
    "configuration_digest",
    "created_at",
    "evidence_digest",
];
const EVALUATION_KEYS: &[&str] = &[
    "status",
    "policy_decision_id",
    "reason_code",
    "recorded_at",
    "evidence_digest",
];
const OBSERVATION_KEYS: &[&str] = &["kind", "submitted_digest", "observed_at", "evidence_digest"];
const EVIDENCE_KEYS: &[&str] = &[
    "schema",
    "policy",
    "approvals",
    "approval_decisions",
    "authorization_attempts",
    "authorizations",
    "operations",
    "consumptions",
    "operation_attempts",
    "receipts",
    "reconciliations",
    "states",
];
const POLICY_KEYS: &[&str] = &[
    "decision_id",
    "evaluated_at",
    "expires_at",
    "decision",
    "requires_approval",
];
const APPROVAL_KEYS: &[&str] = &[
    "approval_request_id",
    "policy_decision_id",
    "status",
    "requested_at",
    "expires_at",
];
const APPROVAL_DECISION_KEYS: &[&str] = &[
    "approval_decision_id",
    "approval_request_id",
    "policy_decision_id",
    "decision",
    "decided_at",
    "expires_at",
    "approval_gate_satisfied",
];
const AUTHORIZATION_ATTEMPT_KEYS: &[&str] = &[
    "authorization_attempt_id",
    "approval_decision_id",
    "approval_request_id",
    "policy_decision_id",
    "result_status",
    "requested_at",
    "expires_at",
];
const AUTHORIZATION_KEYS: &[&str] = &[
    "authorization_id",
    "authorization_attempt_id",
    "approval_decision_id",
    "policy_decision_id",
    "issued_at",
    "expires_at",
];
const OPERATION_KEYS: &[&str] = &[
    "operation_id",
    "authorization_id",
    "created_at",
    "expires_at",
];
const CONSUMPTION_KEYS: &[&str] = &[
    "consumption_id",
    "authorization_id",
    "operation_id",
    "consumed_at",
];
const OPERATION_ATTEMPT_KEYS: &[&str] = &[
    "operation_attempt_id",
    "operation_id",
    "attempt_sequence",
    "created_at",
];
const RECEIPT_KEYS: &[&str] = &[
    "receipt_id",
    "operation_id",
    "operation_attempt_id",
    "authorization_id",
    "consumption_id",
    "verification_status",
    "result_digest",
    "received_at",
];
const RECONCILIATION_KEYS: &[&str] = &[
    "reconciliation_id",
    "operation_id",
    "operation_attempt_id",
    "receipt_id",
    "status",
    "observed_result_digest",
    "recorded_at",
];
const STATE_KEYS: &[&str] = &[
    "state_event_id",
    "target_entity_id",
    "target_entity_kind",
    "state_sequence",
    "state",
    "prior_state_event_id",
    "prior_state_sequence",
    "effective_at",
    "state_digest",
];
const FAMILY_COUNT_KEYS: &[&str] = &[
    "approvals",
    "approval_decisions",
    "authorization_attempts",
    "authorizations",
    "operations",
    "consumptions",
    "operation_attempts",
    "receipts",
    "reconciliations",
    "states",
];

fn reference_object_order(object: &serde_json::Map<String, Value>) -> &'static [&'static str] {
    let has = |key: &str| object.contains_key(key);
    if has("profileVersion") {
        HOST_KEYS
    } else if has("familyCounts") {
        SUMMARY_KEYS
    } else if has("code") {
        ERROR_KEYS
    } else if has("repository") {
        SOURCE_KEYS
    } else if has("installationId") {
        SELECTION_KEYS
    } else if has("authenticity") {
        TRUTH_KEYS
    } else if has("disposition") {
        OMISSION_KEYS
    } else if has("request") && has("observations") {
        NATIVE_KEYS
    } else if has("packet_sha256") {
        REQUEST_KEYS
    } else if has("reason_code") {
        EVALUATION_KEYS
    } else if has("submitted_digest") {
        OBSERVATION_KEYS
    } else if has("schema") && has("policy") && has("approvals") && has("states") {
        EVIDENCE_KEYS
    } else if has("decision_id") {
        POLICY_KEYS
    } else if has("approval_gate_satisfied") {
        APPROVAL_DECISION_KEYS
    } else if has("approval_request_id") && has("status") {
        APPROVAL_KEYS
    } else if has("authorization_attempt_id") && has("result_status") {
        AUTHORIZATION_ATTEMPT_KEYS
    } else if has("authorization_id") && has("issued_at") {
        AUTHORIZATION_KEYS
    } else if has("consumption_id") && has("consumed_at") {
        CONSUMPTION_KEYS
    } else if has("operation_attempt_id") && has("attempt_sequence") {
        OPERATION_ATTEMPT_KEYS
    } else if has("receipt_id") && has("verification_status") {
        RECEIPT_KEYS
    } else if has("reconciliation_id") {
        RECONCILIATION_KEYS
    } else if has("state_event_id") {
        STATE_KEYS
    } else if has("operation_id") && has("expires_at") {
        OPERATION_KEYS
    } else if has("approvals") {
        FAMILY_COUNT_KEYS
    } else {
        panic!("reference serializer saw an unrecognized object shape");
    }
}

fn reference_write(value: &Value, output: &mut String) {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => output.push_str(&value.to_string()),
        Value::String(value) => output.push_str(&serde_json::to_string(value).expect("string")),
        Value::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                reference_write(value, output);
            }
            output.push(']');
        }
        Value::Object(object) => {
            let keys = reference_object_order(object);
            assert_eq!(
                object.len(),
                keys.len(),
                "reference object has no extra field"
            );
            output.push('{');
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&serde_json::to_string(key).expect("key"));
                output.push(':');
                reference_write(object.get(*key).expect("reference field"), output);
            }
            output.push('}');
        }
    }
}

fn reference_bytes(value: &Value) -> Vec<u8> {
    let mut output = String::new();
    reference_write(value, &mut output);
    output.into_bytes()
}

fn reference_truth() -> Value {
    json!({
        "authenticity": "unverified", "authority": "none", "execution": "unavailable",
        "authoritativeOutcome": "unknown", "independentEffect": "not_observed", "currentness": "unverified",
    })
}

fn reference_omissions() -> Value {
    json!([
        {"path":"request.requester_ref","reason":"privacy_projection","disposition":"policy_omission_not_presence_claim"},
        {"path":"evidence.approval_decisions[*].reason","reason":"privacy_projection","disposition":"policy_omission_not_presence_claim"},
    ])
}

fn redact_reference_row(value: &Value, field: &str) -> Value {
    let mut object = value.as_object().expect("fixture row").clone();
    object.remove(field);
    Value::Object(object)
}

fn reference_native(document: &Value) -> Value {
    let mut request = document["request"]
        .as_object()
        .expect("complete request")
        .clone();
    request.remove("requester_ref");
    let evidence = if document["evidence"].is_null() {
        Value::Null
    } else {
        let mut evidence = document["evidence"].as_object().expect("evidence").clone();
        evidence.insert(
            "approval_decisions".into(),
            Value::Array(
                document["evidence"]["approval_decisions"]
                    .as_array()
                    .expect("rows")
                    .iter()
                    .map(|row| redact_reference_row(row, "reason"))
                    .collect(),
            ),
        );
        Value::Object(evidence)
    };
    json!({
        "request": Value::Object(request),
        "evaluation": document["evaluation"].clone(),
        "observations": document["observations"].clone(),
        "evidence": evidence,
    })
}

fn reference_host(
    availability: &str,
    evaluation: &str,
    policy: &str,
    native: Option<Value>,
    security: Option<bool>,
    input_digest: Option<String>,
    failure: Option<&str>,
) -> Value {
    json!({
        "schemaVersion": "rangoon.integrations.lnsat-request-record.v1",
        "profileVersion": 1,
        "source": {"repository":"hypler-dev/LNSAT","commit":"35a41963ad698934c7ee00852c71355e329c34fa","wireContract":"lnsat.action_request_record.v1","compatibility":"source_pinned_not_runtime_qualified","transportAuthentication":"unverified"},
        "selection": {"installationId":"synthetic-installation","environment":"test","requestId":REQUEST,"expectedProjectRef":PROJECT,"expectedResourceRef":RESOURCE,"readAt":at(58)},
        "recordAvailability": availability,
        "reportedEvaluation": evaluation,
        "reportedPolicy": policy,
        "securityActivityTouched": security,
        "inputDigest": input_digest,
        "native": native,
        "readFailure": failure,
        "truth": reference_truth(),
        "omissions": reference_omissions(),
    })
}

fn reference_summary(host: &Value) -> Value {
    let family_counts = host["native"]["evidence"].as_object().map(|evidence| {
        let mut counts = serde_json::Map::new();
        for family in FAMILY_COUNT_KEYS {
            counts.insert(
                (*family).into(),
                json!(evidence[*family].as_array().expect("family").len()),
            );
        }
        Value::Object(counts)
    });
    json!({
        "schemaVersion": "rangoon.integrations.lnsat-request-summary.v1",
        "recordAvailability": host["recordAvailability"].clone(),
        "reportedEvaluation": host["reportedEvaluation"].clone(),
        "reportedPolicy": host["reportedPolicy"].clone(),
        "readFailure": host["readFailure"].clone(),
        "securityActivityTouched": host["securityActivityTouched"].clone(),
        "familyCounts": family_counts,
        "truth": reference_truth(),
        "omissions": reference_omissions(),
    })
}

fn assert_reference_case(
    body: RequestRecordBody,
    exact_body: Option<&[u8]>,
    document: Option<&Value>,
    reported: (&str, &str, &str),
    security: Option<bool>,
    failure: Option<&str>,
) -> (
    rangoon_integrations::lnsat_request_record::RequestRecordInspection,
    Value,
) {
    let (availability, evaluation, policy) = reported;
    let expected_host = reference_host(
        availability,
        evaluation,
        policy,
        document.map(reference_native),
        security,
        exact_body.map(|body| format!("sha256:{}", byte_digest(body))),
        failure,
    );
    let expected_summary = reference_summary(&expected_host);
    let actual = inspect_request_record(input(body)).expect("closed reference fixture accepts");
    assert_eq!(actual.host_json(), reference_bytes(&expected_host));
    assert_eq!(actual.summary_json(), reference_bytes(&expected_summary));
    (actual, expected_host)
}

#[test]
fn s01_complete_allow_without_execution_is_reported_only() {
    let result = inspect(&complete("allow")).expect("S01 valid allow");
    assert_host_contract(&result);
    let host: Value = serde_json::from_slice(result.host_json()).unwrap();
    assert_eq!(host["recordAvailability"], "complete");
    assert_eq!(host["reportedPolicy"], "allow");
    assert_eq!(host["native"]["evidence"]["operations"], json!([]));
    assert_eq!(host["truth"]["authority"], "none");
}

#[test]
fn s02_complete_deny_never_becomes_an_external_path_claim() {
    let host = host(&complete("deny"));
    assert_eq!(host["reportedPolicy"], "deny");
    assert_eq!(host["native"]["evidence"]["policy"]["requires_approval"], 0);
    assert_eq!(host["truth"]["independentEffect"], "not_observed");
}

#[test]
fn s03_approval_required_preserves_distinct_rows() {
    let mut value = complete("approval_required");
    add_minimal_execution(&mut value, "dispatching", None);
    value["evidence"]["approvals"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "approval_request_id": id("apr_", 2),
            "policy_decision_id": id("pol_", 1),
            "status": "requested",
            "requested_at": at(1),
            "expires_at": at(59),
        }));
    value["evidence"]["approval_decisions"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "approval_decision_id": id("apd_", 2),
            "approval_request_id": id("apr_", 2),
            "policy_decision_id": id("pol_", 1),
            "decision": "denied",
            "reason": "approval.operator_denied",
            "decided_at": at(2),
            "expires_at": at(59),
            "approval_gate_satisfied": 0,
        }));
    let host = host(&value);
    assert_eq!(host["reportedPolicy"], "approval_required");
    assert_eq!(
        host["native"]["evidence"]["approvals"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        host["native"]["evidence"]["approval_decisions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(host["native"].get("approvalDuty").is_none());
}

#[test]
fn s04_failed_evaluation_reasons_are_not_unknown() {
    for reason in ["evaluation_rejected", "persistence_failed"] {
        let host = host(&failed(reason));
        assert_eq!(host["reportedEvaluation"], "evaluation_failed");
        assert_eq!(host["reportedPolicy"], "not_observed");
        assert!(host["native"]["evaluation"].is_object());
        assert!(host["native"]["evidence"].is_null());
    }
}

#[test]
fn s05_unknown_evaluation_can_still_be_complete() {
    let host = host(&unknown());
    assert_eq!(host["recordAvailability"], "complete");
    assert_eq!(host["reportedEvaluation"], "evaluation_unknown");
    assert!(host["native"]["evaluation"].is_null());
    assert!(host["native"]["evidence"].is_null());
}

#[test]
fn s06_replay_and_conflict_are_bounded_observations() {
    let mut value = complete("allow");
    value["observations"] = json!([
        observation("replay", digest(1), 1),
        observation("conflict", digest(9), 2),
    ]);
    let host = host(&value);
    assert_eq!(host["native"]["observations"].as_array().unwrap().len(), 2);
    assert!(host["native"].get("retryCount").is_none());
    assert_eq!(host["truth"]["independentEffect"], "not_observed");
}

#[test]
fn s07_issued_unconsumed_authorization_has_no_invented_operation() {
    let mut value = complete("approval_required");
    add_minimal_execution(&mut value, "dispatching", None);
    let evidence = value["evidence"].as_object_mut().unwrap();
    evidence.insert("operations".into(), json!([]));
    evidence.insert("consumptions".into(), json!([]));
    evidence.insert("operation_attempts".into(), json!([]));
    evidence.insert(
        "states".into(),
        json!([state(
            1,
            id("xau_", 1),
            "execution_authorization",
            1,
            None,
            "active",
            4
        )]),
    );
    let host = host(&value);
    assert_eq!(
        host["native"]["evidence"]["authorizations"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(host["native"]["evidence"]["operations"], json!([]));
}

#[test]
fn s08_dispatching_and_unknown_are_not_completion() {
    let mut value = complete("approval_required");
    add_minimal_execution(&mut value, "outcome_unknown", None);
    let host = host(&value);
    let states = host["native"]["evidence"]["states"].as_array().unwrap();
    assert!(states.iter().any(|row| row["state"] == "outcome_unknown"));
    assert_eq!(host["native"]["evidence"]["receipts"], json!([]));
    assert_eq!(host["truth"]["authoritativeOutcome"], "unknown");
}

#[test]
fn s09_completed_receipt_is_reported_not_independent_effect() {
    let mut value = complete("approval_required");
    add_minimal_execution(&mut value, "completed", None);
    let host = host(&value);
    assert_eq!(
        host["native"]["evidence"]["receipts"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(host["truth"]["independentEffect"], "not_observed");
}

#[test]
fn s10_unknown_to_completed_matched_reconciliation_stays_one_document() {
    let mut value = complete("approval_required");
    add_minimal_execution(&mut value, "completed", Some("matched"));
    let host = host(&value);
    assert_eq!(
        host["native"]["evidence"]["reconciliations"][0]["status"],
        "matched"
    );
    assert!(host["native"].get("redispatch").is_none());
    assert_eq!(host["truth"]["execution"], "unavailable");
}

#[test]
fn s11_pending_unresolved_and_mismatch_remain_distinct() {
    for status in ["pending", "unresolved", "mismatch"] {
        let mut value = complete("approval_required");
        add_minimal_execution(&mut value, "completed", Some(status));
        let host = host(&value);
        assert_eq!(
            host["native"]["evidence"]["reconciliations"][0]["status"],
            status
        );
        assert_ne!(
            host["native"]["evidence"]["reconciliations"][0]["status"],
            "matched"
        );
    }
}

#[test]
fn s12_degraded_never_retains_prior_native_rows() {
    let host = host(&degraded());
    assert_eq!(host["recordAvailability"], "degraded");
    assert!(host["native"].is_null());
    assert!(host["readFailure"].is_null());
    assert_eq!(host["securityActivityTouched"], true);
}

#[test]
fn s13_exact_generic_403_has_one_unclassified_form() {
    let refusal = json!({
        "schema": "lnsat.action_request_record.v1",
        "ok": false,
        "error": {"code": "gateway.action_request_record.denied"},
    });
    let result =
        inspect_request_record(input(RequestRecordBody::Refused403(bytes(&refusal)))).unwrap();
    let host: Value = serde_json::from_slice(result.host_json()).unwrap();
    assert_eq!(host["recordAvailability"], "read_rejected_unclassified");
    assert_eq!(host["readFailure"], "native_read_rejected_unclassified");
    assert!(host["native"].is_null());
}

#[test]
fn s14_every_unavailable_condition_has_a_closed_form() {
    let variants = [
        (ReadUnavailable::NotConfigured, "read_not_configured"),
        (
            ReadUnavailable::TransportUnavailable,
            "read_transport_unavailable",
        ),
        (ReadUnavailable::Cancelled, "read_cancelled"),
        (ReadUnavailable::Timeout, "read_timeout"),
        (ReadUnavailable::Unauthenticated, "read_unauthenticated"),
        (
            ReadUnavailable::UnsupportedTransport,
            "read_unsupported_transport",
        ),
    ];
    for (condition, code) in variants {
        let result =
            inspect_request_record(input(RequestRecordBody::ReadUnavailable(condition))).unwrap();
        let host: Value = serde_json::from_slice(result.host_json()).unwrap();
        assert_eq!(host["recordAvailability"], "read_unavailable");
        assert_eq!(host["readFailure"], code);
        assert!(host["inputDigest"].is_null());
        assert!(host["native"].is_null());
    }
}

#[test]
fn s15_request_scope_and_namespace_mismatches_refuse() {
    let mut top_request = complete("allow");
    top_request["request_id"] = json!(id("arq_", 2));
    assert!(matches!(
        err(&top_request),
        RequestRecordError::SelectionMismatch
    ));
    for (field, replacement) in [
        ("project_ref", json!("project:other")),
        ("resource_ref", json!("resource:other")),
    ] {
        let mut value = complete("allow");
        value["request"][field] = replacement;
        assert!(matches!(err(&value), RequestRecordError::ScopeMismatch));
    }
}

#[test]
fn s16_wrong_schema_d3a_wrapper_and_unknown_fields_refuse() {
    let mut wrong_schema = complete("allow");
    wrong_schema["schema"] = json!("lnsat.gateway.runtime_composition.v1_0");
    assert!(matches!(
        err(&wrong_schema),
        RequestRecordError::UnsupportedSchema
    ));
    let mut extra = complete("allow");
    extra["d3a"] = json!(true);
    assert!(matches!(err(&extra), RequestRecordError::InvalidEnvelope));
}

#[test]
fn s17_bom_trailing_invalid_utf8_and_duplicate_decoded_keys_refuse() {
    let valid = bytes(&complete("allow"));
    let mut bom = vec![0xef, 0xbb, 0xbf];
    bom.extend_from_slice(&valid);
    assert!(matches!(
        inspect_request_record(input(RequestRecordBody::Body200(bom))),
        Err(RequestRecordError::InvalidEncoding)
    ));
    let mut trailing = valid.clone();
    trailing.extend_from_slice(b" true");
    assert!(matches!(
        inspect_request_record(input(RequestRecordBody::Body200(trailing))),
        Err(RequestRecordError::InvalidJson)
    ));
    let duplicate =
        br#"{"schema":"lnsat.action_request_record.v1","schema":"lnsat.action_request_record.v1"}"#
            .to_vec();
    let duplicate_error =
        inspect_request_record(input(RequestRecordBody::Body200(duplicate))).unwrap_err();
    assert_error_code(duplicate_error, "duplicate_field");
    let encoding_error =
        inspect_request_record(input(RequestRecordBody::Body200(vec![0xff]))).unwrap_err();
    assert_error_code(encoding_error, "invalid_encoding");
    for duplicate in [
        br#"{"nested":{"key":0,"\u006bey":1}}"#.as_slice(),
        br#"{"nested":[{"a":0,"a":1}]}"#.as_slice(),
    ] {
        assert_eq!(
            inspect_request_record(input(RequestRecordBody::Body200(duplicate.to_vec())))
                .unwrap_err(),
            RequestRecordError::DuplicateField
        );
    }
    // A truncated \u escape followed by multi-byte UTF-8 must return a fixed
    // parse error, never panic by byte-indexing inside a scalar.
    let malformed_unicode = b"\"\\u\xc3\xa9\xc3\xa9\xc3\xa9\"".to_vec();
    assert!(matches!(
        inspect_request_record(input(RequestRecordBody::Body200(malformed_unicode))),
        Err(RequestRecordError::InvalidJson)
    ));
}

#[test]
fn s18_missing_null_and_coercion_are_not_defaulted() {
    let mut missing = complete("allow");
    missing
        .as_object_mut()
        .unwrap()
        .remove("security_activity_touched");
    assert!(matches!(err(&missing), RequestRecordError::InvalidEnvelope));
    let mut bool_as_integer = complete("allow");
    bool_as_integer["evidence"]["policy"]["requires_approval"] = json!(false);
    assert!(matches!(
        err(&bool_as_integer),
        RequestRecordError::InvalidEnvelope
    ));
    let mut string_bool = complete("allow");
    string_bool["security_activity_touched"] = json!("false");
    assert!(matches!(
        err(&string_bool),
        RequestRecordError::InvalidEnvelope
    ));
}

#[test]
fn s19_input_boundaries_and_impossible_reachable_limits_are_distinct() {
    let exact = vec![b' '; rangoon_integrations::lnsat_request_record::MAX_INPUT_BYTES];
    assert!(matches!(
        inspect_request_record(input(RequestRecordBody::Body200(exact))),
        Err(RequestRecordError::InvalidJson)
    ));
    let over = vec![b' '; rangoon_integrations::lnsat_request_record::MAX_INPUT_BYTES + 1];
    assert!(matches!(
        inspect_request_record(input(RequestRecordBody::Body200(over))),
        Err(RequestRecordError::InputLimit)
    ));
    let value = complete("allow");
    assert!(bytes(&value).len() < rangoon_integrations::lnsat_request_record::MAX_INPUT_BYTES);
    assert!(inspect(&value).is_ok());
    let mut boundary_context = input(RequestRecordBody::Body200(bytes(&value)));
    boundary_context.installation_id = "i".repeat(128);
    boundary_context.environment = "e".repeat(128);
    boundary_context.expected_project_ref = PROJECT.into();
    boundary_context.expected_resource_ref = RESOURCE.into();
    assert!(inspect_request_record(boundary_context).is_ok());
    let mut over_context = input(RequestRecordBody::Body200(bytes(&value)));
    over_context.installation_id = "i".repeat(129);
    assert!(matches!(
        inspect_request_record(over_context),
        Err(RequestRecordError::InputLimit)
    ));
    let mut string_limit = complete("allow");
    string_limit["request"]["requester_ref"] = json!("r".repeat(4_096));
    assert!(inspect(&string_limit).is_ok());
    string_limit["request"]["requester_ref"] = json!("r".repeat(4_097));
    assert!(matches!(
        err(&string_limit),
        RequestRecordError::StructuralLimit
    ));
    // A valid closed envelope padded to exactly the byte cap is accepted.
    let mut exact_valid = bytes(&value);
    exact_valid.resize(
        rangoon_integrations::lnsat_request_record::MAX_INPUT_BYTES,
        b' ',
    );
    assert!(inspect_request_record(input(RequestRecordBody::Body200(exact_valid.clone()))).is_ok());
    exact_valid.push(b' ');
    assert_eq!(
        inspect_request_record(input(RequestRecordBody::Body200(exact_valid))).unwrap_err(),
        RequestRecordError::InputLimit
    );
    // At a generic parser boundary the grammar succeeds then the closed envelope
    // refuses; one unit beyond must be rejected by the parser's quota itself.
    let exact_depth = format!("{}0{}", "[".repeat(15), "]".repeat(15));
    assert_eq!(
        inspect_request_record(input(RequestRecordBody::Body200(exact_depth.into_bytes())))
            .unwrap_err(),
        RequestRecordError::InvalidEnvelope
    );
    for (count, expected) in [
        (32, RequestRecordError::InvalidEnvelope),
        (33, RequestRecordError::StructuralLimit),
    ] {
        let object: serde_json::Map<String, Value> =
            (0..count).map(|n| (format!("k{n}"), json!(0))).collect();
        assert_eq!(
            inspect_request_record(input(RequestRecordBody::Body200(bytes(&Value::Object(
                object
            )))))
            .unwrap_err(),
            expected
        );
    }
    // 1 root + 64 inner arrays + 4096 leaf arrays + N scalar values.
    for (total, expected) in [
        (16_384, RequestRecordError::InvalidEnvelope),
        (16_385, RequestRecordError::StructuralLimit),
    ] {
        let mut remaining = total - 1 - 64 - 4096;
        let mut outer = Vec::new();
        for _ in 0..64 {
            let mut inner = Vec::new();
            for _ in 0..64 {
                let count = remaining.min(64);
                remaining -= count;
                inner.push(Value::Array(vec![json!(0); count]));
            }
            outer.push(Value::Array(inner));
        }
        assert_eq!(remaining, 0);
        assert_eq!(
            inspect_request_record(input(RequestRecordBody::Body200(bytes(&Value::Array(
                outer
            )))))
            .unwrap_err(),
            expected
        );
    }
    // The 16-level generic JSON shape is not reachable through this closed wire
    // profile without an unknown field.  Test the parser's rejected over-depth
    // grammar separately rather than mislabeling an impossible "valid" body.
    let deep = format!("{}0{}", "[".repeat(17), "]".repeat(17));
    assert!(matches!(
        inspect_request_record(input(RequestRecordBody::Body200(deep.into_bytes()))),
        Err(RequestRecordError::StructuralLimit)
    ));
}

#[test]
fn s20_linked_row_and_observation_cardinality_are_closed() {
    let mut at_limit = complete("approval_required");
    at_limit["evidence"]["approvals"] = approval_rows(64);
    let host = host(&at_limit);
    assert_eq!(
        host["native"]["evidence"]["approvals"]
            .as_array()
            .unwrap()
            .len(),
        64
    );
    assert_eq!(summary(&at_limit)["familyCounts"]["approvals"], 64);
    let mut observations = complete("allow");
    observations["observations"] = json!([
        observation("replay", digest(1), 1),
        observation("conflict", digest(2), 2),
        observation("replay", digest(1), 3),
    ]);
    assert!(matches!(
        err(&observations),
        RequestRecordError::StructuralLimit | RequestRecordError::LineageConflict
    ));
    let mut rows = complete("allow");
    rows["evidence"]["approvals"] = approval_rows(65);
    assert!(matches!(err(&rows), RequestRecordError::StructuralLimit));
}

#[test]
fn s21_ids_hashes_and_time_aliases_must_be_canonical() {
    for replacement in [
        json!("ARQ_ABC"),
        json!("arq_short"),
        json!("arq_ABCDEF0000000000000000000000000000000000000000000000000000000000"),
    ] {
        let mut value = complete("allow");
        value["request_id"] = replacement;
        assert!(matches!(
            err(&value),
            RequestRecordError::InvalidEnvelope | RequestRecordError::SelectionMismatch
        ));
    }
    let mut time = complete("allow");
    time["request"]["created_at"] = json!("2026-10-26T12:00:00Z");
    assert!(matches!(err(&time), RequestRecordError::InvalidEnvelope));
    let mut bad_digest = complete("allow");
    bad_digest["request"]["packet_sha256"] =
        json!("sha256:ABCDEF0000000000000000000000000000000000000000000000000000000000");
    assert!(matches!(
        err(&bad_digest),
        RequestRecordError::InvalidEnvelope
    ));
    let mut bad_calendar = complete("allow");
    bad_calendar["request"]["created_at"] = json!("2026-02-30T12:00:00.000Z");
    assert!(matches!(
        err(&bad_calendar),
        RequestRecordError::InvalidEnvelope
    ));
}

#[test]
fn s22_dangling_duplicate_and_cross_family_joins_refuse() {
    let mut value = complete("approval_required");
    add_minimal_execution(&mut value, "dispatching", None);
    value["evidence"]["authorizations"][0]["approval_decision_id"] = json!(id("apd_", 9));
    assert!(matches!(err(&value), RequestRecordError::LineageConflict));
    let mut duplicate = complete("approval_required");
    add_minimal_execution(&mut duplicate, "dispatching", None);
    let duplicate_row = duplicate["evidence"]["approvals"][0].clone();
    duplicate["evidence"]["approvals"]
        .as_array_mut()
        .unwrap()
        .push(duplicate_row);
    assert!(matches!(
        err(&duplicate),
        RequestRecordError::LineageConflict
    ));
}

#[test]
fn s23_consumption_attempt_and_receipt_mismatches_refuse() {
    let mut value = complete("approval_required");
    add_minimal_execution(&mut value, "completed", None);
    value["evidence"]["receipts"][0]["consumption_id"] = json!(id("cpc_", 9));
    assert!(matches!(err(&value), RequestRecordError::LineageConflict));
    let mut sequence = complete("approval_required");
    add_minimal_execution(&mut sequence, "completed", None);
    sequence["evidence"]["operation_attempts"][0]["attempt_sequence"] = json!(2);
    assert!(matches!(
        err(&sequence),
        RequestRecordError::LineageConflict
    ));
}

#[test]
fn s24_state_predecessor_sequence_time_and_transition_gaps_refuse() {
    let mut value = complete("approval_required");
    add_minimal_execution(&mut value, "completed", None);
    value["evidence"]["states"][3]["prior_state_sequence"] = json!(9);
    assert!(matches!(err(&value), RequestRecordError::LineageConflict));
    let mut unsupported = complete("approval_required");
    add_minimal_execution(&mut unsupported, "completed", None);
    unsupported["evidence"]["states"][3]["state"] = json!("revived");
    assert!(matches!(
        err(&unsupported),
        RequestRecordError::UnsupportedState
    ));
    let mut backward_time = complete("approval_required");
    add_minimal_execution(&mut backward_time, "completed", None);
    backward_time["evidence"]["states"][3]["effective_at"] = json!(at(4));
    assert!(matches!(
        err(&backward_time),
        RequestRecordError::LineageConflict
    ));
    let mut fork = complete("approval_required");
    add_minimal_execution(&mut fork, "completed", None);
    let copied = fork["evidence"]["states"][3].clone();
    fork["evidence"]["states"]
        .as_array_mut()
        .unwrap()
        .push(copied);
    assert!(matches!(err(&fork), RequestRecordError::LineageConflict));
}

#[test]
fn s25_reconciliation_tuple_mismatches_refuse() {
    let mut value = complete("approval_required");
    add_minimal_execution(&mut value, "completed", Some("matched"));
    value["evidence"]["reconciliations"][0]["observed_result_digest"] = json!(digest(9));
    assert!(matches!(err(&value), RequestRecordError::LineageConflict));
    let mut pending = complete("approval_required");
    add_minimal_execution(&mut pending, "completed", Some("pending"));
    pending["evidence"]["reconciliations"][0]["receipt_id"] = json!(id("rcp_", 1));
    assert!(matches!(err(&pending), RequestRecordError::InvalidEnvelope));
}

#[test]
fn s26_failed_or_receiptless_terminal_chains_do_not_gain_effect_claims() {
    let mut unknown_chain = complete("approval_required");
    add_minimal_execution(&mut unknown_chain, "outcome_unknown", None);
    let host = host(&unknown_chain);
    assert_eq!(host["truth"]["independentEffect"], "not_observed");
    let mut impossible = complete("approval_required");
    add_minimal_execution(&mut impossible, "completed", None);
    impossible["evidence"]["receipts"] = json!([]);
    assert!(matches!(
        err(&impossible),
        RequestRecordError::LineageConflict
    ));
}

#[test]
fn s27_requester_and_approval_reason_are_validated_then_redacted() {
    let mut value = complete("approval_required");
    add_minimal_execution(&mut value, "dispatching", None);
    let result = inspect(&value).unwrap();
    let host_text = std::str::from_utf8(result.host_json()).unwrap();
    let summary_text = std::str::from_utf8(result.summary_json()).unwrap();
    assert!(!host_text.contains(REQUESTER_SENTINEL));
    assert!(!host_text.contains("approval.operator_approved"));
    assert!(!summary_text.contains(REQUESTER_SENTINEL));
    assert!(!summary_text.contains("approval.operator_approved"));
    assert!(!format!("{result:?}").contains(REQUESTER_SENTINEL));
    value["evidence"]["approval_decisions"][0]["reason"] = json!("private-free-text-must-refuse");
    assert_eq!(err(&value), RequestRecordError::UnsupportedState);
    let mut invalid_requester = complete("allow");
    invalid_requester["request"]["requester_ref"] = Value::Null;
    assert_eq!(err(&invalid_requester), RequestRecordError::InvalidEnvelope);
}

#[test]
fn s28_summary_has_no_copied_native_identifiers_or_times() {
    let result = inspect(&complete("allow")).unwrap();
    let summary_text = std::str::from_utf8(result.summary_json()).unwrap();
    let packet = digest(1);
    let created = at(0);
    for secret in [
        REQUEST,
        PROJECT,
        RESOURCE,
        "packet:synthetic",
        packet.as_str(),
        created.as_str(),
    ] {
        assert!(!summary_text.contains(secret));
    }
    assert!(host(&complete("allow"))["native"]["request"]["request_id"].is_string());
}

#[test]
fn s29_unexpected_sensitive_wire_fields_refuse_without_echo() {
    let mut value = complete("allow");
    value["token"] = json!("secret-never-echo");
    let error = err(&value);
    assert!(matches!(error, RequestRecordError::InvalidEnvelope));
    let text = format!("{error:?}");
    assert!(!text.contains("secret-never-echo"));
}

#[test]
fn s30_security_activity_is_only_reported_bookkeeping() {
    let mut value = complete("allow");
    value["security_activity_touched"] = json!(true);
    let host = host(&value);
    assert_eq!(host["securityActivityTouched"], true);
    assert_eq!(host["truth"]["authority"], "none");
    assert_eq!(host["truth"]["independentEffect"], "not_observed");
}

#[test]
fn s31_serialized_output_oracle_covers_every_variant() {
    for (error, code) in [
        (RequestRecordError::InputLimit, "input_limit"),
        (RequestRecordError::InvalidEncoding, "invalid_encoding"),
        (RequestRecordError::InvalidJson, "invalid_json"),
        (RequestRecordError::DuplicateField, "duplicate_field"),
        (RequestRecordError::StructuralLimit, "structural_limit"),
        (RequestRecordError::UnsupportedSchema, "unsupported_schema"),
        (RequestRecordError::InvalidEnvelope, "invalid_envelope"),
        (RequestRecordError::SelectionMismatch, "selection_mismatch"),
        (RequestRecordError::ScopeMismatch, "scope_mismatch"),
        (RequestRecordError::LineageConflict, "lineage_conflict"),
        (RequestRecordError::UnsupportedState, "unsupported_state"),
        (RequestRecordError::OutputLimit, "output_limit"),
        (
            RequestRecordError::SerializationFailed,
            "serialization_failed",
        ),
    ] {
        assert_error_code(error, code);
    }
    let allow = complete("allow");
    let allow_bytes = bytes(&allow);
    let (allow_actual, allow_expected) = assert_reference_case(
        RequestRecordBody::Body200(allow_bytes.clone()),
        Some(&allow_bytes),
        Some(&allow),
        ("complete", "policy_recorded", "allow"),
        Some(false),
        None,
    );

    let failed = failed("evaluation_rejected");
    let failed_bytes = bytes(&failed);
    assert_reference_case(
        RequestRecordBody::Body200(failed_bytes.clone()),
        Some(&failed_bytes),
        Some(&failed),
        ("complete", "evaluation_failed", "not_observed"),
        Some(false),
        None,
    );

    let unknown = unknown();
    let unknown_bytes = bytes(&unknown);
    assert_reference_case(
        RequestRecordBody::Body200(unknown_bytes.clone()),
        Some(&unknown_bytes),
        Some(&unknown),
        ("complete", "evaluation_unknown", "not_observed"),
        Some(false),
        None,
    );

    let mut full_graph = complete("approval_required");
    add_minimal_execution(&mut full_graph, "completed", Some("matched"));
    let full_graph_bytes = bytes(&full_graph);
    assert_reference_case(
        RequestRecordBody::Body200(full_graph_bytes.clone()),
        Some(&full_graph_bytes),
        Some(&full_graph),
        ("complete", "policy_recorded", "approval_required"),
        Some(false),
        None,
    );

    let degraded = degraded();
    let degraded_bytes = bytes(&degraded);
    assert_reference_case(
        RequestRecordBody::Body200(degraded_bytes.clone()),
        Some(&degraded_bytes),
        None,
        ("degraded", "evaluation_unknown", "not_observed"),
        Some(true),
        None,
    );

    let refusal = bytes(&json!({
        "schema":"lnsat.action_request_record.v1",
        "ok":false,
        "error":{"code":"gateway.action_request_record.denied"},
    }));
    assert_reference_case(
        RequestRecordBody::Refused403(refusal.clone()),
        Some(&refusal),
        None,
        ("read_rejected_unclassified", "not_observed", "not_observed"),
        None,
        Some("native_read_rejected_unclassified"),
    );

    for (read, failure) in [
        (ReadUnavailable::NotConfigured, "read_not_configured"),
        (
            ReadUnavailable::TransportUnavailable,
            "read_transport_unavailable",
        ),
        (ReadUnavailable::Cancelled, "read_cancelled"),
        (ReadUnavailable::Timeout, "read_timeout"),
        (ReadUnavailable::Unauthenticated, "read_unauthenticated"),
        (
            ReadUnavailable::UnsupportedTransport,
            "read_unsupported_transport",
        ),
    ] {
        assert_reference_case(
            RequestRecordBody::ReadUnavailable(read),
            None,
            None,
            ("read_unavailable", "not_observed", "not_observed"),
            None,
            Some(failure),
        );
    }

    let permuted_bytes = reverse_top_level_bytes(&allow);
    let (permuted_actual, _) = assert_reference_case(
        RequestRecordBody::Body200(permuted_bytes.clone()),
        Some(&permuted_bytes),
        Some(&allow),
        ("complete", "policy_recorded", "allow"),
        Some(false),
        None,
    );
    assert_ne!(allow_actual.input_digest(), permuted_actual.input_digest());
    assert_eq!(allow_actual.summary_json(), permuted_actual.summary_json());

    let mut nested_wrong = allow_expected.clone();
    nested_wrong["native"]["request"]["project_ref"] = json!("project:wrong");
    assert!(
        std::panic::catch_unwind(|| {
            assert_eq!(allow_actual.host_json(), reference_bytes(&nested_wrong));
        })
        .is_err()
    );
    let mut extra_field = allow_expected.clone();
    extra_field["native"]["evidence"]["policy"]["extra"] = json!(true);
    assert!(std::panic::catch_unwind(|| reference_bytes(&extra_field)).is_err());
    assert!(
        std::panic::catch_unwind(|| {
            assert_eq!(
                allow_actual.host_json(),
                reverse_top_level_bytes(&allow_expected)
            );
        })
        .is_err()
    );
    let expected_bytes = reference_bytes(&allow_expected);
    let policy_bytes = reference_bytes(&allow_expected["native"]["evidence"]["policy"]);
    let reversed_policy = reverse_top_level_bytes(&allow_expected["native"]["evidence"]["policy"]);
    let offset = expected_bytes
        .windows(policy_bytes.len())
        .position(|window| window == policy_bytes)
        .expect("nested policy bytes");
    let mut nested_order = expected_bytes.clone();
    nested_order.splice(offset..offset + policy_bytes.len(), reversed_policy);
    assert!(
        std::panic::catch_unwind(|| {
            assert_eq!(allow_actual.host_json(), nested_order.as_slice());
        })
        .is_err()
    );

    let mut malformed = allow;
    malformed["unexpected"] = json!(true);
    assert_error_code(err(&malformed), "invalid_envelope");
}

#[test]
fn s32_input_digest_binds_exact_bytes_and_commitment_tampering_refuses() {
    let value = complete("allow");
    let original = bytes(&value);
    let result =
        inspect_request_record(input(RequestRecordBody::Body200(original.clone()))).unwrap();
    let expected = format!("sha256:{}", byte_digest(&original));
    assert_eq!(result.input_digest(), Some(expected.as_str()));
    let mut tampered = value;
    tampered["evaluation"]["evidence_digest"] = json!(digest(99));
    assert!(matches!(
        err(&tampered),
        RequestRecordError::LineageConflict
    ));
}

#[test]
fn s33_foreign_openshell_fields_do_not_create_a_bridge() {
    let mut value = complete("allow");
    value["openshell_trace"] = json!(REQUEST);
    assert!(matches!(err(&value), RequestRecordError::InvalidEnvelope));
}

#[test]
fn s34_profile_is_separate_from_d3a_i1_a1_o1_and_o2() {
    let result = inspect(&complete("allow")).unwrap();
    let text = std::str::from_utf8(result.host_json()).unwrap();
    for forbidden in [
        "runtime_composition",
        "DecodedWindow",
        "operator-view.v1",
        "O1",
        "O2",
    ] {
        assert!(!text.contains(forbidden));
    }
    assert!(text.contains("rangoon.integrations.lnsat-request-record.v1"));
}

#[test]
fn s35_public_input_shape_has_no_transport_or_mutation_port() {
    let _ = RequestRecordInput {
        installation_id: "i".into(),
        environment: "e".into(),
        selected_request_id: REQUEST.into(),
        expected_project_ref: PROJECT.into(),
        expected_resource_ref: RESOURCE.into(),
        body: RequestRecordBody::ReadUnavailable(ReadUnavailable::Timeout),
        read_at: None,
    };
    // Compile-time construction is the structural oracle: the public input has
    // only caller context and typed bytes/read condition, never URL, callback,
    // credential, store, process, clock, or reconciliation capability.
}

#[test]
fn s36_decoder_is_provider_free_and_not_an_o1_o2_serializer() {
    let result = inspect(&complete("allow")).unwrap();
    let host_text = std::str::from_utf8(result.host_json()).unwrap();
    let summary_text = std::str::from_utf8(result.summary_json()).unwrap();
    assert!(!host_text.contains("provider"));
    assert!(!summary_text.contains("provider"));
    assert!(!host_text.contains("operator-view"));
}
