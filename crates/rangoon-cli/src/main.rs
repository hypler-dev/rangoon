//! Local stdin-only entry point. No filename is opened or executed by this CLI.
#![forbid(unsafe_code)]

use std::io::{self, Read, Write};
use std::process::ExitCode;

use rangoon_import::{MAX_SOURCE_BYTES, analyze, validate_display_name};
use serde_json::json;

const HELP: &str = "Rangoon source analyzer (development)\n\nUsage: rangoon analyze --name DISPLAY_NAME.md < source.md\n\nReads at most 256 KiB of UTF-8 Markdown from stdin. The name is a display\nlabel, never a path to open. Emits JSON containing original source text,\nhashes, exact spans and unreviewed section proposals. No network, filesystem\nscanning, execution, authorization, or persistence. Experimental v0 contract.\n";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
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

fn run() -> Result<(), Failure> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && matches!(args[0].to_str(), Some("--help" | "-h")) {
        return io::stdout()
            .lock()
            .write_all(HELP.as_bytes())
            .map_err(|_| output_failure());
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
    let encoded = serde_json::to_vec(&report).map_err(|_| Failure {
        code: "serialization",
        message: "could not encode the analysis report",
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
