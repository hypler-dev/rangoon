use super::*;

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
