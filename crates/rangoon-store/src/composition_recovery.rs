//! Explicit composition-aware recovery. Original files are never modified.
use super::*;
use composition_backup::{self as archive, CompositionBackup};
use composition_records::{Birth, Records};
use rangoon_compose::InputReference;
use rangoon_domain::capability::valid_id;
use rangoon_domain::capability_v1::{CapabilitySummary, RevisionProvenance, RevisionSummary};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryCounts {
    pub sources: u32,
    pub capabilities: u32,
    pub revisions: u32,
    pub reviews: u32,
    pub recipes: u32,
    pub applications: u32,
    pub derivations: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionRestorePlan {
    pub schema_version: &'static str,
    pub backup_id: String,
    pub expected_state_id: String,
    pub byte_length: usize,
    pub add: RecoveryCounts,
    pub kept_sources: u32,
    pub kept_capabilities: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionDeletionPlan {
    pub schema_version: &'static str,
    pub kind: RecordKind,
    pub id: String,
    pub title: String,
    pub expected_state_id: String,
    pub remove: RecoveryCounts,
    pub dependencies: Vec<CapabilitySummary>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionWorkspaceData {
    pub schema_version: &'static str,
    pub state_id: String,
    pub records: RecoveryCounts,
    pub database_bytes: u64,
    pub reusable_bytes: u64,
    pub sources: Vec<SnapshotMetadata>,
    pub capabilities: Vec<CapabilitySummary>,
}

fn counts(records: &Records) -> RecoveryCounts {
    RecoveryCounts {
        sources: records.sources.len() as u32,
        capabilities: records.owners.len() as u32,
        revisions: records.revisions.len() as u32,
        reviews: records
            .revisions
            .values()
            .filter(|r| r.revision.review.is_some())
            .count() as u32,
        recipes: records.recipes.len() as u32,
        applications: records.applications.len() as u32,
        derivations: records
            .revisions
            .values()
            .filter(|r| {
                matches!(
                    r.revision.provenance,
                    RevisionProvenance::Composition { .. }
                )
            })
            .count() as u32,
    }
}
fn difference(
    larger: RecoveryCounts,
    smaller: RecoveryCounts,
) -> Result<RecoveryCounts, StoreError> {
    let subtract = |a: u32, b: u32| a.checked_sub(b).ok_or(StoreError::Corrupt);
    Ok(RecoveryCounts {
        sources: subtract(larger.sources, smaller.sources)?,
        capabilities: subtract(larger.capabilities, smaller.capabilities)?,
        revisions: subtract(larger.revisions, smaller.revisions)?,
        reviews: subtract(larger.reviews, smaller.reviews)?,
        recipes: subtract(larger.recipes, smaller.recipes)?,
        applications: subtract(larger.applications, smaller.applications)?,
        derivations: subtract(larger.derivations, smaller.derivations)?,
    })
}
fn data(
    records: &Records,
    database_bytes: u64,
    reusable_bytes: u64,
) -> Result<CompositionWorkspaceData, StoreError> {
    Ok(CompositionWorkspaceData {
        schema_version: "rangoon.workspace-data.v1",
        state_id: records.state_id()?,
        records: counts(records),
        database_bytes,
        reusable_bytes,
        sources: records
            .sources
            .values()
            .map(|s| s.metadata.clone())
            .collect(),
        capabilities: records
            .owners
            .keys()
            .map(|id| records.summary(id))
            .collect::<Result<_, _>>()?,
    })
}
fn trim(records: &mut Records) {
    let applications: BTreeSet<_> = records
        .revisions
        .values()
        .filter_map(|r| match &r.revision.provenance {
            RevisionProvenance::Composition { application_id, .. } => Some(application_id.clone()),
            _ => None,
        })
        .collect();
    records
        .applications
        .retain(|id, _| applications.contains(id));
    let recipes: BTreeSet<_> = records
        .applications
        .values()
        .map(|a| a.composition_id.clone())
        .chain(records.owners.values().filter_map(|o| match &o.birth {
            Birth::Composition { composition_id, .. } => Some(composition_id.clone()),
            _ => None,
        }))
        .collect();
    records.recipes.retain(|id, _| recipes.contains(id));
}
fn union(current: &Records, backup: &Records) -> Result<Records, StoreError> {
    let mut result = current.clone();
    for (id, source) in &backup.sources {
        if let Some(local) = result.sources.get(id) {
            if local.report.source != source.report.source {
                return Err(StoreError::BackupInvalid);
            }
        } else {
            result.sources.insert(id.clone(), source.clone());
        }
    }
    let added: BTreeSet<_> = backup
        .owners
        .keys()
        .filter(|id| !current.owners.contains_key(*id))
        .cloned()
        .collect();
    for (id, owner) in &backup.owners {
        if let Some(local) = result.owners.get(id) {
            if local.birth != owner.birth {
                return Err(StoreError::BackupInvalid);
            }
        } else {
            result.owners.insert(id.clone(), owner.clone());
        }
    }
    for (id, revision) in &backup.revisions {
        if added.contains(&revision.capability_id)
            && result
                .revisions
                .insert(id.clone(), revision.clone())
                .is_some()
        {
            return Err(StoreError::BackupInvalid);
        }
    }
    let needed_apps: BTreeSet<_> = result
        .revisions
        .values()
        .filter_map(|r| match &r.revision.provenance {
            RevisionProvenance::Composition { application_id, .. } => Some(application_id.clone()),
            _ => None,
        })
        .collect();
    for id in needed_apps {
        match (result.applications.get(&id), backup.applications.get(&id)) {
            (Some(local), Some(incoming))
                if local.composition_id != incoming.composition_id
                    || local.targets != incoming.targets =>
            {
                return Err(StoreError::BackupInvalid);
            }
            (None, Some(incoming)) => {
                result.applications.insert(id, incoming.clone());
            }
            (None, None) => return Err(StoreError::CompositionDependencyMissing),
            _ => (),
        }
    }
    let needed_recipes: BTreeSet<_> = result
        .applications
        .values()
        .map(|a| a.composition_id.clone())
        .chain(result.owners.values().filter_map(|o| match &o.birth {
            Birth::Composition { composition_id, .. } => Some(composition_id.clone()),
            _ => None,
        }))
        .collect();
    for id in needed_recipes {
        match (result.recipes.get(&id), backup.recipes.get(&id)) {
            (Some(local), Some(incoming)) if local.draft != incoming.draft => {
                return Err(StoreError::BackupInvalid);
            }
            (None, Some(incoming)) => {
                result.recipes.insert(id, incoming.clone());
            }
            (None, None) => return Err(StoreError::CompositionDependencyMissing),
            _ => (),
        }
    }
    trim(&mut result);
    for recipe in result.recipes.values() {
        for input in &recipe.draft.inputs {
            match input {
                InputReference::Source { source_id, .. }
                    if !result.sources.contains_key(source_id) =>
                {
                    return Err(StoreError::CompositionDependencyMissing);
                }
                InputReference::Revision { revision_id, .. }
                    if !result.revisions.contains_key(revision_id) =>
                {
                    return Err(StoreError::CompositionDependencyMissing);
                }
                _ => (),
            }
        }
    }
    if result.owners.len() > MAX_CAPABILITIES || result.revisions.len() > MAX_TOTAL_REVISIONS {
        return Err(StoreError::CapabilityFull);
    }
    if result.sources.len() > MAX_SNAPSHOTS {
        return Err(StoreError::Full);
    }
    result.version =
        if current.version == 3 || !result.recipes.is_empty() || !result.applications.is_empty() {
            3
        } else if current.version >= 2 || !result.owners.is_empty() {
            2
        } else {
            1
        };
    Records::load(&archive::canonical_database(&result)?)
}
fn restore_plan(
    current: &Records,
    backup: &CompositionBackup,
) -> Result<(CompositionRestorePlan, Records), StoreError> {
    let result = union(current, &backup.records)?;
    let add = difference(counts(&result), counts(current))?;
    let plan = CompositionRestorePlan {
        schema_version: "rangoon.restore-plan.v1",
        backup_id: backup.id.clone(),
        expected_state_id: current.state_id()?,
        byte_length: backup.byte_length,
        kept_sources: backup.records.sources.len() as u32 - add.sources,
        kept_capabilities: backup.records.owners.len() as u32 - add.capabilities,
        add,
    };
    Ok((plan, result))
}
fn valid_record_id(kind: RecordKind, id: &str) -> Result<(), StoreError> {
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
fn deletion_plan(
    current: &Records,
    kind: RecordKind,
    id: &str,
) -> Result<(CompositionDeletionPlan, Records), StoreError> {
    valid_record_id(kind, id)?;
    let title = match kind {
        RecordKind::Source => current
            .sources
            .get(id)
            .ok_or(StoreError::NotFound)?
            .metadata
            .display_name
            .clone(),
        RecordKind::Capability => current.summary(id)?.title,
    };
    let removed_revisions: BTreeSet<_> = if kind == RecordKind::Capability {
        current
            .revisions
            .iter()
            .filter(|(_, r)| r.capability_id == id)
            .map(|(id, _)| id.as_str())
            .collect()
    } else {
        BTreeSet::new()
    };
    let mut dependent = BTreeSet::new();
    for (cap, owner) in &current.owners {
        if kind == RecordKind::Source
            && matches!(&owner.birth,Birth::Source {source_id,..} if source_id==id)
        {
            dependent.insert(cap.clone());
        }
    }
    for row in current.revisions.values() {
        if kind == RecordKind::Capability && row.capability_id == id {
            continue;
        }
        if let RevisionProvenance::Composition { composition_id, .. } = &row.revision.provenance {
            let recipe = &current.recipes[composition_id];
            if recipe.draft.inputs.iter().any(|input| match input {
                InputReference::Source { source_id, .. } => {
                    kind == RecordKind::Source && source_id == id
                }
                InputReference::Revision { revision_id, .. } => {
                    kind == RecordKind::Capability
                        && removed_revisions.contains(revision_id.as_str())
                }
            }) {
                dependent.insert(row.capability_id.clone());
            }
        }
    }
    let mut result = current.clone();
    match kind {
        RecordKind::Source => {
            result.sources.remove(id);
        }
        RecordKind::Capability => {
            result.owners.remove(id);
            result.revisions.retain(|_, r| r.capability_id != id);
        }
    }
    trim(&mut result);
    let plan = CompositionDeletionPlan {
        schema_version: "rangoon.deletion-plan.v1",
        kind,
        id: id.into(),
        title,
        expected_state_id: current.state_id()?,
        remove: difference(counts(current), counts(&result))?,
        dependencies: dependent
            .iter()
            .map(|id| current.summary(id))
            .collect::<Result<_, _>>()?,
    };
    if plan.dependencies.is_empty() {
        result = Records::load(&archive::canonical_database(&result)?)?;
    }
    Ok((plan, result))
}

impl Workspace {
    pub fn composition_data(&self) -> Result<CompositionWorkspaceData, StoreError> {
        let Some(mut db) = self.connect_with_empty(false, true)? else {
            return data(&Records::empty(), 0, 0);
        };
        let tx = db.transaction()?;
        let records = archive::read_records(&tx)?;
        let size: u32 = tx.pragma_query_value(None, "page_size", |r| r.get(0))?;
        let pages: u32 = tx.pragma_query_value(None, "page_count", |r| r.get(0))?;
        let free: u32 = tx.pragma_query_value(None, "freelist_count", |r| r.get(0))?;
        let result = data(
            &records,
            u64::from(size) * u64::from(pages),
            u64::from(size) * u64::from(free),
        )?;
        tx.commit()?;
        Ok(result)
    }
    pub fn prepare_composition_restore(
        &self,
        backup: &CompositionBackup,
    ) -> Result<CompositionRestorePlan, StoreError> {
        let Some(mut db) = self.connect_with_empty(false, true)? else {
            return Ok(restore_plan(&Records::empty(), backup)?.0);
        };
        let tx = db.transaction()?;
        let plan = restore_plan(&archive::read_records(&tx)?, backup)?.0;
        tx.commit()?;
        Ok(plan)
    }
    /// The native host must retain both selected backup and issued confirmation.
    pub fn restore_composition_backup(
        &self,
        backup: &CompositionBackup,
        confirmation: &CompositionRestorePlan,
    ) -> Result<CompositionRestorePlan, StoreError> {
        if confirmation.backup_id != backup.id || confirmation.byte_length != backup.byte_length {
            return Err(StoreError::WorkspaceChanged);
        }
        let mut db = self.connect(true)?.ok_or(StoreError::Unavailable)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = archive::read_records(&tx)?;
        if current.state_id()? != confirmation.expected_state_id {
            return Err(StoreError::WorkspaceChanged);
        }
        let (plan, result) = restore_plan(&current, backup)?;
        if &plan != confirmation {
            return Err(StoreError::WorkspaceChanged);
        }
        if plan.add != RecoveryCounts::default() {
            #[cfg(test)]
            composition_recovery_tests::fault_checkpoint(&tx, "before_migration")?;
            archive::ensure_schema(&tx, result.version)?;
            #[cfg(test)]
            composition_recovery_tests::fault_checkpoint(&tx, "after_migration")?;
            for (id, source) in &result.sources {
                if !current.sources.contains_key(id) {
                    insert(
                        &tx,
                        &source.metadata,
                        source.report.source.content.as_bytes(),
                    )?;
                }
            }
            for (id, owner) in &result.owners {
                if !current.owners.contains_key(id) {
                    archive::insert_owner(&tx, id, &owner.birth, &owner.latest_revision_id)?;
                }
            }
            for (id, row) in &result.revisions {
                if !current.revisions.contains_key(id) {
                    archive::insert_revision(
                        &tx,
                        &row.capability_id,
                        &RevisionSummary::from(&row.revision),
                        row.revision.content.as_bytes(),
                    )?;
                }
            }
            for (id, recipe) in &result.recipes {
                if !current.recipes.contains_key(id) {
                    archive::insert_recipe(&tx, id, recipe)?;
                }
            }
            for (id, app) in &result.applications {
                if !current.applications.contains_key(id) {
                    archive::insert_application(
                        &tx,
                        id,
                        &app.composition_id,
                        &app.targets,
                        app.created_at_ms,
                    )?;
                }
            }
            #[cfg(test)]
            composition_recovery_tests::fault_checkpoint(&tx, "after_restore_rows")?;
            if Records::load(&tx)?.state_id()? != result.state_id()? {
                return Err(StoreError::Corrupt);
            }
        }
        tx.commit()?;
        Ok(plan)
    }
    pub fn inspect_composition_deletion(
        &self,
        kind: RecordKind,
        id: &str,
    ) -> Result<CompositionDeletionPlan, StoreError> {
        valid_record_id(kind, id)?;
        let mut db = self.connect(false)?.ok_or(StoreError::NotFound)?;
        let tx = db.transaction()?;
        let result = deletion_plan(&Records::load(&tx)?, kind, id)?.0;
        tx.commit()?;
        Ok(result)
    }
    pub fn delete_composition_record(
        &self,
        confirmation: &CompositionDeletionPlan,
    ) -> Result<(), StoreError> {
        valid_record_id(confirmation.kind, &confirmation.id)?;
        let Some(mut db) = self.connect(false)? else {
            return Err(StoreError::WorkspaceChanged);
        };
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = Records::load(&tx)?;
        if current.state_id()? != confirmation.expected_state_id {
            return Err(StoreError::WorkspaceChanged);
        }
        let (plan, result) = deletion_plan(&current, confirmation.kind, &confirmation.id)?;
        if &plan != confirmation {
            return Err(StoreError::WorkspaceChanged);
        }
        if !plan.dependencies.is_empty() {
            return Err(StoreError::RecordInUse);
        }
        #[cfg(test)]
        composition_recovery_tests::fault_checkpoint(&tx, "before_delete")?;
        match plan.kind {
            RecordKind::Source => {
                tx.execute("DELETE FROM snapshots WHERE source_id=?1", [&plan.id])?;
            }
            RecordKind::Capability => {
                tx.execute("DELETE FROM reviews WHERE revision_id IN (SELECT revision_id FROM revisions WHERE capability_id=?1)",[&plan.id])?;
                if current.version == 3 {
                    tx.execute("DELETE FROM revision_derivations WHERE revision_id IN (SELECT revision_id FROM revisions WHERE capability_id=?1)",[&plan.id])?;
                }
                tx.execute("DELETE FROM revisions WHERE capability_id=?1", [&plan.id])?;
                tx.execute(
                    "DELETE FROM capabilities WHERE capability_id=?1",
                    [&plan.id],
                )?;
                if current.version == 3 {
                    tx.execute(
                        "DELETE FROM derived_capabilities WHERE capability_id=?1",
                        [&plan.id],
                    )?;
                }
            }
        }
        if current.version == 3 {
            for id in current
                .applications
                .keys()
                .filter(|id| !result.applications.contains_key(*id))
            {
                tx.execute(
                    "DELETE FROM composition_applications WHERE application_id=?1",
                    [id],
                )?;
            }
            for id in current
                .recipes
                .keys()
                .filter(|id| !result.recipes.contains_key(*id))
            {
                tx.execute("DELETE FROM compositions WHERE composition_id=?1", [id])?;
            }
        }
        #[cfg(test)]
        composition_recovery_tests::fault_checkpoint(&tx, "after_delete_rows")?;
        if Records::load(&tx)?.state_id()? != result.state_id()? {
            return Err(StoreError::Corrupt);
        }
        tx.commit()?;
        Ok(())
    }
}
