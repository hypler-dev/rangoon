//! Native immutable authoring custody. Definitions remain inert local data.
#[cfg(test)]
use super::Workspace;
use super::{AppHandle, PublicError, StoreError, workspace_source_access::SourceAccess};
use rangoon_desktop::workspace_lifecycle::GenerationWitness;
use rangoon_domain::capability::valid_id;
use rangoon_store::{WorkflowDetail, WorkflowSavePlan, WorkflowSaveReceipt, WorkflowSummary};
use rangoon_workflow::{
    ValidationReport,
    records::{SaveIntent, WorkflowRevision, prepare_revision, workflow_id_from_nonce},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::value::RawValue;
use std::sync::Mutex;
use tauri::{Manager, ipc::InvokeBody};

const SMALL_LIMIT: usize = 1024;
const INSPECT_LIMIT: usize = 160 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct OpenRequest {
    pub(super) schema_version: String,
    pub(super) workflow_id: String,
    pub(super) selection: Selection,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Selection {
    Head {},
    Historical {
        #[serde(rename = "revisionId")]
        revision_id: String,
    },
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct BeginRequest {
    pub(super) schema_version: String,
    pub(super) target: Target,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Target {
    New {},
    Append {
        #[serde(rename = "workflowId")]
        workflow_id: String,
        #[serde(rename = "expectedHeadId")]
        expected_head_id: String,
    },
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct InspectRequest {
    pub(super) schema_version: String,
    pub(super) draft_id: String,
    pub(super) intent: SaveIntent,
    pub(super) definition: Box<RawValue>,
    pub(super) layout: Box<RawValue>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Confirmation {
    pub(super) schema_version: String,
    pub(super) preview_id: String,
    pub(super) expected_state_id: String,
    pub(super) acknowledged: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ClearRequest {
    pub(super) schema_version: String,
    pub(super) draft_id: String,
}
fn raw<T: DeserializeOwned>(body: &InvokeBody, limit: usize) -> Result<T, StoreError> {
    let InvokeBody::Raw(bytes) = body else {
        return Err(StoreError::WorkflowInvalid);
    };
    if bytes.len() > limit {
        return Err(StoreError::WorkflowInvalid);
    }
    // Tauri already received this buffer. This is a decoding bound, not a transport cap.
    serde_json::from_slice(bytes).map_err(|_| StoreError::WorkflowInvalid)
}
pub(super) fn decode_open(body: &InvokeBody) -> Result<OpenRequest, StoreError> {
    let value: OpenRequest = raw(body, SMALL_LIMIT)?;
    if value.schema_version != "rangoon.workflow-open.v1"
        || !valid_id(&value.workflow_id, "workflow:")
        || matches!(&value.selection, Selection::Historical { revision_id } if !valid_id(revision_id, "workflow-revision:"))
    {
        return Err(StoreError::WorkflowInvalid);
    }
    Ok(value)
}
pub(super) fn decode_begin(body: &InvokeBody) -> Result<BeginRequest, StoreError> {
    let value: BeginRequest = raw(body, SMALL_LIMIT)?;
    if value.schema_version != "rangoon.workflow-draft.v1"
        || matches!(&value.target, Target::Append {workflow_id, expected_head_id} if !valid_id(workflow_id, "workflow:") || !valid_id(expected_head_id, "workflow-revision:"))
    {
        return Err(StoreError::WorkflowInvalid);
    }
    Ok(value)
}
pub(super) fn decode_inspect(body: &InvokeBody) -> Result<InspectRequest, StoreError> {
    let value: InspectRequest = raw(body, INSPECT_LIMIT)?;
    if value.schema_version != "rangoon.workflow-inspect.v1"
        || !valid_id(&value.draft_id, "workflow-draft:")
    {
        return Err(StoreError::WorkflowInvalid);
    }
    Ok(value)
}
pub(super) fn decode_commit(body: &InvokeBody) -> Result<Confirmation, StoreError> {
    let value: Confirmation = raw(body, SMALL_LIMIT)?;
    if value.schema_version != "rangoon.workflow-commit.v1"
        || !valid_id(&value.preview_id, "workflow-preview:")
        || !valid_id(&value.expected_state_id, "workspace:")
        || !value.acknowledged
    {
        return Err(StoreError::WorkflowInvalid);
    }
    Ok(value)
}
pub(super) fn decode_clear(body: &InvokeBody) -> Result<ClearRequest, StoreError> {
    let value: ClearRequest = raw(body, SMALL_LIMIT)?;
    if value.schema_version != "rangoon.workflow-clear.v1"
        || !valid_id(&value.draft_id, "workflow-draft:")
    {
        return Err(StoreError::WorkflowInvalid);
    }
    Ok(value)
}

#[derive(Clone)]
struct DraftContext {
    id: String,
    workflow_id: String,
    parent_id: Option<String>,
}
pub(super) struct IssuedSave {
    id: String,
    pub(super) candidate: WorkflowRevision,
    pub(super) plan: WorkflowSavePlan,
}
#[derive(Default)]
struct AuthoringState {
    witness: Option<GenerationWitness>,
    draft: Option<DraftContext>,
    prepared: Option<IssuedSave>,
}
#[derive(Default)]
pub struct WorkflowSession(Mutex<AuthoringState>);

fn entropy(bytes: &mut [u8]) -> Result<(), StoreError> {
    getrandom::fill(bytes).map_err(|_| StoreError::Unavailable)
}
fn handle(
    prefix: &str,
    random: &mut impl FnMut(&mut [u8]) -> Result<(), StoreError>,
) -> Result<String, StoreError> {
    let mut bytes = [0; 32];
    random(&mut bytes)?;
    Ok(format!(
        "{prefix}{}",
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}
impl WorkflowSession {
    #[cfg(test)]
    pub(super) fn retained_for_close_test(&self) -> bool {
        let state = self.0.lock().unwrap();
        state.draft.is_some() || state.prepared.is_some()
    }
    #[cfg(test)]
    fn begin(&self, store: &Workspace, target: Target) -> Result<WorkflowResult, StoreError> {
        self.begin_with(store, target, entropy)
    }
    #[cfg(test)]
    fn begin_with(
        &self,
        store: &Workspace,
        target: Target,
        random: impl FnMut(&mut [u8]) -> Result<(), StoreError>,
    ) -> Result<WorkflowResult, StoreError> {
        self.begin_using(target, None, random, |id, new| {
            match store.open_workflow(id, None) {
                Ok(detail) => Ok(Some(detail)),
                Err(StoreError::WorkflowNotFound) if new => Ok(None),
                Err(error) => Err(error),
            }
        })
    }
    pub(super) fn begin_routed<E: From<StoreError>>(
        &self,
        target: Target,
        witness: GenerationWitness,
        lookup: impl FnMut(&str, bool) -> Result<Option<WorkflowDetail>, E>,
    ) -> Result<WorkflowResult, E> {
        self.begin_using(target, Some(witness), entropy, lookup)
    }
    fn begin_using<E: From<StoreError>>(
        &self,
        target: Target,
        witness: Option<GenerationWitness>,
        mut random: impl FnMut(&mut [u8]) -> Result<(), StoreError>,
        mut lookup: impl FnMut(&str, bool) -> Result<Option<WorkflowDetail>, E>,
    ) -> Result<WorkflowResult, E> {
        let (workflow_id, parent_id, opened) = match target {
            Target::New {} => {
                let mut nonce = [0; 32];
                random(&mut nonce)?;
                let id = workflow_id_from_nonce(&nonce);
                if lookup(&id, true)?.is_some() {
                    return Err(StoreError::WorkflowConflict.into());
                }
                (id, None, None)
            }
            Target::Append {
                workflow_id,
                expected_head_id,
            } => {
                let detail = lookup(&workflow_id, false)?.ok_or(StoreError::WorkflowNotFound)?;
                if detail.head.latest_revision_id != expected_head_id {
                    return Err(StoreError::WorkflowConflict.into());
                }
                (workflow_id, Some(expected_head_id), Some(Box::new(detail)))
            }
        };
        let id = handle("workflow-draft:", &mut random)?;
        let result = WorkflowResult::DraftReady {
            schema_version: "rangoon.workflow-draft-session.v1",
            draft_id: id.clone(),
            workflow_id: workflow_id.clone(),
            parent_revision_id: parent_id.clone(),
            workflow: opened,
        };
        if witness
            .as_ref()
            .is_some_and(|w| !w.is_current().unwrap_or(false))
        {
            self.purge();
            return Err(StoreError::WorkspaceChanged.into());
        }
        *self.0.lock().map_err(|_| StoreError::Unavailable)? = AuthoringState {
            witness,
            draft: Some(DraftContext {
                id,
                workflow_id,
                parent_id,
            }),
            prepared: None,
        };
        Ok(result)
    }
    fn check_bound(state: &mut AuthoringState) -> Result<(), StoreError> {
        if state
            .witness
            .as_ref()
            .is_some_and(|w| !w.is_current().unwrap_or(false))
        {
            *state = AuthoringState::default();
            return Err(StoreError::WorkspaceChanged);
        }
        Ok(())
    }
    pub(super) fn purge(&self) -> bool {
        let (mut guard, healthy) = match self.0.lock() {
            Ok(guard) => (guard, true),
            Err(error) => (error.into_inner(), false),
        };
        *guard = AuthoringState::default();
        healthy
    }
    fn context(&self, id: &str) -> Result<DraftContext, StoreError> {
        let mut guard = self.0.lock().map_err(|_| StoreError::Unavailable)?;
        Self::check_bound(&mut guard)?;
        guard
            .draft
            .as_ref()
            .filter(|draft| draft.id == id)
            .cloned()
            .ok_or(StoreError::WorkspaceChanged)
    }
    #[cfg(test)]
    fn inspect(
        &self,
        store: &Workspace,
        request: InspectRequest,
    ) -> Result<WorkflowResult, StoreError> {
        self.inspect_using(request, |candidate| store.inspect_workflow_save(candidate))
    }
    pub(super) fn inspect_using<E: From<StoreError>>(
        &self,
        request: InspectRequest,
        inspect: impl FnOnce(&WorkflowRevision) -> Result<WorkflowSavePlan, E>,
    ) -> Result<WorkflowResult, E> {
        let draft = self.context(&request.draft_id)?;
        let candidate = prepare_revision(
            &draft.workflow_id,
            draft.parent_id.as_deref(),
            request.intent,
            request.definition.get().as_bytes(),
            request.layout.get().as_bytes(),
        )
        .map_err(|_| StoreError::WorkflowInvalid)?;
        let plan = inspect(&candidate)?;
        self.stage(&draft, candidate, plan, entropy)
            .map_err(E::from)
    }
    fn stage(
        &self,
        draft: &DraftContext,
        candidate: WorkflowRevision,
        plan: WorkflowSavePlan,
        mut random: impl FnMut(&mut [u8]) -> Result<(), StoreError>,
    ) -> Result<WorkflowResult, StoreError> {
        let expected_head = if plan.already_saved {
            Some(candidate.id())
        } else {
            draft.parent_id.as_deref()
        };
        if candidate.workflow_id() != draft.workflow_id
            || candidate.parent_revision_id() != draft.parent_id.as_deref()
            || plan.workflow_id != candidate.workflow_id()
            || plan.revision_id != candidate.id()
            || plan.expected_head_id.as_deref() != expected_head
        {
            return Err(StoreError::WorkflowInvalid);
        }
        let id = handle("workflow-preview:", &mut random)?;
        let result = WorkflowResult::SaveReady {
            schema_version: "rangoon.workflow-save-session.v1",
            draft_id: draft.id.clone(),
            preview_id: id.clone(),
            report: Box::new(candidate.inspection().report().clone()),
            revision: Box::new(candidate.clone()),
            plan: plan.clone(),
        };
        let mut guard = self.0.lock().map_err(|_| StoreError::Unavailable)?;
        Self::check_bound(&mut guard)?;
        if guard
            .draft
            .as_ref()
            .is_none_or(|current| current.id != draft.id)
        {
            return Err(StoreError::WorkspaceChanged);
        }
        guard.prepared = Some(IssuedSave {
            id,
            candidate,
            plan,
        });
        Ok(result)
    }
    pub(super) fn claim(&self, confirmation: &Confirmation) -> Result<IssuedSave, StoreError> {
        if !confirmation.acknowledged {
            return Err(StoreError::WorkflowInvalid);
        }
        let mut guard = self.0.lock().map_err(|_| StoreError::Unavailable)?;
        Self::check_bound(&mut guard)?;
        let prepared = guard
            .prepared
            .as_ref()
            .ok_or(StoreError::WorkspaceChanged)?;
        if prepared.id != confirmation.preview_id
            || prepared.plan.expected_state_id != confirmation.expected_state_id
        {
            return Err(StoreError::WorkspaceChanged);
        }
        // Consume before reinspection or writing. An uncertain return cannot replay a save.
        guard.prepared.take().ok_or(StoreError::WorkspaceChanged)
    }
    #[cfg(test)]
    fn commit(
        &self,
        store: &Workspace,
        confirmation: &Confirmation,
    ) -> Result<WorkflowSaveReceipt, StoreError> {
        self.commit_with(confirmation, |prepared| {
            let current = store.inspect_workflow_save(&prepared.candidate)?;
            prepared.check_current(&current)?;
            store.save_workflow(&prepared.candidate, &prepared.plan.expected_state_id)
        })
    }
    #[cfg(test)]
    fn commit_with(
        &self,
        confirmation: &Confirmation,
        action: impl FnOnce(&IssuedSave) -> Result<WorkflowSaveReceipt, StoreError>,
    ) -> Result<WorkflowSaveReceipt, StoreError> {
        let prepared = self.claim(confirmation)?;
        action(&prepared)
    }
    pub(super) fn clear(&self, id: &str) -> Result<WorkflowResult, StoreError> {
        let mut guard = self.0.lock().map_err(|_| StoreError::Unavailable)?;
        Self::check_bound(&mut guard)?;
        if guard.draft.as_ref().is_none_or(|draft| draft.id != id) {
            return Err(StoreError::WorkspaceChanged);
        }
        *guard = AuthoringState::default();
        Ok(WorkflowResult::Cleared)
    }
}

impl IssuedSave {
    pub(super) fn check_current(&self, current: &WorkflowSavePlan) -> Result<(), StoreError> {
        let exact_noop = current.schema_version == self.plan.schema_version
            && current.workflow_id == self.candidate.workflow_id()
            && current.revision_id == self.candidate.id()
            && current.already_saved
            && current.expected_head_id.as_deref() == Some(self.candidate.id())
            && current.unresolved_references == self.plan.unresolved_references;
        if *current != self.plan && !exact_noop {
            return Err(StoreError::WorkspaceChanged);
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "outcome",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum WorkflowResult {
    Listed {
        workflows: Vec<WorkflowSummary>,
    },
    Opened {
        workflow: Box<WorkflowDetail>,
    },
    DraftReady {
        schema_version: &'static str,
        draft_id: String,
        workflow_id: String,
        parent_revision_id: Option<String>,
        workflow: Option<Box<WorkflowDetail>>,
    },
    SaveReady {
        schema_version: &'static str,
        draft_id: String,
        preview_id: String,
        revision: Box<WorkflowRevision>,
        report: Box<ValidationReport>,
        plan: WorkflowSavePlan,
    },
    Saved {
        receipt: Box<WorkflowSaveReceipt>,
    },
    Cleared,
    Failed {
        error: PublicError,
    },
}
pub(super) fn failure(error: StoreError) -> WorkflowResult {
    let (code, message) = if error == StoreError::Unavailable {
        (
            "workspace_unavailable",
            "The workflow operation did not return a confirmed result. Keep your draft, refresh saved workflows and inspect a fresh preview before saving again.",
        )
    } else {
        error.public()
    };
    WorkflowResult::Failed {
        error: PublicError { code, message },
    }
}
#[tauri::command]
pub async fn list_workflows(app: AppHandle) -> WorkflowResult {
    let access = app.state::<SourceAccess>().inner().clone();
    access.list_workflows().await
}
#[tauri::command]
pub async fn open_workflow(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
) -> Result<WorkflowResult, ()> {
    let value = match decode_open(request.body()) {
        Ok(value) => value,
        Err(error) => return Ok(failure(error)),
    };
    let access = app.state::<SourceAccess>().inner().clone();
    Ok(access.open_workflow(value).await)
}
#[tauri::command]
pub async fn begin_workflow_draft(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
) -> Result<WorkflowResult, ()> {
    let value = match decode_begin(request.body()) {
        Ok(value) => value,
        Err(error) => return Ok(failure(error)),
    };
    let access = app.state::<SourceAccess>().inner().clone();
    Ok(access.begin_workflow_draft(value).await)
}
#[tauri::command]
pub async fn inspect_workflow_save(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
) -> Result<WorkflowResult, ()> {
    let value = match decode_inspect(request.body()) {
        Ok(value) => value,
        Err(error) => return Ok(failure(error)),
    };
    let access = app.state::<SourceAccess>().inner().clone();
    Ok(access.inspect_workflow_save(value).await)
}
#[tauri::command]
pub async fn commit_workflow_save(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
) -> Result<WorkflowResult, ()> {
    let value = match decode_commit(request.body()) {
        Ok(value) => value,
        Err(error) => return Ok(failure(error)),
    };
    let access = app.state::<SourceAccess>().inner().clone();
    Ok(access.commit_workflow_save(value).await)
}
#[tauri::command]
pub async fn clear_workflow_draft(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
) -> Result<WorkflowResult, ()> {
    let value = match decode_clear(request.body()) {
        Ok(value) => value,
        Err(error) => return Ok(failure(error)),
    };
    let access = app.state::<SourceAccess>().inner().clone();
    Ok(access.clear_workflow_draft(value).await)
}
#[cfg(test)]
#[path = "workflow_tests.rs"]
mod tests;
