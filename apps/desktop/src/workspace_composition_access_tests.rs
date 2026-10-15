use super::*;

use crate::{
    compilation::{self, CompilationResult},
    composition::{self, CompositionHandle, CompositionResult, RoutedCompositionPreview},
};
use rangoon_compile::Profile;
use rangoon_compose::{
    Draft, InputRange, InputReference, Operation, OutputRecipe, Piece,
    application::{self, Request, Target},
};
use rangoon_domain::{
    Authority,
    capability_v1::{CapabilityDetail, Origin, RevisionProvenance},
};
use rangoon_store::{CompositionReceipt, WorkspaceCompilation};
use std::{
    fs,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
};
use tauri::ipc::InvokeBody;

fn source_request(report: &AnalysisReport) -> Request {
    Request {
        schema_version: rangoon_compose::application::REQUEST_SCHEMA.into(),
        draft: Draft {
            schema_version: rangoon_compose::DRAFT_SCHEMA.into(),
            operation: Operation::Decompose,
            inputs: vec![InputReference::Source {
                source_id: report.source.id.clone(),
                sha256: report.source.sha256.clone(),
            }],
            outputs: vec![OutputRecipe {
                title: "Exact routed source 🦀".into(),
                pieces: vec![Piece::Copy {
                    range: InputRange {
                        input_index: 0,
                        start_byte: 0,
                        end_byte: report.source.content.len().try_into().unwrap(),
                    },
                }],
            }],
            exclusions: vec![],
            duplications: vec![],
            conflicts: vec![],
        },
        targets: vec![Target::New {}],
    }
}

fn merge_request(left: &CapabilityDetail, right: &CapabilityDetail) -> Request {
    Request {
        schema_version: rangoon_compose::application::REQUEST_SCHEMA.into(),
        draft: Draft {
            schema_version: rangoon_compose::DRAFT_SCHEMA.into(),
            operation: Operation::Merge,
            inputs: vec![
                InputReference::Revision {
                    capability_id: left.id.clone(),
                    revision_id: left.latest_revision_id.clone(),
                    sha256: left.revision.sha256.clone(),
                },
                InputReference::Revision {
                    capability_id: right.id.clone(),
                    revision_id: right.latest_revision_id.clone(),
                    sha256: right.revision.sha256.clone(),
                },
            ],
            outputs: vec![OutputRecipe {
                title: "Merged exact output".into(),
                pieces: vec![
                    Piece::Copy {
                        range: InputRange {
                            input_index: 0,
                            start_byte: 0,
                            end_byte: left.revision.content.len().try_into().unwrap(),
                        },
                    },
                    Piece::Copy {
                        range: InputRange {
                            input_index: 1,
                            start_byte: 0,
                            end_byte: right.revision.content.len().try_into().unwrap(),
                        },
                    },
                ],
            }],
            exclusions: vec![],
            duplications: vec![],
            conflicts: vec![],
        },
        targets: vec![Target::Append {
            capability_id: left.id.clone(),
            expected_revision_id: left.latest_revision_id.clone(),
        }],
    }
}

fn ready(
    access: &SourceAccess,
    request: Request,
) -> (String, String, application::ApplicationPreview) {
    match tauri::async_runtime::block_on(access.preview_composition(request)) {
        CompositionResult::Ready {
            preview_id,
            expected_state_id,
            preview,
            ..
        } => (preview_id, expected_state_id, *preview),
        _ => panic!("expected routed composition preview"),
    }
}

fn confirmation(
    preview_id: String,
    expected_state_id: String,
    acknowledged: bool,
) -> composition::Confirmation {
    let body = serde_json::json!({
        "schemaVersion": "rangoon.composition-confirmation.v0",
        "previewId": preview_id,
        "expectedStateId": expected_state_id,
        "acknowledged": acknowledged,
    });
    composition::decode_confirmation(&InvokeBody::Raw(serde_json::to_vec(&body).unwrap())).unwrap()
}

fn commit(
    access: &SourceAccess,
    preview_id: String,
    expected_state_id: String,
) -> CompositionResult {
    tauri::async_runtime::block_on(access.commit_composition(confirmation(
        preview_id,
        expected_state_id,
        true,
    )))
}

fn composition_code(result: CompositionResult) -> &'static str {
    match result {
        CompositionResult::Failed { error } => error.code,
        _ => panic!("expected failed composition result"),
    }
}

fn compilation_code(result: CompilationResult) -> &'static str {
    match result {
        CompilationResult::Failed { error } => error.code,
        _ => panic!("expected failed compilation result"),
    }
}

fn compilation_request(detail: &CapabilityDetail) -> compilation::Request {
    let body = serde_json::json!({
        "schemaVersion": "rangoon.compile-request.v1",
        "capabilityId": detail.id,
        "revisionId": detail.revision.id,
        "profile": "agents_md_v1",
    });
    compilation::decode_request(&InvokeBody::Raw(serde_json::to_vec(&body).unwrap())).unwrap()
}

fn compiled(access: &SourceAccess, detail: &CapabilityDetail) -> CompilationResult {
    tauri::async_runtime::block_on(access.compile_capability(compilation_request(detail)))
}

#[test]
fn legacy_routes_exact_decompose_merge_and_read_only_compilation() {
    let root = TestRoot::new("composition-access-legacy");
    let workspace = Workspace::new(root.0.clone());
    let source = report(
        "composition-access.md",
        "\u{feff}# Saved\r\nExact 🦀 content\r\n".as_bytes(),
    );
    workspace.save_v1(&source).unwrap();
    let access = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));

    let (preview_id, expected, mut inspection) = ready(&access, source_request(&source));
    inspection.core.outputs[0].content = "renderer copy cannot commit".into();
    let derived = match commit(&access, preview_id, expected) {
        CompositionResult::Committed { receipt } => {
            receipt.capabilities.into_iter().next().unwrap()
        }
        _ => panic!("expected composition commit"),
    };
    assert_eq!(derived.revision.content, source.source.content);
    assert_eq!(derived.authority, Authority::None);
    assert!(derived.revision.review.is_none());
    assert!(matches!(derived.origin, Origin::Composition { .. }));
    assert!(matches!(
        derived.revision.provenance,
        RevisionProvenance::Composition { .. }
    ));

    let left = workspace
        .review_capability_v1(&derived.id, &derived.latest_revision_id)
        .unwrap()
        .capability;
    let other = report("other.md", b"# Other\r\nExact merge bytes\r\n");
    workspace.save_v1(&other).unwrap();
    let right = workspace
        .create_capability_v1(&other.source.id, &other.fragments[0].id, "Other")
        .unwrap()
        .capability;
    let right = workspace
        .review_capability_v1(&right.id, &right.latest_revision_id)
        .unwrap()
        .capability;
    let (preview_id, expected, _) = ready(&access, merge_request(&left, &right));
    let merged = match commit(&access, preview_id, expected) {
        CompositionResult::Committed { receipt } => {
            receipt.capabilities.into_iter().next().unwrap()
        }
        _ => panic!("expected merge commit"),
    };
    assert_eq!(merged.id, left.id);
    assert_eq!(
        merged.revision.content,
        format!("{}{}", left.revision.content, right.revision.content)
    );
    assert!(merged.revision.review.is_none());
    assert!(matches!(
        merged.revision.provenance,
        RevisionProvenance::Composition { .. }
    ));

    let historical = workspace
        .open_capability_v1(&derived.id, Some(&derived.latest_revision_id))
        .unwrap();
    let before = fs::read(root.0.join("workspace.sqlite3")).unwrap();
    match compiled(&access, &historical) {
        CompilationResult::Compiled {
            observed_current_head,
            compilation,
            candidate_manifest_json,
            ..
        } => {
            assert_eq!(observed_current_head, merged.latest_revision_id);
            assert_eq!(compilation.artifact.content, source.source.content);
            assert!(candidate_manifest_json.is_some());
        }
        _ => panic!("expected historical compilation"),
    }
    assert_eq!(fs::read(root.0.join("workspace.sqlite3")).unwrap(), before);
}

#[test]
fn confirmations_are_exact_one_shot_and_new_preview_supersedes_old() {
    let root = TestRoot::new("composition-access-confirmations");
    let source = report("confirm.md", b"# Confirmation\nExact bytes\n");
    Workspace::new(root.0.clone()).save_v1(&source).unwrap();
    let access = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));

    let first = ready(&access, source_request(&source));
    let second = ready(&access, source_request(&source));
    assert_eq!(
        composition_code(commit(&access, first.0, first.1)),
        "workspace_changed"
    );
    assert_eq!(
        composition_code(commit(
            &access,
            second.0.clone(),
            format!("{}-wrong", second.1),
        )),
        "workspace_changed"
    );
    assert_eq!(
        composition_code(tauri::async_runtime::block_on(access.commit_composition(
            confirmation(second.0.clone(), second.1.clone(), false,)
        ))),
        "composition_acknowledgment_required"
    );
    assert!(matches!(
        commit(&access, second.0.clone(), second.1.clone()),
        CompositionResult::Committed { .. }
    ));
    assert_eq!(
        composition_code(commit(&access, second.0, second.1)),
        "workspace_changed"
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProbeOperation {
    Preview,
    Commit,
    Compile,
}

struct ProbeBlock {
    operation: ProbeOperation,
    started: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}

#[derive(Default)]
struct CompositionProbeState {
    preview_calls: AtomicUsize,
    commit_calls: AtomicUsize,
    compile_calls: AtomicUsize,
    block: Mutex<Option<ProbeBlock>>,
    panic: Mutex<Option<ProbeOperation>>,
}

impl CompositionProbeState {
    fn block(&self, operation: ProbeOperation) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        *self.block.lock().unwrap() = Some(ProbeBlock {
            operation,
            started: started_tx,
            release: release_rx,
        });
        (started_rx, release_tx)
    }

    fn before(&self, operation: ProbeOperation) {
        match operation {
            ProbeOperation::Preview => self.preview_calls.fetch_add(1, Ordering::SeqCst),
            ProbeOperation::Commit => self.commit_calls.fetch_add(1, Ordering::SeqCst),
            ProbeOperation::Compile => self.compile_calls.fetch_add(1, Ordering::SeqCst),
        };
        if self.panic.lock().unwrap().as_ref() == Some(&operation) {
            panic!("synthetic composition backend panic");
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

struct CompositionProbeBackend {
    root: std::path::PathBuf,
    state: Arc<CompositionProbeState>,
}

impl CompositionProbeBackend {
    fn workspace(&self) -> Workspace {
        Workspace::new(self.root.clone())
    }
}

impl Backend for CompositionProbeBackend {
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
        Err(Error::Store(StoreError::Unavailable))
    }
    fn list_workflows(&mut self) -> Result<Vec<WorkflowSummary>, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn open_workflow(&mut self, _: &str, _: Option<&str>) -> Result<WorkflowDetail, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn inspect_workflow_save(&mut self, _: &WorkflowRevision) -> Result<WorkflowSavePlan, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn save_workflow(
        &mut self,
        _: &WorkflowRevision,
        _: &str,
    ) -> Result<WorkflowSaveReceipt, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }

    fn prepare_composition(
        &mut self,
        request: &Request,
    ) -> Result<RoutedCompositionPreview, Error> {
        self.state.before(ProbeOperation::Preview);
        self.workspace()
            .preview_composition(request)
            .map(RoutedCompositionPreview::legacy)
            .map_err(Error::Store)
    }

    fn confirm_composition(
        &mut self,
        handle: CompositionHandle,
        acknowledged: bool,
    ) -> Result<CompositionReceipt, Error> {
        self.state.before(ProbeOperation::Commit);
        match handle {
            CompositionHandle::Legacy(prepared) => self
                .workspace()
                .apply_composition(&prepared, acknowledged)
                .map_err(Error::Store),
            #[cfg(all(test, feature = "encrypted-workspace"))]
            CompositionHandle::Managed(_) => Err(Error::Store(StoreError::CompositionInvalid)),
        }
    }

    fn compile_capability(
        &mut self,
        id: &str,
        revision: &str,
        profile: Profile,
    ) -> Result<WorkspaceCompilation, Error> {
        self.state.before(ProbeOperation::Compile);
        self.workspace()
            .compile_capability(id, revision, profile)
            .map_err(Error::Store)
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

fn composition_probe(root: &TestRoot) -> (SourceAccess, Arc<CompositionProbeState>, Gate) {
    let gate = Gate::new();
    let state = Arc::new(CompositionProbeState::default());
    let access = SourceAccess::new(
        gate.clone(),
        Box::new(CompositionProbeBackend {
            root: root.0.clone(),
            state: Arc::clone(&state),
        }),
    );
    (access, state, gate)
}

#[test]
fn composition_and_compile_close_truthfully_keep_leases_and_never_replay() {
    let root = TestRoot::new("composition-access-drain");
    let source = report("drain.md", b"# Drain\nExact bytes\n");
    let workspace = Workspace::new(root.0.clone());
    workspace.save_v1(&source).unwrap();
    let (access, state, gate) = composition_probe(&root);

    let mut panic_request = source_request(&source);
    panic_request.draft.outputs[0].title = "Panic retained output".into();
    let (preview_id, expected, _) = ready(&access, panic_request);
    let (started, release) = state.block(ProbeOperation::Commit);
    let worker_access = access.clone();
    let worker_preview = preview_id.clone();
    let worker_expected = expected.clone();
    let (result_tx, result_rx) = mpsc::channel();
    let waiter = tauri::async_runtime::spawn_blocking(move || {
        assert!(
            result_tx
                .send(commit(&worker_access, worker_preview, worker_expected))
                .is_ok()
        );
    });
    assert!(started.recv_timeout(WAIT).is_ok());
    drop(waiter);
    let drain = gate.request_drain().unwrap();
    assert!(!drain.is_quiescent().unwrap());
    assert!(release.send(()).is_ok());
    assert_eq!(
        composition_code(result_rx.recv_timeout(WAIT).unwrap()),
        "workspace_write_completed"
    );
    assert_eq!(state.commit_calls.load(Ordering::SeqCst), 1);
    assert!(drain.is_quiescent().unwrap());
    gate.resume(&drain).unwrap();
    assert_eq!(
        composition_code(commit(&access, preview_id, expected)),
        "workspace_changed"
    );

    let mut compile_request = source_request(&source);
    compile_request.draft.outputs[0].title = "Compile retains preview".into();
    let (preview_id, expected, _) = ready(&access, compile_request);
    *state.panic.lock().unwrap() = Some(ProbeOperation::Commit);
    assert_eq!(
        composition_code(commit(&access, preview_id.clone(), expected.clone())),
        "workspace_write_uncertain"
    );
    assert_eq!(
        composition_code(commit(&access, preview_id, expected)),
        "workspace_changed"
    );
    *state.panic.lock().unwrap() = None;

    let root = TestRoot::new("composition-access-compile-drain");
    let source = report("compile-drain.md", b"# Compile drain\nExact bytes\n");
    let workspace = Workspace::new(root.0.clone());
    workspace.save_v1(&source).unwrap();
    let capability = workspace
        .create_capability_v1(&source.source.id, &source.fragments[0].id, "Compile drain")
        .unwrap()
        .capability;
    let capability = workspace
        .review_capability_v1(&capability.id, &capability.latest_revision_id)
        .unwrap()
        .capability;
    let (access, state, gate) = composition_probe(&root);
    let mut current_request = source_request(&source);
    current_request.draft.outputs[0].title = "Current compile keeps preview".into();
    let (current_preview, current_expected, _) = ready(&access, current_request);
    assert!(matches!(
        compiled(&access, &capability),
        CompilationResult::Compiled { .. }
    ));
    assert!(matches!(
        commit(&access, current_preview, current_expected),
        CompositionResult::Committed { .. }
    ));
    let mut retained_request = source_request(&source);
    retained_request.draft.outputs[0].title = "Compile after panic".into();
    let (preview_id, expected, _) = ready(&access, retained_request);
    let (started, release) = state.block(ProbeOperation::Compile);
    let worker_access = access.clone();
    let worker_capability = capability.clone();
    let (result_tx, result_rx) = mpsc::channel();
    tauri::async_runtime::spawn_blocking(move || {
        assert!(
            result_tx
                .send(compiled(&worker_access, &worker_capability))
                .is_ok()
        );
    });
    assert!(started.recv_timeout(WAIT).is_ok());
    let drain = gate.request_drain().unwrap();
    assert!(release.send(()).is_ok());
    assert_eq!(
        compilation_code(result_rx.recv_timeout(WAIT).unwrap()),
        "workspace_unavailable"
    );
    assert!(drain.is_quiescent().unwrap());
    gate.resume(&drain).unwrap();
    assert_eq!(
        composition_code(commit(&access, preview_id, expected)),
        "workspace_changed"
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn managed_routes_keep_owner_token_native_and_ciphertext_unreadable() {
    use rangoon_desktop::workspace_session::Owner;

    let root = TestRoot::new("composition-access-managed");
    let vault = ThreadVault::default();
    let (access, identity) = encrypted_access(&root, &vault);
    let source = report("managed-compose.md", "# Managed\r\nExact 🦀\r\n".as_bytes());
    select(&access, &source);
    assert!(matches!(
        tauri::async_runtime::block_on(access.save_analysis(source.source.id.clone())),
        WorkspaceResult::Saved { .. }
    ));
    let (preview_id, expected, mut inspection) = ready(&access, source_request(&source));
    inspection.core.outputs[0].content = "forged renderer copy".into();
    let detail = match commit(&access, preview_id, expected) {
        CompositionResult::Committed { receipt } => {
            receipt.capabilities.into_iter().next().unwrap()
        }
        _ => panic!("expected managed commit"),
    };
    assert_eq!(detail.revision.content, source.source.content);
    let database = root
        .0
        .join("encrypted-workspaces")
        .join(&identity)
        .join("workspace.sqlite3");
    let bytes = fs::read(&database).unwrap();
    assert!(!bytes.starts_with(b"SQLite format 3\0"));
    assert!(
        !bytes
            .windows(source.source.content.len())
            .any(|window| window == source.source.content.as_bytes())
    );

    let reviewed = match tauri::async_runtime::block_on(
        access.review_capability(detail.id.clone(), detail.latest_revision_id.clone()),
    ) {
        CapabilityResult::Opened { capability, .. } => *capability,
        _ => panic!("expected managed review"),
    };
    let before_compile = fs::read(&database).unwrap();
    match compiled(&access, &reviewed) {
        CompilationResult::Compiled {
            observed_current_head,
            compilation,
            candidate_manifest_json,
            ..
        } => {
            assert_eq!(observed_current_head, reviewed.latest_revision_id);
            assert_eq!(compilation.artifact.content, source.source.content);
            assert!(candidate_manifest_json.is_some());
        }
        _ => panic!("expected managed compilation"),
    }
    assert_eq!(fs::read(&database).unwrap(), before_compile);

    drop(access);
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let unlock = owner.prepare_unlock().unwrap();
    owner.confirm(&unlock.id, &vault).unwrap();
    let reopened = SourceAccess::managed(Gate::new(), owner, vault);
    match tauri::async_runtime::block_on(reopened.open_capability(reviewed.id.clone(), None)) {
        CapabilityResult::Opened { capability, .. } => {
            assert_eq!(capability.revision.content, source.source.content)
        }
        _ => panic!("expected managed composed capability"),
    }
}

#[test]
fn decoders_and_wrappers_stay_bounded_and_source_access_only() {
    let preview = serde_json::to_value(source_request(&report(
        "decode.md",
        b"# Decoder\nExact bytes\n",
    )))
    .unwrap();
    assert!(
        composition::decode_preview(&InvokeBody::Raw(serde_json::to_vec(&preview).unwrap()))
            .is_ok()
    );
    assert!(composition::decode_preview(&InvokeBody::Json(preview)).is_err());
    let confirmation = serde_json::json!({
        "schemaVersion": "rangoon.composition-confirmation.v0",
        "previewId": format!("preview:{}", "a".repeat(64)),
        "expectedStateId": "workspace:fixture",
        "acknowledged": true
    });
    assert!(
        composition::decode_confirmation(&InvokeBody::Raw(
            serde_json::to_vec(&confirmation).unwrap()
        ))
        .is_ok()
    );
    let oversized = InvokeBody::Raw(vec![b' '; 4097]);
    assert!(composition::decode_confirmation(&oversized).is_err());
    assert!(compilation::decode_request(&InvokeBody::Raw(vec![b' '; 4097])).is_err());

    let composition_source = include_str!("composition.rs");
    let compilation_source = include_str!("compilation.rs");
    let main = include_str!("main.rs");
    assert!(
        composition_source.contains("app.state::<SourceAccess>().preview_composition(draft).await")
    );
    assert!(
        composition_source
            .contains(".state::<SourceAccess>()\n        .commit_composition(confirmation)")
    );
    assert!(
        compilation_source
            .contains(".state::<SourceAccess>()\n        .compile_capability(request)")
    );
    assert!(!composition_source.contains("workspace(&app)"));
    assert!(!compilation_source.contains("workspace(&app)"));
    assert!(!main.contains(".manage(CompositionSession::default())"));
}
