use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn source_contract_matches_cross_platform_golden_fixture() {
    let input = include_bytes!("../../../fixtures/contracts/AGENTS.md");
    let expected: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../fixtures/contracts/source-analysis.golden.json"
    ))
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_rangoon"))
        .args(["analyze", "--name", "AGENTS.md"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let actual: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual, expected);
}
