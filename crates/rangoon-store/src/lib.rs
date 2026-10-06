//! Explicit, bounded local source snapshots. Stored bytes never confer authority.
#![forbid(unsafe_code)]

mod capabilities;
mod composition_backup;
mod composition_records;
#[cfg(test)]
mod composition_records_tests;
mod composition_recovery;
#[cfg(test)]
mod composition_recovery_tests;
mod compositions;
mod workspace_data;
#[cfg(test)]
mod workspace_data_tests;
pub use capabilities::{CapabilityReceipt, MAX_CAPABILITIES, MAX_REVISIONS, MAX_TOTAL_REVISIONS};
pub use composition_backup::CompositionBackup;
pub use composition_recovery::{
    CompositionDeletionPlan, CompositionRestorePlan, CompositionWorkspaceData, RecoveryCounts,
};
pub use compositions::CompositionPreview;
pub use workspace_data::{
    Backup, DeletionPlan, MAX_BACKUP_BYTES, RecordKind, RestorePlan, WorkspaceData, WorkspaceUsage,
};

use rangoon_domain::AnalysisReport;
use rangoon_import::{MAX_SOURCE_BYTES, analyze, validate_display_name};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, TransactionBehavior, limits::Limit, params,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const MAX_SNAPSHOTS: usize = 128;
const MAX_DATABASE_BYTES: u64 = 64 * 1024 * 1024;
const APPLICATION_ID: i64 = 0x52474e31;
const SCHEMA: &str = "CREATE TABLE snapshots (source_id TEXT PRIMARY KEY NOT NULL, display_name TEXT NOT NULL, sha256 TEXT NOT NULL, content BLOB NOT NULL, saved_at_ms INTEGER NOT NULL)";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotMetadata {
    pub source_id: String,
    pub display_name: String,
    pub sha256: String,
    pub byte_length: u32,
    pub saved_at_ms: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveReceipt {
    pub snapshot: SnapshotMetadata,
    pub already_saved: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreError {
    Unavailable,
    UnsupportedSchema,
    Corrupt,
    InvalidId,
    NotFound,
    Full,
    CapabilityInvalid,
    CapabilityConflict,
    CapabilityNotFound,
    CapabilityFull,
    BackupInvalid,
    WorkspaceChanged,
    SourceInUse,
    CompositionInvalid,
    CompositionDependencyMissing,
    RecordInUse,
}
impl StoreError {
    pub fn public(self) -> (&'static str, &'static str) {
        match self {
            Self::CompositionDependencyMissing => (
                "composition_dependency_missing",
                "This restore needs a pinned input revision that is absent from the final workspace. Existing skill histories were kept unchanged.",
            ),
            Self::RecordInUse => (
                "record_in_use",
                "Other saved skills still depend on this record. Keep it or remove those dependent skills first.",
            ),
            Self::CompositionInvalid => (
                "composition_invalid",
                "This composition or its destinations failed validation. Refresh the preview and review the draft again.",
            ),
            Self::BackupInvalid => (
                "backup_invalid",
                "This backup is unsupported, incomplete or failed validation. No data was restored.",
            ),
            Self::WorkspaceChanged => (
                "workspace_changed",
                "Saved data changed since this preview. Refresh and review the operation again.",
            ),
            Self::SourceInUse => (
                "source_in_use",
                "This source is referenced by saved skills. Remove those skills first or keep the source.",
            ),
            Self::CapabilityInvalid => (
                "capability_invalid",
                "Choose a saved source section and provide a nonempty title and content within the displayed limits.",
            ),
            Self::CapabilityConflict => (
                "capability_conflict",
                "This skill has a newer revision. Your draft remains available. Open the current revision before saving or reviewing again.",
            ),
            Self::CapabilityNotFound => (
                "capability_not_found",
                "That skill or revision is unavailable. Refresh the skills list and try again.",
            ),
            Self::CapabilityFull => (
                "capability_full",
                "The local skills workspace reached its capability or revision limit. Existing records remain available.",
            ),
            Self::Unavailable => (
                "workspace_unavailable",
                "Local workspace could not be accessed. Retry after checking available disk space and access. A save may have completed; retry safely checks for an existing copy.",
            ),
            Self::UnsupportedSchema => (
                "workspace_schema",
                "This workspace format is unsupported. It was not replaced. Use a compatible Rangoon version.",
            ),
            Self::Corrupt => (
                "workspace_corrupt",
                "The saved workspace or snapshot failed validation. Your current analysis remains open.",
            ),
            Self::InvalidId => (
                "invalid_snapshot_id",
                "Choose a saved snapshot from the local workspace.",
            ),
            Self::NotFound => (
                "snapshot_not_found",
                "That saved snapshot is unavailable. Refresh the saved list and try again.",
            ),
            Self::Full => (
                "workspace_full",
                "This development workspace has reached its 128-snapshot or 64 MiB limit. Existing snapshots remain available.",
            ),
        }
    }
}
impl From<rusqlite::Error> for StoreError {
    fn from(_: rusqlite::Error) -> Self {
        Self::Unavailable
    }
}

/// Construct only from a host-owned application directory, never renderer input.
/// No source paths are stored. Parent directory custody is not a security claim.
#[derive(Clone)]
pub struct Workspace {
    directory: PathBuf,
}
impl Workspace {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }
    fn path(&self) -> PathBuf {
        self.directory.join("workspace.sqlite3")
    }

    pub fn list(&self) -> Result<Vec<SnapshotMetadata>, StoreError> {
        let Some(mut db) = self.connect(false)? else {
            return Ok(Vec::new());
        };
        let tx = db.transaction()?;
        verify_schema(&tx)?;
        let snapshots = list_metadata(&tx)?;
        tx.commit()?;
        Ok(snapshots)
    }

    pub fn open(&self, id: &str) -> Result<AnalysisReport, StoreError> {
        validate_id(id)?;
        let Some(mut db) = self.connect(false)? else {
            return Err(StoreError::NotFound);
        };
        let tx = db.transaction()?;
        verify_schema(&tx)?;
        let report = read_report(&tx, id)?;
        tx.commit()?;
        Ok(report)
    }

    pub fn save(&self, report: &AnalysisReport) -> Result<SaveReceipt, StoreError> {
        // Persist source facts only. Never deserialize a renderer-provided report.
        let checked = analyze(
            &report.source.display_name,
            report.source.content.as_bytes(),
        )
        .map_err(|_| StoreError::Corrupt)?;
        if &checked != report {
            return Err(StoreError::Corrupt);
        }
        let mut db = self.connect(true)?.ok_or(StoreError::Unavailable)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        initialize_or_verify(&tx)?;
        let version: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version == 3 {
            // Schema 3 remains read-only until its recovery format is available.
            return Err(StoreError::UnsupportedSchema);
        }
        let snapshots = list_metadata(&tx)?;
        if let Some(existing) = snapshots
            .into_iter()
            .find(|s| s.source_id == report.source.id)
        {
            if read_report(&tx, &existing.source_id)? != checked {
                return Err(StoreError::Corrupt);
            }
            tx.commit()?;
            return Ok(SaveReceipt {
                snapshot: existing,
                already_saved: true,
            });
        }
        let count: i64 = tx.query_row("SELECT count(*) FROM snapshots", [], |r| r.get(0))?;
        if count >= MAX_SNAPSHOTS as i64 {
            return Err(StoreError::Full);
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| StoreError::Unavailable)?
            .as_millis();
        let saved_at_ms = i64::try_from(now).map_err(|_| StoreError::Unavailable)?;
        let snapshot = SnapshotMetadata {
            source_id: checked.source.id,
            display_name: checked.source.display_name,
            sha256: checked.source.sha256,
            byte_length: checked.source.byte_length as u32,
            saved_at_ms,
        };
        insert(&tx, &snapshot, checked.source.content.as_bytes())?;
        tx.commit()?;
        Ok(SaveReceipt {
            snapshot,
            already_saved: false,
        })
    }

    fn connect(&self, create: bool) -> Result<Option<Connection>, StoreError> {
        self.connect_with_empty(create, false)
    }

    // Recovery previews may inspect a verified empty file left by a failed first
    // write. They never create a file or initialize tables.
    fn connect_with_empty(
        &self,
        create: bool,
        allow_empty: bool,
    ) -> Result<Option<Connection>, StoreError> {
        match fs::symlink_metadata(&self.directory) {
            Ok(meta) if !linked(&meta) && meta.is_dir() => (),
            Ok(_) => return Err(StoreError::Unavailable),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if !create {
                    return Ok(None);
                }
                fs::create_dir_all(&self.directory).map_err(|_| StoreError::Unavailable)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&self.directory, fs::Permissions::from_mode(0o700))
                        .map_err(|_| StoreError::Unavailable)?;
                }
            }
            Err(_) => return Err(StoreError::Unavailable),
        }
        let path = self.path();
        for suffix in ["", "-journal", "-wal", "-shm"] {
            let candidate = self.directory.join(format!("workspace.sqlite3{suffix}"));
            match fs::symlink_metadata(&candidate) {
                Ok(meta) if linked(&meta) || !meta.is_file() => {
                    return Err(StoreError::Unavailable);
                }
                Ok(meta) if meta.len() > MAX_DATABASE_BYTES => return Err(StoreError::Full),
                Ok(_) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(_) => return Err(StoreError::Unavailable),
            }
        }
        if !path.exists() {
            if !create {
                return Ok(None);
            }
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(_) => (),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(_) => return Err(StoreError::Unavailable),
            }
        }
        // No CREATE flag: only explicit Save/Restore creates the file above.
        let db = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        db.busy_timeout(Duration::from_millis(1000))?;
        db.set_limit(Limit::SQLITE_LIMIT_LENGTH, (MAX_SOURCE_BYTES + 4096) as i32)?;
        db.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 16_384)?;
        db.pragma_update(None, "trusted_schema", false)?;
        db.pragma_update(None, "synchronous", "FULL")?;
        // Reject unsupported files before changing their persistent journal mode.
        let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version != 0 {
            verify_schema(&db)?;
            if version == 3 {
                db.set_limit(
                    Limit::SQLITE_LIMIT_LENGTH,
                    (rangoon_compose::MAX_DRAFT_BYTES + 4096) as i32,
                )?;
            }
        } else if !create && !allow_empty {
            return Err(StoreError::UnsupportedSchema);
        } else {
            verify_empty(&db)?;
            // Match canonical backup reconstruction. An existing noncanonical
            // empty SQLite file is rejected rather than vacuumed or replaced.
            db.pragma_update(None, "page_size", 4096)?;
            let initial_page_size: u32 = db.pragma_query_value(None, "page_size", |r| r.get(0))?;
            if initial_page_size != 4096 {
                return Err(StoreError::UnsupportedSchema);
            }
        }
        let mode: String = db.pragma_query_value(None, "journal_mode", |r| r.get(0))?;
        if mode != "delete" {
            return Err(StoreError::UnsupportedSchema);
        }
        let page_size: u32 = db.pragma_query_value(None, "page_size", |r| r.get(0))?;
        if !(512..=65536).contains(&page_size) {
            return Err(StoreError::Corrupt);
        }
        db.pragma_update(
            None,
            "max_page_count",
            (MAX_DATABASE_BYTES / u64::from(page_size)) as u32,
        )?;
        Ok(Some(db))
    }
}

fn linked(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}
fn insert(db: &Connection, s: &SnapshotMetadata, bytes: &[u8]) -> Result<(), StoreError> {
    db.execute("INSERT INTO snapshots (source_id, display_name, sha256, content, saved_at_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![s.source_id, s.display_name, s.sha256, bytes, s.saved_at_ms])?;
    Ok(())
}
fn verify_empty(db: &Connection) -> Result<(), StoreError> {
    let app_id: i64 = db.pragma_query_value(None, "application_id", |r| r.get(0))?;
    let count: i64 = db.query_row("SELECT count(*) FROM sqlite_schema", [], |r| r.get(0))?;
    if app_id != 0 || count != 0 {
        return Err(StoreError::UnsupportedSchema);
    }
    Ok(())
}
fn initialize_or_verify(db: &Connection) -> Result<(), StoreError> {
    let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 0 {
        verify_empty(db)?;
        db.execute_batch(SCHEMA)?;
        db.pragma_update(None, "application_id", APPLICATION_ID)?;
        db.pragma_update(None, "user_version", 1)?;
    }
    verify_schema(db)
}
fn verify_schema(db: &Connection) -> Result<(), StoreError> {
    let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let app_id: i64 = db.pragma_query_value(None, "application_id", |r| r.get(0))?;
    let mut query = db.prepare("SELECT type, name, sql FROM sqlite_schema WHERE name NOT IN ('sqlite_autoindex_snapshots_1','sqlite_autoindex_capabilities_1','sqlite_autoindex_revisions_1','sqlite_autoindex_reviews_1','sqlite_autoindex_compositions_1','sqlite_autoindex_composition_applications_1','sqlite_autoindex_derived_capabilities_1','sqlite_autoindex_derived_capabilities_2','sqlite_autoindex_revision_derivations_1','sqlite_autoindex_revision_derivations_2') ORDER BY name")?;
    let schema: Vec<(String, String, String)> = query
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let mut expected: Vec<(String, String, String)> =
        vec![("table".into(), "snapshots".into(), SCHEMA.into())];
    if version >= 2 {
        expected.extend(
            capabilities::SCHEMAS
                .iter()
                .map(|(name, sql)| ("table".into(), (*name).into(), (*sql).into())),
        );
    }
    if version == 3 {
        expected.extend(
            composition_records::SCHEMAS
                .iter()
                .map(|(name, sql)| ("table".into(), (*name).into(), (*sql).into())),
        );
    }
    expected.sort_by(|a, b| a.1.cmp(&b.1));
    if ![1, 2, 3].contains(&version) || app_id != APPLICATION_ID || schema != expected {
        return Err(StoreError::UnsupportedSchema);
    }
    Ok(())
}
pub fn validate_id(id: &str) -> Result<(), StoreError> {
    if id.strip_prefix("source:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }) {
        Ok(())
    } else {
        Err(StoreError::InvalidId)
    }
}
fn list_metadata(db: &Connection) -> Result<Vec<SnapshotMetadata>, StoreError> {
    let mut q = db.prepare("SELECT source_id, display_name, sha256, length(content), saved_at_ms FROM snapshots ORDER BY saved_at_ms DESC, source_id LIMIT 129")?;
    let rows = q.query_map([], |r| {
        Ok(SnapshotMetadata {
            source_id: r.get(0)?,
            display_name: r.get(1)?,
            sha256: r.get(2)?,
            byte_length: r.get(3)?,
            saved_at_ms: r.get(4)?,
        })
    })?;
    let result = rows.collect::<Result<Vec<_>, _>>()?;
    if result.len() > MAX_SNAPSHOTS {
        return Err(StoreError::Full);
    }
    for item in &result {
        validate_metadata(item)?;
    }
    Ok(result)
}
fn validate_metadata(s: &SnapshotMetadata) -> Result<(), StoreError> {
    if validate_id(&s.source_id).is_err()
        || validate_display_name(&s.display_name).is_err()
        || s.byte_length > MAX_SOURCE_BYTES as u32
        || !(0..=8_640_000_000_000_000).contains(&s.saved_at_ms)
        || s.sha256.len() != 64
        || !s
            .sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(StoreError::Corrupt);
    }
    Ok(())
}
fn read_report(db: &Connection, id: &str) -> Result<AnalysisReport, StoreError> {
    let meta = list_metadata(db)?
        .into_iter()
        .find(|s| s.source_id == id)
        .ok_or(StoreError::NotFound)?;
    // The transaction holds the metadata check and bounded blob read together.
    let bytes: Vec<u8> = db
        .query_row(
            "SELECT content FROM snapshots WHERE source_id = ?1",
            [id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or(StoreError::NotFound)?;
    let report = analyze(&meta.display_name, &bytes).map_err(|_| StoreError::Corrupt)?;
    if report.source.id != meta.source_id
        || report.source.sha256 != meta.sha256
        || report.source.byte_length != u64::from(meta.byte_length)
    {
        return Err(StoreError::Corrupt);
    }
    Ok(report)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod capability_tests;
