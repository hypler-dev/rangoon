//! Complete retained-plan recovery. A preview never authorizes a write.
use super::*;
use composition_records::Records;
use workflow_records::{UnionError, WorkflowRows};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRecoveryCounts {
    pub sources: u32,
    pub capabilities: u32,
    pub revisions: u32,
    pub reviews: u32,
    pub recipes: u32,
    pub applications: u32,
    pub derivations: u32,
    pub workflows: u32,
    pub workflow_revisions: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRestorePlan {
    schema_version: &'static str,
    backup_id: String,
    expected_state_id: String,
    byte_length: usize,
    add: WorkflowRecoveryCounts,
    kept_sources: u32,
    kept_capabilities: u32,
    kept_workflows: u32,
}

impl WorkflowRestorePlan {
    pub fn backup_id(&self) -> &str {
        &self.backup_id
    }
    pub fn expected_state_id(&self) -> &str {
        &self.expected_state_id
    }
}

pub(super) fn counts(records: &Records) -> WorkflowRecoveryCounts {
    let base = composition_recovery::counts(records);
    WorkflowRecoveryCounts {
        sources: base.sources,
        capabilities: base.capabilities,
        revisions: base.revisions,
        reviews: base.reviews,
        recipes: base.recipes,
        applications: base.applications,
        derivations: base.derivations,
        workflows: records.workflows.owners.len() as u32,
        workflow_revisions: records.workflows.revisions.len() as u32,
    }
}

pub(super) fn difference(
    larger: WorkflowRecoveryCounts,
    smaller: WorkflowRecoveryCounts,
) -> Result<WorkflowRecoveryCounts, StoreError> {
    let sub = |a: u32, b: u32| a.checked_sub(b).ok_or(StoreError::Corrupt);
    Ok(WorkflowRecoveryCounts {
        sources: sub(larger.sources, smaller.sources)?,
        capabilities: sub(larger.capabilities, smaller.capabilities)?,
        revisions: sub(larger.revisions, smaller.revisions)?,
        reviews: sub(larger.reviews, smaller.reviews)?,
        recipes: sub(larger.recipes, smaller.recipes)?,
        applications: sub(larger.applications, smaller.applications)?,
        derivations: sub(larger.derivations, smaller.derivations)?,
        workflows: sub(larger.workflows, smaller.workflows)?,
        workflow_revisions: sub(larger.workflow_revisions, smaller.workflow_revisions)?,
    })
}

pub(super) fn base_projection(records: &Records) -> Records {
    let mut result = records.clone();
    if result.version == 4 {
        result.version = 3;
    }
    result.workflows = WorkflowRows::default();
    result
}

pub(super) fn restore_plan(
    current: &Records,
    backup: &WorkspaceBackup,
) -> Result<(WorkflowRestorePlan, Records), StoreError> {
    let mut result =
        composition_recovery::union(&base_projection(current), &base_projection(&backup.records))?;
    let (workflows, kept_workflows) = current
        .workflows
        .union(&backup.records.workflows, &result)
        .map_err(|error| match error {
            UnionError::Invalid => StoreError::BackupInvalid,
            UnionError::Full => StoreError::WorkflowFull,
            UnionError::DependencyMissing => StoreError::WorkflowDependencyMissing,
        })?;
    result.workflows = workflows;
    let add = difference(counts(&result), counts(current))?;
    result.version = if add == WorkflowRecoveryCounts::default() {
        current.version
    } else {
        current.version.max(backup.records.version)
    };
    // Read back a complete canonical candidate, including database allocation.
    // This creates only an in-memory database, never the destination directory.
    let result = Records::load_complete(&composition_backup::canonical_database(&result)?)?;
    let plan = WorkflowRestorePlan {
        schema_version: "rangoon.restore-plan.v2",
        backup_id: backup.id().into(),
        expected_state_id: current.state_id()?,
        byte_length: backup.byte_length(),
        kept_sources: (backup.records.sources.len() as u32)
            .checked_sub(add.sources)
            .ok_or(StoreError::Corrupt)?,
        kept_capabilities: (backup.records.owners.len() as u32)
            .checked_sub(add.capabilities)
            .ok_or(StoreError::Corrupt)?,
        add,
        kept_workflows,
    };
    Ok((plan, result))
}

fn checked_restore(
    current: &Records,
    backup: &WorkspaceBackup,
    retained: &WorkflowRestorePlan,
) -> Result<(WorkflowRestorePlan, Records), StoreError> {
    if current.state_id()? != retained.expected_state_id
        || backup.id() != retained.backup_id
        || backup.byte_length() != retained.byte_length
    {
        return Err(StoreError::WorkspaceChanged);
    }
    let (plan, result) = restore_plan(current, backup)?;
    if &plan != retained {
        return Err(StoreError::WorkspaceChanged);
    }
    Ok((plan, result))
}

fn commit_restore(
    tx: &rusqlite::Transaction<'_>,
    backup: &WorkspaceBackup,
    retained: &WorkflowRestorePlan,
) -> Result<WorkflowWorkspaceData, StoreError> {
    let current = workflow_mutations::read_records(tx)?;
    let (plan, result) = checked_restore(&current, backup, retained)?;
    if plan.add != WorkflowRecoveryCounts::default() {
        #[cfg(test)]
        composition_recovery_tests::fault_checkpoint(tx, "before_workflow_restore_migration")?;
        workflow_mutations::ensure_schema(tx, result.version)?;
        #[cfg(test)]
        composition_recovery_tests::fault_checkpoint(tx, "after_workflow_restore_migration")?;
        workflow_mutations::insert_additions(tx, &current, &result)?;
        #[cfg(test)]
        composition_recovery_tests::fault_checkpoint(tx, "after_workflow_restore_rows")?;
    }
    let actual = workflow_mutations::read_records(tx)?;
    if actual.state_id()? != result.state_id()? {
        return Err(StoreError::Corrupt);
    }
    workflow_mutations::data(tx, &actual)
}

impl Workspace {
    /// The host must retain the exact validated archive and issued plan.
    pub fn restore_workflow_backup(
        &self,
        backup: &WorkspaceBackup,
        retained: &WorkflowRestorePlan,
    ) -> Result<WorkflowWorkspaceData, StoreError> {
        // Preflight is noncreating. No-op absence is linearized at this read.
        let (absent, current) = match self.connect_complete_read_only(true)? {
            None => (true, Records::empty()),
            Some(mut db) => {
                let tx = db.transaction()?;
                let records = workflow_mutations::read_records(&tx)?;
                tx.commit()?;
                (false, records)
            }
        };
        let (plan, _) = checked_restore(&current, backup, retained)?;
        let additions = plan.add != WorkflowRecoveryCounts::default();
        if absent && !additions {
            return workflow_views::data(&current, 0, 0);
        }
        if !additions {
            // BEGIN IMMEDIATE can initialize an existing zero-byte SQLite file
            // even without row writes. Revalidate no-ops in a read-only snapshot.
            let Some(mut db) = self.connect_complete_read_only(true)? else {
                return Err(StoreError::WorkspaceChanged);
            };
            let tx = db.transaction()?;
            let result = commit_restore(&tx, backup, retained)?;
            tx.commit()?;
            return Ok(result);
        }
        let Some(mut db) = self.connect_complete_with_empty(additions, true)? else {
            return Err(StoreError::WorkspaceChanged);
        };
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = commit_restore(&tx, backup, retained)?;
        tx.commit()?;
        Ok(result)
    }

    pub fn prepare_workflow_restore(
        &self,
        backup: &WorkspaceBackup,
    ) -> Result<WorkflowRestorePlan, StoreError> {
        let Some(mut db) = self.connect_complete_read_only(true)? else {
            return Ok(restore_plan(&Records::empty(), backup)?.0);
        };
        let tx = db.transaction()?;
        let current = workflow_mutations::read_records(&tx)?;
        let plan = restore_plan(&current, backup)?.0;
        tx.commit()?;
        Ok(plan)
    }
}

#[cfg(test)]
#[path = "workflow_recovery_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "workflow_restore_tests.rs"]
mod restore_tests;
