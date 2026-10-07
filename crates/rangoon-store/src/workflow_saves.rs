//! Immutable workflow authoring transactions; execution authority is unchanged.
use super::*;
use composition_records::Records;
use rangoon_domain::capability::valid_id;
use rangoon_workflow::records::{SaveIntent, WorkflowRevision, decode_revision};
use workflow_records::{ReferenceStatus, SavedRevision};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSavePlan {
    pub schema_version: &'static str,
    pub workflow_id: String,
    pub revision_id: String,
    pub expected_state_id: String,
    pub expected_head_id: Option<String>,
    pub already_saved: bool,
    pub unresolved_references: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSaveReceipt {
    pub schema_version: &'static str,
    pub workflow: WorkflowDetail,
    pub already_saved: bool,
    pub state_id: String,
}

fn canonical(candidate: &WorkflowRevision) -> Result<WorkflowRevision, StoreError> {
    decode_revision(
        &candidate
            .serialized_bytes()
            .map_err(|_| StoreError::WorkflowInvalid)?,
    )
    .map_err(|_| StoreError::WorkflowInvalid)
}

fn retry(current: &Records, candidate: &WorkflowRevision) -> Result<bool, StoreError> {
    if current
        .workflows
        .owners
        .get(candidate.workflow_id())
        .map(String::as_str)
        != Some(candidate.id())
    {
        return Ok(false);
    }
    let saved = current
        .workflows
        .revisions
        .get(candidate.id())
        .ok_or(StoreError::Corrupt)?;
    Ok(saved
        .record
        .serialized_bytes()
        .map_err(|_| StoreError::Corrupt)?
        == candidate
            .serialized_bytes()
            .map_err(|_| StoreError::WorkflowInvalid)?)
}

fn prepare(
    current: &Records,
    candidate: &WorkflowRevision,
) -> Result<(WorkflowSavePlan, Records), StoreError> {
    let candidate = canonical(candidate)?;
    let already_saved = retry(current, &candidate)?;
    let head = current.workflows.owners.get(candidate.workflow_id());
    let unresolved = workflow_records::references(&candidate, current)
        .iter()
        .filter(|(_, status)| *status != ReferenceStatus::Resolved)
        .count() as u32;
    if candidate.intent() == SaveIntent::Validated && unresolved != 0 {
        return Err(StoreError::WorkflowDependencyMissing);
    }
    let plan = WorkflowSavePlan {
        schema_version: "rangoon.workflow-save-plan.v1",
        workflow_id: candidate.workflow_id().into(),
        revision_id: candidate.id().into(),
        expected_state_id: current.state_id()?,
        expected_head_id: head.cloned(),
        already_saved,
        unresolved_references: unresolved,
    };
    if already_saved {
        return Ok((plan, current.clone()));
    }
    match head {
        None if candidate.parent_revision_id().is_some() => {
            return Err(StoreError::WorkflowInvalid);
        }
        Some(id) if Some(id.as_str()) != candidate.parent_revision_id() => {
            return Err(StoreError::WorkflowConflict);
        }
        _ => (),
    }
    if (head.is_none() && current.workflows.owners.len() >= 128)
        || current.workflows.revisions.len() >= 1024
        || current
            .workflows
            .history(candidate.workflow_id())
            .is_some_and(|h| h.len() >= 32)
    {
        return Err(StoreError::WorkflowFull);
    }
    let saved_at_ms = capabilities::now_ms()?;
    if !(0..=8_640_000_000_000_000).contains(&saved_at_ms) {
        return Err(StoreError::Unavailable);
    }
    let mut result = current.clone();
    result.version = 4;
    result
        .workflows
        .owners
        .insert(candidate.workflow_id().into(), candidate.id().into());
    result.workflows.revisions.insert(
        candidate.id().into(),
        SavedRevision {
            record: candidate,
            saved_at_ms,
        },
    );
    let result = Records::load_complete(&composition_backup::canonical_database(&result)?)?;
    Ok((plan, result))
}

fn commit_save(
    tx: &rusqlite::Transaction<'_>,
    candidate: &WorkflowRevision,
    expected_state_id: &str,
) -> Result<WorkflowSaveReceipt, StoreError> {
    if !valid_id(expected_state_id, "workspace:") {
        return Err(StoreError::WorkflowInvalid);
    }
    let candidate = canonical(candidate)?;
    let current = workflow_mutations::read_records(tx)?;
    if current.state_id()? != expected_state_id && !retry(&current, &candidate)? {
        return Err(StoreError::WorkspaceChanged);
    }
    let (plan, result) = prepare(&current, &candidate)?;
    if !plan.already_saved {
        #[cfg(test)]
        composition_recovery_tests::fault_checkpoint(tx, "before_workflow_save_migration")?;
        workflow_mutations::ensure_schema(tx, 4)?;
        #[cfg(test)]
        composition_recovery_tests::fault_checkpoint(tx, "after_workflow_save_migration")?;
        let saved = result
            .workflows
            .revisions
            .get(candidate.id())
            .ok_or(StoreError::Corrupt)?;
        tx.execute(
            "INSERT INTO workflow_revisions VALUES (?1,?2,?3,?4)",
            params![
                candidate.id(),
                candidate.workflow_id(),
                candidate
                    .serialized_bytes()
                    .map_err(|_| StoreError::WorkflowInvalid)?,
                saved.saved_at_ms
            ],
        )?;
        #[cfg(test)]
        composition_recovery_tests::fault_checkpoint(tx, "after_workflow_save_revision")?;
        match plan.expected_head_id.as_deref() {
            None => { tx.execute("INSERT INTO workflows VALUES (?1,?2)", params![candidate.workflow_id(), candidate.id()])?; },
            Some(head) => {
                if tx.execute("UPDATE workflows SET latest_revision_id=?1 WHERE workflow_id=?2 AND latest_revision_id=?3", params![candidate.id(), candidate.workflow_id(), head])? != 1 {
                    return Err(StoreError::WorkflowConflict);
                }
            }
        }
        #[cfg(test)]
        composition_recovery_tests::fault_checkpoint(tx, "after_workflow_save_head")?;
    }
    let actual = workflow_mutations::read_records(tx)?;
    if actual.state_id()? != result.state_id()? {
        return Err(StoreError::Corrupt);
    }
    workflow_mutations::data(tx, &actual)?;
    Ok(WorkflowSaveReceipt {
        schema_version: "rangoon.workflow-save-receipt.v1",
        workflow: workflow_views::detail(&actual, candidate.workflow_id(), None)?,
        already_saved: plan.already_saved,
        state_id: actual.state_id()?,
    })
}

impl Workspace {
    pub fn inspect_workflow_save(
        &self,
        candidate: &WorkflowRevision,
    ) -> Result<WorkflowSavePlan, StoreError> {
        let candidate = canonical(candidate)?;
        let Some(mut db) = self.connect_complete_read_only(true)? else {
            return Ok(prepare(&Records::empty(), &candidate)?.0);
        };
        let tx = db.transaction()?;
        let result = prepare(&workflow_mutations::read_records(&tx)?, &candidate)?.0;
        tx.commit()?;
        Ok(result)
    }

    pub fn save_workflow(
        &self,
        candidate: &WorkflowRevision,
        expected_state_id: &str,
    ) -> Result<WorkflowSaveReceipt, StoreError> {
        if !valid_id(expected_state_id, "workspace:") {
            return Err(StoreError::WorkflowInvalid);
        }
        let candidate = canonical(candidate)?;
        // Validate against a noncreating read before a first write can create a file.
        let current = match self.connect_complete_read_only(true)? {
            None => Records::empty(),
            Some(mut db) => {
                let tx = db.transaction()?;
                let records = workflow_mutations::read_records(&tx)?;
                tx.commit()?;
                records
            }
        };
        if current.state_id()? != expected_state_id && !retry(&current, &candidate)? {
            return Err(StoreError::WorkspaceChanged);
        }
        prepare(&current, &candidate)?;
        let mut db = self
            .connect_complete_with_empty(true, true)?
            .ok_or(StoreError::Unavailable)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = commit_save(&tx, &candidate, expected_state_id)?;
        tx.commit()?;
        Ok(result)
    }
}

#[cfg(test)]
#[path = "workflow_saves_tests.rs"]
mod tests;
