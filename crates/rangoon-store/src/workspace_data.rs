//! Portable inert data, additive restore and explicit owner-managed deletion.
use super::*;
use rangoon_domain::{
    byte_digest,
    capability::{CapabilitySummary, valid_id},
};
use std::collections::BTreeSet;

const MAGIC: &[u8] = b"RANGOON-BACKUP-V1\n";
const MAX_MANIFEST_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_BACKUP_BYTES: usize = 68 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Source {
    metadata: SnapshotMetadata,
    #[serde(skip)]
    content: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Capability {
    id: String,
    source_id: String,
    fragment_id: String,
    latest_revision_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Revision {
    id: String,
    capability_id: String,
    #[serde(deserialize_with = "required_parent")]
    parent_revision_id: Option<String>,
    title: String,
    sha256: String,
    created_at_ms: i64,
    byte_length: u32,
    #[serde(skip)]
    content: Vec<u8>,
}
fn required_parent<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Review {
    revision_id: String,
    reviewer: String,
    reviewed_at_ms: i64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Image {
    version: u32,
    sources: Vec<Source>,
    capabilities: Vec<Capability>,
    revisions: Vec<Revision>,
    reviews: Vec<Review>,
}
impl Default for Image {
    fn default() -> Self {
        Self {
            version: 1,
            sources: vec![],
            capabilities: vec![],
            revisions: vec![],
            reviews: vec![],
        }
    }
}

/// Validated application records only. The caller cannot construct unchecked data.
#[derive(Debug)]
pub struct Backup {
    image: Image,
    id: String,
    byte_length: usize,
}
impl Backup {
    pub(super) fn records(&self) -> Result<composition_records::Records, StoreError> {
        composition_records::Records::load(&image_database(&self.image)?)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, StoreError> {
        decode(bytes).map_err(|_| StoreError::BackupInvalid)
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn byte_length(&self) -> usize {
        self.byte_length
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceUsage {
    pub sources: u32,
    pub capabilities: u32,
    pub revisions: u32,
    pub reviews: u32,
    pub database_bytes: u64,
    pub reusable_bytes: u64,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceData {
    pub schema_version: &'static str,
    pub state_id: String,
    pub usage: WorkspaceUsage,
    pub sources: Vec<SnapshotMetadata>,
    pub capabilities: Vec<CapabilitySummary>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RestorePlan {
    pub backup_id: String,
    pub expected_state_id: String,
    pub byte_length: usize,
    pub add_sources: u32,
    pub kept_sources: u32,
    pub add_capabilities: u32,
    pub kept_capabilities: u32,
    pub add_revisions: u32,
    pub add_reviews: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    Source,
    Capability,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletionPlan {
    pub kind: RecordKind,
    pub id: String,
    pub title: String,
    pub expected_state_id: String,
    pub revisions: u32,
    pub reviews: u32,
    pub dependencies: Vec<CapabilitySummary>,
}

impl Workspace {
    pub fn data(&self) -> Result<WorkspaceData, StoreError> {
        let Some(mut db) = self.connect_with_empty(false, true)? else {
            return data(&Image::default(), 0, 0);
        };
        let tx = db.transaction()?;
        let image = read_image(&tx)?;
        let page_size: u32 = tx.pragma_query_value(None, "page_size", |r| r.get(0))?;
        let pages: u32 = tx.pragma_query_value(None, "page_count", |r| r.get(0))?;
        let free: u32 = tx.pragma_query_value(None, "freelist_count", |r| r.get(0))?;
        let result = data(
            &image,
            u64::from(pages) * u64::from(page_size),
            u64::from(free) * u64::from(page_size),
        )?;
        tx.commit()?;
        Ok(result)
    }

    pub fn export_backup(&self) -> Result<Vec<u8>, StoreError> {
        let Some(mut db) = self.connect_with_empty(false, true)? else {
            return encode(&Image::default());
        };
        let tx = db.transaction()?;
        let bytes = encode(&read_image(&tx)?)?;
        tx.commit()?;
        Ok(bytes)
    }

    pub fn prepare_restore(&self, backup: &Backup) -> Result<RestorePlan, StoreError> {
        let Some(mut db) = self.connect_with_empty(false, true)? else {
            return plan(&Image::default(), backup);
        };
        let tx = db.transaction()?;
        let result = plan(&read_image(&tx)?, backup)?;
        tx.commit()?;
        Ok(result)
    }

    pub fn restore_backup(
        &self,
        backup: &Backup,
        expected: &str,
    ) -> Result<RestorePlan, StoreError> {
        check_state_id(expected)?;
        let mut db = self.connect(true)?.ok_or(StoreError::Unavailable)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        initialize_or_verify(&tx)?;
        let current = read_image(&tx)?;
        if state_id(&current)? != expected {
            return Err(StoreError::WorkspaceChanged);
        }
        let receipt = plan(&current, backup)?;
        let sources: BTreeSet<_> = current
            .sources
            .iter()
            .map(|s| s.metadata.source_id.as_str())
            .collect();
        let capabilities: BTreeSet<_> =
            current.capabilities.iter().map(|c| c.id.as_str()).collect();
        let added: BTreeSet<_> = backup
            .image
            .capabilities
            .iter()
            .filter(|c| !capabilities.contains(c.id.as_str()))
            .map(|c| c.id.as_str())
            .collect();
        for source in &backup.image.sources {
            if !sources.contains(source.metadata.source_id.as_str()) {
                insert(&tx, &source.metadata, &source.content)?;
            }
        }
        if !added.is_empty() {
            capabilities::migrate(&tx)?;
            for capability in &backup.image.capabilities {
                if added.contains(capability.id.as_str()) {
                    insert_capability(&tx, capability)?;
                }
            }
            let mut revisions = BTreeSet::new();
            for revision in &backup.image.revisions {
                if added.contains(revision.capability_id.as_str()) {
                    insert_revision(&tx, revision)?;
                    revisions.insert(revision.id.as_str());
                }
            }
            for review in &backup.image.reviews {
                if revisions.contains(review.revision_id.as_str()) {
                    insert_review(&tx, review)?;
                }
            }
        }
        read_image(&tx)?;
        tx.commit()?;
        Ok(receipt)
    }

    pub fn inspect_deletion(&self, kind: RecordKind, id: &str) -> Result<DeletionPlan, StoreError> {
        check_record_id(kind, id)?;
        let mut db = self.connect(false)?.ok_or(StoreError::NotFound)?;
        let tx = db.transaction()?;
        let result = deletion_plan(&read_image(&tx)?, kind, id)?;
        tx.commit()?;
        Ok(result)
    }

    /// Every confirmation is checked against current saved state. Original files are never touched.
    pub fn delete_record(
        &self,
        kind: RecordKind,
        id: &str,
        expected: &str,
    ) -> Result<(), StoreError> {
        check_record_id(kind, id)?;
        check_state_id(expected)?;
        let Some(mut db) = self.connect(false)? else {
            return Err(StoreError::WorkspaceChanged);
        };
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let image = read_image(&tx)?;
        if state_id(&image)? != expected {
            return Err(StoreError::WorkspaceChanged);
        }
        let preview = deletion_plan(&image, kind, id)?;
        if !preview.dependencies.is_empty() {
            return Err(StoreError::SourceInUse);
        }
        match kind {
            RecordKind::Source => {
                tx.execute("DELETE FROM snapshots WHERE source_id=?1", [id])?;
            }
            RecordKind::Capability => {
                tx.execute("DELETE FROM reviews WHERE revision_id IN (SELECT revision_id FROM revisions WHERE capability_id=?1)", [id])?;
                tx.execute("DELETE FROM revisions WHERE capability_id=?1", [id])?;
                tx.execute("DELETE FROM capabilities WHERE capability_id=?1", [id])?;
            }
        }
        read_image(&tx)?;
        tx.commit()?;
        Ok(())
    }
}

fn manifest(image: &Image) -> Result<Vec<u8>, StoreError> {
    let bytes = serde_json::to_vec(image).map_err(|_| StoreError::Corrupt)?;
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(StoreError::Full);
    }
    Ok(bytes)
}
fn state_id(image: &Image) -> Result<String, StoreError> {
    // Every content digest was checked against its bytes by read_image/decode.
    Ok(format!("workspace:{}", byte_digest(&manifest(image)?)))
}
fn check_state_id(id: &str) -> Result<(), StoreError> {
    if valid_id(id, "workspace:") {
        Ok(())
    } else {
        Err(StoreError::WorkspaceChanged)
    }
}
fn check_record_id(kind: RecordKind, id: &str) -> Result<(), StoreError> {
    if valid_id(
        id,
        match kind {
            RecordKind::Source => "source:",
            RecordKind::Capability => "capability:",
        },
    ) {
        Ok(())
    } else {
        Err(StoreError::InvalidId)
    }
}
fn summary(image: &Image, cap: &Capability) -> Result<CapabilitySummary, StoreError> {
    let latest = image
        .revisions
        .iter()
        .find(|r| r.id == cap.latest_revision_id)
        .ok_or(StoreError::Corrupt)?;
    Ok(CapabilitySummary {
        id: cap.id.clone(),
        source_id: cap.source_id.clone(),
        fragment_id: cap.fragment_id.clone(),
        latest_revision_id: latest.id.clone(),
        title: latest.title.clone(),
        reviewed: image.reviews.iter().any(|r| r.revision_id == latest.id),
        revision_count: image
            .revisions
            .iter()
            .filter(|r| r.capability_id == cap.id)
            .count() as u32,
    })
}
fn data(
    image: &Image,
    database_bytes: u64,
    reusable_bytes: u64,
) -> Result<WorkspaceData, StoreError> {
    Ok(WorkspaceData {
        schema_version: "rangoon.workspace-data.v0",
        state_id: state_id(image)?,
        usage: WorkspaceUsage {
            sources: image.sources.len() as u32,
            capabilities: image.capabilities.len() as u32,
            revisions: image.revisions.len() as u32,
            reviews: image.reviews.len() as u32,
            database_bytes,
            reusable_bytes,
        },
        sources: image.sources.iter().map(|s| s.metadata.clone()).collect(),
        capabilities: image
            .capabilities
            .iter()
            .map(|c| summary(image, c))
            .collect::<Result<_, _>>()?,
    })
}
fn plan(current: &Image, backup: &Backup) -> Result<RestorePlan, StoreError> {
    let source_ids: BTreeSet<_> = current
        .sources
        .iter()
        .map(|s| s.metadata.source_id.as_str())
        .collect();
    let cap_ids: BTreeSet<_> = current.capabilities.iter().map(|c| c.id.as_str()).collect();
    let add_sources = backup
        .image
        .sources
        .iter()
        .filter(|s| !source_ids.contains(s.metadata.source_id.as_str()))
        .count();
    let added: BTreeSet<_> = backup
        .image
        .capabilities
        .iter()
        .filter(|c| !cap_ids.contains(c.id.as_str()))
        .map(|c| c.id.as_str())
        .collect();
    let revisions: BTreeSet<_> = backup
        .image
        .revisions
        .iter()
        .filter(|r| added.contains(r.capability_id.as_str()))
        .map(|r| r.id.as_str())
        .collect();
    let add_reviews = backup
        .image
        .reviews
        .iter()
        .filter(|r| revisions.contains(r.revision_id.as_str()))
        .count();
    if current.sources.len() + add_sources > MAX_SNAPSHOTS {
        return Err(StoreError::Full);
    }
    if current.capabilities.len() + added.len() > MAX_CAPABILITIES
        || current.revisions.len() + revisions.len() > MAX_TOTAL_REVISIONS
    {
        return Err(StoreError::CapabilityFull);
    }
    Ok(RestorePlan {
        backup_id: backup.id.clone(),
        expected_state_id: state_id(current)?,
        byte_length: backup.byte_length,
        add_sources: add_sources as u32,
        kept_sources: (backup.image.sources.len() - add_sources) as u32,
        add_capabilities: added.len() as u32,
        kept_capabilities: (backup.image.capabilities.len() - added.len()) as u32,
        add_revisions: revisions.len() as u32,
        add_reviews: add_reviews as u32,
    })
}
fn deletion_plan(image: &Image, kind: RecordKind, id: &str) -> Result<DeletionPlan, StoreError> {
    let (title, revisions, reviews, dependencies) = match kind {
        RecordKind::Source => {
            let source = image
                .sources
                .iter()
                .find(|s| s.metadata.source_id == id)
                .ok_or(StoreError::NotFound)?;
            let dependencies = image
                .capabilities
                .iter()
                .filter(|c| c.source_id == id)
                .map(|c| summary(image, c))
                .collect::<Result<_, _>>()?;
            (source.metadata.display_name.clone(), 0, 0, dependencies)
        }
        RecordKind::Capability => {
            let cap = image
                .capabilities
                .iter()
                .find(|c| c.id == id)
                .ok_or(StoreError::CapabilityNotFound)?;
            let revisions: BTreeSet<_> = image
                .revisions
                .iter()
                .filter(|r| r.capability_id == id)
                .map(|r| r.id.as_str())
                .collect();
            let reviews = image
                .reviews
                .iter()
                .filter(|r| revisions.contains(r.revision_id.as_str()))
                .count();
            (
                summary(image, cap)?.title,
                revisions.len() as u32,
                reviews as u32,
                vec![],
            )
        }
    };
    Ok(DeletionPlan {
        kind,
        id: id.into(),
        title,
        expected_state_id: state_id(image)?,
        revisions,
        reviews,
        dependencies,
    })
}

fn read_image(db: &Connection) -> Result<Image, StoreError> {
    let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 0 {
        verify_empty(db)?;
        return Ok(Image::default());
    }
    verify_schema(db)?;
    if version == 3 {
        // A legacy archive cannot represent composed revisions. Fail explicitly
        // instead of silently exporting or restoring a source-only projection.
        return Err(StoreError::UnsupportedSchema);
    }
    let mut image = Image::default();
    for metadata in list_metadata(db)? {
        let report = read_report(db, &metadata.source_id)?;
        image.sources.push(Source {
            metadata,
            content: report.source.content.into_bytes(),
        });
    }
    image
        .sources
        .sort_by(|a, b| a.metadata.source_id.cmp(&b.metadata.source_id));
    let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 2 {
        capabilities::validate_links_and_limits(db)?;
        let mut query = db.prepare("SELECT capability_id,source_id,fragment_id,latest_revision_id FROM capabilities ORDER BY capability_id LIMIT 129")?;
        image.capabilities = query
            .query_map([], |r| {
                Ok(Capability {
                    id: r.get(0)?,
                    source_id: r.get(1)?,
                    fragment_id: r.get(2)?,
                    latest_revision_id: r.get(3)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        for cap in &image.capabilities {
            capabilities::read_capability(db, &cap.id, None)?;
        }
        let mut query = db.prepare("SELECT revision_id,capability_id,parent_revision_id,title,sha256,created_at_ms,length(content),content FROM revisions ORDER BY revision_id LIMIT 1025")?;
        image.revisions = query
            .query_map([], |r| {
                Ok(Revision {
                    id: r.get(0)?,
                    capability_id: r.get(1)?,
                    parent_revision_id: r.get(2)?,
                    title: r.get(3)?,
                    sha256: r.get(4)?,
                    created_at_ms: r.get(5)?,
                    byte_length: r.get(6)?,
                    content: r.get(7)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        let mut query = db.prepare("SELECT revision_id,reviewer,reviewed_at_ms FROM reviews ORDER BY revision_id LIMIT 1025")?;
        image.reviews = query
            .query_map([], |r| {
                Ok(Review {
                    revision_id: r.get(0)?,
                    reviewer: r.get(1)?,
                    reviewed_at_ms: r.get(2)?,
                })
            })?
            .collect::<Result<_, _>>()?;
    }
    bounds(&image)?;
    Ok(image)
}
fn bounds(image: &Image) -> Result<(), StoreError> {
    if image.version != 1
        || image.sources.len() > MAX_SNAPSHOTS
        || image.capabilities.len() > MAX_CAPABILITIES
        || image.revisions.len() > MAX_TOTAL_REVISIONS
        || image.reviews.len() > MAX_TOTAL_REVISIONS
    {
        return Err(StoreError::BackupInvalid);
    }
    let mut total = 0usize;
    for length in image
        .sources
        .iter()
        .map(|s| s.metadata.byte_length)
        .chain(image.revisions.iter().map(|r| r.byte_length))
    {
        if length as usize > MAX_SOURCE_BYTES {
            return Err(StoreError::BackupInvalid);
        }
        total = total
            .checked_add(length as usize)
            .ok_or(StoreError::BackupInvalid)?;
    }
    if total > MAX_DATABASE_BYTES as usize {
        return Err(StoreError::BackupInvalid);
    }
    fn ordered<'a>(ids: impl Iterator<Item = &'a str>) -> bool {
        let mut previous: Option<&str> = None;
        for id in ids {
            if previous.is_some_and(|p| p >= id) {
                return false;
            }
            previous = Some(id);
        }
        true
    }
    if !ordered(image.sources.iter().map(|s| s.metadata.source_id.as_str()))
        || !ordered(image.capabilities.iter().map(|c| c.id.as_str()))
        || !ordered(image.revisions.iter().map(|r| r.id.as_str()))
        || !ordered(image.reviews.iter().map(|r| r.revision_id.as_str()))
    {
        return Err(StoreError::BackupInvalid);
    }
    Ok(())
}
fn encode(image: &Image) -> Result<Vec<u8>, StoreError> {
    bounds(image)?;
    validate_image(image)?;
    let manifest = manifest(image)?;
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&(manifest.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&manifest);
    for content in image
        .sources
        .iter()
        .map(|s| &s.content)
        .chain(image.revisions.iter().map(|r| &r.content))
    {
        bytes.extend_from_slice(content);
    }
    let digest = byte_digest(&bytes);
    bytes.extend_from_slice(digest.as_bytes());
    if bytes.len() > MAX_BACKUP_BYTES {
        return Err(StoreError::Full);
    }
    Ok(bytes)
}
fn decode(bytes: &[u8]) -> Result<Backup, StoreError> {
    if bytes.len() > MAX_BACKUP_BYTES
        || bytes.len() < MAGIC.len() + 4 + 64
        || !bytes.starts_with(MAGIC)
    {
        return Err(StoreError::BackupInvalid);
    }
    let body_end = bytes.len() - 64;
    if byte_digest(&bytes[..body_end]).as_bytes() != &bytes[body_end..] {
        return Err(StoreError::BackupInvalid);
    }
    let offset = MAGIC.len();
    let length = u32::from_be_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .map_err(|_| StoreError::BackupInvalid)?,
    ) as usize;
    let mut cursor = offset + 4;
    if length > MAX_MANIFEST_BYTES || cursor + length > body_end {
        return Err(StoreError::BackupInvalid);
    }
    let mut image: Image = serde_json::from_slice(&bytes[cursor..cursor + length])
        .map_err(|_| StoreError::BackupInvalid)?;
    bounds(&image)?;
    cursor += length;
    for (length, content) in image
        .sources
        .iter_mut()
        .map(|s| (s.metadata.byte_length, &mut s.content))
        .chain(
            image
                .revisions
                .iter_mut()
                .map(|r| (r.byte_length, &mut r.content)),
        )
    {
        let end = cursor
            .checked_add(length as usize)
            .ok_or(StoreError::BackupInvalid)?;
        if end > body_end {
            return Err(StoreError::BackupInvalid);
        }
        *content = bytes[cursor..end].to_vec();
        cursor = end;
    }
    if cursor != body_end {
        return Err(StoreError::BackupInvalid);
    }
    validate_image(&image)?;
    Ok(Backup {
        image,
        id: format!("backup:{}", byte_digest(bytes)),
        byte_length: bytes.len(),
    })
}
fn validate_image(image: &Image) -> Result<(), StoreError> {
    let db = image_database(image)?;
    if read_image(&db)? != *image {
        return Err(StoreError::BackupInvalid);
    }
    Ok(())
}
fn image_database(image: &Image) -> Result<Connection, StoreError> {
    // Reuse the complete store validators against only application-owned tables.
    let mut db = Connection::open_in_memory()?;
    db.pragma_update(None, "trusted_schema", false)?;
    db.set_limit(Limit::SQLITE_LIMIT_LENGTH, (MAX_SOURCE_BYTES + 4096) as i32)?;
    db.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 16_384)?;
    // A valid archive must fit a newly created workspace including SQLite overhead.
    db.pragma_update(None, "page_size", 4096)?;
    db.pragma_update(None, "max_page_count", (MAX_DATABASE_BYTES / 4096) as u32)?;
    let tx = db.transaction()?;
    initialize_or_verify(&tx)?;
    for source in &image.sources {
        insert(&tx, &source.metadata, &source.content)?;
    }
    if !image.capabilities.is_empty() || !image.revisions.is_empty() || !image.reviews.is_empty() {
        capabilities::migrate(&tx)?;
        for cap in &image.capabilities {
            insert_capability(&tx, cap)?;
        }
        for rev in &image.revisions {
            insert_revision(&tx, rev)?;
        }
        for review in &image.reviews {
            insert_review(&tx, review)?;
        }
    }
    tx.commit()?;
    Ok(db)
}
fn insert_capability(db: &Connection, c: &Capability) -> Result<(), StoreError> {
    db.execute(
        "INSERT INTO capabilities VALUES (?1,?2,?3,?4)",
        params![c.id, c.source_id, c.fragment_id, c.latest_revision_id],
    )?;
    Ok(())
}
fn insert_revision(db: &Connection, r: &Revision) -> Result<(), StoreError> {
    db.execute(
        "INSERT INTO revisions VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            r.id,
            r.capability_id,
            r.parent_revision_id,
            r.title,
            r.content,
            r.sha256,
            r.created_at_ms
        ],
    )?;
    Ok(())
}
fn insert_review(db: &Connection, r: &Review) -> Result<(), StoreError> {
    db.execute(
        "INSERT INTO reviews VALUES (?1,?2,?3)",
        params![r.revision_id, r.reviewer, r.reviewed_at_ms],
    )?;
    Ok(())
}
