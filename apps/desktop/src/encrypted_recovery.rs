//! Native-only recovery key custody and private encrypted staging. No IPC keys/paths.
use rangoon_host::PublicError;
use rangoon_store::WorkspaceBackup;
use std::path::Path;
#[cfg(all(unix, feature = "encrypted-workspace"))]
use zeroize::Zeroizing;

pub(super) const FORMAT: &str = "rangoon.recovery.sqlcipher.v1";
#[cfg(all(unix, feature = "encrypted-workspace"))]
pub(super) const MAX_BYTES: u64 = 64 * 1024 * 1024;
#[cfg(all(unix, feature = "encrypted-workspace"))]
const KEY_PREFIX: &[u8] = b"RANGOON-RECOVERY-KEY-V1\n";
#[cfg(feature = "encrypted-workspace")]
pub(crate) fn supported() -> bool {
    cfg!(all(unix, feature = "encrypted-workspace"))
}
pub(crate) fn invalid() -> PublicError {
    PublicError {
        code: "workspace_recovery_invalid",
        message: "The recovery file or key could not be verified. The workspace was not restored. Check the selected files and their permissions.",
    }
}
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn uncertain() -> PublicError {
    PublicError {
        code: "workspace_recovery_output_uncertain",
        message: "The file could not be verified after writing. A file may remain; inspect your selected output and use a new name before retrying.",
    }
}
#[cfg(all(unix, feature = "encrypted-workspace"))]
pub(super) struct RecoveryKey(Zeroizing<[u8; 32]>);
#[cfg(not(all(unix, feature = "encrypted-workspace")))]
pub(super) enum RecoveryKey {}
#[cfg(all(test, unix, feature = "encrypted-workspace"))]
std::thread_local! {
    static DECODE_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static ENCODE_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[cfg(all(test, unix, feature = "encrypted-workspace"))]
pub(super) fn encode_calls() -> usize {
    ENCODE_CALLS.get()
}
#[cfg(all(test, unix, feature = "encrypted-workspace"))]
#[derive(Clone, Copy, PartialEq, Eq)]
enum ReadPhase {
    BeforeOpen,
    AfterRead,
}
#[cfg(all(test, unix, feature = "encrypted-workspace"))]
type ReadProbe = (ReadPhase, Box<dyn FnOnce()>);
#[cfg(all(test, unix, feature = "encrypted-workspace"))]
std::thread_local! { static READ_PROBE: std::cell::RefCell<Option<ReadProbe>> = const { std::cell::RefCell::new(None) }; }
#[cfg(all(test, unix, feature = "encrypted-workspace"))]
fn read_probe(phase: ReadPhase) {
    let action = READ_PROBE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.as_ref().is_some_and(|(at, _)| *at == phase) {
            slot.take()
        } else {
            None
        }
    });
    if let Some((_, action)) = action {
        action();
    }
}
#[cfg(all(test, unix, feature = "encrypted-workspace"))]
pub(super) fn decode_calls() -> usize {
    DECODE_CALLS.get()
}
impl std::fmt::Debug for RecoveryKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RecoveryKey(<redacted>)")
    }
}

#[cfg(all(unix, feature = "encrypted-workspace"))]
mod platform {
    use super::*;
    use rangoon_store::{Workspace, WorkspaceKey};
    use std::{
        fs::{self, File, OpenOptions},
        io::{Read, Write},
        os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
        path::PathBuf,
    };

    struct Stage {
        path: PathBuf,
        held: File,
        original: fs::Metadata,
    }
    impl Stage {
        fn new() -> Result<Self, PublicError> {
            let mut random = [0u8; 16];
            getrandom::fill(&mut random).map_err(|_| invalid())?;
            let name: String = random.iter().map(|b| format!("{b:02x}")).collect();
            let path = std::env::temp_dir()
                .canonicalize()
                .map_err(|_| invalid())?
                .join(format!("rangoon-recovery-{name}"));
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .map_err(|_| invalid())?;
            let held = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(&path)
                .map_err(|_| invalid())?;
            let original = held.metadata().map_err(|_| invalid())?;
            let stage = Self {
                path,
                held,
                original,
            };
            stage.check()?;
            Ok(stage)
        }
        fn check(&self) -> Result<(), PublicError> {
            let current = fs::symlink_metadata(&self.path).map_err(|_| invalid())?;
            let held = self.held.metadata().map_err(|_| invalid())?;
            if !current.is_dir()
                || current.mode() & 0o777 != 0o700
                || current.uid() != self.original.uid()
                || !same(&current, &held)
                || !same(&current, &self.original)
            {
                return Err(invalid());
            }
            Ok(())
        }
        fn cleanup(&self) -> Result<(), PublicError> {
            self.check()?;
            for suffix in ["", "-journal", "-wal", "-shm"] {
                let path = self.path.join(format!("workspace.sqlite3{suffix}"));
                match fs::symlink_metadata(&path) {
                    Ok(meta)
                        if meta.is_file()
                            && meta.uid() == self.original.uid()
                            && meta.nlink() == 1 =>
                    {
                        self.check()?;
                        fs::remove_file(path).map_err(|_| invalid())?;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                    _ => return Err(invalid()),
                }
            }
            self.check()?;
            fs::remove_dir(&self.path).map_err(|_| invalid())
        }
    }
    impl Drop for Stage {
        fn drop(&mut self) {
            let _ = self.cleanup();
        }
    }
    fn same(a: &fs::Metadata, b: &fs::Metadata) -> bool {
        a.dev() == b.dev() && a.ino() == b.ino()
    }
    fn stable(a: &fs::Metadata, b: &fs::Metadata) -> bool {
        same(a, b)
            && a.len() == b.len()
            && a.mtime() == b.mtime()
            && a.mtime_nsec() == b.mtime_nsec()
            && a.ctime() == b.ctime()
            && a.ctime_nsec() == b.ctime_nsec()
            && a.mode() == b.mode()
            && a.uid() == b.uid()
            && a.nlink() == b.nlink()
    }
    fn with_stage<T>(
        action: impl FnOnce(&Stage) -> Result<T, PublicError>,
    ) -> Result<T, PublicError> {
        let stage = Stage::new()?;
        let result = action(&stage);
        // Explicit cleanup is part of the result, including unsuccessful attempts.
        stage.cleanup().map_err(|_| PublicError{code:"workspace_recovery_cleanup_failed",message:"Private recovery staging could not be removed. Recovery did not return a verified result; inspect temporary-file permissions before retrying."})?;
        result
    }
    fn read(
        path: &Path,
        limit: u64,
        owner: Option<u32>,
    ) -> Result<Zeroizing<Vec<u8>>, PublicError> {
        let before = fs::symlink_metadata(path).map_err(|_| invalid())?;
        if !before.is_file()
            || before.len() == 0
            || before.len() > limit
            || before.nlink() != 1
            || owner.is_some_and(|uid| before.uid() != uid || before.mode() & 0o077 != 0)
        {
            return Err(invalid());
        }
        #[cfg(test)]
        read_probe(ReadPhase::BeforeOpen);
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| invalid())?;
        let opened = file.metadata().map_err(|_| invalid())?;
        if !stable(&before, &opened) {
            return Err(invalid());
        }
        let mut bytes = Zeroizing::new(Vec::new());
        (&mut file)
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| invalid())?;
        #[cfg(test)]
        read_probe(ReadPhase::AfterRead);
        let after = file.metadata().map_err(|_| invalid())?;
        let named = fs::symlink_metadata(path).map_err(|_| invalid())?;
        if !stable(&opened, &after)
            || !stable(&opened, &named)
            || bytes.len() as u64 != opened.len()
        {
            return Err(invalid());
        }
        Ok(bytes)
    }
    fn write(path: &Path, bytes: &[u8]) -> Result<(), PublicError> {
        let mut file=OpenOptions::new().read(true).write(true).create_new(true).mode(0o600).custom_flags(libc::O_NOFOLLOW|libc::O_NONBLOCK).open(path).map_err(|_|PublicError{code:"workspace_recovery_output_unavailable",message:"The output could not be created. Choose a new file name; existing files are never overwritten."})?;
        let original = file.metadata().map_err(|_| uncertain())?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| uncertain())?;
        let after = file.metadata().map_err(|_| uncertain())?;
        let named = fs::symlink_metadata(path).map_err(|_| uncertain())?;
        if !same(&original, &after)
            || !stable(&after, &named)
            || after.len() != bytes.len() as u64
            || !after.is_file()
            || after.nlink() != 1
            || after.mode() & 0o077 != 0
        {
            return Err(uncertain());
        }
        Ok(())
    }
    pub(crate) fn read_key(path: &Path) -> Result<RecoveryKey, PublicError> {
        with_stage(|stage| {
            let bytes = read(path, 128, Some(stage.original.uid()))?;
            if bytes.len() != KEY_PREFIX.len() + 65
                || !bytes.starts_with(KEY_PREFIX)
                || bytes.last() != Some(&b'\n')
            {
                return Err(invalid());
            }
            let hex = &bytes[KEY_PREFIX.len()..bytes.len() - 1];
            let mut key = Zeroizing::new([0; 32]);
            for (i, pair) in hex.as_chunks::<2>().0.iter().enumerate() {
                fn nibble(b: u8) -> Option<u8> {
                    match b {
                        b'0'..=b'9' => Some(b - b'0'),
                        b'a'..=b'f' => Some(b - b'a' + 10),
                        _ => None,
                    }
                }
                key[i] = (nibble(pair[0]).ok_or_else(invalid)? << 4)
                    | nibble(pair[1]).ok_or_else(invalid)?;
            }
            Ok(RecoveryKey(key))
        })
    }
    pub(crate) fn create_key(path: &Path) -> Result<(), PublicError> {
        let mut key = Zeroizing::new([0u8; 32]);
        getrandom::fill(key.as_mut()).map_err(|_| invalid())?;
        let mut bytes = Zeroizing::new(Vec::with_capacity(KEY_PREFIX.len() + 65));
        bytes.extend_from_slice(KEY_PREFIX);
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for byte in key.iter() {
            bytes.push(HEX[(byte >> 4) as usize]);
            bytes.push(HEX[(byte & 15) as usize]);
        }
        bytes.push(b'\n');
        write(path, &bytes)?;
        let reread = read_key(path).map_err(|_| uncertain())?;
        if *reread.0 != *key {
            return Err(uncertain());
        }
        Ok(())
    }
    pub(crate) fn encode(
        backup: &WorkspaceBackup,
        key: &RecoveryKey,
    ) -> Result<Vec<u8>, PublicError> {
        #[cfg(test)]
        ENCODE_CALLS.set(ENCODE_CALLS.get() + 1);
        with_stage(|stage| {
            stage.check()?;
            let workspace =
                Workspace::encrypted(stage.path.clone(), WorkspaceKey::from_bytes(*key.0))
                    .map_err(|_| invalid())?;
            let bytes = workspace
                .create_encrypted_recovery_copy(backup)
                .map_err(|_| invalid())?;
            stage.check()?;
            Ok(bytes)
        })
    }
    pub(crate) fn decode(bytes: &[u8], key: &RecoveryKey) -> Result<WorkspaceBackup, PublicError> {
        #[cfg(test)]
        DECODE_CALLS.set(DECODE_CALLS.get() + 1);
        if bytes.is_empty() || bytes.len() as u64 > MAX_BYTES || !bytes.len().is_multiple_of(4096) {
            return Err(invalid());
        }
        with_stage(|stage| {
            stage.check()?;
            write(&stage.path.join("workspace.sqlite3"), bytes)?;
            let workspace =
                Workspace::encrypted(stage.path.clone(), WorkspaceKey::from_bytes(*key.0))
                    .map_err(|_| invalid())?;
            let backup = workspace
                .inspect_encrypted_recovery_copy(bytes.len() as u64)
                .map_err(|_| invalid())?;
            stage.check()?;
            Ok(backup)
        })
    }
    pub(crate) fn read_cipher(path: &Path) -> Result<Vec<u8>, PublicError> {
        Ok(read(path, MAX_BYTES, None)?.to_vec())
    }
    pub(crate) fn write_cipher(path: &Path, bytes: &[u8]) -> Result<(), PublicError> {
        if bytes.is_empty() || bytes.len() as u64 > MAX_BYTES {
            return Err(invalid());
        }
        write(path, bytes)
    }
}

#[cfg(all(unix, feature = "encrypted-workspace"))]
pub(super) use platform::{create_key, decode, encode, read_cipher, read_key, write_cipher};

#[cfg(not(all(unix, feature = "encrypted-workspace")))]
mod unsupported {
    use super::*;
    pub(crate) fn create_key(_: &Path) -> Result<(), PublicError> {
        Err(invalid())
    }
    pub(crate) fn read_key(_: &Path) -> Result<RecoveryKey, PublicError> {
        Err(invalid())
    }
    pub(crate) fn encode(_: &WorkspaceBackup, _: &RecoveryKey) -> Result<Vec<u8>, PublicError> {
        Err(invalid())
    }
    pub(crate) fn decode(_: &[u8], _: &RecoveryKey) -> Result<WorkspaceBackup, PublicError> {
        Err(invalid())
    }
    pub(crate) fn read_cipher(_: &Path) -> Result<Vec<u8>, PublicError> {
        Err(invalid())
    }
    pub(crate) fn write_cipher(_: &Path, _: &[u8]) -> Result<(), PublicError> {
        Err(invalid())
    }
}
#[cfg(not(all(unix, feature = "encrypted-workspace")))]
pub(super) use unsupported::{create_key, decode, encode, read_cipher, read_key, write_cipher};

#[cfg(all(test, unix, feature = "encrypted-workspace"))]
#[path = "encrypted_recovery_tests.rs"]
mod tests;
