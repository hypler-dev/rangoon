use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use rangoon_compile::{Profile, compile, encode_bundle};
use rangoon_domain::{
    byte_digest,
    capability::{self, ContentReview, Reviewer},
    capability_v1::{Revision, RevisionProvenance},
};

const NOTICE: &str = "Rangoon: plaintext artifact; internal consistency only; target runtime untested; authority none.\n";

fn run(args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rangoon"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("bundle CLI starts");
    let mut stdin = child.stdin.take().expect("stdin pipe");
    let _ = stdin.write_all(input);
    drop(stdin);
    child.wait_with_output().expect("bundle CLI completes")
}

fn run_without_stdin(args: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rangoon"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("bundle CLI starts");
    let stdin = child.stdin.take().expect("stdin pipe");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill overdue bundle command");
            drop(stdin);
            let _ = child.wait_with_output();
            panic!("bundle argument rejection read stdin");
        }
        thread::sleep(Duration::from_millis(10));
    }
    drop(stdin);
    child.wait_with_output().expect("bundle CLI completes")
}

fn hex(input: &str) -> Vec<u8> {
    fn nibble(byte: u8) -> u8 {
        match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => panic!("fixture has non-hex byte"),
        }
    }
    input
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| (nibble(pair[0]) << 4) | nibble(pair[1]))
        .collect()
}

fn vectors() -> serde_json::Value {
    serde_json::from_slice(include_bytes!(
        "../../../fixtures/compilation/instruction-bundle-v1.json"
    ))
    .expect("independent bundle corpus parses")
}

fn case(name: &str) -> serde_json::Value {
    let vector = vectors();
    vector["cases"]
        .as_array()
        .expect("case list")
        .iter()
        .find(|case| case["name"] == name)
        .cloned()
        .expect("named case")
}

fn maximum_bundle() -> (Vec<u8>, serde_json::Value) {
    let expected = vectors()["maximumContent"].clone();
    let input = &expected["input"];
    let capability_id = input["capabilityId"].as_str().unwrap();
    let title = input["title"].as_str().unwrap();
    let content = input["repeat"]
        .as_str()
        .unwrap()
        .repeat(input["repeatCount"].as_u64().unwrap() as usize);
    let revision = Revision {
        id: capability::revision_id(capability_id, None, title, &content),
        parent_revision_id: None,
        title: title.into(),
        sha256: byte_digest(content.as_bytes()),
        content,
        created_at_ms: input["createdAtMs"].as_i64().unwrap(),
        review: Some(ContentReview {
            reviewer: Reviewer::LocalOperator,
            reviewed_at_ms: input["reviewedAtMs"].as_i64().unwrap(),
        }),
        provenance: RevisionProvenance::Ordinary {},
    };
    let report = compile(capability_id, &revision, Profile::AgentsMdV1, &[]).unwrap();
    let encoded = encode_bundle(&report).unwrap();
    assert_eq!(encoded.inspection.bundle_id, expected["bundleId"]);
    assert_eq!(encoded.inspection.sha256, expected["sha256"]);
    assert_eq!(encoded.inspection.byte_length, expected["byteLength"]);
    assert_eq!(
        encoded.inspection.compilation.compilation_id,
        expected["compilationId"]
    );
    (encoded.bytes, expected)
}

#[test]
fn inspect_and_artifact_preserve_both_profile_golden_outputs() {
    for (name, expected_bytes) in [
        (
            "agents_exact_bom_crlf_unicode",
            b"\xef\xbb\xbf# Review\r\nKeep originals \xf0\x9f\xa6\x80.\r\nNo final newline"
                .as_slice(),
        ),
        (
            "claude_exact_bom_crlf_unicode",
            b"\xef\xbb\xbf# Review\r\nKeep originals \xf0\x9f\xa6\x80.\r\nNo final newline"
                .as_slice(),
        ),
    ] {
        let vector = case(name);
        let input = hex(vector["bundleHex"].as_str().unwrap());
        let inspect = run(&["bundle", "inspect"], &input);
        assert!(inspect.status.success(), "{name}: {inspect:?}");
        assert!(inspect.stderr.is_empty());
        assert_eq!(inspect.stdout.last(), Some(&b'\n'));
        let report: serde_json::Value = serde_json::from_slice(&inspect.stdout).unwrap();
        assert_eq!(report["bundleId"], vector["bundleId"]);
        assert_eq!(report["sha256"], vector["sha256"]);
        assert_eq!(report["byteLength"], vector["byteLength"]);
        assert_eq!(
            report["compilation"]["compilationId"],
            vector["compilationId"]
        );
        assert_eq!(
            report["compilation"]["candidate"]["candidateId"],
            vector["candidateId"]
        );
        assert_eq!(
            report["compilation"]["artifact"]["content"].as_str(),
            Some(String::from_utf8_lossy(expected_bytes).as_ref())
        );
        assert_eq!(report["compilation"]["authority"], "none");
        assert_eq!(
            report["compilation"]["profile"]["runtimeQualification"],
            "untested"
        );
        assert_eq!(
            report["compilation"]["profile"]["semanticEquivalence"],
            "unverified"
        );
        assert_eq!(report["authority"], "none");

        let artifact = run(&["bundle", "artifact"], &input);
        assert!(artifact.status.success(), "{name}: {artifact:?}");
        assert_eq!(artifact.stdout, expected_bytes);
        assert_eq!(artifact.stderr, NOTICE.as_bytes());
        assert_eq!(artifact.stdout, run(&["bundle", "artifact"], &input).stdout);
    }
}

#[test]
fn artifact_preserves_the_maximum_golden_content() {
    let (input, expected) = maximum_bundle();
    let artifact = run(&["bundle", "artifact"], &input);
    assert!(artifact.status.success());
    assert_eq!(artifact.stderr, NOTICE.as_bytes());
    assert_eq!(
        artifact.stdout.len(),
        expected["artifactByteLength"].as_u64().unwrap() as usize
    );
    assert!(artifact.stdout.iter().all(|byte| *byte == b'x'));

    let inspect = run(&["bundle", "inspect"], &input);
    assert!(inspect.status.success());
    assert!(inspect.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&inspect.stdout).unwrap();
    assert_eq!(report["bundleId"], expected["bundleId"]);
    assert_eq!(report["sha256"], expected["sha256"]);
    assert_eq!(report["byteLength"], expected["byteLength"]);
    assert_eq!(
        report["compilation"]["artifact"]["byteLength"],
        expected["artifactByteLength"]
    );
}

#[test]
fn bundle_rejections_have_no_stdout_and_do_not_echo_input() {
    let vector = case("agents_exact_bom_crlf_unicode");
    let valid = hex(vector["bundleHex"].as_str().unwrap());
    let mut checksum_invalid = valid.clone();
    *checksum_invalid.last_mut().unwrap() = b'0';
    let mut trailing = valid.clone();
    trailing.push(b'x');
    let mut short_oversized_header = b"RANGOON-INSTRUCTIONS-V1\n".to_vec();
    short_oversized_header.extend_from_slice(&32_769_u64.to_be_bytes());
    short_oversized_header.extend_from_slice(&1_u64.to_be_bytes());
    let mut oversized_header = short_oversized_header.clone();
    oversized_header.resize(106, b'x');
    let mut zero_then_oversized = b"RANGOON-INSTRUCTIONS-V1\n".to_vec();
    zero_then_oversized.extend_from_slice(&0_u64.to_be_bytes());
    zero_then_oversized.extend_from_slice(&262_145_u64.to_be_bytes());
    zero_then_oversized.resize(106, b'x');
    let oversized_input = vec![b'x'; rangoon_compile::bundle::MAX_BUNDLE_BYTES + 1];

    for (input, code) in [
        (b"secret malformed input".as_slice(), "bundle_invalid"),
        (&valid[..valid.len() - 1], "bundle_invalid"),
        (&checksum_invalid, "bundle_invalid"),
        (&trailing, "bundle_invalid"),
        (&short_oversized_header, "bundle_invalid"),
        (&oversized_header, "bundle_too_large"),
        (&zero_then_oversized, "bundle_invalid"),
        (&oversized_input, "bundle_too_large"),
    ] {
        for command in ["inspect", "artifact"] {
            let output = run(&["bundle", command], input);
            assert_eq!(output.status.code(), Some(2), "{command}");
            assert!(output.stdout.is_empty(), "{command}");
            let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
            assert_eq!(error["schemaVersion"], "rangoon.cli-error.v0");
            assert_eq!(error["error"]["code"], code);
            assert!(!String::from_utf8_lossy(&output.stderr).contains("secret malformed input"));
        }
    }
}

#[test]
fn bundle_arguments_reject_before_stdin_read() {
    for args in [
        vec!["bundle"],
        vec!["bundle", "unknown"],
        vec!["bundle", "inspect", "extra"],
        vec!["bundle", "artifact", "secret"],
    ] {
        let output = run_without_stdin(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "usage");
        assert!(!String::from_utf8_lossy(&output.stderr).contains("secret"));
    }
}
