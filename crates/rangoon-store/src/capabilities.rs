use super::*;
use rangoon_domain::{Authority, byte_digest, capability::*};

pub const MAX_CAPABILITIES: usize = 128;
pub const MAX_REVISIONS: usize = 32;
pub const MAX_TOTAL_REVISIONS: usize = 1024;
pub(super) const SCHEMAS: &[(&str, &str)] = &[
    (
        "capabilities",
        "CREATE TABLE capabilities (capability_id TEXT PRIMARY KEY NOT NULL, source_id TEXT NOT NULL, fragment_id TEXT NOT NULL, latest_revision_id TEXT NOT NULL)",
    ),
    (
        "revisions",
        "CREATE TABLE revisions (revision_id TEXT PRIMARY KEY NOT NULL, capability_id TEXT NOT NULL, parent_revision_id TEXT, title TEXT NOT NULL, content BLOB NOT NULL, sha256 TEXT NOT NULL, created_at_ms INTEGER NOT NULL)",
    ),
    (
        "reviews",
        "CREATE TABLE reviews (revision_id TEXT PRIMARY KEY NOT NULL, reviewer TEXT NOT NULL, reviewed_at_ms INTEGER NOT NULL)",
    ),
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityReceipt {
    pub capability: CapabilityDetail,
    pub already_applied: bool,
}

impl Workspace {
    pub fn list_capabilities(&self) -> Result<Vec<CapabilitySummary>, StoreError> {
        let Some(mut db) = self.connect(false)? else {
            return Ok(Vec::new());
        };
        let tx = db.transaction()?;
        verify_schema(&tx)?;
        if schema_version(&tx)? == 3 {
            return Err(StoreError::UnsupportedSchema);
        }
        if schema_version(&tx)? == 1 {
            return Ok(Vec::new());
        }
        validate_links_and_limits(&tx)?;
        let mut query =
            tx.prepare("SELECT capability_id FROM capabilities ORDER BY capability_id LIMIT 129")?;
        let ids = query
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let result = ids
            .into_iter()
            .map(|id| {
                let detail = read_capability(&tx, &id, None)?;
                Ok(CapabilitySummary {
                    id: detail.id,
                    source_id: detail.source_id,
                    fragment_id: detail.fragment_id,
                    latest_revision_id: detail.latest_revision_id,
                    title: detail.revision.title,
                    reviewed: detail.revision.review.is_some(),
                    revision_count: detail.history.len() as u32,
                })
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        drop(query);
        tx.commit()?;
        Ok(result)
    }

    pub fn open_capability(
        &self,
        id: &str,
        revision: Option<&str>,
    ) -> Result<CapabilityDetail, StoreError> {
        check_ids(id, revision)?;
        let mut db = self.connect(false)?.ok_or(StoreError::CapabilityNotFound)?;
        let tx = db.transaction()?;
        require_v2(&tx)?;
        let detail = read_capability(&tx, id, revision)?;
        tx.commit()?;
        Ok(detail)
    }

    pub fn create_capability(
        &self,
        source_id: &str,
        fragment_id: &str,
        title: &str,
    ) -> Result<CapabilityReceipt, StoreError> {
        if !valid_id(source_id, "source:")
            || !valid_id(fragment_id, "fragment:")
            || !valid_title(title)
        {
            return Err(StoreError::CapabilityInvalid);
        }
        // Creation cannot implicitly save a source or create a new source workspace.
        let mut db = self.connect(false)?.ok_or(StoreError::NotFound)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        verify_schema(&tx)?;
        let report = read_report(&tx, source_id)?;
        if schema_version(&tx)? == 3 {
            return Err(StoreError::UnsupportedSchema);
        }
        let fragment = report
            .fragments
            .iter()
            .find(|f| f.id == fragment_id)
            .ok_or(StoreError::CapabilityInvalid)?;
        if !valid_content(&fragment.text) {
            return Err(StoreError::CapabilityInvalid);
        }
        migrate(&tx)?;
        validate_links_and_limits(&tx)?;
        let id = capability_id(source_id, fragment_id, title);
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM capabilities WHERE capability_id=?1)",
            [&id],
            |r| r.get(0),
        )?;
        if exists {
            let capability = read_capability(&tx, &id, None)?;
            tx.commit()?;
            return Ok(CapabilityReceipt {
                capability,
                already_applied: true,
            });
        }
        if count(&tx, "SELECT count(*) FROM capabilities")? >= MAX_CAPABILITIES {
            return Err(StoreError::CapabilityFull);
        }
        ensure_revision_capacity(&tx)?;
        let revision = new_revision(&id, None, title, &fragment.text)?;
        tx.execute(
            "INSERT INTO capabilities VALUES (?1,?2,?3,?4)",
            params![id, source_id, fragment_id, revision.id],
        )?;
        insert_revision(&tx, &id, &revision)?;
        let capability = read_capability(&tx, &id, None)?;
        tx.commit()?;
        Ok(CapabilityReceipt {
            capability,
            already_applied: false,
        })
    }

    pub fn revise_capability(
        &self,
        id: &str,
        expected: &str,
        title: &str,
        content: &str,
    ) -> Result<CapabilityReceipt, StoreError> {
        check_ids(id, Some(expected))?;
        if !valid_title(title) || !valid_content(content) {
            return Err(StoreError::CapabilityInvalid);
        }
        let mut db = self.connect(false)?.ok_or(StoreError::CapabilityNotFound)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_v2(&tx)?;
        let current = read_capability(&tx, id, None)?;
        let candidate = revision_id(id, Some(expected), title, content);
        let retry = candidate == current.latest_revision_id;
        if !retry && expected != current.latest_revision_id {
            return Err(StoreError::CapabilityConflict);
        }
        if retry || (title == current.revision.title && content == current.revision.content) {
            tx.commit()?;
            return Ok(CapabilityReceipt {
                capability: current,
                already_applied: true,
            });
        }
        if current.history.len() >= MAX_REVISIONS {
            return Err(StoreError::CapabilityFull);
        }
        ensure_revision_capacity(&tx)?;
        let revision = new_revision(id, Some(expected), title, content)?;
        insert_revision(&tx, id, &revision)?;
        let changed = tx.execute("UPDATE capabilities SET latest_revision_id=?1 WHERE capability_id=?2 AND latest_revision_id=?3", params![revision.id, id, expected])?;
        if changed != 1 {
            return Err(StoreError::CapabilityConflict);
        }
        let capability = read_capability(&tx, id, None)?;
        tx.commit()?;
        Ok(CapabilityReceipt {
            capability,
            already_applied: false,
        })
    }

    pub fn review_capability(
        &self,
        id: &str,
        expected: &str,
    ) -> Result<CapabilityReceipt, StoreError> {
        check_ids(id, Some(expected))?;
        let mut db = self.connect(false)?.ok_or(StoreError::CapabilityNotFound)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_v2(&tx)?;
        let current = read_capability(&tx, id, None)?;
        if current.latest_revision_id != expected {
            return Err(StoreError::CapabilityConflict);
        }
        let already_applied = current.revision.review.is_some();
        if !already_applied {
            tx.execute(
                "INSERT INTO reviews VALUES (?1,'local_operator',?2)",
                params![expected, now_ms()?],
            )?;
        }
        let capability = read_capability(&tx, id, None)?;
        tx.commit()?;
        Ok(CapabilityReceipt {
            capability,
            already_applied,
        })
    }
}

fn schema_version(db: &Connection) -> Result<i64, StoreError> {
    Ok(db.pragma_query_value(None, "user_version", |r| r.get(0))?)
}
pub(super) fn migrate(db: &Connection) -> Result<(), StoreError> {
    verify_schema(db)?;
    if schema_version(db)? == 1 {
        for (_, sql) in SCHEMAS {
            db.execute_batch(sql)?;
        }
        db.pragma_update(None, "user_version", 2)?;
    }
    verify_schema(db)
}
fn require_v2(db: &Connection) -> Result<(), StoreError> {
    verify_schema(db)?;
    match schema_version(db)? {
        1 => return Err(StoreError::CapabilityNotFound),
        2 => (),
        _ => return Err(StoreError::UnsupportedSchema),
    }
    validate_links_and_limits(db)
}
fn count(db: &Connection, sql: &str) -> Result<usize, StoreError> {
    let value: i64 = db.query_row(sql, [], |r| r.get(0))?;
    usize::try_from(value).map_err(|_| StoreError::Corrupt)
}
fn ensure_revision_capacity(db: &Connection) -> Result<(), StoreError> {
    if count(db, "SELECT count(*) FROM revisions")? >= MAX_TOTAL_REVISIONS {
        Err(StoreError::CapabilityFull)
    } else {
        Ok(())
    }
}
pub(super) fn validate_links_and_limits(db: &Connection) -> Result<(), StoreError> {
    if count(db, "SELECT count(*) FROM capabilities")? > MAX_CAPABILITIES
        || count(db, "SELECT count(*) FROM revisions")? > MAX_TOTAL_REVISIONS
        || count(db, "SELECT count(*) FROM reviews")? > MAX_TOTAL_REVISIONS
    {
        return Err(StoreError::CapabilityFull);
    }
    let orphan: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM capabilities c LEFT JOIN snapshots s ON c.source_id=s.source_id WHERE s.source_id IS NULL) OR EXISTS(SELECT 1 FROM revisions r LEFT JOIN capabilities c ON r.capability_id=c.capability_id WHERE c.capability_id IS NULL) OR EXISTS(SELECT 1 FROM reviews v LEFT JOIN revisions r ON v.revision_id=r.revision_id WHERE r.revision_id IS NULL)", [], |r| r.get(0))?;
    if orphan {
        return Err(StoreError::Corrupt);
    }
    Ok(())
}
fn check_ids(id: &str, revision: Option<&str>) -> Result<(), StoreError> {
    if !valid_id(id, "capability:") || revision.is_some_and(|id| !valid_id(id, "revision:")) {
        Err(StoreError::CapabilityInvalid)
    } else {
        Ok(())
    }
}
fn now_ms() -> Result<i64, StoreError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StoreError::Unavailable)?
        .as_millis();
    i64::try_from(now).map_err(|_| StoreError::Unavailable)
}
fn valid_time(time: i64) -> bool {
    (0..=8_640_000_000_000_000).contains(&time)
}
fn new_revision(
    id: &str,
    parent: Option<&str>,
    title: &str,
    content: &str,
) -> Result<Revision, StoreError> {
    Ok(Revision {
        id: revision_id(id, parent, title, content),
        parent_revision_id: parent.map(str::to_owned),
        title: title.into(),
        content: content.into(),
        sha256: byte_digest(content.as_bytes()),
        created_at_ms: now_ms()?,
        review: None,
    })
}
fn insert_revision(db: &Connection, id: &str, r: &Revision) -> Result<(), StoreError> {
    db.execute(
        "INSERT INTO revisions VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            r.id,
            id,
            r.parent_revision_id,
            r.title,
            r.content.as_bytes(),
            r.sha256,
            r.created_at_ms
        ],
    )?;
    Ok(())
}
pub(super) fn read_capability(
    db: &Connection,
    id: &str,
    selected: Option<&str>,
) -> Result<CapabilityDetail, StoreError> {
    check_ids(id, selected)?;
    let (source_id, fragment_id, latest): (String, String, String) = db.query_row("SELECT source_id,fragment_id,latest_revision_id FROM capabilities WHERE capability_id=?1", [id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?.ok_or(StoreError::CapabilityNotFound)?;
    let report = read_report(db, &source_id)?;
    let fragment = report
        .fragments
        .iter()
        .find(|f| f.id == fragment_id)
        .ok_or(StoreError::Corrupt)?;
    let mut query = db.prepare("SELECT r.revision_id,r.parent_revision_id,r.title,r.content,r.sha256,r.created_at_ms,v.reviewer,v.reviewed_at_ms FROM revisions r LEFT JOIN reviews v ON r.revision_id=v.revision_id WHERE r.capability_id=?1 LIMIT 33")?;
    let rows = query
        .query_map([id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<i64>>(7)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if rows.is_empty() || rows.len() > MAX_REVISIONS {
        return Err(StoreError::Corrupt);
    }
    let mut revisions = std::collections::BTreeMap::new();
    for (revision, parent, title, bytes, sha256, created_at_ms, reviewer, reviewed_at_ms) in rows {
        let content = String::from_utf8(bytes).map_err(|_| StoreError::Corrupt)?;
        if !valid_title(&title)
            || !valid_content(&content)
            || !valid_time(created_at_ms)
            || parent.as_ref().is_some_and(|p| !valid_id(p, "revision:"))
            || revision != revision_id(id, parent.as_deref(), &title, &content)
            || sha256 != byte_digest(content.as_bytes())
        {
            return Err(StoreError::Corrupt);
        }
        let review = match (reviewer.as_deref(), reviewed_at_ms) {
            (None, None) => None,
            (Some("local_operator"), Some(time)) if valid_time(time) => Some(ContentReview {
                reviewer: Reviewer::LocalOperator,
                reviewed_at_ms: time,
            }),
            _ => return Err(StoreError::Corrupt),
        };
        revisions.insert(
            revision.clone(),
            Revision {
                id: revision,
                parent_revision_id: parent,
                title,
                content,
                sha256,
                created_at_ms,
                review,
            },
        );
    }
    // Walk the entire unique chain. Disconnected rows, cycles and forged roots fail.
    let mut chain = Vec::new();
    let mut next = Some(latest.as_str());
    while let Some(current) = next {
        let value = revisions.get(current).ok_or(StoreError::Corrupt)?;
        if chain.len() >= revisions.len() {
            return Err(StoreError::Corrupt);
        }
        chain.push(value);
        next = value.parent_revision_id.as_deref();
    }
    if chain.len() != revisions.len() {
        return Err(StoreError::Corrupt);
    }
    chain.reverse();
    let first = chain[0];
    if first.content != fragment.text || capability_id(&source_id, &fragment_id, &first.title) != id
    {
        return Err(StoreError::Corrupt);
    }
    let revision = revisions
        .get(selected.unwrap_or(&latest))
        .ok_or(StoreError::CapabilityNotFound)?
        .clone();
    Ok(CapabilityDetail {
        schema_version: SCHEMA_VERSION.into(),
        id: id.into(),
        source_id,
        fragment_id,
        source_name: report.source.display_name,
        span: fragment.span,
        original_text: fragment.text.clone(),
        latest_revision_id: latest,
        revision,
        history: chain.into_iter().map(RevisionSummary::from).collect(),
        authority: Authority::None,
    })
}
