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

#[derive(Debug)]
pub struct WorkspaceBackup {
    pub(super) records: Records,
    id: String,
    byte_length: usize,
}

impl WorkspaceBackup {
    pub fn decode(bytes: &[u8]) -> Result<Self, StoreError> {
        let records = if bytes.starts_with(MAGIC) {
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
        let bytes = encode(&workflow_mutations::read_records(&tx)?)?;
        tx.commit()?;
        Ok(bytes)
    }
}

#[cfg(test)]
#[path = "workflow_backup_tests.rs"]
mod tests;
