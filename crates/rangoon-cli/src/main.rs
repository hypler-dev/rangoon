//! Local source analysis, portable bundle inspection, and inert engine diagnostics.
//! Input commands read stdin only; no filename is opened or executed by this CLI.
#![forbid(unsafe_code)]

use std::io::{self, Read, Write};
use std::process::ExitCode;

use rangoon_compile::bundle::MAX_BUNDLE_BYTES;
use rangoon_compile::{BundleError, inspect_bundle};
use rangoon_engine::{EngineOperation, GovernancePort, LnsatPlaceholder};
use rangoon_import::{MAX_SOURCE_BYTES, analyze, validate_display_name};
use serde_json::json;

const HELP: &str = "Rangoon source analyzer (development)\n\nUsage:\n  rangoon analyze --name DISPLAY_NAME.md < source.md\n  rangoon bundle inspect < exported.rangoon-instructions\n  rangoon bundle artifact < exported.rangoon-instructions\n  rangoon engine status\n  rangoon engine check --operation OPERATION\n\nOperations:\n  negotiate_contract | inspect_configuration | read_evidence\n  submit_operation | reconcile_operation\n\nAnalyze reads at most 256 KiB of UTF-8 Markdown from stdin. The name is a\ndisplay label, never a path to open. Bundle commands read a verified portable\nbundle from stdin; inspect reports internal consistency and artifact emits\nplaintext instruction bytes for independent inspection. Engine commands are\ninert diagnostics and never read stdin. No network, filesystem scanning,\nexecution, authorization, or persistence. Experimental v0 contract.\n";
const ARTIFACT_NOTICE: &[u8] = b"Rangoon: plaintext artifact; internal consistency only; target runtime untested; authority none.\n";

fn main() -> ExitCode {
    match run() {
        Ok(exit) => exit,
        Err(error) => {
            let payload = json!({
                "schemaVersion": "rangoon.cli-error.v0",
                "error": {"code": error.code, "message": error.message}
            });
            // Error messages are fixed strings; rejected input is never echoed.
            let _ = writeln!(io::stderr().lock(), "{payload}");
            ExitCode::from(error.exit)
        }
    }
}

#[derive(Debug)]
struct Failure {
    code: &'static str,
    message: &'static str,
    exit: u8,
}

fn run() -> Result<ExitCode, Failure> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && matches!(args[0].to_str(), Some("--help" | "-h")) {
        io::stdout()
            .lock()
            .write_all(HELP.as_bytes())
            .map_err(|_| output_failure())?;
        return Ok(ExitCode::SUCCESS);
    }
    if args.len() == 2 && args[0] == "engine" && args[1] == "status" {
        write_json(&LnsatPlaceholder.status())?;
        return Ok(ExitCode::SUCCESS);
    }
    if args.len() == 4 && args[0] == "engine" && args[1] == "check" && args[2] == "--operation" {
        let operation = args[3].to_str().and_then(parse_operation).ok_or(Failure {
            code: "usage",
            message: "expected: rangoon engine check --operation OPERATION",
            exit: 2,
        })?;
        write_json(&LnsatPlaceholder.invoke(operation))?;
        return Ok(ExitCode::from(4));
    }
    if args.first().is_some_and(|arg| arg == "engine") {
        return Err(Failure {
            code: "usage",
            message: "expected: rangoon engine status | engine check --operation OPERATION",
            exit: 2,
        });
    }
    if args.first().is_some_and(|arg| arg == "bundle") {
        let mut input = io::stdin().lock();
        let mut output = io::stdout().lock();
        let mut diagnostic = io::stderr().lock();
        run_bundle(&args, &mut input, &mut output, &mut diagnostic)?;
        return Ok(ExitCode::SUCCESS);
    }
    if args.len() != 3 || args[0] != "analyze" || args[1] != "--name" {
        return Err(Failure {
            code: "usage",
            message: "expected: rangoon analyze --name DISPLAY_NAME.md < source.md",
            exit: 2,
        });
    }
    let name = args[2].to_str().ok_or(Failure {
        code: "invalid_name",
        message: "display name must be UTF-8",
        exit: 2,
    })?;
    validate_display_name(name).map_err(|error| Failure {
        code: error.code.as_str(),
        message: error.message,
        exit: 2,
    })?;

    // One extra byte distinguishes an exact-limit input from a truncated input.
    // This is a byte bound, not a wall-clock deadline on a caller-controlled pipe.
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .take((MAX_SOURCE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Failure {
            code: "input_io",
            message: "could not read source bytes from stdin",
            exit: 3,
        })?;
    let report = analyze(name, &bytes).map_err(|error| Failure {
        code: error.code.as_str(),
        message: error.message,
        exit: 2,
    })?;
    write_json(&report)?;
    Ok(ExitCode::SUCCESS)
}

fn run_bundle(
    args: &[std::ffi::OsString],
    input: &mut impl Read,
    output: &mut impl Write,
    diagnostic: &mut impl Write,
) -> Result<(), Failure> {
    let command = match args {
        [bundle, command] if bundle == "bundle" && command == "inspect" => BundleCommand::Inspect,
        [bundle, command] if bundle == "bundle" && command == "artifact" => BundleCommand::Artifact,
        _ => {
            return Err(Failure {
                code: "usage",
                message: "expected: rangoon bundle inspect | bundle artifact < exported.rangoon-instructions",
                exit: 2,
            });
        }
    };

    // One extra byte distinguishes an exact-limit bundle from over-limit input.
    // This is a byte bound, not a wall-clock deadline on a caller-controlled pipe.
    let mut bytes = Vec::new();
    input
        .take((MAX_BUNDLE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Failure {
            code: "input_io",
            message: "could not read instruction bundle bytes from stdin",
            exit: 3,
        })?;
    if bytes.len() > MAX_BUNDLE_BYTES {
        return Err(bundle_too_large());
    }
    let inspection = inspect_bundle(&bytes).map_err(bundle_failure)?;

    match command {
        BundleCommand::Inspect => write_bundle_json(output, &inspection),
        BundleCommand::Artifact => {
            // This notice is deliberately committed before plaintext output begins.
            diagnostic
                .write_all(ARTIFACT_NOTICE)
                .and_then(|_| diagnostic.flush())
                .map_err(|_| bundle_output_failure())?;
            output
                .write_all(inspection.compilation.artifact.content.as_bytes())
                .and_then(|_| output.flush())
                .map_err(|_| bundle_output_failure())
        }
    }
}

enum BundleCommand {
    Inspect,
    Artifact,
}

fn bundle_failure(error: BundleError) -> Failure {
    match error {
        BundleError::TooLarge => bundle_too_large(),
        BundleError::Invalid | BundleError::NotCandidate => Failure {
            code: "bundle_invalid",
            message: "could not verify instruction bundle",
            exit: 2,
        },
        BundleError::SerializationFailed => Failure {
            code: "bundle_serialization_failed",
            message: "could not verify instruction bundle",
            exit: 3,
        },
    }
}

fn bundle_too_large() -> Failure {
    Failure {
        code: "bundle_too_large",
        message: "instruction bundle exceeds the size limit",
        exit: 2,
    }
}

fn write_bundle_json(
    output: &mut impl Write,
    value: &impl serde::Serialize,
) -> Result<(), Failure> {
    let encoded = serde_json::to_vec(value).map_err(|_| Failure {
        code: "serialization",
        message: "could not encode bundle inspection output",
        exit: 3,
    })?;
    output
        .write_all(&encoded)
        .and_then(|_| output.write_all(b"\n"))
        .and_then(|_| output.flush())
        .map_err(|_| bundle_output_failure())
}

fn bundle_output_failure() -> Failure {
    Failure {
        code: "output_io",
        message: "could not write instruction bundle output",
        exit: 3,
    }
}

fn parse_operation(value: &str) -> Option<EngineOperation> {
    match value {
        "negotiate_contract" => Some(EngineOperation::NegotiateContract),
        "inspect_configuration" => Some(EngineOperation::InspectConfiguration),
        "read_evidence" => Some(EngineOperation::ReadEvidence),
        "submit_operation" => Some(EngineOperation::SubmitOperation),
        "reconcile_operation" => Some(EngineOperation::ReconcileOperation),
        _ => None,
    }
}

fn write_json(value: &impl serde::Serialize) -> Result<(), Failure> {
    let encoded = serde_json::to_vec(value).map_err(|_| Failure {
        code: "serialization",
        message: "could not encode JSON output",
        exit: 3,
    })?;
    let mut output = io::stdout().lock();
    output.write_all(&encoded).map_err(|_| output_failure())?;
    output.write_all(b"\n").map_err(|_| output_failure())
}

fn output_failure() -> Failure {
    Failure {
        code: "output_io",
        message: "could not write the analysis report",
        exit: 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    enum WriterFailure {
        Write,
        Flush,
    }

    struct FailingWriter {
        failure: WriterFailure,
        bytes: Vec<u8>,
    }

    impl FailingWriter {
        fn write() -> Self {
            Self {
                failure: WriterFailure::Write,
                bytes: Vec::new(),
            }
        }

        fn flush() -> Self {
            Self {
                failure: WriterFailure::Flush,
                bytes: Vec::new(),
            }
        }
    }

    impl Write for FailingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            match self.failure {
                WriterFailure::Write => Err(io::Error::other("injected write failure")),
                WriterFailure::Flush => {
                    self.bytes.extend_from_slice(bytes);
                    Ok(bytes.len())
                }
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            match self.failure {
                WriterFailure::Write => Ok(()),
                WriterFailure::Flush => Err(io::Error::other("injected flush failure")),
            }
        }
    }

    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("injected read failure"))
        }
    }

    struct CountingReader {
        remaining: usize,
        read: usize,
    }

    impl Read for CountingReader {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            let count = self.remaining.min(bytes.len());
            bytes[..count].fill(b'x');
            self.remaining -= count;
            self.read += count;
            Ok(count)
        }
    }

    fn bundle_bytes() -> Vec<u8> {
        let vectors: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../fixtures/compilation/instruction-bundle-v1.json"
        ))
        .unwrap();
        let hex = vectors["cases"].as_array().unwrap().first().unwrap()["bundleHex"]
            .as_str()
            .unwrap()
            .as_bytes();
        hex.chunks_exact(2)
            .map(|pair| {
                fn nibble(byte: u8) -> u8 {
                    match byte {
                        b'0'..=b'9' => byte - b'0',
                        b'a'..=b'f' => byte - b'a' + 10,
                        _ => panic!("fixture must be lowercase hex"),
                    }
                }
                (nibble(pair[0]) << 4) | nibble(pair[1])
            })
            .collect()
    }

    fn args(command: &str) -> Vec<std::ffi::OsString> {
        ["bundle", command].map(std::ffi::OsString::from).to_vec()
    }

    #[test]
    fn inspect_never_touches_stderr_writer() {
        let mut input = Cursor::new(bundle_bytes());
        let mut output = Vec::new();
        let mut diagnostic = FailingWriter::write();
        run_bundle(&args("inspect"), &mut input, &mut output, &mut diagnostic).unwrap();
        assert!(!output.is_empty());
        assert!(diagnostic.bytes.is_empty());
    }

    #[test]
    fn artifact_stderr_failures_emit_no_artifact() {
        for mut diagnostic in [FailingWriter::write(), FailingWriter::flush()] {
            let mut input = Cursor::new(bundle_bytes());
            let mut output = Vec::new();
            let failure = run_bundle(&args("artifact"), &mut input, &mut output, &mut diagnostic)
                .unwrap_err();
            assert_eq!(failure.exit, 3);
            assert!(output.is_empty());
        }
    }

    #[test]
    fn stdout_failures_are_exit_three_for_both_bundle_commands() {
        for command in ["inspect", "artifact"] {
            for mut output in [FailingWriter::write(), FailingWriter::flush()] {
                let mut input = Cursor::new(bundle_bytes());
                let mut diagnostic = Vec::new();
                let failure = run_bundle(&args(command), &mut input, &mut output, &mut diagnostic)
                    .unwrap_err();
                assert_eq!(failure.exit, 3, "{command}");
            }
        }
    }

    #[test]
    fn stdin_failure_is_exit_three_before_output() {
        let mut input = FailingReader;
        let mut output = Vec::new();
        let mut diagnostic = Vec::new();
        let failure =
            run_bundle(&args("inspect"), &mut input, &mut output, &mut diagnostic).unwrap_err();
        assert_eq!(failure.code, "input_io");
        assert_eq!(failure.exit, 3);
        assert!(output.is_empty());
        assert!(diagnostic.is_empty());
    }

    #[test]
    fn stdin_reads_exactly_one_byte_past_the_bundle_limit() {
        let mut input = CountingReader {
            remaining: MAX_BUNDLE_BYTES + 2,
            read: 0,
        };
        let mut output = Vec::new();
        let mut diagnostic = Vec::new();
        let failure =
            run_bundle(&args("inspect"), &mut input, &mut output, &mut diagnostic).unwrap_err();
        assert_eq!(failure.code, "bundle_too_large");
        assert_eq!(failure.exit, 2);
        assert_eq!(input.read, MAX_BUNDLE_BYTES + 1);
        assert!(output.is_empty());
        assert!(diagnostic.is_empty());
    }
}
