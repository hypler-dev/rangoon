use super::*;

struct SelectedWindow(std::path::PathBuf);

impl SelectedWindow {
    fn new(bytes: &[u8]) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "rangoon-operations-native-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("selected-window.json"), bytes).unwrap();
        Self(directory)
    }

    fn path(&self) -> std::path::PathBuf {
        self.0.join("selected-window.json")
    }
}

impl Drop for SelectedWindow {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const EMPTY_WINDOW: &[u8] = br#"{"schemaVersion":"rangoon.ops.window.v1","snapshot":{"snapshotId":"ops:1111111111111111111111111111111111111111111111111111111111111111","reducerVersion":"rangoon.ops.reducer.v1","scope":{"workspaceId":"ops:1111111111111111111111111111111111111111111111111111111111111111","systemIds":[],"environmentIds":[],"engineIds":[]},"topology":{"revisionId":"ops:1111111111111111111111111111111111111111111111111111111111111111","nodes":[],"edges":[],"policyRevisionIds":[],"coverageRevisionIds":[]},"prefixes":[],"asOfMs":0},"tail":[]}"#;

#[test]
fn empty_window_returns_only_safe_unverified_view() {
    let result = serde_json::to_value(inspect_bytes(EMPTY_WINDOW)).unwrap();
    assert_eq!(result["schemaVersion"], RESULT_SCHEMA);
    assert_eq!(result["outcome"], "inspected");
    assert_eq!(
        result["view"]["schemaVersion"],
        "rangoon.ops.operator-view.v1"
    );
    assert_eq!(result["view"]["authority"], "none");
    assert_eq!(result["view"]["authenticity"], "unverified");
    assert_eq!(result["view"]["eligibility"], "unknown");
    assert_eq!(result["view"]["execution"], "unavailable");
    assert_eq!(result["view"]["sourceCounts"]["eventCount"], 0);
    assert_eq!(result["view"]["events"], serde_json::json!([]));
    let serialized = result.to_string();
    assert!(!serialized.contains("1111111111111111"));
    assert!(!serialized.contains("8ab43d0754e994dc"));
    assert!(!serialized.contains("canonicalDigest"));
    assert!(!serialized.contains("externalRef"));
}

#[test]
fn invalid_and_cancelled_results_are_fixed_and_payload_free() {
    let invalid = serde_json::to_value(inspect_bytes(b"\xffnot-json")).unwrap();
    assert_eq!(invalid["outcome"], "rejected");
    assert_eq!(invalid["error"]["code"], "operations_window_invalid");
    assert_eq!(invalid.as_object().unwrap().len(), 3);

    let cancelled = serde_json::to_value(OperationsResult::cancelled()).unwrap();
    assert_eq!(cancelled["outcome"], "cancelled");
    assert_eq!(cancelled["schemaVersion"], RESULT_SCHEMA);
    assert_eq!(cancelled.as_object().unwrap().len(), 2);
}

#[test]
fn stage_failure_is_atomic_and_contains_no_view_or_source() {
    let result = serde_json::to_value(stage_failed("operations_response_limit")).unwrap();
    assert_eq!(result["schemaVersion"], RESULT_SCHEMA);
    assert_eq!(result["outcome"], "failed");
    assert_eq!(result["error"]["code"], "operations_response_limit");
    assert_eq!(result.as_object().unwrap().len(), 3);
    let serialized = result.to_string();
    for forbidden in [
        "\"view\"",
        "\"events\"",
        "snapshotId",
        "externalRef",
        "sourceId",
    ] {
        assert!(!serialized.contains(forbidden));
    }
}

#[test]
fn selected_file_runs_host_read_and_all_native_inspectors_into_the_safe_view() {
    let source = include_bytes!("../../../fixtures/operations/synthetic-local-window-v1.json");
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/operations/synthetic-operator-view-v1.json"
    ))
    .unwrap();
    let selected = SelectedWindow::new(source);

    let result = serde_json::to_value(inspect_selected(&selected.path())).unwrap();
    assert_eq!(result["schemaVersion"], RESULT_SCHEMA);
    assert_eq!(result["outcome"], "inspected");
    assert_eq!(result["view"], expected);
    assert_eq!(result["view"]["sourceCounts"]["eventCount"], 2);
    assert_eq!(result["view"]["events"][0]["inputOrdinal"], 0);
    assert_eq!(result["view"]["events"][1]["inputOrdinal"], 1);

    let serialized = result.to_string();
    assert!(!serialized.contains("synthetic-secret-200"));
    assert!(!serialized.contains("selected-window.json"));
    assert!(!serialized.contains("externalRef"));
}

#[test]
fn selected_file_keeps_reported_approval_and_incomplete_links_unverified() {
    let mut source: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../fixtures/operations/synthetic-local-window-v1.json"
    ))
    .unwrap();
    let oid = |number: u16| format!("ops:{number:064x}");
    let actor_id = source["tail"][0]["record"]["id"].clone();
    let producer_id = source["tail"][0]["producerId"].clone();
    let fence_id = source["tail"][0]["fenceId"].clone();
    let mut correlation = source["tail"][0]["correlation"].clone();
    correlation["requestId"] = serde_json::json!(oid(240));
    correlation["evaluationId"] = serde_json::json!(oid(241));
    correlation["policyRevisionId"] = serde_json::json!(oid(242));
    let context = serde_json::json!({
        "workspaceId": correlation["workspaceId"],
        "systemId": correlation["systemId"],
        "environmentId": correlation["environmentId"],
        "engineId": correlation["engineId"],
        "requestId": correlation["requestId"],
        "traceId": correlation["traceId"],
        "spanId": correlation["spanId"],
        "policyRevisionId": correlation["policyRevisionId"],
    });
    let plan_id = oid(243);
    let stage_id = oid(244);
    let plan = serde_json::json!({
        "kind": "approval_plan", "id": plan_id, "context": context,
        "evaluationId": correlation["evaluationId"], "previousPlanId": null,
        "changedByActorId": actor_id, "expiresMs": null,
        "stages": [{
            "id": stage_id, "dependsOnStageIds": [], "approverIds": [actor_id],
            "humanRequired": true, "mode": "all", "quorum": null,
            "expiresMs": null, "escalationActorIds": []
        }],
        "evidenceIds": []
    });
    let decision = serde_json::json!({
        "kind": "approval_decision", "id": oid(245), "context": context,
        "evaluationId": correlation["evaluationId"], "planId": plan_id,
        "stageId": stage_id, "actorId": actor_id, "delegationId": null,
        "decision": "approve", "supersedesDecisionId": null,
        "reason": "none", "evidenceIds": []
    });
    let mut previous_id = source["tail"][1]["eventId"].clone();
    for (sequence, record) in [(3, plan), (4, decision)] {
        let event_id = oid(245 + sequence);
        source["tail"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "eventId": event_id, "producerId": producer_id, "fenceId": fence_id,
                "sequence": sequence, "previousEventId": previous_id,
                "observedMs": null, "receivedMs": null,
                "correlation": correlation, "record": record
            }));
        previous_id = serde_json::json!(event_id);
    }
    let selected = SelectedWindow::new(&serde_json::to_vec(&source).unwrap());
    let result = serde_json::to_value(inspect_selected(&selected.path())).unwrap();

    assert_eq!(result["outcome"], "inspected");
    assert_eq!(result["view"]["authority"], "none");
    assert_eq!(result["view"]["authenticity"], "unverified");
    assert_eq!(result["view"]["eligibility"], "unknown");
    assert_eq!(result["view"]["execution"], "unavailable");
    assert_eq!(result["view"]["outcome"], "unknown");
    assert_eq!(result["view"]["coverage"], "unknown");
    assert_eq!(result["view"]["sourceCounts"]["eventCount"], 4);
    assert_eq!(
        result["view"]["decisionCounts"]["approvalDecision"]["approve"],
        1
    );
    assert_eq!(
        result["view"]["events"][3]["recordKind"],
        "approval_decision"
    );
    assert_eq!(result["view"]["events"][3]["reportedValue"], "approve");
    assert!(
        result["view"]["events"][3]["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reference| {
                reference["status"] == "unique_unverified"
                    && reference["matchingEventOrdinals"] == serde_json::json!([2])
            })
    );
    assert!(
        result["view"]["events"][3]["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reference| reference["status"] == "missing")
    );
    let serialized = result.to_string();

    fn assert_source_ids_absent(value: &serde_json::Value, response: &str) {
        match value {
            serde_json::Value::String(id) if id.starts_with("ops:") => {
                assert!(!response.contains(id), "source ID crossed IPC: {id}");
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    assert_source_ids_absent(value, response);
                }
            }
            serde_json::Value::Object(fields) => {
                for value in fields.values() {
                    assert_source_ids_absent(value, response);
                }
            }
            _ => {}
        }
    }
    assert_source_ids_absent(&source, &serialized);
    for forbidden in ["synthetic-secret-200", "externalRef"] {
        assert!(!serialized.contains(forbidden));
    }
}
