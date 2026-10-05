use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rangoon"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("test binary starts");
    let mut stdin = child.stdin.take().expect("stdin pipe");
    // Invalid arguments can exit before reading stdin, so a broken pipe is valid.
    let _ = stdin.write_all(input);
    drop(stdin);
    child.wait_with_output().expect("test binary completes")
}

#[test]
fn analyzes_stdin_without_opening_display_name_and_preserves_bytes() {
    let bytes = b"# Example\r\nNever execute this input.\r\n## Review\r\nRetain provenance.\r\n";
    let args = ["analyze", "--name", "not-a-real-file.md"];
    let output = run(&args, bytes);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schemaVersion"], "rangoon.source-analysis.v0");
    assert_eq!(
        report["source"]["content"],
        std::str::from_utf8(bytes).unwrap()
    );
    assert_eq!(report["authority"], "none");
    assert_eq!(report["fragments"].as_array().unwrap().len(), 2);
    assert_eq!(report["fragments"][0]["reviewState"], "unreviewed");
    assert_eq!(output.stdout, run(&args, bytes).stdout);
}

#[test]
fn rejects_invalid_input_without_stdout_or_echoing_source() {
    for (name, input, code) in [
        (
            "../private.md",
            b"private source".as_slice(),
            "invalid_name",
        ),
        (
            "script.sh",
            b"private source".as_slice(),
            "unsupported_format",
        ),
        ("AGENTS.md", b"private\0source".as_slice(), "binary_input"),
        ("AGENTS.md", &[0xff], "invalid_utf8"),
    ] {
        let output = run(&["analyze", "--name", name], input);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], code);
        assert!(!String::from_utf8_lossy(&output.stderr).contains("private source"));
    }
}

#[test]
fn rejects_oversized_input_instead_of_silently_truncating() {
    let input = vec![b'a'; rangoon_import::MAX_SOURCE_BYTES + 1];
    let output = run(&["analyze", "--name", "large.md"], &input);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "input_too_large");
}

#[test]
fn help_and_usage_do_not_need_input() {
    let help = run(&["--help"], b"");
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("No network"));
    for args in [
        vec![],
        vec!["analyze"],
        vec!["analyze", "--name", "a.md", "extra"],
    ] {
        let output = run(&args, b"");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}
