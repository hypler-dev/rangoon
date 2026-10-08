//! Optional keyed SQLite backend. This module never discovers or persists keys.
use crate::StoreError;
use zeroize::Zeroizing;

/// Secret owned by the trusted host. No serialization or debug representation.
/// The caller must provide suitable entropy and manage recovery separately.
pub struct WorkspaceKey(Zeroizing<[u8; 32]>);

impl WorkspaceKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }
}

#[cfg(not(feature = "encrypted-sqlite"))]
pub(super) fn verify_backend(key: &WorkspaceKey) -> Result<(), StoreError> {
    // Keep the same callable host API in builds without encrypted support.
    // Reading the owned length creates no secret representation or disk state.
    let _ = key.0.len();
    Err(StoreError::EncryptionUnavailable)
}

#[cfg(not(feature = "encrypted-sqlite"))]
pub(super) fn prepare_file(
    _: &rusqlite::Connection,
    _: &WorkspaceKey,
    _: u64,
) -> Result<(), StoreError> {
    Err(StoreError::EncryptionUnavailable)
}

#[cfg(feature = "encrypted-sqlite")]
mod keyed {
    use super::*;
    use rusqlite::Connection;
    use std::fmt::Write;

    fn profile(db: &Connection) -> rusqlite::Result<()> {
        for (name, expected) in [
            ("cipher_version", "4.14.0 community"),
            ("cipher_page_size", "4096"),
            ("cipher_use_hmac", "1"),
            ("cipher_plaintext_header_size", "0"),
            ("kdf_iter", "256000"),
            ("cipher_hmac_algorithm", "HMAC_SHA512"),
            ("cipher_kdf_algorithm", "PBKDF2_HMAC_SHA512"),
        ] {
            let observed: String = db.pragma_query_value(None, name, |row| row.get(0))?;
            if observed != expected {
                return Err(rusqlite::Error::InvalidQuery);
            }
        }
        let provider: String = db.pragma_query_value(None, "cipher_provider", |row| row.get(0))?;
        if !matches!(provider.as_str(), "openssl" | "commoncrypto") {
            return Err(rusqlite::Error::InvalidQuery);
        }
        Ok(())
    }

    fn key(db: &Connection, secret: &WorkspaceKey) -> rusqlite::Result<()> {
        // SQLCipher logging is process-wide. Disable it before any secret SQL;
        // no tracing or statement profiling is installed by this store.
        db.pragma_update(None, "cipher_log_level", "NONE")?;
        let logging: String = db.pragma_query_value(None, "cipher_log_level", |row| row.get(0))?;
        if logging != "NONE" {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let mut raw = Zeroizing::new(String::with_capacity(67));
        raw.push_str("x'");
        for byte in secret.0.iter() {
            write!(&mut *raw, "{byte:02x}").map_err(|_| rusqlite::Error::InvalidQuery)?;
        }
        raw.push('\'');
        // Safe quoting is owned by rusqlite. SQLite/parser copies are outside
        // the zeroization guarantee for our owned buffers.
        db.pragma_update(None, "key", raw.as_str())?;
        profile(db)?;
        db.pragma_update(None, "temp_store", "MEMORY")?;
        let temp: i64 = db.pragma_query_value(None, "temp_store", |row| row.get(0))?;
        if temp != 2 {
            return Err(rusqlite::Error::InvalidQuery);
        }
        Ok(())
    }

    pub(crate) fn verify_backend(secret: &WorkspaceKey) -> Result<(), StoreError> {
        let db = Connection::open_in_memory().map_err(|_| StoreError::EncryptionUnavailable)?;
        key(&db, secret).map_err(|_| StoreError::EncryptionUnavailable)
    }

    pub(crate) fn prepare_file(
        db: &Connection,
        secret: &WorkspaceKey,
        file_bytes: u64,
    ) -> Result<(), StoreError> {
        let verify = || -> rusqlite::Result<()> {
            key(db, secret)?;
            if file_bytes != 0 {
                if file_bytes % 4096 != 0 {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                // Consume only row existence. Never retrieve page/error text.
                let mut statement = db.prepare("PRAGMA cipher_integrity_check")?;
                if statement.query([])?.next()?.is_some() {
                    return Err(rusqlite::Error::InvalidQuery);
                }
            }
            // Key validation is lazy; profile getters alone do not verify data.
            let _: i64 =
                db.query_row("SELECT count(*) FROM sqlite_schema", [], |row| row.get(0))?;
            Ok(())
        };
        verify().map_err(|_| StoreError::EncryptedStoreInvalid)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn pinned_backend_identity() {
            let db = Connection::open_in_memory().unwrap();
            key(&db, &WorkspaceKey::from_bytes([0x51; 32])).unwrap();
            let provider: String = db
                .pragma_query_value(None, "cipher_provider", |r| r.get(0))
                .unwrap();
            let version: String = db
                .pragma_query_value(None, "cipher_provider_version", |r| r.get(0))
                .unwrap();
            println!(
                "SQLCipher 4.14.0 community; crypto provider: {provider}; crypto version: {version}"
            );
        }

        #[test]
        fn incompatible_process_defaults_fail_before_filesystem_access() {
            const CHILD: &str = "RANGOON_CIPHER_PROFILE_TEST_CHILD";
            if std::env::var_os(CHILD).is_none() {
                let result = std::process::Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", "encryption::keyed::tests::incompatible_process_defaults_fail_before_filesystem_access"])
                    .env(CHILD, "1")
                    .output().unwrap();
                assert!(
                    result.status.success(),
                    "isolated cipher-default test failed"
                );
                return;
            }
            // Isolate process-global defaults from every other test connection.
            let db = Connection::open_in_memory().unwrap();
            db.pragma_update(None, "cipher_default_use_hmac", false)
                .unwrap();
            let directory = std::env::temp_dir()
                .join(format!("rangoon-rejected-profile-{}", std::process::id()));
            assert!(!directory.exists());
            let result = crate::Workspace::encrypted(
                directory.clone(),
                WorkspaceKey::from_bytes([0x51; 32]),
            );
            assert!(matches!(result, Err(StoreError::EncryptionUnavailable)));
            assert!(!directory.exists());
        }

        #[test]
        fn altered_connection_profile_is_rejected_without_reconfiguration() {
            let db = Connection::open_in_memory().unwrap();
            key(&db, &WorkspaceKey::from_bytes([0x51; 32])).unwrap();
            // A new connection has not used its key yet. Local profile changes
            // model an incompatible caller; do not modify process-wide defaults.
            db.pragma_update(None, "cipher_use_hmac", false).unwrap();
            assert!(profile(&db).is_err());
            let observed: String = db
                .pragma_query_value(None, "cipher_use_hmac", |r| r.get(0))
                .unwrap();
            assert_eq!(observed, "0");
        }
    }
}

#[cfg(feature = "encrypted-sqlite")]
pub(super) use keyed::{prepare_file, verify_backend};
