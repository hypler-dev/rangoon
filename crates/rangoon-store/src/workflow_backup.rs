//! Flat V3 archives with complete validated workspace records.
//! Checksums establish internal consistency, not authentication or encryption.
use super::*;
use composition_backup::{
    ApplicationDescriptor, Manifest as BaseManifest, OwnerDescriptor, RecipeDescriptor,
    RevisionDescriptor, bounded,
};
use composition_records::Records;
use rangoon_domain::{byte_digest, capability::valid_id};
use workflow_records::WorkflowRows;

const MAGIC: &[u8] = b"RANGOON-BACKUP-V3\n";
const SCHEMA: &str = "rangoon.backup.v3";
const MAX_MANIFEST: usize = 2 * 1024 * 1024;
const MAX_RECORD: usize = 160 * 1024;

pub struct WorkspaceBackup {
    pub(super) records: Records,
    id: String,
    byte_length: usize,
}

impl std::fmt::Debug for WorkspaceBackup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("WorkspaceBackup(<redacted>)")
    }
}

/// A bounded, existing-only Legacy capture. This is not migration authority.
/// Paths, decoded records and plaintext bytes never enter diagnostics.
///
/// ```compile_fail
/// fn serializable<T: serde::Serialize>() {}
/// serializable::<rangoon_store::ExistingWorkspaceCapture>();
/// ```
pub struct ExistingWorkspaceCapture {
    content: Option<CapturedContent>,
    #[cfg(unix)]
    source: ExistingSourceProof,
}

struct CapturedContent {
    archive: zeroize::Zeroizing<Vec<u8>>,
    digest: String,
    schema: u32,
    inventory: WorkflowRecoveryCounts,
}

impl std::fmt::Debug for ExistingWorkspaceCapture {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ExistingWorkspaceCapture(<redacted>)")
    }
}

impl ExistingWorkspaceCapture {
    fn content(&self) -> Result<&CapturedContent, StoreError> {
        self.content.as_ref().ok_or(StoreError::WorkspaceChanged)
    }
    pub fn schema_version(&self) -> Result<u32, StoreError> {
        Ok(self.content()?.schema)
    }
    pub fn inventory(&self) -> Result<&WorkflowRecoveryCounts, StoreError> {
        Ok(&self.content()?.inventory)
    }
    pub fn digest(&self) -> Result<&str, StoreError> {
        Ok(&self.content()?.digest)
    }
    /// Restart-stable identity metadata, not authentication or content authority.
    pub fn identity_fingerprint(&self) -> Result<String, StoreError> {
        self.content()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            self.source.check()?;
            let mut bytes = b"rangoon.legacy-identity.v1\0".to_vec();
            bytes.push(
                u8::try_from(self.source.ancestors.len()).map_err(|_| StoreError::Unavailable)?,
            );
            for metadata in self
                .source
                .ancestors
                .iter()
                .rev()
                .map(|(_, metadata)| metadata)
                .chain(std::iter::once(&self.source.database))
            {
                bytes.extend_from_slice(&metadata.dev().to_be_bytes());
                bytes.extend_from_slice(&metadata.ino().to_be_bytes());
            }
            Ok(byte_digest(&bytes))
        }
        #[cfg(not(unix))]
        {
            Err(StoreError::Unavailable)
        }
    }
    /// Trusted Rust host access only. No renderer/export endpoint is registered.
    pub fn archive(&self) -> Result<&[u8], StoreError> {
        Ok(&self.content()?.archive)
    }
    pub fn cancel(&mut self) {
        self.content = None;
    }
}

#[cfg(unix)]
struct ExistingSourceProof {
    directory: PathBuf,
    ancestors: Vec<(PathBuf, fs::Metadata)>,
    database: fs::Metadata,
}

#[cfg(unix)]
impl ExistingSourceProof {
    fn observe(directory: &std::path::Path) -> Result<Self, StoreError> {
        use std::path::Component;
        if !directory.is_absolute()
            || directory.components().count() > 64
            || directory
                .components()
                .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
        {
            return Err(StoreError::Unavailable);
        }
        let mut ancestors = Vec::new();
        for path in directory.ancestors() {
            let metadata = fs::symlink_metadata(path).map_err(|_| StoreError::Unavailable)?;
            if linked(&metadata) || !metadata.is_dir() {
                return Err(StoreError::Unavailable);
            }
            ancestors.push((path.to_owned(), metadata));
        }
        let database = Self::database(directory)?;
        let result = Self {
            directory: directory.to_owned(),
            ancestors,
            database,
        };
        result.check()?;
        Ok(result)
    }
    fn same_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
        use std::os::unix::fs::MetadataExt;
        left.dev() == right.dev() && left.ino() == right.ino()
    }
    fn database(directory: &std::path::Path) -> Result<fs::Metadata, StoreError> {
        use std::os::unix::fs::MetadataExt;
        // Refuse all sidecars before SQLite can inspect or recover them. A
        // later appearance is a refusal too, never a repair/checkpoint path.
        let mut count = 0;
        for entry in fs::read_dir(directory).map_err(|_| StoreError::Unavailable)? {
            let entry = entry.map_err(|_| StoreError::Unavailable)?;
            count += 1;
            if count > 1 || entry.file_name() != "workspace.sqlite3" {
                return Err(StoreError::Unavailable);
            }
        }
        let metadata = fs::symlink_metadata(directory.join("workspace.sqlite3"))
            .map_err(|_| StoreError::Unavailable)?;
        if linked(&metadata) || !metadata.is_file() || metadata.nlink() != 1 || metadata.len() == 0
        {
            return Err(StoreError::Unavailable);
        }
        if metadata.len() > MAX_DATABASE_BYTES {
            return Err(StoreError::Full);
        }
        Ok(metadata)
    }
    fn check(&self) -> Result<(), StoreError> {
        for (path, held) in &self.ancestors {
            let current = fs::symlink_metadata(path).map_err(|_| StoreError::WorkspaceChanged)?;
            if linked(&current) || !current.is_dir() || !Self::same_identity(held, &current) {
                return Err(StoreError::WorkspaceChanged);
            }
        }
        if !Self::same_identity(&self.database, &Self::database(&self.directory)?) {
            return Err(StoreError::WorkspaceChanged);
        }
        Ok(())
    }
    fn unchanged_during_capture(&self) -> Result<(), StoreError> {
        use std::os::unix::fs::MetadataExt;
        self.check()?;
        let current = Self::database(&self.directory)?;
        if self.database.len() != current.len()
            || self.database.mtime() != current.mtime()
            || self.database.mtime_nsec() != current.mtime_nsec()
            || self.database.ctime() != current.ctime()
            || self.database.ctime_nsec() != current.ctime_nsec()
        {
            return Err(StoreError::WorkspaceChanged);
        }
        Ok(())
    }
}

#[cfg(test)]
std::thread_local! {
    static CAPTURE_PROBE: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = std::cell::RefCell::new(None);
}

impl Workspace {
    /// Captures only an existing, initialized Legacy database. No create,
    /// upgrade, journal recovery, vault access or temporary archive is allowed.
    pub fn capture_existing_snapshot(&self) -> Result<ExistingWorkspaceCapture, StoreError> {
        #[cfg(not(unix))]
        {
            Err(StoreError::Unavailable)
        }
        #[cfg(unix)]
        {
            if self.key.is_some() {
                return Err(StoreError::Unavailable);
            }
            let source = ExistingSourceProof::observe(&self.directory)?;
            let mut database = self
                .connect_complete_read_only(true)?
                .ok_or(StoreError::Unavailable)?;
            source.check()?;
            let transaction = database.transaction()?;
            let schema: u32 =
                transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
            let records = workflow_mutations::read_records(&transaction)?;
            let inventory = workflow_recovery::counts(&records);
            let archive = zeroize::Zeroizing::new(encode(&records)?);
            if archive.len() > MAX_BACKUP_BYTES {
                return Err(StoreError::Full);
            }
            WorkspaceBackup::decode(&archive)?;
            let digest = byte_digest(&archive);
            #[cfg(test)]
            CAPTURE_PROBE.with(|probe| {
                if let Some(probe) = probe.borrow_mut().take() {
                    probe();
                }
            });
            transaction.commit()?;
            drop(database);
            source.unchanged_during_capture()?;
            Ok(ExistingWorkspaceCapture {
                content: Some(CapturedContent {
                    archive,
                    digest,
                    schema,
                    inventory,
                }),
                source,
            })
        }
    }

    /// Full logical recapture, not an exclusive writer fence or cutover.
    /// Every failure invalidates retained plaintext, including path mismatch.
    pub fn revalidate_existing_capture(
        &self,
        retained: &mut ExistingWorkspaceCapture,
    ) -> Result<(), StoreError> {
        let result = (|| {
            #[cfg(not(unix))]
            {
                let _ = retained;
                Err(StoreError::Unavailable)
            }
            #[cfg(unix)]
            {
                retained.content()?;
                if self.directory != retained.source.directory {
                    return Err(StoreError::WorkspaceChanged);
                }
                retained.source.check()?;
                let fresh = self.capture_existing_snapshot()?;
                let old = retained.content()?;
                let new = fresh.content()?;
                if old.schema != new.schema
                    || old.inventory != new.inventory
                    || old.digest != new.digest
                    || old.archive != new.archive
                {
                    return Err(StoreError::WorkspaceChanged);
                }
                Ok(())
            }
        })();
        if result.is_err() {
            retained.cancel();
        }
        result
    }
}

impl WorkspaceBackup {
    pub fn decode(bytes: &[u8]) -> Result<Self, StoreError> {
        // Protected history has no plaintext import/export surface. The complete
        // codec is used only inside authenticated encrypted recovery custody.
        if bytes.starts_with(b"RANGOON-BACKUP-V4\n") {
            return Err(StoreError::BackupInvalid);
        }
        Self::decode_complete(bytes)
    }
    pub(super) fn decode_complete(bytes: &[u8]) -> Result<Self, StoreError> {
        let records = if bytes.starts_with(b"RANGOON-BACKUP-V4\n") {
            instruction_history_backup::decode(bytes)
        } else if bytes.starts_with(MAGIC) {
            decode(bytes).map_err(|error| match error {
                StoreError::Full => StoreError::Full,
                _ => StoreError::BackupInvalid,
            })
        } else if bytes.starts_with(b"RANGOON-BACKUP-V1\n")
            || bytes.starts_with(b"RANGOON-BACKUP-V2\n")
        {
            CompositionBackup::decode(bytes).map(|backup| backup.records)
        } else {
            Err(StoreError::BackupInvalid)
        }?;
        Ok(Self {
            records,
            id: format!("backup:{}", byte_digest(bytes)),
            byte_length: bytes.len(),
        })
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn byte_length(&self) -> usize {
        self.byte_length
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema_version: String,
    #[serde(deserialize_with = "composition_backup::sources")]
    sources: Vec<SnapshotMetadata>,
    #[serde(deserialize_with = "composition_backup::owners")]
    owners: Vec<OwnerDescriptor>,
    #[serde(deserialize_with = "composition_backup::revisions")]
    revisions: Vec<RevisionDescriptor>,
    #[serde(deserialize_with = "composition_backup::recipes")]
    recipes: Vec<RecipeDescriptor>,
    #[serde(deserialize_with = "composition_backup::applications")]
    applications: Vec<ApplicationDescriptor>,
    #[serde(deserialize_with = "workflows")]
    workflows: Vec<Owner>,
    #[serde(deserialize_with = "workflow_revisions")]
    workflow_revisions: Vec<Revision>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Owner {
    id: String,
    latest_revision_id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Revision {
    id: String,
    workflow_id: String,
    byte_length: u32,
    sha256: String,
    saved_at_ms: i64,
}

fn workflows<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Owner>, D::Error> {
    bounded::<_, _, 128>(d)
}
fn workflow_revisions<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Revision>, D::Error> {
    bounded::<_, _, 1024>(d)
}

impl Manifest {
    fn from_records(base: &Records, workflows: &WorkflowRows) -> Result<Self, StoreError> {
        let base = BaseManifest::from_records(base)?;
        Ok(Self {
            schema_version: SCHEMA.into(),
            sources: base.sources,
            owners: base.owners,
            revisions: base.revisions,
            recipes: base.recipes,
            applications: base.applications,
            workflows: workflows
                .owners
                .iter()
                .map(|(id, head)| Owner {
                    id: id.clone(),
                    latest_revision_id: head.clone(),
                })
                .collect(),
            workflow_revisions: workflows
                .revisions
                .iter()
                .map(|(id, saved)| {
                    let bytes = saved
                        .record
                        .serialized_bytes()
                        .map_err(|_| StoreError::Corrupt)?;
                    Ok(Revision {
                        id: id.clone(),
                        workflow_id: saved.record.workflow_id().into(),
                        byte_length: bytes.len() as u32,
                        sha256: byte_digest(&bytes),
                        saved_at_ms: saved.saved_at_ms,
                    })
                })
                .collect::<Result<_, StoreError>>()?,
        })
    }
    fn base(&self) -> BaseManifest {
        BaseManifest {
            schema_version: "rangoon.backup.v2".into(),
            sources: self.sources.clone(),
            owners: self.owners.clone(),
            revisions: self.revisions.clone(),
            recipes: self.recipes.clone(),
            applications: self.applications.clone(),
        }
    }
    fn preflight(&self) -> Result<usize, StoreError> {
        fn ordered<'a>(mut ids: impl Iterator<Item = &'a str>) -> bool {
            let mut previous = ids.next();
            for id in ids {
                if previous.is_some_and(|p| p >= id) {
                    return false;
                }
                previous = Some(id);
            }
            true
        }
        if self.schema_version != SCHEMA
            || self.workflows.len() > 128
            || self.workflow_revisions.len() > 1024
            || !ordered(self.workflows.iter().map(|w| w.id.as_str()))
            || !ordered(self.workflow_revisions.iter().map(|r| r.id.as_str()))
            || self.workflows.iter().any(|w| {
                !valid_id(&w.id, "workflow:")
                    || !valid_id(&w.latest_revision_id, "workflow-revision:")
            })
        {
            return Err(StoreError::BackupInvalid);
        }
        let mut total = self.base().preflight()?;
        for revision in &self.workflow_revisions {
            if !valid_id(&revision.id, "workflow-revision:")
                || !valid_id(&revision.workflow_id, "workflow:")
                || !valid_id(&revision.sha256, "")
                || revision.byte_length as usize > MAX_RECORD
                || !(0..=8_640_000_000_000_000).contains(&revision.saved_at_ms)
            {
                return Err(StoreError::BackupInvalid);
            }
            total = total
                .checked_add(revision.byte_length as usize)
                .ok_or(StoreError::BackupInvalid)?;
        }
        if total > MAX_DATABASE_BYTES as usize {
            return Err(StoreError::BackupInvalid);
        }
        Ok(total)
    }
    fn bytes(&self) -> Result<Vec<u8>, StoreError> {
        let bytes = serde_json::to_vec(self).map_err(|_| StoreError::BackupInvalid)?;
        if bytes.len() > MAX_MANIFEST {
            return Err(StoreError::Full);
        }
        Ok(bytes)
    }
}

/// Reconstruct all records and validate allocation in a disposable database.
/// Schema downgrade here is only the in-memory base reconstruction step; no
/// destination database or original record version is changed.
pub(super) fn canonical_database(base: &Records) -> Result<Connection, StoreError> {
    if base.version == 5 {
        return instruction_history_backup::canonical_database(base);
    }
    if !base.history.changes.is_empty() || !base.history.events.is_empty() {
        return Err(StoreError::Corrupt);
    }
    let workflows = &base.workflows;
    if !(1..=4).contains(&base.version) {
        return Err(StoreError::UnsupportedSchema);
    }
    let mut projected = base.clone();
    projected.version = 3;
    projected.workflows = WorkflowRows::default();
    let mut db = composition_backup::canonical_database(&projected)?;
    let tx = db.transaction()?;
    for (_, sql) in workflow_records::SCHEMAS {
        tx.execute_batch(sql)?;
    }
    tx.pragma_update(None, "user_version", 4)?;
    for (id, head) in &workflows.owners {
        tx.execute("INSERT INTO workflows VALUES (?1,?2)", params![id, head])?;
    }
    for (id, saved) in &workflows.revisions {
        tx.execute(
            "INSERT INTO workflow_revisions VALUES (?1,?2,?3,?4)",
            params![
                id,
                saved.record.workflow_id(),
                saved
                    .record
                    .serialized_bytes()
                    .map_err(|_| StoreError::Corrupt)?,
                saved.saved_at_ms
            ],
        )?;
    }
    Records::load_complete(&tx)?;
    let pages: u32 = tx.pragma_query_value(None, "page_count", |r| r.get(0))?;
    if u64::from(pages).checked_mul(4096).ok_or(StoreError::Full)? > MAX_DATABASE_BYTES {
        return Err(StoreError::Full);
    }
    tx.commit()?;
    Ok(db)
}

pub(super) fn encode(base: &Records) -> Result<Vec<u8>, StoreError> {
    if base.version == 5 {
        return instruction_history_backup::encode(base);
    }
    if !base.history.changes.is_empty() || !base.history.events.is_empty() {
        return Err(StoreError::Corrupt);
    }
    let workflows = &base.workflows;
    if !(1..=4).contains(&base.version) {
        return Err(StoreError::UnsupportedSchema);
    }
    if base.version != 4 {
        if !workflows.owners.is_empty() || !workflows.revisions.is_empty() {
            return Err(StoreError::Corrupt);
        }
        return composition_backup::encode(base);
    }
    let manifest = Manifest::from_records(base, workflows)?;
    let payload = manifest.preflight()?;
    let metadata = manifest.bytes()?;
    let length = MAGIC
        .len()
        .checked_add(4)
        .and_then(|n| n.checked_add(metadata.len()))
        .and_then(|n| n.checked_add(payload))
        .and_then(|n| n.checked_add(64))
        .ok_or(StoreError::Full)?;
    if length > MAX_BACKUP_BYTES {
        return Err(StoreError::Full);
    }
    canonical_database(base)?;
    let mut bytes = Vec::with_capacity(length);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&(metadata.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&metadata);
    composition_backup::append_payload(base, &mut bytes)?;
    for saved in workflows.revisions.values() {
        bytes.extend_from_slice(
            &saved
                .record
                .serialized_bytes()
                .map_err(|_| StoreError::Corrupt)?,
        );
    }
    bytes.extend_from_slice(byte_digest(&bytes).as_bytes());
    if bytes.len() != length {
        return Err(StoreError::Corrupt);
    }
    Ok(bytes)
}

fn decode(bytes: &[u8]) -> Result<Records, StoreError> {
    if bytes.len() > MAX_BACKUP_BYTES
        || bytes.len() < MAGIC.len() + 4 + 64
        || !bytes.starts_with(MAGIC)
    {
        return Err(StoreError::BackupInvalid);
    }
    let end = bytes.len() - 64;
    if byte_digest(&bytes[..end]).as_bytes() != &bytes[end..] {
        return Err(StoreError::BackupInvalid);
    }
    let start = MAGIC.len() + 4;
    let length = u32::from_be_bytes(
        bytes[MAGIC.len()..start]
            .try_into()
            .map_err(|_| StoreError::BackupInvalid)?,
    ) as usize;
    let payload_start = start
        .checked_add(length)
        .filter(|n| *n <= end)
        .ok_or(StoreError::BackupInvalid)?;
    if length > MAX_MANIFEST {
        return Err(StoreError::BackupInvalid);
    }
    let manifest: Manifest = serde_json::from_slice(&bytes[start..payload_start])
        .map_err(|_| StoreError::BackupInvalid)?;
    let payload_length = manifest.preflight()?;
    if payload_start.checked_add(payload_length) != Some(end)
        || manifest.bytes().map_err(|_| StoreError::BackupInvalid)? != bytes[start..payload_start]
    {
        return Err(StoreError::BackupInvalid);
    }
    let base_manifest = manifest.base();
    let base_end = payload_start
        .checked_add(base_manifest.preflight()?)
        .ok_or(StoreError::BackupInvalid)?;
    let base = composition_backup::read_payload(&base_manifest, &bytes[payload_start..base_end])?;
    let mut db = composition_backup::canonical_database(&base)?;
    let tx = db.transaction()?;
    for (_, sql) in workflow_records::SCHEMAS {
        tx.execute_batch(sql)?;
    }
    tx.pragma_update(None, "user_version", 4)?;
    for owner in &manifest.workflows {
        tx.execute(
            "INSERT INTO workflows VALUES (?1,?2)",
            params![owner.id, owner.latest_revision_id],
        )?;
    }
    let mut cursor = base_end;
    for revision in &manifest.workflow_revisions {
        let next = cursor
            .checked_add(revision.byte_length as usize)
            .filter(|n| *n <= end)
            .ok_or(StoreError::BackupInvalid)?;
        let record = &bytes[cursor..next];
        cursor = next;
        if byte_digest(record) != revision.sha256 {
            return Err(StoreError::BackupInvalid);
        }
        tx.execute(
            "INSERT INTO workflow_revisions VALUES (?1,?2,?3,?4)",
            params![
                revision.id,
                revision.workflow_id,
                record,
                revision.saved_at_ms
            ],
        )?;
    }
    let complete = Records::load_complete(&tx)?;
    if cursor != end
        || Manifest::from_records(&complete, &complete.workflows)?
            .bytes()
            .map_err(|_| StoreError::BackupInvalid)?
            != manifest.bytes().map_err(|_| StoreError::BackupInvalid)?
    {
        return Err(StoreError::BackupInvalid);
    }
    let pages: u32 = tx.pragma_query_value(None, "page_count", |r| r.get(0))?;
    if u64::from(pages).checked_mul(4096).ok_or(StoreError::Full)? > MAX_DATABASE_BYTES {
        return Err(StoreError::Full);
    }
    Ok(complete)
}

impl Workspace {
    /// Read-only export. Persistent schema 4 remains disabled until the complete
    /// transactional lifecycle is integrated; existing workspaces export V2.
    pub fn export_workflow_backup(&self) -> Result<Vec<u8>, StoreError> {
        let Some(mut db) = self.connect_complete_read_only(true)? else {
            return encode(&Records::empty());
        };
        let tx = db.transaction()?;
        let records = workflow_mutations::read_records(&tx)?;
        if records.version == 5 {
            return Err(StoreError::UnsupportedSchema);
        }
        let bytes = encode(&records)?;
        tx.commit()?;
        Ok(bytes)
    }
}

#[cfg(test)]
#[path = "workflow_backup_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "workspace_capture_tests.rs"]
mod capture_tests;
