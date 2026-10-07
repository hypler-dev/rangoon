//! Complete records, monotonic schema migration and allocation readback.
use super::*;

use composition_records::Records;
use rangoon_domain::capability_v1::RevisionSummary;

pub(super) fn read_records(db: &Connection) -> Result<Records, StoreError> {
    let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 0 {
        verify_empty(db)?;
        Ok(Records::empty())
    } else {
        Records::load_complete(db)
    }
}

pub(super) fn ensure_schema(db: &Connection, target: i64) -> Result<(), StoreError> {
    let current: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if !(1..=4).contains(&target) || !(0..=4).contains(&current) || target < current {
        return Err(StoreError::UnsupportedSchema);
    }
    if current == 4 {
        return verify_schema_mode(db, true);
    }
    composition_backup::ensure_schema(db, target.min(3))?;
    if target == 4 {
        for (_, sql) in workflow_records::SCHEMAS {
            db.execute_batch(sql)?;
        }
        db.pragma_update(None, "user_version", 4)?;
    }
    verify_schema_mode(db, true)
}

pub(super) fn insert_additions(
    db: &Connection,
    current: &Records,
    result: &Records,
) -> Result<(), StoreError> {
    for (id, source) in &result.sources {
        if !current.sources.contains_key(id) {
            insert(
                db,
                &source.metadata,
                source.report.source.content.as_bytes(),
            )?;
        }
    }
    for (id, owner) in &result.owners {
        if !current.owners.contains_key(id) {
            composition_backup::insert_owner(db, id, &owner.birth, &owner.latest_revision_id)?;
        }
    }
    for (id, row) in &result.revisions {
        if !current.revisions.contains_key(id) {
            composition_backup::insert_revision(
                db,
                &row.capability_id,
                &RevisionSummary::from(&row.revision),
                row.revision.content.as_bytes(),
            )?;
        }
    }
    for (id, recipe) in &result.recipes {
        if !current.recipes.contains_key(id) {
            composition_backup::insert_recipe(db, id, recipe)?;
        }
    }
    for (id, app) in &result.applications {
        if !current.applications.contains_key(id) {
            composition_backup::insert_application(
                db,
                id,
                &app.composition_id,
                &app.targets,
                app.created_at_ms,
            )?;
        }
    }
    #[cfg(test)]
    composition_recovery_tests::fault_checkpoint(db, "after_workflow_restore_base")?;
    for (id, head) in &result.workflows.owners {
        if !current.workflows.owners.contains_key(id) {
            db.execute("INSERT INTO workflows VALUES (?1,?2)", params![id, head])?;
        }
    }
    for (id, row) in &result.workflows.revisions {
        if !current.workflows.revisions.contains_key(id) {
            db.execute(
                "INSERT INTO workflow_revisions VALUES (?1,?2,?3,?4)",
                params![
                    id,
                    row.record.workflow_id(),
                    row.record
                        .serialized_bytes()
                        .map_err(|_| StoreError::Corrupt)?,
                    row.saved_at_ms
                ],
            )?;
        }
    }
    Ok(())
}

pub(super) fn data(
    db: &Connection,
    records: &Records,
) -> Result<WorkflowWorkspaceData, StoreError> {
    let size: u32 = db.pragma_query_value(None, "page_size", |r| r.get(0))?;
    let pages: u32 = db.pragma_query_value(None, "page_count", |r| r.get(0))?;
    let free: u32 = db.pragma_query_value(None, "freelist_count", |r| r.get(0))?;
    let bytes = u64::from(size) * u64::from(pages);
    if bytes > MAX_DATABASE_BYTES {
        return Err(StoreError::Full);
    }
    workflow_views::data(records, bytes, u64::from(size) * u64::from(free))
}
