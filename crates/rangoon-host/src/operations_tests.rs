use super::*;
use std::{
    fs::{self, File},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Directory(std::path::PathBuf);

impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rangoon-operations-window-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_read_failure(path: &Path) {
    let error = read_selected_operations_window(path).unwrap_err();
    assert_eq!(error.code, "operations_window_read_failed");
    assert_eq!(
        error.message,
        "The selected operations window could not be read. Choose a regular local file again."
    );
    assert!(!format!("{error:?}").contains(&path.display().to_string()));
}

#[test]
fn reads_exact_limit_and_refuses_one_extra_byte() {
    let dir = Directory::new();
    let exact = dir.0.join("exact.json");
    File::create(&exact)
        .unwrap()
        .set_len(MAX_OPERATIONS_WINDOW_BYTES as u64)
        .unwrap();
    assert_eq!(
        read_selected_operations_window(&exact).unwrap().len(),
        MAX_OPERATIONS_WINDOW_BYTES
    );

    let oversized = dir.0.join("oversized.json");
    File::create(&oversized)
        .unwrap()
        .set_len((MAX_OPERATIONS_WINDOW_BYTES + 1) as u64)
        .unwrap();
    let error = read_selected_operations_window(&oversized).unwrap_err();
    assert_eq!(error.code, "operations_window_too_large");
    assert_eq!(
        error.message,
        "Choose an operations window no larger than 2 MiB."
    );
}

#[test]
fn rejects_relative_missing_and_directory_paths_without_path_leaks() {
    let dir = Directory::new();
    assert_read_failure(Path::new("relative-operations-window.json"));
    assert_read_failure(&dir.0.join("missing-secret-window.json"));
    assert_read_failure(&dir.0);
}

#[cfg(unix)]
#[test]
fn rejects_links_special_files_and_final_component_substitution() {
    use std::{os::unix::fs::symlink, process::Command};

    let dir = Directory::new();
    let target = dir.0.join("target.json");
    let link = dir.0.join("link.json");
    fs::write(&target, b"must not be returned").unwrap();
    symlink(&target, &link).unwrap();
    assert_read_failure(&link);
    assert_read_failure(Path::new("/dev/null"));

    let fifo = dir.0.join("events.fifo");
    let status = Command::new("mkfifo").arg(&fifo).status();
    if let Ok(status) = status {
        assert!(status.success());
        assert_read_failure(&fifo);
    }

    let selected = dir.0.join("selected.json");
    fs::write(&selected, b"original").unwrap();
    let result = read_selected_operations_window_with(&selected, || {
        fs::remove_file(&selected).unwrap();
        symlink(&target, &selected).unwrap();
    });
    let error = result.unwrap_err();
    assert_eq!(error.code, "operations_window_read_failed");
    assert!(!format!("{error:?}").contains(&selected.display().to_string()));
    assert_eq!(fs::read(&target).unwrap(), b"must not be returned");
}

#[cfg(windows)]
#[test]
fn rejects_final_reparse_points() {
    use std::os::windows::fs::symlink_file;

    let dir = Directory::new();
    let target = dir.0.join("target.json");
    let link = dir.0.join("link.json");
    fs::write(&target, b"must not be returned").unwrap();
    symlink_file(&target, &link).expect("Windows test requires symlink privilege");
    assert_read_failure(&link);
}
