//! Exact-state, dependency-aware logical deletion. Never secure erasure.
use super::*;
use composition_records::Records;
use rangoon_domain::{capability::valid_id, capability_v1::CapabilitySummary};
use rangoon_workflow::Operation;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowRecordKind {
    Source,
    Capability,
    Workflow,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDeletionPlan {
    schema_version: &'static str,
    kind: WorkflowRecordKind,
    id: String,
    label: String,
    expected_state_id: String,
    remove: WorkflowRecoveryCounts,
    capability_dependencies: Vec<CapabilitySummary>,
    workflow_dependencies: Vec<WorkflowSummary>,
}

impl WorkflowDeletionPlan {
    pub fn expected_state_id(&self) -> &str {
        &self.expected_state_id
    }
    pub fn kind(&self) -> WorkflowRecordKind {
        self.kind
    }
    pub fn id(&self) -> &str {
        &self.id
    }
}

fn validate_id(kind: WorkflowRecordKind, id: &str) -> Result<(), StoreError> {
    let prefix = match kind {
        WorkflowRecordKind::Source => "source:",
        WorkflowRecordKind::Capability => "capability:",
        WorkflowRecordKind::Workflow => "workflow:",
    };
    if valid_id(id, prefix) {
        Ok(())
    } else {
        Err(StoreError::WorkflowInvalid)
    }
}

fn deletion_plan(
    current: &Records,
    kind: WorkflowRecordKind,
    id: &str,
) -> Result<(WorkflowDeletionPlan, Records), StoreError> {
    validate_id(kind, id)?;
    let (label, capability_dependencies, mut result) = match kind {
        WorkflowRecordKind::Workflow => {
            let label = workflow_views::summary(current, id)?.label;
            let mut result = current.clone();
            result.workflows.owners.remove(id);
            result
                .workflows
                .revisions
                .retain(|_, row| row.record.workflow_id() != id);
            (label, Vec::new(), result)
        }
        WorkflowRecordKind::Source | WorkflowRecordKind::Capability => {
            let legacy_kind = if kind == WorkflowRecordKind::Source {
                RecordKind::Source
            } else {
                RecordKind::Capability
            };
            let (base_plan, mut result) = composition_recovery::deletion_plan(
                &workflow_recovery::base_projection(current),
                legacy_kind,
                id,
            )?;
            result.version = current.version;
            result.workflows = current.workflows.clone();
            (base_plan.title, base_plan.dependencies, result)
        }
    };
    // Every saved revision participates. A draft's malformed revision ID does
    // not make its syntactically valid capability-owner reference disposable.
    let mut dependent_workflows = BTreeSet::new();
    if kind == WorkflowRecordKind::Capability {
        for row in current.workflows.revisions.values() {
            if row.record.inspection().definition().nodes.iter().any(|node| {
                matches!(&node.operation, Operation::Capability { capability_id, .. } if valid_id(capability_id, "capability:") && capability_id == id)
            }) {
                dependent_workflows.insert(row.record.workflow_id());
            }
        }
    }
    let workflow_dependencies = dependent_workflows
        .into_iter()
        .map(|id| workflow_views::summary(current, id))
        .collect::<Result<Vec<_>, _>>()?;
    let plan = WorkflowDeletionPlan {
        schema_version: "rangoon.deletion-plan.v2",
        kind,
        id: id.into(),
        label,
        expected_state_id: current.state_id()?,
        remove: workflow_recovery::difference(
            workflow_recovery::counts(current),
            workflow_recovery::counts(&result),
        )?,
        capability_dependencies,
        workflow_dependencies,
    };
    // A blocked preview must remain inspectable without pretending its invalid
    // prospective dependency graph can be committed.
    if plan.capability_dependencies.is_empty() && plan.workflow_dependencies.is_empty() {
        result = Records::load_complete(&composition_backup::canonical_database(&result)?)?;
    }
    Ok((plan, result))
}

fn commit_deletion(
    tx: &rusqlite::Transaction<'_>,
    retained: &WorkflowDeletionPlan,
) -> Result<WorkflowWorkspaceData, StoreError> {
    let current = Records::load_complete(tx)?;
    if current.state_id()? != retained.expected_state_id {
        return Err(StoreError::WorkspaceChanged);
    }
    let (plan, result) = deletion_plan(&current, retained.kind, &retained.id)?;
    if &plan != retained {
        return Err(StoreError::WorkspaceChanged);
    }
    if !plan.capability_dependencies.is_empty() || !plan.workflow_dependencies.is_empty() {
        return Err(StoreError::RecordInUse);
    }
    #[cfg(test)]
    composition_recovery_tests::fault_checkpoint(tx, "before_workflow_delete")?;
    match plan.kind {
        WorkflowRecordKind::Source => {
            tx.execute("DELETE FROM snapshots WHERE source_id=?1", [&plan.id])?;
        }
        WorkflowRecordKind::Capability => {
            tx.execute("DELETE FROM reviews WHERE revision_id IN (SELECT revision_id FROM revisions WHERE capability_id=?1)", [&plan.id])?;
            if matches!(current.version, 3 | 4) {
                tx.execute("DELETE FROM revision_derivations WHERE revision_id IN (SELECT revision_id FROM revisions WHERE capability_id=?1)", [&plan.id])?;
            }
            tx.execute("DELETE FROM revisions WHERE capability_id=?1", [&plan.id])?;
            tx.execute(
                "DELETE FROM capabilities WHERE capability_id=?1",
                [&plan.id],
            )?;
            if matches!(current.version, 3 | 4) {
                tx.execute(
                    "DELETE FROM derived_capabilities WHERE capability_id=?1",
                    [&plan.id],
                )?;
            }
        }
        WorkflowRecordKind::Workflow => {
            tx.execute(
                "DELETE FROM workflow_revisions WHERE workflow_id=?1",
                [&plan.id],
            )?;
            #[cfg(test)]
            composition_recovery_tests::fault_checkpoint(tx, "after_workflow_history_delete")?;
            tx.execute("DELETE FROM workflows WHERE workflow_id=?1", [&plan.id])?;
        }
    }
    if matches!(current.version, 3 | 4) {
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
    composition_recovery_tests::fault_checkpoint(tx, "after_workflow_delete_rows")?;
    let actual = Records::load_complete(tx)?;
    if actual.state_id()? != result.state_id()? {
        return Err(StoreError::Corrupt);
    }
    let size: u32 = tx.pragma_query_value(None, "page_size", |r| r.get(0))?;
    let pages: u32 = tx.pragma_query_value(None, "page_count", |r| r.get(0))?;
    let free: u32 = tx.pragma_query_value(None, "freelist_count", |r| r.get(0))?;
    workflow_views::data(
        &actual,
        u64::from(size) * u64::from(pages),
        u64::from(size) * u64::from(free),
    )
}

impl Workspace {
    pub fn inspect_workflow_deletion(
        &self,
        kind: WorkflowRecordKind,
        id: &str,
    ) -> Result<WorkflowDeletionPlan, StoreError> {
        validate_id(kind, id)?;
        let Some(mut db) = self.connect_complete_read_only(true)? else {
            return Err(if kind == WorkflowRecordKind::Workflow {
                StoreError::WorkflowNotFound
            } else {
                StoreError::NotFound
            });
        };
        let tx = db.transaction()?;
        let result = deletion_plan(&workflow_mutations::read_records(&tx)?, kind, id)?.0;
        tx.commit()?;
        Ok(result)
    }

    pub fn delete_workflow_record(
        &self,
        retained: &WorkflowDeletionPlan,
    ) -> Result<WorkflowWorkspaceData, StoreError> {
        validate_id(retained.kind, &retained.id)?;
        // Check stale impact and dependencies using a noncreating read before
        // opening a write connection. The transaction below must still derive
        // and compare the complete plan again.
        let Some(mut preflight) = self.connect_complete_read_only(true)? else {
            return Err(StoreError::WorkspaceChanged);
        };
        let read = preflight.transaction()?;
        let current = workflow_mutations::read_records(&read)?;
        if current.state_id()? != retained.expected_state_id {
            return Err(StoreError::WorkspaceChanged);
        }
        let (plan, _) = deletion_plan(&current, retained.kind, &retained.id)?;
        if &plan != retained {
            return Err(StoreError::WorkspaceChanged);
        }
        if !plan.capability_dependencies.is_empty() || !plan.workflow_dependencies.is_empty() {
            return Err(StoreError::RecordInUse);
        }
        read.commit()?;
        drop(preflight);
        let Some(mut db) = self.connect_complete_with_empty(false, true)? else {
            return Err(StoreError::WorkspaceChanged);
        };
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // A previously issued plan cannot describe an empty file. Treat a reset
        // workspace as changed before attempting the qualified schema loader.
        let version: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version == 0 {
            return Err(StoreError::WorkspaceChanged);
        }
        let result = commit_deletion(&tx, retained)?;
        tx.commit()?;
        Ok(result)
    }
}

#[cfg(test)]
#[path = "workflow_deletion_tests.rs"]
mod tests;
