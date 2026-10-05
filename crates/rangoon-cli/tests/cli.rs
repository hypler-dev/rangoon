use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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

fn run_without_stdin(args: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rangoon"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("test binary starts");
    let stdin = child.stdin.take().expect("stdin pipe");
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill stdin-reading engine command");
            drop(stdin);
            let _ = child.wait_with_output();
            panic!("engine command read stdin");
        }
        thread::sleep(Duration::from_millis(10));
    }
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

#[test]
fn engine_status_reports_unknown_runtime_without_reading_stdin() {
    let output = run_without_stdin(&["engine", "status"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(status["schemaVersion"], "rangoon.engine-status.v0");
    assert_eq!(status["connectionState"], "not_attempted");
    assert_eq!(status["runtimeState"], "not_checked");
    assert!(status["installedVersion"].is_null());
    assert_eq!(status["executionAuthority"], "none");
    assert_eq!(status["networkAttempted"], false);
}

#[test]
fn engine_check_returns_unavailable_exit_and_never_echoes_bad_operation() {
    for operation in [
        "negotiate_contract",
        "inspect_configuration",
        "read_evidence",
        "submit_operation",
        "reconcile_operation",
    ] {
        let output = run_without_stdin(&["engine", "check", "--operation", operation]);
        assert_eq!(output.status.code(), Some(4));
        assert!(output.stderr.is_empty());
        let diagnostic: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(diagnostic["operation"], operation);
        assert_eq!(diagnostic["outcome"], "unavailable");
        assert_eq!(diagnostic["executionAuthorized"], false);
        assert_eq!(diagnostic["mutationAuthority"], false);
        assert_eq!(diagnostic["sideEffects"], serde_json::json!([]));
    }

    for args in [
        vec!["engine"],
        vec!["engine", "status", "extra"],
        vec!["engine", "check"],
        vec!["engine", "check", "--operation", "activate-secret"],
    ] {
        let output = run(&args, b"secret stdin");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "usage");
        assert!(!String::from_utf8_lossy(&output.stderr).contains("activate-secret"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("secret stdin"));
    }
}
