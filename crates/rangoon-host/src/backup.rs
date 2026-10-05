//! File I/O restricted to paths selected by the native backup dialogs.
use super::{PublicError, open_selected, regular_file};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

pub const MAX_BACKUP_FILE_BYTES: usize = 68 * 1024 * 1024;

fn error(code: &'static str, message: &'static str) -> PublicError {
    PublicError { code, message }
}

/// Reads bounded bytes only. The application validates the portable format next.
pub fn read_selected_backup(path: &Path) -> Result<Vec<u8>, PublicError> {
    let failed = || {
        error(
            "backup_read_failed",
            "The selected backup could not be read. Choose a regular local backup file again.",
        )
    };
    let before = fs::symlink_metadata(path).map_err(|_| failed())?;
    if !regular_file(&before) {
        return Err(failed());
    }
    let file = open_selected(path).map_err(|_| failed())?;
    let metadata = file.metadata().map_err(|_| failed())?;
    if !regular_file(&metadata) {
        return Err(failed());
    }
    let oversized = || error("backup_too_large", "Choose a backup no larger than 68 MiB.");
    if metadata.len() > MAX_BACKUP_FILE_BYTES as u64 {
        return Err(oversized());
    }
    let mut bytes = Vec::new();
    file.take((MAX_BACKUP_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| failed())?;
    if bytes.len() > MAX_BACKUP_FILE_BYTES {
        return Err(oversized());
    }
    Ok(bytes)
}

/// Never truncates or overwrites an existing destination. A partial failed file
/// may remain; its creation does not establish successful export or durability.
pub fn write_selected_backup(path: &Path, bytes: &[u8]) -> Result<(), PublicError> {
    if bytes.len() > MAX_BACKUP_FILE_BYTES {
        return Err(error(
            "backup_too_large",
            "The backup exceeds this build's export limit.",
        ));
    }
    let mut options = fs::OpenOptions::new();
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
        options.custom_flags(0x0020_0000);
    }
    let mut file = options.open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            error("backup_exists", "That destination already exists. Choose a new name; existing files are never overwritten.")
        } else { error("backup_write_failed", "The backup destination could not be created. Choose another location.") }
    })?;
    let failed = || {
        error(
            "backup_write_failed",
            "Backup export did not finish. A partial file may remain at the selected location. Choose a new name and try again.",
        )
    };
    if !regular_file(&file.metadata().map_err(|_| failed())?) {
        return Err(failed());
    }
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| failed())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    struct Directory(std::path::PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "rangoon-backup-host-{}-{}",
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
    #[test]
    fn exclusive_export_and_bounded_regular_read() {
        let dir = Directory::new();
        let path = dir.0.join("test.rangoon-backup");
        write_selected_backup(&path, b"exact bytes\r\n").unwrap();
        assert_eq!(read_selected_backup(&path).unwrap(), b"exact bytes\r\n");
        assert_eq!(
            write_selected_backup(&path, b"replacement")
                .unwrap_err()
                .code,
            "backup_exists"
        );
        assert_eq!(fs::read(&path).unwrap(), b"exact bytes\r\n");
        assert!(read_selected_backup(&dir.0).is_err());
        assert!(write_selected_backup(&dir.0.join("missing/backup"), b"data").is_err());
        let large = dir.0.join("large");
        fs::File::create(&large)
            .unwrap()
            .set_len((MAX_BACKUP_FILE_BYTES + 1) as u64)
            .unwrap();
        assert_eq!(
            read_selected_backup(&large).unwrap_err().code,
            "backup_too_large"
        );
    }
    #[cfg(unix)]
    #[test]
    fn links_and_special_files_are_not_followed() {
        use std::os::unix::fs::symlink;
        let dir = Directory::new();
        let target = dir.0.join("original");
        let link = dir.0.join("link");
        fs::write(&target, b"preserve").unwrap();
        symlink(&target, &link).unwrap();
        assert!(read_selected_backup(&link).is_err());
        assert!(write_selected_backup(&link, b"changed").is_err());
        assert_eq!(fs::read(&target).unwrap(), b"preserve");
        assert!(read_selected_backup(Path::new("/dev/null")).is_err());
        assert!(write_selected_backup(Path::new("/dev/null"), b"data").is_err());
    }
}
