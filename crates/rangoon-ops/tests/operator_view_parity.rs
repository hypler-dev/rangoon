use rangoon_ops::{
    build_operator_view, decode_window, inspect_approvals, inspect_delegations, inspect_lifecycle,
    project_window, relate_window,
};

const INPUT: &[u8] =
    include_bytes!("../../../fixtures/operations/synthetic-decision-window-v1.json");
const EXPECTED: &str = include_str!("../../../fixtures/operations/synthetic-decision-view-v1.json");

#[test]
fn decision_window_matches_renderer_safe_golden() {
    let decoded = decode_window(INPUT).expect("fixture decodes");
    let projection = project_window(&decoded).expect("projection");
    let relationships = relate_window(&decoded).expect("relationships");
    let delegations = inspect_delegations(&decoded).expect("delegations");
    let approvals = inspect_approvals(&decoded).expect("approvals");
    let lifecycle = inspect_lifecycle(&decoded).expect("lifecycle");
    let view = build_operator_view(
        &decoded,
        &projection,
        &relationships,
        &delegations,
        &approvals,
        &lifecycle,
    )
    .expect("operator view");
    let actual = serde_json::to_string(view.document()).expect("serialize");

    assert_eq!(actual, EXPECTED.trim_end());
    assert!(
        std::str::from_utf8(INPUT)
            .expect("fixture UTF-8")
            .contains("synthetic-decision-window-sentinel")
    );
    for source_only in [
        "synthetic-decision-window-sentinel",
        "ops:0000000000000000000000000000000000000000000000000000000000000001",
        "ops:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        assert!(
            !actual.contains(source_only),
            "source field leaked: {source_only}"
        );
    }
    assert!(!actual.contains("externalRef"));
    assert!(!actual.contains("canonicalDigest"));
    let parsed: serde_json::Value = serde_json::from_str(&actual).expect("JSON view");
    assert_eq!(parsed["authority"], "none");
    assert_eq!(parsed["authenticity"], "unverified");
    assert_eq!(parsed["eligibility"], "unknown");
    assert_eq!(parsed["execution"], "unavailable");
    assert_eq!(parsed["outcome"], "unknown");
    assert_eq!(parsed["coverage"], "unknown");
    assert_eq!(parsed["structuralState"], "conflict");
    assert!(
        parsed["events"]
            .as_array()
            .expect("events array")
            .iter()
            .any(|event| event["structuralDiagnostics"]
                .as_array()
                .expect("diagnostics array")
                .iter()
                .any(|code| code == "event_identity_conflict"))
    );
    assert_eq!(parsed["events"].as_array().map(Vec::len), Some(24));
    assert_eq!(
        parsed["decisionCounts"],
        serde_json::json!({
            "evaluation": {"allow": 1, "deny": 1, "unknown": 1},
            "approvalDecision": {"approve": 1, "deny": 1, "abstain": 1},
            "authorization": {"reportedIssued": 1, "reportedRefused": 1, "reportedExpired": 1, "reportedRevoked": 1, "unknown": 1},
            "attempt": {"reportedAccepted": 1, "reportedActive": 1, "reportedCompleted": 1, "reportedFailed": 1, "reportedCancelled": 1, "outcomeUnknown": 1, "reportedReconciledCompleted": 1, "reportedReconciledFailed": 1},
        })
    );
    for (kind, total) in [
        ("evaluation", 3),
        ("approval_decision", 3),
        ("authorization", 5),
        ("attempt", 8),
    ] {
        assert_eq!(
            parsed["sourceCounts"]["recordCounts"]
                .as_array()
                .and_then(|counts| counts.iter().find(|row| row["kind"] == kind))
                .map(|row| &row["count"]),
            Some(&serde_json::json!(total)),
            "{kind} keeps its observed-record denominator"
        );
    }
    assert_eq!(parsed["events"][21]["recordKind"], "attempt");
    assert_eq!(parsed["events"][21]["reportedValue"], "outcome_unknown");
    assert!(
        parsed["events"]
            .as_array()
            .expect("events array")
            .iter()
            .any(|event| event["references"].as_array().is_some_and(|refs| {
                refs.iter()
                    .any(|reference| reference["status"] == "missing")
            }))
    );
    assert!(
        parsed["events"]
            .as_array()
            .expect("events array")
            .iter()
            .any(|event| !event["structuralDiagnostics"]
                .as_array()
                .expect("diagnostics array")
                .is_empty()
                || !event["issueStages"]
                    .as_array()
                    .expect("stages array")
                    .is_empty())
    );
}

const LOCAL_INPUT: &[u8] =
    include_bytes!("../../../fixtures/operations/synthetic-local-window-v1.json");
const LOCAL_EXPECTED: &str =
    include_str!("../../../fixtures/operations/synthetic-operator-view-v1.json");

#[test]
fn populated_local_window_matches_renderer_safe_golden() {
    let decoded = decode_window(LOCAL_INPUT).expect("fixture decodes");
    let projection = project_window(&decoded).expect("projection");
    let relationships = relate_window(&decoded).expect("relationships");
    let delegations = inspect_delegations(&decoded).expect("delegations");
    let approvals = inspect_approvals(&decoded).expect("approvals");
    let lifecycle = inspect_lifecycle(&decoded).expect("lifecycle");
    let view = build_operator_view(
        &decoded,
        &projection,
        &relationships,
        &delegations,
        &approvals,
        &lifecycle,
    )
    .expect("operator view");
    let actual = serde_json::to_string(view.document()).expect("serialize");

    assert_eq!(actual, LOCAL_EXPECTED.trim_end());
    assert!(
        std::str::from_utf8(LOCAL_INPUT)
            .expect("fixture UTF-8")
            .contains("synthetic-secret-200")
    );
    assert!(!actual.contains("synthetic-secret-200"));
    assert!(!actual.contains("externalRef"));
    assert!(!actual.contains("canonicalDigest"));
    let parsed: serde_json::Value = serde_json::from_str(&actual).expect("JSON view");
    assert_eq!(parsed["events"].as_array().map(Vec::len), Some(2));
}
