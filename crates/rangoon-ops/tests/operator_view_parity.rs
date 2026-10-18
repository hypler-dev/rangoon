use rangoon_ops::{
    build_operator_view, decode_window, inspect_approvals, inspect_delegations, inspect_lifecycle,
    project_window, relate_window,
};

const INPUT: &[u8] = include_bytes!("../../../fixtures/operations/synthetic-local-window-v1.json");
const EXPECTED: &str = include_str!("../../../fixtures/operations/synthetic-operator-view-v1.json");

#[test]
fn populated_local_window_matches_renderer_safe_golden() {
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
            .contains("synthetic-secret-200")
    );
    assert!(!actual.contains("synthetic-secret-200"));
    assert!(!actual.contains("externalRef"));
    assert!(!actual.contains("canonicalDigest"));
    let parsed: serde_json::Value = serde_json::from_str(&actual).expect("JSON view");
    assert_eq!(parsed["events"].as_array().map(Vec::len), Some(2));
}
