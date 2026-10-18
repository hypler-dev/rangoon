//! Bounded operations-window reads for one path returned by the native picker.
//! This does not confine parent directories or prove immutable filesystem state.
use super::{PublicError, open_selected, regular_file};
use std::{fs, io::Read, path::Path};

/// Must match `rangoon_ops::MAX_INPUT_BYTES` without making the host depend on
/// the operations decoder crate.
pub const MAX_OPERATIONS_WINDOW_BYTES: usize = 2_097_152;

fn error(code: &'static str, message: &'static str) -> PublicError {
    PublicError { code, message }
}

fn read_failed() -> PublicError {
    error(
        "operations_window_read_failed",
        "The selected operations window could not be read. Choose a regular local file again.",
    )
}

fn too_large() -> PublicError {
    error(
        "operations_window_too_large",
        "Choose an operations window no larger than 2 MiB.",
    )
}

/// Reads at most the O1 input limit from one native-picker-selected regular
/// file. Callers still validate the returned bytes as an operations window.
pub fn read_selected_operations_window(path: &Path) -> Result<Vec<u8>, PublicError> {
    read_selected_operations_window_with(path, || {})
}

fn read_selected_operations_window_with(
    path: &Path,
    after_pre_open_check: impl FnOnce(),
) -> Result<Vec<u8>, PublicError> {
    if !path.is_absolute() {
        return Err(read_failed());
    }
    let before = fs::symlink_metadata(path).map_err(|_| read_failed())?;
    if !regular_file(&before) {
        return Err(read_failed());
    }
    after_pre_open_check();
    let file = open_selected(path).map_err(|_| read_failed())?;
    let metadata = file.metadata().map_err(|_| read_failed())?;
    if !regular_file(&metadata) {
        return Err(read_failed());
    }
    if metadata.len() > MAX_OPERATIONS_WINDOW_BYTES as u64 {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    file.take((MAX_OPERATIONS_WINDOW_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| read_failed())?;
    if bytes.len() > MAX_OPERATIONS_WINDOW_BYTES {
        return Err(too_large());
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "operations_tests.rs"]
mod tests;
