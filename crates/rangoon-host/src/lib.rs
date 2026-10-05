//! Bounded reads for a host-selected file. The caller owns the selection UI.
//! This is not a generic frontend path API or a directory confinement boundary.
#![forbid(unsafe_code)]

use std::fs::{self, File, Metadata, OpenOptions};
use std::io::Read;
use std::path::Path;

use rangoon_domain::AnalysisReport;
use rangoon_import::{MAX_SOURCE_BYTES, analyze, validate_display_name};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SelectionResult {
    Cancelled,
    Analyzed { report: Box<AnalysisReport> },
    Rejected { error: PublicError },
    Failed { error: PublicError },
}

#[derive(Debug, Serialize)]
pub struct PublicError {
    pub code: &'static str,
    pub message: &'static str,
}

impl SelectionResult {
    pub fn failed(code: &'static str, message: &'static str) -> Self {
        Self::Failed {
            error: PublicError { code, message },
        }
    }

    fn rejected(code: &'static str, message: &'static str) -> Self {
        Self::Rejected {
            error: PublicError { code, message },
        }
    }
}

/// Call only with the result of the native one-file picker. No path is returned
/// to the frontend. The byte digest identifies what was read, not immutable
/// filesystem state or an authenticated origin. Parent directories are not held.
pub fn analyze_selected_path(path: &Path) -> SelectionResult {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return SelectionResult::rejected(
            "invalid_name",
            "Choose a Markdown file with a UTF-8 name.",
        );
    };
    if let Err(error) = validate_display_name(name) {
        return SelectionResult::rejected(error.code.as_str(), error.message);
    }
    let Ok(before) = fs::symlink_metadata(path) else {
        return read_failed();
    };
    if !regular_file(&before) {
        return unsupported_file();
    }
    let Ok(file) = open_selected(path) else {
        return read_failed();
    };
    let Ok(metadata) = file.metadata() else {
        return read_failed();
    };
    if !regular_file(&metadata) {
        return unsupported_file();
    }
    if metadata.len() > MAX_SOURCE_BYTES as u64 {
        return SelectionResult::rejected(
            "input_too_large",
            "Choose a file no larger than 256 KiB.",
        );
    }
    let mut bytes = Vec::new();
    if file
        .take((MAX_SOURCE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return read_failed();
    }
    match analyze(name, &bytes) {
        Ok(report) => SelectionResult::Analyzed {
            report: Box::new(report),
        },
        Err(error) => SelectionResult::rejected(error.code.as_str(), error.message),
    }
}

fn read_failed() -> SelectionResult {
    SelectionResult::failed(
        "read_failed",
        "The selected file could not be read. Choose it again or check its permissions.",
    )
}

fn unsupported_file() -> SelectionResult {
    SelectionResult::rejected(
        "unsupported_file",
        "Choose a regular file, not a folder, link or special file.",
    )
}

fn regular_file(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Reject all reparse points, not only links recognized by std.
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    metadata.file_type().is_file() && !metadata.file_type().is_symlink()
}

fn open_selected(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Do not follow a substituted final symlink or block opening a FIFO.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Inspect the opened reparse point itself before permitting any read.
        options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    options.open(path)
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;
    use std::os::windows::fs::{MetadataExt, symlink_file};

    #[test]
    fn opened_reparse_handle_is_rejected_before_reading() {
        let directory =
            std::env::temp_dir().join(format!("rangoon-open-reparse-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let target = directory.join("target.md");
        let selected = directory.join("selected.md");
        fs::write(&target, b"# target must not be read\n").unwrap();
        fs::write(&selected, b"# original selection\n").unwrap();
        assert!(regular_file(&fs::symlink_metadata(&selected).unwrap()));
        // Deterministically reproduce a replacement after the pre-open check.
        fs::remove_file(&selected).unwrap();
        symlink_file(&target, &selected).expect("Windows test requires symlink privilege");
        let handle = open_selected(&selected).unwrap();
        let metadata = handle.metadata().unwrap();
        assert_ne!(metadata.file_attributes() & 0x400, 0);
        assert!(!regular_file(&metadata));
        drop(handle);
        fs::remove_file(selected).unwrap();
        fs::remove_file(target).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
