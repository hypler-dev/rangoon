//! Bounded instruction-bundle I/O for native picker selections only.
//! Parent directories are not held: these helpers do not confine a directory.
use super::{PublicError, open_selected, regular_file};
use rangoon_compile::bundle::{MAX_BUNDLE_BYTES, inspect_bundle};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

fn error(code: &'static str, message: &'static str) -> PublicError {
    PublicError { code, message }
}

fn too_large() -> PublicError {
    error(
        "bundle_too_large",
        "Instruction bundles must not exceed 295,016 bytes.",
    )
}

fn invalid() -> PublicError {
    error(
        "bundle_invalid",
        "The instruction bundle is invalid or inconsistent. No output was created by this attempt.",
    )
}

fn failed_write() -> PublicError {
    error(
        "instruction_bundle_write_failed",
        "Export did not finish. A partial or complete file may remain at the selected location. Choose a new name before retrying.",
    )
}

fn portable_component(name: &str) -> bool {
    if name.is_empty()
        || name.trim() != name
        || name.ends_with('.')
        || name.chars().any(|c| {
            c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
        })
    {
        return false;
    }
    let first = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end()
        .to_ascii_uppercase();
    if matches!(first.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return false;
    }
    for prefix in ["COM", "LPT"] {
        if first.strip_prefix(prefix).is_some_and(|number| {
            matches!(
                number,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        }) {
            return false;
        }
    }
    true
}

fn selected_path(path: &Path) -> Result<(), PublicError> {
    let failure = || {
        error(
            "instruction_bundle_name",
            "Choose an absolute file location with a portable name ending in .rangoon-instructions. Device names and stream syntax are not allowed.",
        )
    };
    if !path.is_absolute() {
        return Err(failure());
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(failure)?;
    let stem = name
        .strip_suffix(".rangoon-instructions")
        .ok_or_else(failure)?;
    if !portable_component(stem) || !portable_component(name) {
        return Err(failure());
    }
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        for component in path.components() {
            match component {
                Component::Prefix(prefix) => match prefix.kind() {
                    Prefix::Disk(_)
                    | Prefix::VerbatimDisk(_)
                    | Prefix::UNC(_, _)
                    | Prefix::VerbatimUNC(_, _) => {}
                    _ => return Err(failure()),
                },
                Component::Normal(name) => {
                    if !name.to_str().is_some_and(portable_component) {
                        return Err(failure());
                    }
                }
                Component::ParentDir | Component::CurDir => return Err(failure()),
                Component::RootDir => {}
            }
        }
    }
    Ok(())
}

/// Reads bounded file bytes; the caller must inspect them before presenting any
/// content or candidate evidence. No path is returned to a renderer.
pub fn read_selected_instruction_bundle(path: &Path) -> Result<Vec<u8>, PublicError> {
    selected_path(path)?;
    let failure = || {
        error(
            "instruction_bundle_read_failed",
            "The selected instruction bundle could not be read. Choose a regular local file again.",
        )
    };
    let before = fs::symlink_metadata(path).map_err(|_| failure())?;
    if !regular_file(&before) {
        return Err(failure());
    }
    let file = open_selected(path).map_err(|_| failure())?;
    let metadata = file.metadata().map_err(|_| failure())?;
    if !regular_file(&metadata) {
        return Err(failure());
    }
    if metadata.len() > MAX_BUNDLE_BYTES as u64 {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    file.take((MAX_BUNDLE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| failure())?;
    if bytes.len() > MAX_BUNDLE_BYTES {
        return Err(too_large());
    }
    Ok(bytes)
}

/// Checks the complete format before exclusively creating an output. Successful
/// write/sync is not authentication, directory durability or future integrity.
pub fn write_selected_instruction_bundle(path: &Path, bytes: &[u8]) -> Result<(), PublicError> {
    write_selected_with(path, bytes, |file, bytes| {
        file.write_all(bytes)?;
        file.sync_all()
    })
}

fn write_selected_with(
    path: &Path,
    bytes: &[u8],
    finish: impl FnOnce(&mut File, &[u8]) -> std::io::Result<()>,
) -> Result<(), PublicError> {
    selected_path(path)?;
    if bytes.len() > MAX_BUNDLE_BYTES {
        return Err(too_large());
    }
    inspect_bundle(bytes).map_err(|e| {
        if e.code() == "bundle_too_large" {
            too_large()
        } else {
            invalid()
        }
    })?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let mut file = options.open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            error(
                "instruction_bundle_exists",
                "That destination exists. Choose a new name; existing files are never overwritten.",
            )
        } else {
            error(
                "instruction_bundle_write_failed",
                "The export destination could not be created. Choose another location.",
            )
        }
    })?;
    if !regular_file(&file.metadata().map_err(|_| failed_write())?) {
        return Err(failed_write());
    }
    finish(&mut file, bytes).map_err(|_| failed_write())
}

#[cfg(test)]
#[path = "instructions_tests.rs"]
mod tests;
