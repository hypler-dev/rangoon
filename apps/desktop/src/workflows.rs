//! Native immutable authoring custody. Definitions remain inert local data.
use super::{
    AdmissionError, AppHandle, Gate, PublicError, StoreError, Workspace, begin_operation,
    dispatch_read, workspace,
};
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
struct OpenRequest {
    schema_version: String,
    workflow_id: String,
    selection: Selection,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Selection {
    Head {},
    Historical {
        #[serde(rename = "revisionId")]
        revision_id: String,
    },
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BeginRequest {
    schema_version: String,
    target: Target,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Target {
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
struct InspectRequest {
    schema_version: String,
    draft_id: String,
    intent: SaveIntent,
    definition: Box<RawValue>,
    layout: Box<RawValue>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Confirmation {
    schema_version: String,
    preview_id: String,
    expected_state_id: String,
    acknowledged: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ClearRequest {
    schema_version: String,
    draft_id: String,
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
fn decode_open(body: &InvokeBody) -> Result<OpenRequest, StoreError> {
    let value: OpenRequest = raw(body, SMALL_LIMIT)?;
    if value.schema_version != "rangoon.workflow-open.v1"
        || !valid_id(&value.workflow_id, "workflow:")
        || matches!(&value.selection, Selection::Historical { revision_id } if !valid_id(revision_id, "workflow-revision:"))
    {
        return Err(StoreError::WorkflowInvalid);
    }
    Ok(value)
}
fn decode_begin(body: &InvokeBody) -> Result<BeginRequest, StoreError> {
    let value: BeginRequest = raw(body, SMALL_LIMIT)?;
    if value.schema_version != "rangoon.workflow-draft.v1"
        || matches!(&value.target, Target::Append {workflow_id, expected_head_id} if !valid_id(workflow_id, "workflow:") || !valid_id(expected_head_id, "workflow-revision:"))
    {
        return Err(StoreError::WorkflowInvalid);
    }
    Ok(value)
}
fn decode_inspect(body: &InvokeBody) -> Result<InspectRequest, StoreError> {
    let value: InspectRequest = raw(body, INSPECT_LIMIT)?;
    if value.schema_version != "rangoon.workflow-inspect.v1"
        || !valid_id(&value.draft_id, "workflow-draft:")
    {
        return Err(StoreError::WorkflowInvalid);
    }
    Ok(value)
}
fn decode_commit(body: &InvokeBody) -> Result<Confirmation, StoreError> {
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
fn decode_clear(body: &InvokeBody) -> Result<ClearRequest, StoreError> {
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
struct IssuedSave {
    id: String,
    candidate: WorkflowRevision,
    plan: WorkflowSavePlan,
}
#[derive(Default)]
struct AuthoringState {
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
    fn begin(&self, store: &Workspace, target: Target) -> Result<WorkflowResult, StoreError> {
        self.begin_with(store, target, entropy)
    }
    fn begin_with(
        &self,
        store: &Workspace,
        target: Target,
        mut random: impl FnMut(&mut [u8]) -> Result<(), StoreError>,
    ) -> Result<WorkflowResult, StoreError> {
        let (workflow_id, parent_id, opened) = match target {
            Target::New {} => {
                let mut nonce = [0; 32];
                random(&mut nonce)?;
                let id = workflow_id_from_nonce(&nonce);
                // Validate the complete workspace and refuse an improbable occupied identity.
                match store.open_workflow(&id, None) {
                    Err(StoreError::WorkflowNotFound) => (),
                    Ok(_) => return Err(StoreError::WorkflowConflict),
                    Err(error) => return Err(error),
                }
                (id, None, None)
            }
            Target::Append {
                workflow_id,
                expected_head_id,
            } => {
                let detail = store.open_workflow(&workflow_id, None)?;
                if detail.head.latest_revision_id != expected_head_id {
                    return Err(StoreError::WorkflowConflict);
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
        *self.0.lock().map_err(|_| StoreError::Unavailable)? = AuthoringState {
            draft: Some(DraftContext {
                id,
                workflow_id,
                parent_id,
            }),
            prepared: None,
        };
        Ok(result)
    }
    fn context(&self, id: &str) -> Result<DraftContext, StoreError> {
        self.0
            .lock()
            .map_err(|_| StoreError::Unavailable)?
            .draft
            .as_ref()
            .filter(|draft| draft.id == id)
            .cloned()
            .ok_or(StoreError::WorkspaceChanged)
    }
    fn inspect(
        &self,
        store: &Workspace,
        request: InspectRequest,
    ) -> Result<WorkflowResult, StoreError> {
        let draft = self.context(&request.draft_id)?;
        let candidate = prepare_revision(
            &draft.workflow_id,
            draft.parent_id.as_deref(),
            request.intent,
            request.definition.get().as_bytes(),
            request.layout.get().as_bytes(),
        )
        .map_err(|_| StoreError::WorkflowInvalid)?;
        let plan = store.inspect_workflow_save(&candidate)?;
        self.stage(&draft, candidate, plan, entropy)
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
    fn claim(&self, confirmation: &Confirmation) -> Result<IssuedSave, StoreError> {
        if !confirmation.acknowledged {
            return Err(StoreError::WorkflowInvalid);
        }
        let mut guard = self.0.lock().map_err(|_| StoreError::Unavailable)?;
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
    fn commit(
        &self,
        store: &Workspace,
        confirmation: &Confirmation,
    ) -> Result<WorkflowSaveReceipt, StoreError> {
        self.commit_with(confirmation, |prepared| {
            let current = store.inspect_workflow_save(&prepared.candidate)?;
            let exact_noop = current.schema_version == prepared.plan.schema_version
                && current.workflow_id == prepared.candidate.workflow_id()
                && current.revision_id == prepared.candidate.id()
                && current.already_saved
                && current.expected_head_id.as_deref() == Some(prepared.candidate.id())
                && current.unresolved_references == prepared.plan.unresolved_references;
            if current != prepared.plan && !exact_noop {
                return Err(StoreError::WorkspaceChanged);
            }
            store.save_workflow(&prepared.candidate, &prepared.plan.expected_state_id)
        })
    }
    fn commit_with(
        &self,
        confirmation: &Confirmation,
        action: impl FnOnce(&IssuedSave) -> Result<WorkflowSaveReceipt, StoreError>,
    ) -> Result<WorkflowSaveReceipt, StoreError> {
        let prepared = self.claim(confirmation)?;
        action(&prepared)
    }
    fn clear(&self, id: &str) -> Result<WorkflowResult, StoreError> {
        let mut guard = self.0.lock().map_err(|_| StoreError::Unavailable)?;
        if guard.draft.as_ref().is_none_or(|draft| draft.id != id) {
            return Err(StoreError::WorkspaceChanged);
        }
        *guard = AuthoringState::default();
        Ok(WorkflowResult::Cleared)
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
fn failure(error: StoreError) -> WorkflowResult {
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
async fn guarded(
    app: AppHandle,
    action: impl FnOnce(AppHandle) -> Result<WorkflowResult, StoreError> + Send + 'static,
) -> WorkflowResult {
    let pending = match begin_operation(&app) {
        Ok(lease) => lease,
        Err(AdmissionError::Busy) => {
            return WorkflowResult::Failed {
                error: PublicError {
                    code: "workspace_busy",
                    message: "Finish the current local workspace operation first. Your workflow draft remains available.",
                },
            };
        }
        Err(AdmissionError::Unavailable) => {
            return failure(StoreError::Unavailable);
        }
    };
    tauri::async_runtime::spawn_blocking(move || {
        let _pending = pending;
        action(app).unwrap_or_else(failure)
    })
    .await
    .unwrap_or_else(|_| failure(StoreError::Unavailable))
}
#[tauri::command]
pub async fn list_workflows(app: AppHandle) -> WorkflowResult {
    let gate = app.state::<Gate>().inner().clone();
    let worker = match dispatch_read(&gate, move || {
        workspace(&app)
            .and_then(|store| store.list_workflows())
            .map(|workflows| WorkflowResult::Listed { workflows })
            .unwrap_or_else(failure)
    }) {
        Ok(worker) => worker,
        Err(AdmissionError::Busy) => {
            return WorkflowResult::Failed {
                error: PublicError {
                    code: "workspace_busy",
                    message: "Finish the current local workspace operation first.",
                },
            };
        }
        Err(AdmissionError::Unavailable) => return failure(StoreError::Unavailable),
    };
    worker
        .await
        .unwrap_or_else(|_| failure(StoreError::Unavailable))
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
    let gate = app.state::<Gate>().inner().clone();
    let worker = match dispatch_read(&gate, move || {
        let selected = match &value.selection {
            Selection::Head {} => None,
            Selection::Historical { revision_id } => Some(revision_id.as_str()),
        };
        workspace(&app)
            .and_then(|store| store.open_workflow(&value.workflow_id, selected))
            .map(|workflow| WorkflowResult::Opened {
                workflow: Box::new(workflow),
            })
            .unwrap_or_else(failure)
    }) {
        Ok(worker) => worker,
        Err(AdmissionError::Busy) => {
            return Ok(WorkflowResult::Failed {
                error: PublicError {
                    code: "workspace_busy",
                    message: "Finish the current local workspace operation first.",
                },
            });
        }
        Err(AdmissionError::Unavailable) => return Ok(failure(StoreError::Unavailable)),
    };
    Ok(worker
        .await
        .unwrap_or_else(|_| failure(StoreError::Unavailable)))
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
    Ok(guarded(app, move |app| {
        app.state::<WorkflowSession>()
            .begin(&workspace(&app)?, value.target)
    })
    .await)
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
    Ok(guarded(app, move |app| {
        app.state::<WorkflowSession>()
            .inspect(&workspace(&app)?, value)
    })
    .await)
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
    Ok(guarded(app, move |app| {
        app.state::<WorkflowSession>()
            .commit(&workspace(&app)?, &value)
            .map(|receipt| WorkflowResult::Saved {
                receipt: Box::new(receipt),
            })
    })
    .await)
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
    Ok(guarded(app, move |app| {
        app.state::<WorkflowSession>().clear(&value.draft_id)
    })
    .await)
}
#[cfg(test)]
#[path = "workflow_tests.rs"]
mod tests;
