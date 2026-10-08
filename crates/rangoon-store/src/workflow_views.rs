//! Read-only workflow inventory and historical inspection over validated records.
use super::*;
use composition_records::Records;
use rangoon_domain::{capability::valid_id, capability_v1::CapabilitySummary};
use rangoon_workflow::{
    Operation,
    records::{SaveIntent, WorkflowRevision},
};
use workflow_records::ReferenceStatus;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSummary {
    pub id: String,
    pub latest_revision_id: String,
    pub label: String,
    pub label_adjusted: bool,
    pub intent: SaveIntent,
    pub structurally_valid: bool,
    pub revision_count: u32,
    pub saved_at_ms: i64,
    pub unresolved_references: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRevisionSummary {
    pub id: String,
    pub parent_revision_id: Option<String>,
    pub intent: SaveIntent,
    pub structurally_valid: bool,
    pub saved_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowReference {
    pub node_index: u32,
    pub capability_id: String,
    pub revision_id: String,
    pub status: WorkflowReferenceStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowReferenceStatus {
    InvalidReference,
    MissingCapability,
    MissingRevision,
    Resolved,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDetail {
    pub schema_version: &'static str,
    pub head: WorkflowSummary,
    pub revision: WorkflowRevision,
    pub saved_at_ms: i64,
    pub history: Vec<WorkflowRevisionSummary>,
    pub references: Vec<WorkflowReference>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowWorkspaceData {
    pub schema_version: &'static str,
    pub state_id: String,
    pub records: WorkflowRecoveryCounts,
    pub database_bytes: u64,
    pub reusable_bytes: u64,
    pub sources: Vec<SnapshotMetadata>,
    pub capabilities: Vec<CapabilitySummary>,
    pub workflows: Vec<WorkflowSummary>,
}

fn label(title: &str) -> String {
    let filtered: String = title
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '\u{2028}' | '\u{2029}'))
        .collect();
    let trimmed = filtered.trim();
    let mut end = trimmed.len().min(160);
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    let result = trimmed[..end].trim();
    if result.is_empty() {
        "Untitled workflow".into()
    } else {
        result.into()
    }
}

pub(super) fn summary(records: &Records, id: &str) -> Result<WorkflowSummary, StoreError> {
    let head = records
        .workflows
        .owners
        .get(id)
        .ok_or(StoreError::WorkflowNotFound)?;
    let row = records
        .workflows
        .revisions
        .get(head)
        .ok_or(StoreError::Corrupt)?;
    let title = &row.record.inspection().definition().title;
    let display = label(title);
    let history = records.workflows.history(id).ok_or(StoreError::Corrupt)?;
    Ok(WorkflowSummary {
        id: id.into(),
        latest_revision_id: head.clone(),
        label_adjusted: display != *title,
        label: display,
        intent: row.record.intent(),
        structurally_valid: row.record.inspection().report().structurally_valid,
        revision_count: history.len() as u32,
        saved_at_ms: row.saved_at_ms,
        unresolved_references: workflow_records::references(&row.record, records)
            .iter()
            .filter(|(_, status)| *status != ReferenceStatus::Resolved)
            .count() as u32,
    })
}

pub(super) fn detail(
    records: &Records,
    id: &str,
    selected: Option<&str>,
) -> Result<WorkflowDetail, StoreError> {
    let head = summary(records, id)?;
    let row = records
        .workflows
        .revisions
        .get(selected.unwrap_or(&head.latest_revision_id))
        .filter(|row| row.record.workflow_id() == id)
        .ok_or(StoreError::WorkflowNotFound)?;
    let history = records
        .workflows
        .history(id)
        .ok_or(StoreError::Corrupt)?
        .iter()
        .map(|id| {
            let row = records
                .workflows
                .revisions
                .get(id)
                .ok_or(StoreError::Corrupt)?;
            Ok(WorkflowRevisionSummary {
                id: id.clone(),
                parent_revision_id: row.record.parent_revision_id().map(String::from),
                intent: row.record.intent(),
                structurally_valid: row.record.inspection().report().structurally_valid,
                saved_at_ms: row.saved_at_ms,
            })
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    let references = workflow_records::references(&row.record, records)
        .into_iter()
        .map(|(index, status)| {
            let Operation::Capability {
                capability_id,
                revision_id,
            } = &row.record.inspection().definition().nodes[index as usize].operation
            else {
                return Err(StoreError::Corrupt);
            };
            Ok(WorkflowReference {
                node_index: index,
                capability_id: capability_id.clone(),
                revision_id: revision_id.clone(),
                status: match status {
                    ReferenceStatus::InvalidReference => WorkflowReferenceStatus::InvalidReference,
                    ReferenceStatus::MissingCapability => {
                        WorkflowReferenceStatus::MissingCapability
                    }
                    ReferenceStatus::MissingRevision => WorkflowReferenceStatus::MissingRevision,
                    ReferenceStatus::Resolved => WorkflowReferenceStatus::Resolved,
                },
            })
        })
        .collect::<Result<_, StoreError>>()?;
    Ok(WorkflowDetail {
        schema_version: "rangoon.workflow-detail.v1",
        head,
        revision: row.record.clone(),
        saved_at_ms: row.saved_at_ms,
        history,
        references,
    })
}

pub(super) fn data(
    records: &Records,
    database_bytes: u64,
    reusable_bytes: u64,
) -> Result<WorkflowWorkspaceData, StoreError> {
    Ok(WorkflowWorkspaceData {
        schema_version: "rangoon.workspace-data.v2",
        state_id: records.state_id()?,
        records: workflow_recovery::counts(records),
        database_bytes,
        reusable_bytes,
        sources: records
            .sources
            .values()
            .map(|row| row.metadata.clone())
            .collect(),
        capabilities: records
            .owners
            .keys()
            .map(|id| records.summary(id))
            .collect::<Result<_, _>>()?,
        workflows: records
            .workflows
            .owners
            .keys()
            .map(|id| summary(records, id))
            .collect::<Result<_, _>>()?,
    })
}

impl Workspace {
    pub fn workflow_data(&self) -> Result<WorkflowWorkspaceData, StoreError> {
        let Some(mut db) = self.connect_complete_read_only(true)? else {
            return data(&Records::empty(), 0, 0);
        };
        let tx = db.transaction()?;
        let records = workflow_mutations::read_records(&tx)?;
        let page_size: u32 = database_page_size(&tx)?;
        let pages: u32 = tx.pragma_query_value(None, "page_count", |r| r.get(0))?;
        let free: u32 = tx.pragma_query_value(None, "freelist_count", |r| r.get(0))?;
        let result = data(
            &records,
            u64::from(page_size) * u64::from(pages),
            u64::from(page_size) * u64::from(free),
        )?;
        tx.commit()?;
        Ok(result)
    }

    pub fn list_workflows(&self) -> Result<Vec<WorkflowSummary>, StoreError> {
        Ok(self.workflow_data()?.workflows)
    }

    pub fn open_workflow(
        &self,
        workflow_id: &str,
        revision_id: Option<&str>,
    ) -> Result<WorkflowDetail, StoreError> {
        if !valid_id(workflow_id, "workflow:")
            || revision_id.is_some_and(|id| !valid_id(id, "workflow-revision:"))
        {
            return Err(StoreError::WorkflowInvalid);
        }
        let Some(mut db) = self.connect_complete_read_only(true)? else {
            return Err(StoreError::WorkflowNotFound);
        };
        let tx = db.transaction()?;
        let records = workflow_mutations::read_records(&tx)?;
        let result = detail(&records, workflow_id, revision_id)?;
        tx.commit()?;
        Ok(result)
    }
}

#[cfg(test)]
#[path = "workflow_views_tests.rs"]
mod tests;
