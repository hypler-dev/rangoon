//! Pinned SQLCipher recovery images. The host owns private directory/key custody.
use super::*;
use std::io::Read;
use zeroize::Zeroizing;

impl WorkspaceBackup {
    /// Complete canonical record comparison for trusted recovery verification.
    pub fn same_recovery_records(&self, other: &Self) -> Result<bool, StoreError> {
        let left = Zeroizing::new(workflow_backup::encode(&self.records)?);
        let right = Zeroizing::new(workflow_backup::encode(&other.records)?);
        Ok(*left == *right)
    }
}

impl Workspace {
    /// Opaque, validated recovery snapshot for the trusted native host. Protected
    /// instruction history is never returned as a plaintext portable byte buffer.
    pub fn snapshot_encrypted_recovery(&self) -> Result<WorkspaceBackup, StoreError> {
        let key = self.key.as_ref().ok_or(StoreError::EncryptionUnavailable)?;
        encryption::verify_backend(key)?;
        let mut db = self
            .connect_complete_read_only(false)?
            .ok_or(StoreError::NotFound)?;
        let tx = db.transaction()?;
        let bytes = Zeroizing::new(workflow_backup::encode(&workflow_mutations::read_records(
            &tx,
        )?)?);
        let snapshot = WorkspaceBackup::decode_complete(&bytes)?;
        tx.commit()?;
        Ok(snapshot)
    }

    /// Creates a new encrypted canonical copy, never modifies an existing file.
    /// Logical records stay in host memory; the returned bytes are ciphertext.
    pub fn create_encrypted_recovery_copy(
        &self,
        backup: &WorkspaceBackup,
    ) -> Result<Vec<u8>, StoreError> {
        self.key.as_ref().ok_or(StoreError::EncryptionUnavailable)?;
        self.initialize_empty_encrypted()?;
        let schema =
            u32::try_from(backup.records.version).map_err(|_| StoreError::BackupInvalid)?;
        let plan = self.prepare_empty_workflow_restore(backup, schema)?;
        self.restore_empty_workflow_backup(backup, &plan)?;
        let length = fs::symlink_metadata(self.path())
            .map_err(|_| StoreError::Unavailable)?
            .len();
        let checked = self.inspect_encrypted_recovery_copy(length)?;
        if !checked.same_recovery_records(backup)? {
            return Err(StoreError::Corrupt);
        }
        // All SQLite handles have closed before reading the encrypted image.
        let mut file = fs::File::open(self.path()).map_err(|_| StoreError::Unavailable)?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(MAX_DATABASE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| StoreError::Unavailable)?;
        if bytes.len() as u64 != length
            || file.metadata().map_err(|_| StoreError::Unavailable)?.len() != length
        {
            return Err(StoreError::WorkspaceChanged);
        }
        Ok(bytes)
    }

    /// Existing-only, read-only inspection of a host-staged encrypted image.
    /// Rejects trailing pages/bytes, sidecars, wrong keys and domain corruption.
    pub fn inspect_encrypted_recovery_copy(
        &self,
        expected_file_bytes: u64,
    ) -> Result<WorkspaceBackup, StoreError> {
        self.key.as_ref().ok_or(StoreError::EncryptionUnavailable)?;
        if expected_file_bytes == 0 || expected_file_bytes > MAX_DATABASE_BYTES {
            return Err(StoreError::BackupInvalid);
        }
        let before = fs::symlink_metadata(self.path()).map_err(|_| StoreError::Unavailable)?;
        if !before.is_file() || before.len() != expected_file_bytes {
            return Err(StoreError::BackupInvalid);
        }
        self.require_absent_sidecars()?;
        let mut database = self
            .connect_complete_read_only(false)?
            .ok_or(StoreError::BackupInvalid)?;
        let transaction = database.transaction()?;
        let pages: i64 = transaction.query_row("PRAGMA page_count", [], |row| row.get(0))?;
        if u64::try_from(pages)
            .ok()
            .and_then(|pages| pages.checked_mul(4096))
            != Some(expected_file_bytes)
        {
            return Err(StoreError::BackupInvalid);
        }
        let archive = Zeroizing::new(workflow_backup::encode(&workflow_mutations::read_records(
            &transaction,
        )?)?);
        let backup = WorkspaceBackup::decode_complete(&archive)?;
        transaction.commit()?;
        drop(database);
        self.require_absent_sidecars()?;
        let after = fs::symlink_metadata(self.path()).map_err(|_| StoreError::Unavailable)?;
        if !after.is_file() || after.len() != expected_file_bytes {
            return Err(StoreError::WorkspaceChanged);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if before.dev() != after.dev()
                || before.ino() != after.ino()
                || before.mtime() != after.mtime()
                || before.mtime_nsec() != after.mtime_nsec()
            {
                return Err(StoreError::WorkspaceChanged);
            }
        }
        Ok(backup)
    }
}

#[cfg(test)]
#[path = "recovery_copy_tests.rs"]
mod tests;
