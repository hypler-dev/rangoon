use super::*;
use std::sync::{
    Arc, Barrier,
    atomic::{AtomicU64, Ordering},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        // macOS's long per-user temp root can exceed Unix socket path limits.
        #[cfg(unix)]
        let root = std::path::PathBuf::from("/tmp");
        #[cfg(not(unix))]
        let root = std::env::temp_dir();
        let path = root.join(format!(
            "rangoon-instruction-host-{}-{}",
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

fn bundle() -> Vec<u8> {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/compilation/instruction-bundle-v1.json"
    ))
    .unwrap();
    let hex = fixture["cases"][0]["bundleHex"].as_str().unwrap();
    (0..hex.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&hex[offset..offset + 2], 16).unwrap())
        .collect()
}

#[test]
fn exact_exclusive_export_read_and_owner_permissions() {
    let dir = Directory::new();
    let path = dir.0.join("Review 🦀.rangoon-instructions");
    let bytes = bundle();
    write_selected_instruction_bundle(&path, &bytes).unwrap();
    assert_eq!(read_selected_instruction_bundle(&path).unwrap(), bytes);
    assert_eq!(
        write_selected_instruction_bundle(&path, &bytes)
            .unwrap_err()
            .code,
        "instruction_bundle_exists"
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(path).unwrap().permissions().mode() & 0o077, 0);
    }
}

#[test]
fn invalid_format_or_nonportable_name_never_creates_a_file() {
    let dir = Directory::new();
    let bytes = bundle();
    for name in [
        "AGENTS.md",
        "CLAUDE.md",
        "test.RANGOON-INSTRUCTIONS",
        ".rangoon-instructions",
        "NUL.rangoon-instructions",
        "con.extra.rangoon-instructions",
        "COM¹.rangoon-instructions",
        "lpt9.rangoon-instructions",
        "base:stream.rangoon-instructions",
        "bad?.rangoon-instructions",
        "name .rangoon-instructions",
    ] {
        let path = dir.0.join(name);
        assert_eq!(
            write_selected_instruction_bundle(&path, &bytes)
                .unwrap_err()
                .code,
            "instruction_bundle_name",
            "{name}"
        );
        assert!(!path.exists());
    }
    let path = dir.0.join("valid.rangoon-instructions");
    assert_eq!(
        write_selected_instruction_bundle(&path, b"not a bundle")
            .unwrap_err()
            .code,
        "bundle_invalid"
    );
    assert!(!path.exists());
    assert_eq!(
        write_selected_instruction_bundle(&path, &vec![0; MAX_BUNDLE_BYTES + 1])
            .unwrap_err()
            .code,
        "bundle_too_large"
    );
    assert!(!path.exists());
    assert_eq!(
        write_selected_instruction_bundle(Path::new("relative.rangoon-instructions"), &bytes)
            .unwrap_err()
            .code,
        "instruction_bundle_name"
    );
}

#[test]
fn selected_file_type_missing_parent_and_read_size_are_bounded() {
    let dir = Directory::new();
    let bytes = bundle();
    let folder = dir.0.join("directory.rangoon-instructions");
    fs::create_dir(&folder).unwrap();
    assert!(read_selected_instruction_bundle(&folder).is_err());
    assert!(write_selected_instruction_bundle(&folder, &bytes).is_err());
    assert!(
        write_selected_instruction_bundle(&dir.0.join("missing/file.rangoon-instructions"), &bytes)
            .is_err()
    );
    assert!(!dir.0.join("missing").exists());
    let large = dir.0.join("large.rangoon-instructions");
    File::create(&large)
        .unwrap()
        .set_len((MAX_BUNDLE_BYTES + 1) as u64)
        .unwrap();
    assert_eq!(
        read_selected_instruction_bundle(&large).unwrap_err().code,
        "bundle_too_large"
    );
}

#[test]
fn competing_creates_have_one_winner_and_never_replace() {
    let dir = Directory::new();
    let path = dir.0.join("race.rangoon-instructions");
    let barrier = Arc::new(Barrier::new(2));
    let bytes = Arc::new(bundle());
    let mut workers = Vec::new();
    for _ in 0..2 {
        let (path, barrier, bytes) = (path.clone(), barrier.clone(), bytes.clone());
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            write_selected_instruction_bundle(&path, &bytes).map_err(|e| e.code)
        }));
    }
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| **r == Err("instruction_bundle_exists"))
            .count(),
        1
    );
    assert_eq!(fs::read(path).unwrap(), *bytes);
}

#[test]
fn injected_partial_write_and_sync_failures_preserve_uncertain_output() {
    let dir = Directory::new();
    let bytes = bundle();
    for complete in [false, true] {
        let path = dir
            .0
            .join(format!("failure-{complete}.rangoon-instructions"));
        let expected = if complete { bytes.len() } else { 31 };
        let failed = write_selected_with(&path, &bytes, |file, bytes| {
            file.write_all(&bytes[..expected])?;
            Err(std::io::Error::other("injected write or sync failure"))
        })
        .unwrap_err();
        assert_eq!(failed.code, "instruction_bundle_write_failed");
        assert!(
            failed
                .message
                .contains("partial or complete file may remain")
        );
        assert_eq!(fs::read(&path).unwrap(), bytes[..expected]);
        assert_eq!(
            write_selected_instruction_bundle(&path, &bytes)
                .unwrap_err()
                .code,
            "instruction_bundle_exists"
        );
        assert_eq!(fs::read(&path).unwrap(), bytes[..expected]);
    }
}

#[cfg(unix)]
#[test]
fn unix_final_links_reject_but_parent_link_is_not_confinement() {
    use std::os::unix::fs::symlink;
    let dir = Directory::new();
    let target = dir.0.join("target.rangoon-instructions");
    let link = dir.0.join("link.rangoon-instructions");
    let bytes = bundle();
    fs::write(&target, &bytes).unwrap();
    symlink(&target, &link).unwrap();
    assert!(read_selected_instruction_bundle(&link).is_err());
    assert!(write_selected_instruction_bundle(&link, &bytes).is_err());
    assert_eq!(fs::read(&target).unwrap(), bytes);
    let special = dir.0.join("special.rangoon-instructions");
    symlink("/dev/null", &special).unwrap();
    assert!(read_selected_instruction_bundle(&special).is_err());
    assert!(write_selected_instruction_bundle(&special, &bytes).is_err());
    let socket = dir.0.join("socket.rangoon-instructions");
    let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    assert!(read_selected_instruction_bundle(&socket).is_err());
    assert!(write_selected_instruction_bundle(&socket, &bytes).is_err());
    let parent = dir.0.join("linked-parent");
    let actual = dir.0.join("actual-parent");
    fs::create_dir(&actual).unwrap();
    symlink(&actual, &parent).unwrap();
    write_selected_instruction_bundle(&parent.join("new.rangoon-instructions"), &bytes).unwrap();
    assert_eq!(
        fs::read(actual.join("new.rangoon-instructions")).unwrap(),
        bytes
    );
}

#[cfg(windows)]
#[test]
fn windows_final_reparse_points_and_stream_or_device_paths_reject() {
    use std::os::windows::fs::symlink_file;
    let dir = Directory::new();
    let target = dir.0.join("target.rangoon-instructions");
    let link = dir.0.join("link.rangoon-instructions");
    let bytes = bundle();
    fs::write(&target, &bytes).unwrap();
    symlink_file(&target, &link).expect("Windows test requires symlink privilege");
    assert!(read_selected_instruction_bundle(&link).is_err());
    assert!(write_selected_instruction_bundle(&link, &bytes).is_err());
    assert_eq!(fs::read(&target).unwrap(), bytes);
    for path in [
        r"C:\folder\file.rangoon-instructions",
        r"C:\folder\.\file.rangoon-instructions",
        r"\\server\share\folder\file.rangoon-instructions",
        r"\\?\C:\folder\file.rangoon-instructions",
        r"\\?\UNC\server\share\folder\file.rangoon-instructions",
    ] {
        assert!(selected_path(Path::new(path)).is_ok(), "{path}");
    }
    for path in [
        r"\\?\GLOBALROOT\Device\file.rangoon-instructions",
        r"\\?\C:\folder\..\file.rangoon-instructions",
        r"\\?\C:\folder\.\file.rangoon-instructions",
        r"\\?\UNC\server\share\folder:stream\file.rangoon-instructions",
        r"\\.\C:\file.rangoon-instructions",
        r"C:\parent:stream\file.rangoon-instructions",
        r"C:\NUL\file.rangoon-instructions",
    ] {
        assert_eq!(
            selected_path(Path::new(path)).unwrap_err().code,
            "instruction_bundle_name"
        );
    }
}
