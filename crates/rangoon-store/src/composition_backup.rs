//! Canonical portable records for composition-aware recovery.
//! Archives are plaintext; their hashes do not authenticate stored data.
use super::*;
use composition_records::{Birth, Records};
use rangoon_compose::application::Target;
use rangoon_domain::{
    byte_digest,
    capability_v1::{RevisionProvenance, RevisionSummary},
};

const MAGIC: &[u8] = b"RANGOON-BACKUP-V2\n";
const SCHEMA_VERSION: &str = "rangoon.backup.v2";
const MAX_MANIFEST: usize = 2 * 1024 * 1024;

/// Validated archive data. Decoding never opens a destination workspace.
#[derive(Debug)]
pub struct CompositionBackup {
    pub(super) records: Records,
    pub(super) id: String,
    pub(super) byte_length: usize,
}
impl CompositionBackup {
    pub fn decode(bytes: &[u8]) -> Result<Self, StoreError> {
        let records = if bytes.starts_with(b"RANGOON-BACKUP-V1\n") {
            Backup::decode(bytes)?.records()
        } else {
            decode(bytes)
        }
        .map_err(|_| StoreError::BackupInvalid)?;
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
    #[serde(deserialize_with = "sources")]
    sources: Vec<SnapshotMetadata>,
    #[serde(deserialize_with = "owners")]
    owners: Vec<OwnerDescriptor>,
    #[serde(deserialize_with = "revisions")]
    revisions: Vec<RevisionDescriptor>,
    #[serde(deserialize_with = "recipes")]
    recipes: Vec<RecipeDescriptor>,
    #[serde(deserialize_with = "applications")]
    applications: Vec<ApplicationDescriptor>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OwnerDescriptor {
    id: String,
    birth: Birth,
    latest_revision_id: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RevisionDescriptor {
    capability_id: String,
    revision: RevisionSummary,
    byte_length: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecipeDescriptor {
    id: String,
    transformation_version: String,
    byte_length: u32,
    sha256: String,
    created_at_ms: i64,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApplicationDescriptor {
    id: String,
    composition_id: String,
    #[serde(deserialize_with = "targets")]
    targets: Vec<Target>,
    created_at_ms: i64,
}

fn bounded<'de, T: Deserialize<'de>, D: serde::Deserializer<'de>, const N: usize>(
    decoder: D,
) -> Result<Vec<T>, D::Error> {
    struct Items<T, const N: usize>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const N: usize> serde::de::Visitor<'de> for Items<T, N> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a bounded record list")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut items = Vec::new();
            while items.len() < N {
                match seq.next_element()? {
                    Some(item) => items.push(item),
                    None => return Ok(items),
                }
            }
            if seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
                return Err(serde::de::Error::custom("record limit"));
            }
            Ok(items)
        }
    }
    decoder.deserialize_seq(Items::<T, N>(std::marker::PhantomData))
}
fn sources<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<SnapshotMetadata>, D::Error> {
    bounded::<_, _, 128>(d)
}
fn owners<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<OwnerDescriptor>, D::Error> {
    bounded::<_, _, 128>(d)
}
fn revisions<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<RevisionDescriptor>, D::Error> {
    bounded::<_, _, 1024>(d)
}
fn recipes<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<RecipeDescriptor>, D::Error> {
    bounded::<_, _, 128>(d)
}
fn applications<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<ApplicationDescriptor>, D::Error> {
    bounded::<_, _, 128>(d)
}
fn targets<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Target>, D::Error> {
    bounded::<_, _, 16>(d)
}

impl Manifest {
    fn from_records(records: &Records) -> Result<Self, StoreError> {
        Ok(Self {
            schema_version: SCHEMA_VERSION.into(),
            sources: records
                .sources
                .values()
                .map(|s| s.metadata.clone())
                .collect(),
            owners: records
                .owners
                .iter()
                .map(|(id, o)| OwnerDescriptor {
                    id: id.clone(),
                    birth: o.birth.clone(),
                    latest_revision_id: o.latest_revision_id.clone(),
                })
                .collect(),
            revisions: records
                .revisions
                .values()
                .map(|r| RevisionDescriptor {
                    capability_id: r.capability_id.clone(),
                    revision: RevisionSummary::from(&r.revision),
                    byte_length: r.revision.content.len() as u32,
                })
                .collect(),
            recipes: records
                .recipes
                .iter()
                .map(|(id, r)| {
                    let bytes = rangoon_compose::canonical_draft_bytes(&r.draft)
                        .map_err(|_| StoreError::Corrupt)?;
                    Ok(RecipeDescriptor {
                        id: id.clone(),
                        transformation_version: rangoon_compose::TRANSFORMATION_VERSION.into(),
                        byte_length: bytes.len() as u32,
                        sha256: byte_digest(&bytes),
                        created_at_ms: r.created_at_ms,
                    })
                })
                .collect::<Result<_, StoreError>>()?,
            applications: records
                .applications
                .iter()
                .map(|(id, a)| ApplicationDescriptor {
                    id: id.clone(),
                    composition_id: a.composition_id.clone(),
                    targets: a.targets.clone(),
                    created_at_ms: a.created_at_ms,
                })
                .collect(),
        })
    }
    fn preflight(&self) -> Result<usize, StoreError> {
        fn ordered<'a>(ids: impl Iterator<Item = &'a str>) -> bool {
            let mut previous = None;
            for id in ids {
                if previous.is_some_and(|p| p >= id) {
                    return false;
                }
                previous = Some(id);
            }
            true
        }
        if self.schema_version != SCHEMA_VERSION
            || self.sources.len() > MAX_SNAPSHOTS
            || self.owners.len() > MAX_CAPABILITIES
            || self.revisions.len() > MAX_TOTAL_REVISIONS
            || self.recipes.len() > 128
            || self.applications.len() > 128
            || !ordered(self.sources.iter().map(|s| s.source_id.as_str()))
            || !ordered(self.owners.iter().map(|o| o.id.as_str()))
            || !ordered(self.revisions.iter().map(|r| r.revision.id.as_str()))
            || !ordered(self.recipes.iter().map(|r| r.id.as_str()))
            || !ordered(self.applications.iter().map(|a| a.id.as_str()))
        {
            return Err(StoreError::BackupInvalid);
        }
        let mut total = 0usize;
        for (length, limit) in self
            .sources
            .iter()
            .map(|s| (s.byte_length, MAX_SOURCE_BYTES))
            .chain(
                self.revisions
                    .iter()
                    .map(|r| (r.byte_length, MAX_SOURCE_BYTES)),
            )
            .chain(
                self.recipes
                    .iter()
                    .map(|r| (r.byte_length, rangoon_compose::MAX_DRAFT_BYTES)),
            )
        {
            if length as usize > limit {
                return Err(StoreError::BackupInvalid);
            }
            total = total
                .checked_add(length as usize)
                .ok_or(StoreError::BackupInvalid)?;
        }
        if total > MAX_DATABASE_BYTES as usize
            || self.applications.iter().any(|a| a.targets.len() > 16)
        {
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

pub(super) fn encode(records: &Records) -> Result<Vec<u8>, StoreError> {
    let manifest = Manifest::from_records(records)?;
    let payload = manifest.preflight()?;
    let metadata = manifest.bytes()?;
    let length = MAGIC.len() + 4 + metadata.len() + payload + 64;
    if length > MAX_BACKUP_BYTES {
        return Err(StoreError::Full);
    }
    canonical_database(records)?;
    let mut bytes = Vec::with_capacity(length);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&(metadata.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&metadata);
    for source in records.sources.values() {
        bytes.extend_from_slice(source.report.source.content.as_bytes());
    }
    for revision in records.revisions.values() {
        bytes.extend_from_slice(revision.revision.content.as_bytes());
    }
    for recipe in records.recipes.values() {
        bytes.extend_from_slice(
            &rangoon_compose::canonical_draft_bytes(&recipe.draft)
                .map_err(|_| StoreError::Corrupt)?,
        );
    }
    let digest = byte_digest(&bytes);
    bytes.extend_from_slice(digest.as_bytes());
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
    let body_end = bytes.len() - 64;
    if byte_digest(&bytes[..body_end]).as_bytes() != &bytes[body_end..] {
        return Err(StoreError::BackupInvalid);
    }
    let mut cursor = MAGIC.len() + 4;
    let length = u32::from_be_bytes(
        bytes[MAGIC.len()..cursor]
            .try_into()
            .map_err(|_| StoreError::BackupInvalid)?,
    ) as usize;
    if length > MAX_MANIFEST || cursor + length > body_end {
        return Err(StoreError::BackupInvalid);
    }
    let manifest: Manifest = serde_json::from_slice(&bytes[cursor..cursor + length])
        .map_err(|_| StoreError::BackupInvalid)?;
    let total = manifest.preflight()?;
    if cursor + length + total != body_end || manifest.bytes()? != bytes[cursor..cursor + length] {
        return Err(StoreError::BackupInvalid);
    }
    cursor += length;
    // Counts and all lengths are checked before payload allocation or SQLite.
    let mut db = memory_database()?;
    let tx = db.transaction()?;
    ensure_schema(&tx, 3)?;
    for source in &manifest.sources {
        insert(&tx, source, take(bytes, &mut cursor, source.byte_length))?;
    }
    for owner in &manifest.owners {
        insert_owner(&tx, &owner.id, &owner.birth, &owner.latest_revision_id)?;
    }
    for revision in &manifest.revisions {
        insert_revision(
            &tx,
            &revision.capability_id,
            &revision.revision,
            take(bytes, &mut cursor, revision.byte_length),
        )?;
    }
    for recipe in &manifest.recipes {
        let content = take(bytes, &mut cursor, recipe.byte_length);
        if byte_digest(content) != recipe.sha256 {
            return Err(StoreError::BackupInvalid);
        }
        tx.execute(
            "INSERT INTO compositions VALUES (?1,?2,?3,?4)",
            params![
                recipe.id,
                content,
                recipe.transformation_version,
                recipe.created_at_ms
            ],
        )?;
    }
    for app in &manifest.applications {
        insert_application(
            &tx,
            &app.id,
            &app.composition_id,
            &app.targets,
            app.created_at_ms,
        )?;
    }
    let records = Records::load(&tx)?;
    if Manifest::from_records(&records)?.bytes()? != manifest.bytes()? {
        return Err(StoreError::BackupInvalid);
    }
    Ok(records)
}
fn take<'a>(bytes: &'a [u8], cursor: &mut usize, length: u32) -> &'a [u8] {
    let start = *cursor;
    *cursor += length as usize;
    &bytes[start..*cursor]
}

pub(super) fn memory_database() -> Result<Connection, StoreError> {
    let db = Connection::open_in_memory()?;
    db.pragma_update(None, "trusted_schema", false)?;
    db.set_limit(
        Limit::SQLITE_LIMIT_LENGTH,
        (rangoon_compose::MAX_DRAFT_BYTES + 4096) as i32,
    )?;
    db.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 16384)?;
    db.pragma_update(None, "page_size", 4096)?;
    db.pragma_update(None, "max_page_count", (MAX_DATABASE_BYTES / 4096) as u32)?;
    Ok(db)
}
pub(super) fn ensure_schema(db: &Connection, version: i64) -> Result<(), StoreError> {
    initialize_or_verify(db)?;
    let current: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current < 2 && version >= 2 {
        capabilities::migrate(db)?;
    }
    if current < 3 && version == 3 {
        for (_, sql) in composition_records::SCHEMAS {
            db.execute_batch(sql)?;
        }
        db.pragma_update(None, "user_version", 3)?;
    }
    if version == 3 {
        db.set_limit(
            Limit::SQLITE_LIMIT_LENGTH,
            (rangoon_compose::MAX_DRAFT_BYTES + 4096) as i32,
        )?;
    }
    verify_schema(db)
}
pub(super) fn canonical_database(records: &Records) -> Result<Connection, StoreError> {
    let mut db = memory_database()?;
    let tx = db.transaction()?;
    ensure_schema(&tx, records.version)?;
    for source in records.sources.values() {
        insert(
            &tx,
            &source.metadata,
            source.report.source.content.as_bytes(),
        )?;
    }
    for (id, owner) in &records.owners {
        insert_owner(&tx, id, &owner.birth, &owner.latest_revision_id)?;
    }
    for revision in records.revisions.values() {
        insert_revision(
            &tx,
            &revision.capability_id,
            &RevisionSummary::from(&revision.revision),
            revision.revision.content.as_bytes(),
        )?;
    }
    for (id, recipe) in &records.recipes {
        insert_recipe(&tx, id, recipe)?;
    }
    for (id, app) in &records.applications {
        insert_application(
            &tx,
            id,
            &app.composition_id,
            &app.targets,
            app.created_at_ms,
        )?;
    }
    Records::load(&tx)?;
    tx.commit()?;
    Ok(db)
}
pub(super) fn insert_owner(
    db: &Connection,
    id: &str,
    birth: &Birth,
    latest: &str,
) -> Result<(), StoreError> {
    match birth {
        Birth::Source {
            source_id,
            fragment_id,
        } => {
            db.execute(
                "INSERT INTO capabilities VALUES (?1,?2,?3,?4)",
                params![id, source_id, fragment_id, latest],
            )?;
        }
        Birth::Composition {
            composition_id,
            output_index,
        } => {
            db.execute(
                "INSERT INTO derived_capabilities VALUES (?1,?2,?3,?4)",
                params![id, composition_id, output_index, latest],
            )?;
        }
    }
    Ok(())
}
pub(super) fn insert_revision(
    db: &Connection,
    capability_id: &str,
    revision: &RevisionSummary,
    content: &[u8],
) -> Result<(), StoreError> {
    db.execute(
        "INSERT INTO revisions VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            revision.id,
            capability_id,
            revision.parent_revision_id,
            revision.title,
            content,
            revision.sha256,
            revision.created_at_ms
        ],
    )?;
    if let Some(review) = &revision.review {
        db.execute(
            "INSERT INTO reviews VALUES (?1,'local_operator',?2)",
            params![revision.id, review.reviewed_at_ms],
        )?;
    }
    if let RevisionProvenance::Composition {
        application_id,
        output_index,
        ..
    } = &revision.provenance
    {
        db.execute(
            "INSERT INTO revision_derivations VALUES (?1,?2,?3)",
            params![revision.id, application_id, output_index],
        )?;
    }
    Ok(())
}
pub(super) fn insert_recipe(
    db: &Connection,
    id: &str,
    recipe: &composition_records::Recipe,
) -> Result<(), StoreError> {
    let bytes =
        rangoon_compose::canonical_draft_bytes(&recipe.draft).map_err(|_| StoreError::Corrupt)?;
    db.execute(
        "INSERT INTO compositions VALUES (?1,?2,?3,?4)",
        params![
            id,
            bytes,
            rangoon_compose::TRANSFORMATION_VERSION,
            recipe.created_at_ms
        ],
    )?;
    Ok(())
}
pub(super) fn insert_application(
    db: &Connection,
    id: &str,
    composition_id: &str,
    targets: &[Target],
    time: i64,
) -> Result<(), StoreError> {
    let bytes = serde_json::to_vec(targets).map_err(|_| StoreError::Corrupt)?;
    db.execute(
        "INSERT INTO composition_applications VALUES (?1,?2,?3,'local_operator',?4)",
        params![id, composition_id, bytes, time],
    )?;
    Ok(())
}

impl Workspace {
    pub fn export_composition_backup(&self) -> Result<Vec<u8>, StoreError> {
        let Some(mut db) = self.connect_with_empty(false, true)? else {
            return encode(&Records::empty());
        };
        let tx = db.transaction()?;
        let bytes = encode(&read_records(&tx)?)?;
        tx.commit()?;
        Ok(bytes)
    }
}
pub(super) fn read_records(db: &Connection) -> Result<Records, StoreError> {
    let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 0 {
        verify_empty(db)?;
        Ok(Records::empty())
    } else {
        Records::load(db)
    }
}
