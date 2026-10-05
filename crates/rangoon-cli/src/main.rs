//! Local source analysis and inert engine diagnostics.
//! Analyze reads stdin only; no filename is opened or executed by this CLI.
#![forbid(unsafe_code)]

use std::io::{self, Read, Write};
use std::process::ExitCode;

use rangoon_engine::{EngineOperation, GovernancePort, LnsatPlaceholder};
use rangoon_import::{MAX_SOURCE_BYTES, analyze, validate_display_name};
use serde_json::json;

const HELP: &str = "Rangoon source analyzer (development)\n\nUsage:\n  rangoon analyze --name DISPLAY_NAME.md < source.md\n  rangoon engine status\n  rangoon engine check --operation OPERATION\n\nOperations:\n  negotiate_contract | inspect_configuration | read_evidence\n  submit_operation | reconcile_operation\n\nAnalyze reads at most 256 KiB of UTF-8 Markdown from stdin. The name is a\ndisplay label, never a path to open. Engine commands are inert diagnostics and\nnever read stdin. No network, filesystem scanning, execution, authorization,\nor persistence. Experimental v0 contract.\n";

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
