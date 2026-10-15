use super::*;

use crate::workflows::{
    BeginRequest, ClearRequest, Confirmation, InspectRequest, OpenRequest, Selection, Target,
    WorkflowResult, decode_begin, decode_clear, decode_commit, decode_inspect, decode_open,
};
use rangoon_workflow::records::SaveIntent;
use serde_json::value::RawValue;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};

fn workflow_code(result: WorkflowResult) -> &'static str {
    match result {
        WorkflowResult::Failed { error } => error.code,
        _ => panic!("expected failed workflow result"),
    }
}

fn definition(title: &str) -> Box<RawValue> {
    RawValue::from_string(
        serde_json::json!({
            "schemaVersion": "rangoon.workflow-definition.v1",
            "title": title,
            "nodes": [],
            "controlEdges": [],
            "dataEdges": []
        })
        .to_string(),
    )
    .unwrap()
}

fn definition_with_capability(
    title: &str,
    capability_id: &str,
    revision_id: &str,
) -> Box<RawValue> {
    RawValue::from_string(
        serde_json::json!({
            "schemaVersion": "rangoon.workflow-definition.v1",
            "title": title,
            "nodes": [
                {"id":"start","title":"Start","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"text","dataType":"text"}]},
                {"id":"skill","title":"Pinned skill","operation":{"kind":"capability","capabilityId":capability_id,"revisionId":revision_id},"inputs":[{"name":"text","dataType":"text"}],"outputs":[{"name":"text","dataType":"text"}]},
                {"id":"finish","title":"Finish","operation":{"kind":"output"},"inputs":[{"name":"text","dataType":"text"}],"outputs":[]}
            ],
            "controlEdges":[{"fromNode":"start","outlet":"next","toNode":"skill"},{"fromNode":"skill","outlet":"next","toNode":"finish"}],
            "dataEdges":[{"fromNode":"start","fromPort":"text","toNode":"skill","toPort":"text"},{"fromNode":"skill","fromPort":"text","toNode":"finish","toPort":"text"}]
        }).to_string(),
    ).unwrap()
}

fn layout() -> Box<RawValue> {
    RawValue::from_string(r#"{"positions":[]}"#.into()).unwrap()
}

fn new_draft(access: &SourceAccess) -> (String, String) {
    match tauri::async_runtime::block_on(access.begin_workflow_draft(BeginRequest {
        schema_version: "rangoon.workflow-draft.v1".into(),
        target: Target::New {},
    })) {
        WorkflowResult::DraftReady {
            draft_id,
            workflow_id,
            workflow,
            ..
        } => {
            assert!(workflow.is_none());
            (draft_id, workflow_id)
        }
        _ => panic!("expected workflow draft"),
    }
}

fn inspect(access: &SourceAccess, draft_id: &str, title: &str) -> (String, String, String) {
    inspect_intent(access, draft_id, title, SaveIntent::Draft)
}

fn inspect_intent(
    access: &SourceAccess,
    draft_id: &str,
    title: &str,
    intent: SaveIntent,
) -> (String, String, String) {
    match tauri::async_runtime::block_on(access.inspect_workflow_save(InspectRequest {
        schema_version: "rangoon.workflow-inspect.v1".into(),
        draft_id: draft_id.into(),
        intent,
        definition: definition(title),
        layout: layout(),
    })) {
        WorkflowResult::SaveReady {
            draft_id,
            preview_id,
            plan,
            ..
        } => (draft_id, preview_id, plan.expected_state_id),
        _ => panic!("expected workflow save preview"),
    }
}

fn commit(access: &SourceAccess, preview_id: &str, expected_state_id: &str) -> WorkflowResult {
    tauri::async_runtime::block_on(access.commit_workflow_save(Confirmation {
        schema_version: "rangoon.workflow-commit.v1".into(),
        preview_id: preview_id.into(),
        expected_state_id: expected_state_id.into(),
        acknowledged: true,
    }))
}

#[test]
fn legacy_workflow_routing_keeps_inventory_history_exact_bytes_and_source_clear_independent() {
    let root = TestRoot::new("workflow-routing-legacy");
    let source = report(
        "workflow-source.md",
        b"# Workflow source\r\nExact bytes \xF0\x9F\xA6\x80\r\n",
    );
    Workspace::new(root.0.clone()).save_v1(&source).unwrap();
    let access = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));

    match tauri::async_runtime::block_on(access.workflow_data()) {
        DataResult::Loaded { workspace } => {
            assert_eq!(workspace.sources.len(), 1);
            assert!(workspace.workflows.is_empty());
        }
        _ => panic!("expected workflow inventory"),
    }
    let (draft_id, workflow_id) = new_draft(&access);
    let (_, preview_id, expected_state_id) =
        inspect(&access, &draft_id, "Exact workflow \u{1f980}");
    assert!(matches!(
        commit(&access, &preview_id, &expected_state_id),
        WorkflowResult::Saved { .. }
    ));
    select(&access, &source);
    assert!(matches!(access.clear_analysis(), WorkspaceResult::Cleared));

    let head = match tauri::async_runtime::block_on(access.open_workflow(OpenRequest {
        schema_version: "rangoon.workflow-open.v1".into(),
        workflow_id: workflow_id.clone(),
        selection: Selection::Head {},
    })) {
        WorkflowResult::Opened { workflow } => *workflow,
        _ => panic!("expected workflow head"),
    };
    assert_eq!(
        head.revision.serialized_bytes().unwrap(),
        Workspace::new(root.0.clone())
            .open_workflow(&workflow_id, None)
            .unwrap()
            .revision
            .serialized_bytes()
            .unwrap()
    );
    assert_eq!(head.history.len(), 1);
    let prior_revision_id = head.head.latest_revision_id.clone();
    let prior_bytes = head.revision.serialized_bytes().unwrap();
    let append_draft =
        match tauri::async_runtime::block_on(access.begin_workflow_draft(BeginRequest {
            schema_version: "rangoon.workflow-draft.v1".into(),
            target: Target::Append {
                workflow_id: workflow_id.clone(),
                expected_head_id: prior_revision_id.clone(),
            },
        })) {
            WorkflowResult::DraftReady {
                draft_id, workflow, ..
            } => {
                assert!(workflow.is_some());
                draft_id
            }
            _ => panic!("expected append draft"),
        };
    let (_, append_preview, append_state) = inspect(&access, &append_draft, "Exact workflow child");
    assert!(matches!(
        commit(&access, &append_preview, &append_state),
        WorkflowResult::Saved { .. }
    ));
    match tauri::async_runtime::block_on(access.open_workflow(OpenRequest {
        schema_version: "rangoon.workflow-open.v1".into(),
        workflow_id: workflow_id.clone(),
        selection: Selection::Historical {
            revision_id: prior_revision_id,
        },
    })) {
        WorkflowResult::Opened { workflow } => {
            assert_eq!(workflow.revision.serialized_bytes().unwrap(), prior_bytes);
            assert_eq!(workflow.history.len(), 2);
        }
        _ => panic!("expected workflow history"),
    }
    match tauri::async_runtime::block_on(access.list_workflows()) {
        WorkflowResult::Listed { workflows } => {
            assert_eq!(workflows.len(), 1);
            assert_eq!(workflows[0].id, workflow_id);
        }
        _ => panic!("expected workflow list"),
    }
}

#[test]
fn workflow_confirmations_are_exact_one_shot_and_preview_replacement_keeps_newest_plan() {
    let root = TestRoot::new("workflow-confirmations");
    let access = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));
    let (draft_id, _) = new_draft(&access);
    let (_, old_preview, old_state) = inspect(&access, &draft_id, "Older preview");
    let (_, preview, state) = inspect(&access, &draft_id, "Newest preview");

    assert_eq!(
        workflow_code(commit(&access, &old_preview, &old_state)),
        "workspace_changed"
    );
    assert_eq!(
        workflow_code(commit(&access, "workflow-preview:bad", &state)),
        "workspace_changed"
    );
    assert!(matches!(
        commit(&access, &preview, &state),
        WorkflowResult::Saved { .. }
    ));
    assert_eq!(
        workflow_code(commit(&access, &preview, &state)),
        "workspace_changed"
    );
}

#[test]
fn workflow_changed_workspace_state_refuses_stale_preview_and_consumes_it() {
    let root = TestRoot::new("workflow-stale-state");
    let access = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));
    let (draft_id, _) = new_draft(&access);
    let (_, preview, state) = inspect(&access, &draft_id, "Stale workspace preview");
    let intervening = report("intervening.md", b"# State changed\n");
    Workspace::new(root.0.clone())
        .save_v1(&intervening)
        .unwrap();
    assert_eq!(
        workflow_code(commit(&access, &preview, &state)),
        "workspace_changed"
    );
    assert_eq!(
        workflow_code(commit(&access, &preview, &state)),
        "workspace_changed"
    );
    assert!(
        Workspace::new(root.0.clone())
            .list_workflows()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn workflow_append_rejects_stale_head_without_changing_current_history() {
    let root = TestRoot::new("workflow-stale-head");
    let access = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));
    let (draft_id, workflow_id) = new_draft(&access);
    let (_, preview, state) = inspect(&access, &draft_id, "Root workflow");
    assert!(matches!(
        commit(&access, &preview, &state),
        WorkflowResult::Saved { .. }
    ));
    let root_detail = Workspace::new(root.0.clone())
        .open_workflow(&workflow_id, None)
        .unwrap();
    let old_head = root_detail.head.latest_revision_id.clone();
    let other = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));
    let append_draft =
        match tauri::async_runtime::block_on(other.begin_workflow_draft(BeginRequest {
            schema_version: "rangoon.workflow-draft.v1".into(),
            target: Target::Append {
                workflow_id: workflow_id.clone(),
                expected_head_id: old_head.clone(),
            },
        })) {
            WorkflowResult::DraftReady { draft_id, .. } => draft_id,
            _ => panic!("expected current append draft"),
        };
    let (_, preview, state) = inspect(&other, &append_draft, "Current child");
    assert!(matches!(
        commit(&other, &preview, &state),
        WorkflowResult::Saved { .. }
    ));
    let before = Workspace::new(root.0.clone())
        .open_workflow(&workflow_id, None)
        .unwrap();
    assert_eq!(
        workflow_code(tauri::async_runtime::block_on(access.begin_workflow_draft(
            BeginRequest {
                schema_version: "rangoon.workflow-draft.v1".into(),
                target: Target::Append {
                    workflow_id: workflow_id.clone(),
                    expected_head_id: old_head
                },
            }
        ))),
        "workflow_conflict"
    );
    assert_eq!(
        Workspace::new(root.0.clone())
            .open_workflow(&workflow_id, None)
            .unwrap(),
        before
    );
}

#[test]
fn workflow_exact_already_saved_noop_preserves_history_after_out_of_band_commit() {
    let root = TestRoot::new("workflow-exact-noop");
    let access = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));
    let (draft_id, workflow_id) = new_draft(&access);
    let (preview, plan, candidate) =
        match tauri::async_runtime::block_on(access.inspect_workflow_save(InspectRequest {
            schema_version: "rangoon.workflow-inspect.v1".into(),
            draft_id,
            intent: SaveIntent::Draft,
            definition: definition("Exact no-op"),
            layout: layout(),
        })) {
            WorkflowResult::SaveReady {
                preview_id,
                plan,
                revision,
                ..
            } => (preview_id, plan, *revision),
            _ => panic!("expected exact no-op preview"),
        };
    let direct = Workspace::new(root.0.clone())
        .save_workflow(&candidate, &plan.expected_state_id)
        .unwrap();
    assert!(!direct.already_saved);
    let before = Workspace::new(root.0.clone())
        .open_workflow(&workflow_id, None)
        .unwrap();
    match commit(&access, &preview, &plan.expected_state_id) {
        WorkflowResult::Saved { receipt } => assert!(receipt.already_saved),
        _ => panic!("expected native exact no-op"),
    }
    let after = Workspace::new(root.0.clone())
        .open_workflow(&workflow_id, None)
        .unwrap();
    assert_eq!(after, before);
    assert_eq!(after.history.len(), 1);
    assert_eq!(
        after.revision.serialized_bytes().unwrap(),
        candidate.serialized_bytes().unwrap()
    );
}

#[test]
fn routed_source_skill_validated_workflow_keeps_pinned_reference_and_inert_authority() {
    let root = TestRoot::new("workflow-validated-reference");
    let access = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));
    let source = report("validated.md", b"# Routed source\nExact skill input\n");
    select(&access, &source);
    assert!(matches!(
        tauri::async_runtime::block_on(access.save_analysis(source.source.id.clone())),
        WorkspaceResult::Saved { .. }
    ));
    let capability = match tauri::async_runtime::block_on(access.create_capability(
        source.source.id.clone(),
        source.fragments[0].id.clone(),
        "Pinned workflow skill".into(),
    )) {
        CapabilityResult::Opened { capability, .. } => *capability,
        _ => panic!("expected routed skill"),
    };
    let (draft_id, workflow_id) = new_draft(&access);
    let (preview, state) =
        match tauri::async_runtime::block_on(access.inspect_workflow_save(InspectRequest {
            schema_version: "rangoon.workflow-inspect.v1".into(),
            draft_id,
            intent: SaveIntent::Validated,
            definition: definition_with_capability(
                "Validated routed workflow",
                &capability.id,
                &capability.latest_revision_id,
            ),
            layout: layout(),
        })) {
            WorkflowResult::SaveReady {
                preview_id, plan, ..
            } => {
                assert_eq!(plan.unresolved_references, 0);
                (preview_id, plan.expected_state_id)
            }
            _ => panic!("expected resolved validated preview"),
        };
    assert!(matches!(
        commit(&access, &preview, &state),
        WorkflowResult::Saved { .. }
    ));
    match tauri::async_runtime::block_on(access.open_workflow(OpenRequest {
        schema_version: "rangoon.workflow-open.v1".into(),
        workflow_id,
        selection: Selection::Head {},
    })) {
        WorkflowResult::Opened { workflow } => {
            assert_eq!(workflow.head.intent, SaveIntent::Validated);
            assert_eq!(workflow.references.len(), 1);
            assert_eq!(workflow.references[0].capability_id, capability.id);
            assert_eq!(
                workflow.references[0].revision_id,
                capability.latest_revision_id
            );
            let report = serde_json::to_value(workflow.revision.inspection().report()).unwrap();
            assert_eq!(report["authority"], "none");
            assert_eq!(report["executionStatus"], "unavailable");
        }
        _ => panic!("expected validated workflow reopen"),
    }
}

#[test]
fn workflow_clear_purge_and_cross_instance_handles_do_not_reopen_or_change_inventory() {
    let root = TestRoot::new("workflow-clear-purge");
    let gate = Gate::new();
    let access = SourceAccess::legacy(gate.clone(), Ok(root.0.clone()));
    let other = SourceAccess::legacy(gate.clone(), Ok(root.0.clone()));
    let (draft_id, _) = new_draft(&access);
    let (_, preview, state) = inspect(&access, &draft_id, "Discarded draft");
    assert!(access.purge_workflow_authoring());
    assert_eq!(
        workflow_code(commit(&access, &preview, &state)),
        "workspace_changed"
    );
    assert_eq!(
        workflow_code(tauri::async_runtime::block_on(other.clear_workflow_draft(
            ClearRequest {
                schema_version: "rangoon.workflow-clear.v1".into(),
                draft_id,
            }
        ))),
        "workspace_changed"
    );
    match tauri::async_runtime::block_on(access.workflow_data()) {
        DataResult::Loaded { workspace } => assert!(workspace.workflows.is_empty()),
        _ => panic!("expected unchanged inventory"),
    }
    let drain = gate.request_drain().unwrap();
    assert!(drain.is_quiescent().unwrap());
    gate.resume(&drain).unwrap();
    assert!(matches!(
        tauri::async_runtime::block_on(access.list_workflows()),
        WorkflowResult::Listed { .. }
    ));
}

#[test]
fn workflow_ticket_refuses_stale_work_and_witness_is_nonowning_generation_guard() {
    let root = TestRoot::new("workflow-stale");
    let gate = Gate::new();
    let access = SourceAccess::legacy(gate.clone(), Ok(root.0.clone()));
    let lease = gate.begin(Lane::Exclusive).unwrap();
    let witness = lease.witness();
    assert!(witness.is_current().unwrap());
    drop(lease);
    let ticket = access.begin_capability(Lane::Read).unwrap();
    let drain = gate.request_drain().unwrap();
    let completion = ticket.workflow(WorkflowOperation::List);
    assert_eq!(workflow_code(completion.deliver()), "workspace_unavailable");
    assert!(drain.is_quiescent().unwrap());
    assert!(!witness.is_current().unwrap());
}

#[test]
fn workflow_malformed_requests_are_bounded_and_leave_legacy_inventory_empty() {
    let root = TestRoot::new("workflow-malformed");
    let access = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));
    assert_eq!(
        workflow_code(tauri::async_runtime::block_on(access.open_workflow(
            OpenRequest {
                schema_version: "rangoon.workflow-open.v1".into(),
                workflow_id: "workflow:not-canonical".into(),
                selection: Selection::Head {},
            }
        ))),
        "workflow_invalid"
    );
    assert_eq!(
        workflow_code(tauri::async_runtime::block_on(access.clear_workflow_draft(
            ClearRequest {
                schema_version: "rangoon.workflow-clear.v1".into(),
                draft_id: "workflow-draft:bad".into(),
            }
        ))),
        "workspace_changed"
    );
    match tauri::async_runtime::block_on(access.list_workflows()) {
        WorkflowResult::Listed { workflows } => assert!(workflows.is_empty()),
        _ => panic!("expected no malformed write"),
    }
}

#[test]
fn workflow_raw_decoders_and_native_forwarder_bindings_stay_exact() {
    use tauri::ipc::InvokeBody;

    let draft = format!("workflow-draft:{}", "a".repeat(64));
    let preview = format!("workflow-preview:{}", "b".repeat(64));
    let workspace = format!("workspace:{}", "c".repeat(64));
    let workflow = format!("workflow:{}", "d".repeat(64));
    let begin = r#"{"schemaVersion":"rangoon.workflow-draft.v1","target":{"kind":"new"}}"#;
    let padded_begin = format!("{begin}{}", " ".repeat(1024 - begin.len()));
    assert!(decode_begin(&InvokeBody::Raw(padded_begin.into_bytes())).is_ok());
    let oversized_begin = format!("{begin}{}", " ".repeat(1025 - begin.len()));
    assert!(matches!(
        decode_begin(&InvokeBody::Raw(oversized_begin.into_bytes())),
        Err(StoreError::WorkflowInvalid)
    ));
    for body in [
        br#"{}"#.to_vec(),
        br#"{"schemaVersion":"wrong","target":{"kind":"new"}}"#.to_vec(),
        br#"{"schemaVersion":"rangoon.workflow-draft.v1","target":{"kind":"new"},"extra":true}"#
            .to_vec(),
    ] {
        assert!(matches!(
            decode_begin(&InvokeBody::Raw(body)),
            Err(StoreError::WorkflowInvalid)
        ));
    }
    let commit = format!(
        r#"{{"schemaVersion":"rangoon.workflow-commit.v1","previewId":"{preview}","expectedStateId":"{workspace}","acknowledged":true}}"#
    );
    assert!(decode_commit(&InvokeBody::Raw(commit.clone().into_bytes())).is_ok());
    let false_ack = commit.replace("true", "false");
    assert!(matches!(
        decode_commit(&InvokeBody::Raw(false_ack.into_bytes())),
        Err(StoreError::WorkflowInvalid)
    ));
    let clear = format!(r#"{{"schemaVersion":"rangoon.workflow-clear.v1","draftId":"{draft}"}}"#);
    assert!(decode_clear(&InvokeBody::Raw(clear.clone().into_bytes())).is_ok());
    let duplicate_clear = clear.replacen('}', &format!(r#", "draftId":"{draft}"}}"#), 1);
    assert!(matches!(
        decode_clear(&InvokeBody::Raw(duplicate_clear.into_bytes())),
        Err(StoreError::WorkflowInvalid)
    ));
    let open = format!(
        r#"{{"schemaVersion":"rangoon.workflow-open.v1","workflowId":"{workflow}","selection":{{"kind":"head"}}}}"#
    );
    assert!(decode_open(&InvokeBody::Raw(open.clone().into_bytes())).is_ok());
    let unknown = open.replace("head", "unknown");
    assert!(matches!(
        decode_open(&InvokeBody::Raw(unknown.into_bytes())),
        Err(StoreError::WorkflowInvalid)
    ));
    let inspect = format!(
        r#"{{"schemaVersion":"rangoon.workflow-inspect.v1","draftId":"{draft}","intent":"draft","definition":{{}},"layout":{{}}}}"#
    );
    let padded_inspect = format!("{inspect}{}", " ".repeat(160 * 1024 - inspect.len()));
    assert!(decode_inspect(&InvokeBody::Raw(padded_inspect.into_bytes())).is_ok());
    let oversized_inspect = format!("{inspect}{}", " ".repeat(160 * 1024 + 1 - inspect.len()));
    assert!(matches!(
        decode_inspect(&InvokeBody::Raw(oversized_inspect.into_bytes())),
        Err(StoreError::WorkflowInvalid)
    ));
    let wrappers = include_str!("workflows.rs");
    for forwarder in [
        "access.list_workflows().await",
        "access.open_workflow(value).await",
        "access.begin_workflow_draft(value).await",
        "access.inspect_workflow_save(value).await",
        "access.commit_workflow_save(value).await",
        "access.clear_workflow_draft(value).await",
    ] {
        assert!(wrappers.contains(forwarder));
    }
    assert!(include_str!("data_controls.rs").contains("access.workflow_data().await"));
    assert!(!include_str!("main.rs").contains(".manage(WorkflowSession::default())"));
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProbeWorkflowOperation {
    List,
    Inspect,
    Save,
}

struct WorkflowBlock {
    operation: ProbeWorkflowOperation,
    started: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}

#[derive(Default)]
struct WorkflowProbeState {
    list_calls: AtomicUsize,
    inspect_calls: AtomicUsize,
    save_calls: AtomicUsize,
    block: Mutex<Option<WorkflowBlock>>,
    panic: Mutex<Option<ProbeWorkflowOperation>>,
}

impl WorkflowProbeState {
    fn block(&self, operation: ProbeWorkflowOperation) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        *self.block.lock().unwrap() = Some(WorkflowBlock {
            operation,
            started: started_tx,
            release: release_rx,
        });
        (started_rx, release_tx)
    }

    fn before(&self, operation: ProbeWorkflowOperation) {
        match operation {
            ProbeWorkflowOperation::List => self.list_calls.fetch_add(1, Ordering::SeqCst),
            ProbeWorkflowOperation::Inspect => self.inspect_calls.fetch_add(1, Ordering::SeqCst),
            ProbeWorkflowOperation::Save => self.save_calls.fetch_add(1, Ordering::SeqCst),
        };
        if self.panic.lock().unwrap().as_ref() == Some(&operation) {
            panic!("synthetic workflow backend panic");
        }
        let block = {
            let mut pending = self.block.lock().unwrap();
            if pending
                .as_ref()
                .is_some_and(|block| block.operation == operation)
            {
                pending.take()
            } else {
                None
            }
        };
        if let Some(block) = block {
            assert!(block.started.send(()).is_ok());
            assert!(block.release.recv_timeout(WAIT).is_ok());
        }
    }
}

struct WorkflowProbeBackend {
    state: Arc<WorkflowProbeState>,
    root: PathBuf,
}

impl WorkflowProbeBackend {
    fn workspace(&self) -> Workspace {
        Workspace::new(self.root.clone())
    }
}

impl Backend for WorkflowProbeBackend {
    fn list(&mut self) -> Result<Vec<SnapshotMetadata>, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn open(&mut self, _: &str) -> Result<AnalysisReport, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn save(&mut self, _: &AnalysisReport) -> Result<SaveReceipt, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn list_capabilities(&mut self) -> Result<Vec<CapabilitySummary>, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn open_capability(&mut self, _: &str, _: Option<&str>) -> Result<CapabilityDetail, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn create_capability(
        &mut self,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<CapabilityReceiptV1, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn revise_capability(
        &mut self,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<CapabilityReceiptV1, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn review_capability(&mut self, _: &str, _: &str) -> Result<CapabilityReceiptV1, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn workflow_data(&mut self) -> Result<WorkflowWorkspaceData, Error> {
        self.workspace().workflow_data().map_err(Error::Store)
    }
    fn list_workflows(&mut self) -> Result<Vec<WorkflowSummary>, Error> {
        self.state.before(ProbeWorkflowOperation::List);
        self.workspace().list_workflows().map_err(Error::Store)
    }
    fn open_workflow(&mut self, id: &str, revision: Option<&str>) -> Result<WorkflowDetail, Error> {
        self.workspace()
            .open_workflow(id, revision)
            .map_err(Error::Store)
    }
    fn inspect_workflow_save(
        &mut self,
        candidate: &WorkflowRevision,
    ) -> Result<WorkflowSavePlan, Error> {
        self.state.before(ProbeWorkflowOperation::Inspect);
        self.workspace()
            .inspect_workflow_save(candidate)
            .map_err(Error::Store)
    }
    fn save_workflow(
        &mut self,
        candidate: &WorkflowRevision,
        expected: &str,
    ) -> Result<WorkflowSaveReceipt, Error> {
        self.state.before(ProbeWorkflowOperation::Save);
        self.workspace()
            .save_workflow(candidate, expected)
            .map_err(Error::Store)
    }

    fn prepare_composition(
        &mut self,
        _request: &rangoon_compose::application::Request,
    ) -> Result<RoutedCompositionPreview, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn confirm_composition(
        &mut self,
        _handle: CompositionHandle,
        _acknowledged: bool,
    ) -> Result<CompositionReceipt, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn compile_capability(
        &mut self,
        _id: &str,
        _revision: &str,
        _profile: rangoon_compile::Profile,
    ) -> Result<WorkspaceCompilation, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }

    fn plaintext_backup_export_allowed(&self) -> bool {
        false
    }
    fn export_workflow_backup(&mut self) -> Result<Vec<u8>, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn prepare_workflow_restore(&mut self, _backup: &Backup) -> Result<RestorePlan, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn restore_workflow_backup(
        &mut self,
        _backup: &Backup,
        _plan: &RestorePlan,
    ) -> Result<WorkflowWorkspaceData, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn inspect_workflow_deletion(
        &mut self,
        _kind: RecordKind,
        _id: &str,
    ) -> Result<DeletionPlan, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn delete_workflow_record(
        &mut self,
        _plan: &DeletionPlan,
    ) -> Result<WorkflowWorkspaceData, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
}

fn workflow_probe(root: &TestRoot) -> (SourceAccess, Arc<WorkflowProbeState>, Gate) {
    let gate = Gate::new();
    let state = Arc::new(WorkflowProbeState::default());
    let access = SourceAccess::new(
        gate.clone(),
        Box::new(WorkflowProbeBackend {
            state: Arc::clone(&state),
            root: root.0.clone(),
        }),
    );
    (access, state, gate)
}

#[test]
fn workflow_late_save_is_truthful_and_panic_is_uncertain_without_retry() {
    let root = TestRoot::new("workflow-late-save");
    let (access, state, gate) = workflow_probe(&root);
    let (draft_id, _) = new_draft(&access);
    let (_, preview, expected) = inspect(&access, &draft_id, "Late saved workflow");
    let (started, release) = state.block(ProbeWorkflowOperation::Save);
    let worker_access = access.clone();
    let worker_preview = preview.clone();
    let worker_expected = expected.clone();
    let (result_tx, result_rx) = mpsc::channel();
    let waiter = tauri::async_runtime::spawn_blocking(move || {
        assert!(
            result_tx
                .send(commit(&worker_access, &worker_preview, &worker_expected))
                .is_ok()
        );
    });
    assert!(started.recv_timeout(WAIT).is_ok());
    drop(waiter);
    let drain = gate.request_drain().unwrap();
    assert!(!drain.is_quiescent().unwrap());
    assert!(release.send(()).is_ok());
    assert_eq!(
        workflow_code(result_rx.recv_timeout(WAIT).unwrap()),
        "workspace_write_completed"
    );
    assert_eq!(state.save_calls.load(Ordering::SeqCst), 1);
    assert!(drain.is_quiescent().unwrap());

    let root = TestRoot::new("workflow-held-completion");
    let (access, state, gate) = workflow_probe(&root);
    let (draft_id, _) = new_draft(&access);
    let (_, preview, expected) = inspect(&access, &draft_id, "Held completion workflow");
    let (started, release) = state.block(ProbeWorkflowOperation::Save);
    let ticket = access.begin_capability(Lane::Exclusive).unwrap();
    let (completion_tx, completion_rx) = mpsc::channel();
    let waiter = tauri::async_runtime::spawn_blocking(move || {
        assert!(
            completion_tx
                .send(ticket.workflow(WorkflowOperation::Commit(Confirmation {
                    schema_version: "rangoon.workflow-commit.v1".into(),
                    preview_id: preview,
                    expected_state_id: expected,
                    acknowledged: true,
                })))
                .is_ok()
        );
    });
    assert!(started.recv_timeout(WAIT).is_ok());
    drop(waiter);
    let drain = gate.request_drain().unwrap();
    assert!(!drain.is_quiescent().unwrap());
    assert!(release.send(()).is_ok());
    let completion = completion_rx.recv_timeout(WAIT).unwrap();
    assert!(!drain.is_quiescent().unwrap());
    assert_eq!(
        workflow_code(completion.deliver()),
        "workspace_write_completed"
    );
    assert!(drain.is_quiescent().unwrap());

    let root = TestRoot::new("workflow-panic");
    let (access, state, _) = workflow_probe(&root);
    let (draft_id, _) = new_draft(&access);
    let (_, preview, expected) = inspect(&access, &draft_id, "Panic workflow");
    *state.panic.lock().unwrap() = Some(ProbeWorkflowOperation::Save);
    assert_eq!(
        workflow_code(commit(&access, &preview, &expected)),
        "workspace_write_uncertain"
    );
    assert_eq!(state.save_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        workflow_code(commit(&access, &preview, &expected)),
        "workspace_changed"
    );
}

#[test]
fn workflow_close_during_reinspection_refuses_write_and_stale_preview_stays_consumed() {
    let root = TestRoot::new("workflow-close-reinspect");
    let (access, state, gate) = workflow_probe(&root);
    let (draft_id, _) = new_draft(&access);
    let (_, preview, expected) = inspect(&access, &draft_id, "Close before save");
    let (started, release) = state.block(ProbeWorkflowOperation::Inspect);
    let worker_access = access.clone();
    let worker_preview = preview.clone();
    let worker_expected = expected.clone();
    let (result_tx, result_rx) = mpsc::channel();
    tauri::async_runtime::spawn_blocking(move || {
        assert!(
            result_tx
                .send(commit(&worker_access, &worker_preview, &worker_expected))
                .is_ok()
        );
    });
    assert!(started.recv_timeout(WAIT).is_ok());
    let drain = gate.request_drain().unwrap();
    assert!(release.send(()).is_ok());
    assert_eq!(
        workflow_code(result_rx.recv_timeout(WAIT).unwrap()),
        "workspace_unavailable"
    );
    assert_eq!(state.save_calls.load(Ordering::SeqCst), 0);
    assert!(drain.is_quiescent().unwrap());
    gate.resume(&drain).unwrap();
    assert_eq!(
        workflow_code(commit(&access, &preview, &expected)),
        "workspace_changed"
    );
}

#[test]
fn workflow_completion_drop_keeps_read_only_draft_but_purges_unobserved_authoring() {
    let root = TestRoot::new("workflow-completion-drop");
    let (access, _, _) = workflow_probe(&root);
    let (draft_id, _) = new_draft(&access);
    let read = access
        .begin_capability(Lane::Read)
        .unwrap()
        .workflow(WorkflowOperation::List);
    drop(read);
    assert!(matches!(
        tauri::async_runtime::block_on(access.clear_workflow_draft(ClearRequest {
            schema_version: "rangoon.workflow-clear.v1".into(),
            draft_id,
        })),
        WorkflowResult::Cleared
    ));
    let (draft_id, _) = new_draft(&access);
    let authoring =
        access
            .begin_capability(Lane::Exclusive)
            .unwrap()
            .workflow(WorkflowOperation::Inspect(InspectRequest {
                schema_version: "rangoon.workflow-inspect.v1".into(),
                draft_id: draft_id.clone(),
                intent: SaveIntent::Draft,
                definition: definition("Unobserved preview"),
                layout: layout(),
            }));
    drop(authoring);
    assert_eq!(
        workflow_code(tauri::async_runtime::block_on(access.clear_workflow_draft(
            ClearRequest {
                schema_version: "rangoon.workflow-clear.v1".into(),
                draft_id,
            }
        ))),
        "workspace_changed"
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn managed_workflow_routing_encrypts_reopens_and_refuses_locked_without_fallback() {
    use rangoon_desktop::workspace_session::Owner;

    let root = TestRoot::new("managed-workflow-routing");
    let vault = ThreadVault::default();
    let locked = SourceAccess::managed(
        Gate::new(),
        Owner::acquire(root.0.clone()).unwrap(),
        vault.clone(),
    );
    assert_eq!(
        workflow_code(tauri::async_runtime::block_on(locked.list_workflows())),
        "workspace_locked"
    );
    drop(locked);

    let (access, identity) = encrypted_access(&root, &vault);
    let (draft_id, workflow_id) = new_draft(&access);
    let (_, preview, state) = inspect(&access, &draft_id, "Encrypted workflow");
    assert!(matches!(
        commit(&access, &preview, &state),
        WorkflowResult::Saved { .. }
    ));
    let database = root
        .0
        .join("encrypted-workspaces")
        .join(&identity)
        .join("workspace.sqlite3");
    let ciphertext = std::fs::read(&database).unwrap();
    assert!(!ciphertext.starts_with(b"SQLite format 3\0"));
    assert!(
        !ciphertext
            .windows(b"Encrypted workflow".len())
            .any(|bytes| bytes == b"Encrypted workflow")
    );
    drop(access);

    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let unlock = owner.prepare_unlock().unwrap();
    owner.confirm(&unlock.id, &vault).unwrap();
    let reopened = SourceAccess::managed(Gate::new(), owner, vault.clone());
    match tauri::async_runtime::block_on(reopened.open_workflow(OpenRequest {
        schema_version: "rangoon.workflow-open.v1".into(),
        workflow_id,
        selection: Selection::Head {},
    })) {
        WorkflowResult::Opened { workflow } => {
            assert_eq!(workflow.head.label, "Encrypted workflow")
        }
        _ => panic!("expected encrypted workflow reopen"),
    }
    let (draft_id, _) = new_draft(&reopened);
    let (_, pending_preview, pending_state) =
        inspect(&reopened, &draft_id, "Discard on binding loss");
    std::fs::write(
        root.0.join("encrypted-workspace.binding"),
        b"invalid binding",
    )
    .unwrap();
    assert_eq!(
        workflow_code(tauri::async_runtime::block_on(reopened.list_workflows())),
        "workspace_unavailable"
    );
    assert_eq!(
        workflow_code(commit(&reopened, &pending_preview, &pending_state)),
        "workspace_changed"
    );
    assert_eq!(std::fs::read(&database).unwrap(), ciphertext);
    assert!(!root.0.join("workspace.sqlite3").exists());
}
