use super::*;
#[cfg(all(feature = "encrypted-workspace", unix))]
use std::collections::BTreeMap;
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

const WAIT: std::time::Duration = std::time::Duration::from_secs(10);

static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);

struct TestRoot(PathBuf);

impl TestRoot {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rangoon-source-access-{label}-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_ROOT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn report(name: &str, bytes: &[u8]) -> AnalysisReport {
    rangoon_import::analyze(name, bytes).unwrap()
}

fn select(access: &SourceAccess, report: &AnalysisReport) {
    let operation = access.begin_selection().unwrap();
    assert!(matches!(
        operation
            .complete(SelectionResult::Analyzed {
                report: Box::new(report.clone())
            })
            .deliver(),
        SelectionResult::Analyzed { .. }
    ));
}

fn workspace_code(result: WorkspaceResult) -> &'static str {
    match result {
        WorkspaceResult::Failed { error } => error.code,
        _ => panic!("expected failed workspace result"),
    }
}

#[test]
fn unavailable_native_backend_rejects_legacy_requests_without_latching_corruption() {
    let access = SourceAccess::unavailable(Gate::new());
    assert!(access.encrypted_close_available());
    let view = serde_json::to_value(access.request_view()).unwrap();
    assert_eq!(view["storage"], "managed_encrypted");
    assert!(view["clientId"].is_null());
    assert!(matches!(
        access.request_context("main", None),
        crate::workspace_requests::WorkspaceRequest::Refused
    ));
    let legacy = access.for_request(&crate::workspace_requests::WorkspaceRequest::Legacy);
    assert!(legacy.begin_selection().is_err());
    assert!(legacy.begin_model_inputs().is_err());
    assert!(matches!(
        tauri::async_runtime::block_on(legacy.list_snapshots()),
        WorkspaceResult::Failed { .. }
    ));
    assert!(!access.failure_latched());
    assert!(serde_json::to_value(access.request_view()).unwrap()["clientId"].is_null());
}

fn selection_code(result: SelectionResult) -> &'static str {
    match result {
        SelectionResult::Failed { error } => error.code,
        _ => panic!("expected failed source selection"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    List,
    Open,
    Save,
}

struct Block {
    operation: Operation,
    started: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}

struct ProbeState {
    list_calls: AtomicUsize,
    open_calls: AtomicUsize,
    save_calls: AtomicUsize,
    block: Mutex<Option<Block>>,
    panic: Mutex<Option<Operation>>,
    save_error: Mutex<Option<Error>>,
    saved: Mutex<Vec<AnalysisReport>>,
}

impl Default for ProbeState {
    fn default() -> Self {
        Self {
            list_calls: AtomicUsize::new(0),
            open_calls: AtomicUsize::new(0),
            save_calls: AtomicUsize::new(0),
            block: Mutex::new(None),
            panic: Mutex::new(None),
            save_error: Mutex::new(None),
            saved: Mutex::new(Vec::new()),
        }
    }
}

impl ProbeState {
    fn block(&self, operation: Operation) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        *self.block.lock().unwrap() = Some(Block {
            operation,
            started: started_tx,
            release: release_rx,
        });
        (started_rx, release_tx)
    }

    fn before(&self, operation: Operation) {
        match operation {
            Operation::List => self.list_calls.fetch_add(1, Ordering::SeqCst),
            Operation::Open => self.open_calls.fetch_add(1, Ordering::SeqCst),
            Operation::Save => self.save_calls.fetch_add(1, Ordering::SeqCst),
        };
        if self.panic.lock().unwrap().as_ref() == Some(&operation) {
            panic!("synthetic source backend panic");
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

struct ProbeBackend {
    state: Arc<ProbeState>,
    report: AnalysisReport,
}

impl ProbeBackend {
    fn new(state: Arc<ProbeState>, report: AnalysisReport) -> Self {
        Self { state, report }
    }
}

impl Backend for ProbeBackend {
    fn list(&mut self) -> Result<Vec<SnapshotMetadata>, Error> {
        self.state.before(Operation::List);
        Ok(vec![SnapshotMetadata {
            source_id: self.report.source.id.clone(),
            display_name: self.report.source.display_name.clone(),
            sha256: self.report.source.sha256.clone(),
            byte_length: self.report.source.content.len().try_into().unwrap(),
            saved_at_ms: 0,
        }])
    }

    fn open(&mut self, source_id: &str) -> Result<AnalysisReport, Error> {
        self.state.before(Operation::Open);
        if source_id == self.report.source.id {
            Ok(self.report.clone())
        } else {
            Err(Error::Store(StoreError::NotFound))
        }
    }

    fn save(&mut self, report: &AnalysisReport) -> Result<SaveReceipt, Error> {
        self.state.before(Operation::Save);
        if let Some(error) = *self.state.save_error.lock().unwrap() {
            return Err(error);
        }
        self.state.saved.lock().unwrap().push(report.clone());
        Ok(SaveReceipt {
            snapshot: SnapshotMetadata {
                source_id: report.source.id.clone(),
                display_name: report.source.display_name.clone(),
                sha256: report.source.sha256.clone(),
                byte_length: report.source.content.len().try_into().unwrap(),
                saved_at_ms: 0,
            },
            already_saved: false,
        })
    }
    fn list_capabilities(&mut self) -> Result<Vec<CapabilitySummary>, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn open_capability(
        &mut self,
        _id: &str,
        _revision: Option<&str>,
    ) -> Result<CapabilityDetail, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn create_capability(
        &mut self,
        _source: &str,
        _fragment: &str,
        _title: &str,
    ) -> Result<CapabilityReceiptV1, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn revise_capability(
        &mut self,
        _id: &str,
        _expected: &str,
        _title: &str,
        _content: &str,
    ) -> Result<CapabilityReceiptV1, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn review_capability(
        &mut self,
        _id: &str,
        _expected: &str,
    ) -> Result<CapabilityReceiptV1, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn workflow_data(&mut self) -> Result<WorkflowWorkspaceData, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn list_workflows(&mut self) -> Result<Vec<WorkflowSummary>, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn open_workflow(
        &mut self,
        _id: &str,
        _revision: Option<&str>,
    ) -> Result<WorkflowDetail, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn inspect_workflow_save(
        &mut self,
        _candidate: &WorkflowRevision,
    ) -> Result<WorkflowSavePlan, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }
    fn save_workflow(
        &mut self,
        _candidate: &WorkflowRevision,
        _expected: &str,
    ) -> Result<WorkflowSaveReceipt, Error> {
        Err(Error::Store(StoreError::Unavailable))
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

fn probe(report: AnalysisReport) -> (SourceAccess, Arc<ProbeState>, Gate) {
    let gate = Gate::new();
    let state = Arc::new(ProbeState::default());
    let access = SourceAccess::new(
        gate.clone(),
        Box::new(ProbeBackend::new(Arc::clone(&state), report)),
    );
    (access, state, gate)
}

#[test]
fn legacy_dispatch_preserves_exact_source_bytes_and_import_selection() {
    let root = TestRoot::new("legacy");
    let imported = report("import.md", b"# Import\r\nExact import\r\n");
    let saved = report(
        "saved.md",
        "\u{feff}# Saved\r\nExact \u{1f980}\r\n".as_bytes(),
    );
    let store = Workspace::new(root.0.clone());
    store.save_v1(&saved).unwrap();
    let access = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));

    select(&access, &imported);
    match tauri::async_runtime::block_on(access.list_snapshots()) {
        WorkspaceResult::Listed { snapshots } => {
            assert_eq!(snapshots[0].source_id, saved.source.id)
        }
        _ => panic!("expected saved source list"),
    }
    match tauri::async_runtime::block_on(access.open_snapshot(saved.source.id.clone())) {
        SelectionResult::Analyzed { report } => assert_eq!(*report, saved),
        _ => panic!("expected saved source open"),
    }
    select(&access, &imported);
    match tauri::async_runtime::block_on(access.read_composition_source(saved.source.id.clone())) {
        SelectionResult::Analyzed { report } => assert_eq!(*report, saved),
        _ => panic!("expected composition source"),
    }
    assert!(matches!(
        tauri::async_runtime::block_on(access.save_analysis(imported.source.id.clone())),
        WorkspaceResult::Saved { .. }
    ));
    assert_eq!(store.open(&imported.source.id).unwrap(), imported);
}

#[test]
fn invalid_ids_refuse_before_backend_action() {
    let source = report("probe.md", b"# Probe\n");
    let (access, state, _) = probe(source);

    assert_eq!(
        selection_code(tauri::async_runtime::block_on(
            access.open_snapshot("../../outside".into())
        )),
        "invalid_snapshot_id"
    );
    assert_eq!(
        selection_code(tauri::async_runtime::block_on(
            access.read_composition_source("bad".into())
        )),
        "invalid_snapshot_id"
    );
    assert_eq!(
        workspace_code(tauri::async_runtime::block_on(
            access.save_analysis("bad".into())
        )),
        "invalid_snapshot_id"
    );
    assert_eq!(state.list_calls.load(Ordering::SeqCst), 0);
    assert_eq!(state.open_calls.load(Ordering::SeqCst), 0);
    assert_eq!(state.save_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn stale_ticket_refuses_action_before_backend_call() {
    let source = report("stale.md", b"# Stale\n");
    let (access, state, gate) = probe(source);
    let ticket = access.begin(Lane::Read).unwrap();
    let drain = gate.request_drain().unwrap();
    let completed = ticket.read(|backend| backend.list());

    assert!(matches!(completed.deliver(), Err(Error::Cleared)));
    assert_eq!(state.list_calls.load(Ordering::SeqCst), 0);
    assert!(drain.is_quiescent().unwrap());
}

#[test]
fn blocked_callback_and_retained_result_hold_drain_until_drop() {
    let source = report("blocked.md", b"# Blocked\n");
    let (access, state, gate) = probe(source);
    let (started, release) = state.block(Operation::List);
    let ticket = access.begin(Lane::Read).unwrap();
    let (completion_tx, completion_rx) = mpsc::channel();
    let waiter = tauri::async_runtime::spawn_blocking(move || {
        let completion = ticket.read(|backend| backend.list());
        assert!(completion_tx.send(completion).is_ok());
    });

    assert!(started.recv_timeout(WAIT).is_ok());
    drop(waiter);
    let drain = gate.request_drain().unwrap();
    assert!(!drain.is_quiescent().unwrap());
    assert!(release.send(()).is_ok());
    let completion = completion_rx.recv_timeout(WAIT).unwrap();
    assert!(!drain.is_quiescent().unwrap());
    drop(completion);
    assert!(drain.is_quiescent().unwrap());
}

#[test]
fn clear_refuses_late_picker_open_list_and_composition_content() {
    let source = report("late.md", b"# Late\n");
    let (access, state, _) = probe(source.clone());

    let picker = access.begin_selection().unwrap();
    assert!(matches!(access.clear_analysis(), WorkspaceResult::Cleared));
    assert_eq!(
        selection_code(
            picker
                .complete(SelectionResult::Analyzed {
                    report: Box::new(source.clone())
                })
                .deliver()
        ),
        "analysis_cleared"
    );

    for operation in [Operation::Open, Operation::List, Operation::Open] {
        let (started, release) = state.block(operation);
        let pending = match operation {
            Operation::Open if state.open_calls.load(Ordering::SeqCst) == 0 => {
                let access = access.clone();
                let id = source.source.id.clone();
                tauri::async_runtime::spawn(async move {
                    selection_code(access.open_snapshot(id).await)
                })
            }
            Operation::Open => {
                let access = access.clone();
                let id = source.source.id.clone();
                tauri::async_runtime::spawn(async move {
                    selection_code(access.read_composition_source(id).await)
                })
            }
            Operation::List => {
                let access = access.clone();
                tauri::async_runtime::spawn(
                    async move { workspace_code(access.list_snapshots().await) },
                )
            }
            Operation::Save => unreachable!(),
        };
        assert!(started.recv_timeout(WAIT).is_ok());
        assert!(matches!(access.clear_analysis(), WorkspaceResult::Cleared));
        assert!(release.send(()).is_ok());
        assert_eq!(
            tauri::async_runtime::block_on(pending).unwrap(),
            "analysis_cleared"
        );
    }
}

#[test]
fn read_only_picker_never_selects_source_and_discards_late_result() {
    let source = report("operations.md", b"# Existing source\n");
    let (access, _state, _gate) = probe(source.clone());

    let current = access.begin_selection().unwrap();
    assert_eq!(
        current.complete_read_only("inspected").deliver(|| "stale"),
        "inspected"
    );
    assert!(access.0.session.snapshot(&source.source.id).is_none());

    let pending = access.begin_selection().unwrap();
    assert!(matches!(access.clear_analysis(), WorkspaceResult::Cleared));
    assert_eq!(
        pending.complete_read_only("inspected").deliver(|| "stale"),
        "stale"
    );
    assert!(access.0.session.snapshot(&source.source.id).is_none());
}

#[test]
fn folder_listing_uses_counted_read_only_selection_and_fences_stale_delivery() {
    let (access, _, _) = probe(report("existing.md", b"# Existing\n"));
    let operation = access.begin_selection().unwrap();
    assert!(matches!(
        access.begin_selection(),
        Err(AdmissionError::Busy)
    ));
    let result = rangoon_host::FolderCollectionResult::Cancelled;
    let pending = operation.complete_read_only(result);
    assert!(matches!(access.clear_analysis(), WorkspaceResult::Cleared));
    let delivered = pending.deliver(rangoon_host::FolderCollectionResult::stale);
    assert_eq!(
        serde_json::to_value(delivered).unwrap()["error"]["code"],
        "folder_inventory_cleared"
    );
}

#[test]
fn operations_inspection_stops_before_read_when_picker_clears() {
    let (access, _, _) = probe(report("operations.md", b"# Source\n"));
    let picker = access.begin_selection().unwrap();
    let reads = Arc::new(AtomicUsize::new(0));
    let read_count = reads.clone();
    let access_for_picker = access.clone();
    let completed = crate::operations::inspect_selection(
        picker,
        move || {
            assert!(matches!(
                access_for_picker.clear_analysis(),
                WorkspaceResult::Cleared
            ));
            Ok(Some(PathBuf::from("/unused-operations-window.json")))
        },
        move |_| {
            read_count.fetch_add(1, Ordering::SeqCst);
            crate::operations::OperationsResult::Cancelled {
                schema_version: "rangoon.ops.local-inspection.v1",
            }
        },
    );
    let result = serde_json::to_value(completed.deliver(crate::operations::stale)).unwrap();
    assert_eq!(reads.load(Ordering::SeqCst), 0);
    assert_eq!(result["outcome"], "failed");
    assert_eq!(result["error"]["code"], "operations_selection_cleared");
}

#[test]
fn operations_inspection_discards_result_after_read_or_before_delivery() {
    let (access, _, _) = probe(report("operations.md", b"# Source\n"));
    let picker = access.begin_selection().unwrap();
    let access_for_read = access.clone();
    let completed = crate::operations::inspect_selection(
        picker,
        || Ok(Some(PathBuf::from("/unused-operations-window.json"))),
        move |_| {
            assert!(matches!(
                access_for_read.clear_analysis(),
                WorkspaceResult::Cleared
            ));
            crate::operations::OperationsResult::Cancelled {
                schema_version: "rangoon.ops.local-inspection.v1",
            }
        },
    );
    let result = serde_json::to_value(completed.deliver(crate::operations::stale)).unwrap();
    assert_eq!(result["error"]["code"], "operations_selection_cleared");

    let picker = access.begin_selection().unwrap();
    let completed = crate::operations::inspect_selection(
        picker,
        || Ok(None),
        |_| panic!("cancelled selection cannot inspect a file"),
    );
    assert!(matches!(access.clear_analysis(), WorkspaceResult::Cleared));
    let result = serde_json::to_value(completed.deliver(crate::operations::stale)).unwrap();
    assert_eq!(result["error"]["code"], "operations_selection_cleared");
}

#[test]
fn captured_save_survives_clear_but_closed_gate_reports_committed_without_metadata() {
    let source = report("save.md", b"# Save\nExact\n");
    let (access, state, gate) = probe(source.clone());
    select(&access, &source);
    let (started, release) = state.block(Operation::Save);
    let pending = {
        let access = access.clone();
        let id = source.source.id.clone();
        tauri::async_runtime::spawn(async move { access.save_analysis(id).await })
    };
    assert!(started.recv_timeout(WAIT).is_ok());
    assert!(matches!(access.clear_analysis(), WorkspaceResult::Cleared));
    let drain = gate.request_drain().unwrap();
    assert!(release.send(()).is_ok());
    let result = tauri::async_runtime::block_on(pending).unwrap();
    match &result {
        WorkspaceResult::Failed { error } => assert_eq!(error.code, "workspace_save_completed"),
        _ => panic!("expected committed-save notice"),
    }
    let serialized = serde_json::to_string(&result).unwrap();
    assert!(serialized.contains("workspace_save_completed"));
    assert!(serialized.contains("The save completed before the workspace closed"));
    assert!(!serialized.contains("snapshot"));
    assert!(!serialized.contains("sourceId"));
    assert!(!serialized.contains(&source.source.id));
    assert!(!serialized.contains(&source.source.display_name));
    assert!(!serialized.contains(&source.source.sha256));
    assert!(!serialized.contains(source.source.content.as_str()));
    assert_eq!(state.saved.lock().unwrap().as_slice(), [source]);
    assert!(drain.is_quiescent().unwrap());
}

#[test]
fn captured_save_survives_clear_without_drain_and_does_not_revive_import() {
    let source = report("save-without-drain.md", b"# Save\nExact retained copy\n");
    let (access, state, _) = probe(source.clone());
    select(&access, &source);
    let (started, release) = state.block(Operation::Save);
    let pending = {
        let access = access.clone();
        let id = source.source.id.clone();
        tauri::async_runtime::spawn(async move { access.save_analysis(id).await })
    };
    assert!(started.recv_timeout(WAIT).is_ok());
    assert!(matches!(access.clear_analysis(), WorkspaceResult::Cleared));
    assert!(access.0.session.snapshot(&source.source.id).is_none());
    assert!(release.send(()).is_ok());

    match tauri::async_runtime::block_on(pending).unwrap() {
        WorkspaceResult::Saved {
            snapshot,
            already_saved,
        } => {
            assert!(!already_saved);
            assert_eq!(snapshot.source_id, source.source.id);
            assert_eq!(snapshot.display_name, source.source.display_name);
            assert_eq!(snapshot.sha256, source.source.sha256);
            assert_eq!(snapshot.byte_length, source.source.content.len() as u32);
        }
        _ => panic!("expected captured save after source clear"),
    }
    assert_eq!(
        state.saved.lock().unwrap().as_slice(),
        std::slice::from_ref(&source)
    );
    assert!(access.0.session.snapshot(&source.source.id).is_none());
}

#[test]
fn uncertain_or_panicking_save_and_panicking_read_release_custody() {
    let source = report("uncertain.md", b"# Uncertain\n");
    let (access, state, gate) = probe(source.clone());
    select(&access, &source);
    *state.save_error.lock().unwrap() = Some(Error::WriteUncertain);
    assert_eq!(
        workspace_code(tauri::async_runtime::block_on(
            access.save_analysis(source.source.id.clone())
        )),
        "workspace_write_uncertain"
    );
    *state.save_error.lock().unwrap() = None;
    *state.panic.lock().unwrap() = Some(Operation::Save);
    assert_eq!(
        workspace_code(tauri::async_runtime::block_on(
            access.save_analysis(source.source.id.clone())
        )),
        "workspace_write_uncertain"
    );
    let read_source = report("panic-read.md", b"# Panic read\n");
    let (read_access, read_state, read_gate) = probe(read_source);
    *read_state.panic.lock().unwrap() = Some(Operation::List);
    assert_eq!(
        workspace_code(tauri::async_runtime::block_on(read_access.list_snapshots())),
        "workspace_unavailable"
    );
    assert_eq!(read_state.list_calls.load(Ordering::SeqCst), 1);
    let drain = gate.request_drain().unwrap();
    assert!(drain.is_quiescent().unwrap());
    let read_drain = read_gate.request_drain().unwrap();
    assert!(read_drain.is_quiescent().unwrap());
}

#[test]
fn dropping_unscheduled_ticket_releases_exclusive_custody() {
    let source = report("ticket.md", b"# Ticket\n");
    let (access, _, gate) = probe(source);
    let ticket = access.begin(Lane::Exclusive).unwrap();
    drop(ticket);
    let drain = gate.request_drain().unwrap();
    assert!(drain.is_quiescent().unwrap());
}

#[test]
fn session_overflow_and_poison_close_source_acceptance() {
    let source = report("session.md", b"# Session\n");
    let (access, _, _) = probe(source);
    access.0.session.seed_generation(u64::MAX);
    assert_eq!(
        workspace_code(access.clear_analysis()),
        "workspace_unavailable"
    );
    assert_eq!(
        workspace_code(tauri::async_runtime::block_on(access.list_snapshots())),
        "workspace_unavailable"
    );

    let source = report("poison.md", b"# Poison\n");
    let source_id = source.source.id.clone();
    let (access, _, _) = probe(source);
    access.0.session.poison_for_test();
    assert_eq!(
        selection_code(tauri::async_runtime::block_on(
            access.open_snapshot(source_id)
        )),
        "session_unavailable"
    );
}

#[test]
fn closed_admission_maps_to_unavailable_not_busy() {
    assert_eq!(
        workspace_code(admission_workspace(
            AdmissionError::Unavailable,
            "workspace_busy",
            "synthetic busy copy",
        )),
        "workspace_unavailable"
    );
    assert_eq!(
        selection_code(admission_selection(AdmissionError::Unavailable)),
        "session_unavailable"
    );
}

#[test]
fn source_binding_keeps_private_dispatch_and_native_owned_startup_factory() {
    let main = include_str!("main.rs");
    let source = include_str!("workspace_source_access.rs");
    let bootstrap = include_str!("workspace_native_bootstrap.rs");
    assert!(main.contains("mod workspace_source_access;"));
    assert!(
        main.contains("workspace_native_bootstrap::install_workbench_state(app.handle(), anchor)")
    );
    assert!(bootstrap.contains("SourceAccess::managed_deferred"));
    assert!(bootstrap.contains("SourceAccess::unavailable"));
    assert!(!bootstrap.contains("SourceAccess::new("));
    for call in [
        "sources.list_snapshots().await",
        "sources.save_analysis(source_id).await",
        "sources.open_snapshot(source_id).await",
        "sources.read_composition_source(source_id).await",
    ] {
        assert!(main.contains(call));
    }
    assert!(!main.contains("read_saved_composition_source"));
    assert!(
        source
            .contains("#[cfg(all(test, feature = \"encrypted-workspace\", unix))]\n    fn managed")
    );
    assert!(!source.contains("pub(super) fn managed<"));
    assert!(!main.contains("SourceAccess::managed_locked"));
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[derive(Default)]
struct ThreadVaultState {
    slots: BTreeMap<String, Option<Vec<u8>>>,
    opens: usize,
    substitute_identity: Option<rangoon_desktop::workspace_keys::WorkspaceId>,
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[derive(Clone, Default)]
struct ThreadVault(Arc<Mutex<ThreadVaultState>>);

#[cfg(all(feature = "encrypted-workspace", unix))]
struct ThreadSlot {
    identity: rangoon_desktop::workspace_keys::WorkspaceId,
    lookup: String,
    state: Arc<Mutex<ThreadVaultState>>,
}

#[cfg(all(feature = "encrypted-workspace", unix))]
impl ThreadVault {
    fn substitute_identity(&self, identity: rangoon_desktop::workspace_keys::WorkspaceId) {
        self.0.lock().unwrap().substitute_identity = Some(identity);
    }
}

#[cfg(all(feature = "encrypted-workspace", unix))]
impl rangoon_desktop::workspace_session::Vault for ThreadVault {
    type Slot = ThreadSlot;

    fn open(
        &self,
        identity: &rangoon_desktop::workspace_keys::WorkspaceId,
    ) -> Result<Self::Slot, rangoon_desktop::workspace_keys::Error> {
        let mut state = self.0.lock().unwrap();
        state.opens += 1;
        let lookup = identity.as_hex();
        state.slots.entry(lookup.clone()).or_insert(None);
        Ok(ThreadSlot {
            identity: state
                .substitute_identity
                .clone()
                .unwrap_or_else(|| identity.clone()),
            lookup,
            state: Arc::clone(&self.0),
        })
    }
}

#[cfg(all(feature = "encrypted-workspace", unix))]
impl rangoon_desktop::workspace_keys::Store for ThreadSlot {
    fn identity(&self) -> &rangoon_desktop::workspace_keys::WorkspaceId {
        &self.identity
    }

    fn read(
        &self,
    ) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>, rangoon_desktop::workspace_keys::Error> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .slots
            .get(&self.lookup)
            .cloned()
            .flatten()
            .map(zeroize::Zeroizing::new))
    }

    fn write(&self, envelope: &[u8]) -> Result<(), rangoon_desktop::workspace_keys::Error> {
        self.state
            .lock()
            .unwrap()
            .slots
            .insert(self.lookup.clone(), Some(envelope.to_vec()));
        Ok(())
    }
}

#[cfg(all(feature = "encrypted-workspace", unix))]
fn encrypted_access(root: &TestRoot, vault: &ThreadVault) -> (SourceAccess, String) {
    use rangoon_desktop::workspace_session::Owner;

    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let preview = owner.prepare_create_empty().unwrap();
    let identity = preview.workspace_id.clone();
    owner.confirm(&preview.id, vault).unwrap();
    (
        SourceAccess::managed(Gate::new(), owner, vault.clone()),
        identity,
    )
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn managed_owner_dispatches_encrypted_sources_and_reopens_without_plaintext() {
    use rangoon_desktop::workspace_session::Owner;

    let root = TestRoot::new("managed");
    let source = report(
        "managed.md",
        "\u{feff}# Managed\r\nsecret \u{1f980}\r\n".as_bytes(),
    );
    let vault = ThreadVault::default();
    let (access, identity) = encrypted_access(&root, &vault);
    select(&access, &source);
    assert!(matches!(
        tauri::async_runtime::block_on(access.save_analysis(source.source.id.clone())),
        WorkspaceResult::Saved { .. }
    ));
    match tauri::async_runtime::block_on(access.list_snapshots()) {
        WorkspaceResult::Listed { snapshots } => {
            assert_eq!(snapshots[0].source_id, source.source.id)
        }
        _ => panic!("expected managed list"),
    }
    match tauri::async_runtime::block_on(access.open_snapshot(source.source.id.clone())) {
        SelectionResult::Analyzed { report } => assert_eq!(*report, source),
        _ => panic!("expected managed open"),
    }
    drop(access);

    let database = root
        .0
        .join("encrypted-workspaces")
        .join(&identity)
        .join("workspace.sqlite3");
    let bytes = fs::read(database).unwrap();
    assert!(!bytes.starts_with(b"SQLite format 3\0"));
    let marker = b"secret \xf0\x9f\xa6\x80";
    assert!(!bytes.windows(marker.len()).any(|window| window == marker));

    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let unlock = owner.prepare_unlock().unwrap();
    owner.confirm(&unlock.id, &vault).unwrap();
    let reopened = SourceAccess::managed(Gate::new(), owner, vault);
    match tauri::async_runtime::block_on(reopened.open_snapshot(source.source.id.clone())) {
        SelectionResult::Analyzed { report } => assert_eq!(*report, source),
        _ => panic!("expected reopened managed source"),
    }
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn managed_locked_wrong_slot_and_bad_binding_fail_without_legacy_fallback() {
    use rangoon_desktop::{workspace_keys::WorkspaceId, workspace_session::Owner};

    let root = TestRoot::new("managed-refusal");
    let source = report("managed-refusal.md", b"# Managed refusal\n");
    let vault = ThreadVault::default();
    let owner = Owner::acquire(root.0.clone()).unwrap();
    let locked = SourceAccess::managed(Gate::new(), owner, vault.clone());
    assert_eq!(
        workspace_code(tauri::async_runtime::block_on(locked.list_snapshots())),
        "workspace_locked"
    );
    drop(locked);

    let (access, identity) = encrypted_access(&root, &vault);
    select(&access, &source);
    assert!(matches!(
        tauri::async_runtime::block_on(access.save_analysis(source.source.id.clone())),
        WorkspaceResult::Saved { .. }
    ));
    let database = root
        .0
        .join("encrypted-workspaces")
        .join(&identity)
        .join("workspace.sqlite3");
    let ciphertext = fs::read(&database).unwrap();
    vault.substitute_identity(WorkspaceId::parse(&"ab".repeat(32)).unwrap());
    assert_eq!(
        workspace_code(tauri::async_runtime::block_on(access.list_snapshots())),
        "workspace_unavailable"
    );
    assert_eq!(fs::read(&database).unwrap(), ciphertext);
    assert!(!root.0.join("workspace.sqlite3").exists());

    drop(access);
    vault.0.lock().unwrap().substitute_identity = None;
    let binding_root = TestRoot::new("managed-binding-refusal");
    let (binding_access, binding_identity) = encrypted_access(&binding_root, &vault);
    select(&binding_access, &source);
    assert!(matches!(
        tauri::async_runtime::block_on(binding_access.save_analysis(source.source.id.clone())),
        WorkspaceResult::Saved { .. }
    ));
    let binding_database = binding_root
        .0
        .join("encrypted-workspaces")
        .join(binding_identity)
        .join("workspace.sqlite3");
    let binding_ciphertext = fs::read(&binding_database).unwrap();
    fs::write(
        binding_root.0.join("encrypted-workspace.binding"),
        b"bad binding",
    )
    .unwrap();
    assert_eq!(
        workspace_code(tauri::async_runtime::block_on(
            binding_access.list_snapshots()
        )),
        "workspace_unavailable"
    );
    assert_eq!(fs::read(&binding_database).unwrap(), binding_ciphertext);
    assert!(!binding_root.0.join("workspace.sqlite3").exists());
}

#[path = "workspace_capability_access_tests.rs"]
mod workspace_capability_access;

#[path = "workspace_workflow_access_tests.rs"]
mod workspace_workflow_access;

#[path = "workspace_composition_access_tests.rs"]
mod composition_access;

#[path = "workspace_data_access_tests.rs"]
mod data_access;

#[path = "workspace_model_input_tests.rs"]
mod model_inputs;

#[path = "workspace_close_tests.rs"]
mod workspace_close;

#[cfg(unix)]
#[path = "workspace_reopen_tests.rs"]
mod reopen_controls;

#[cfg(unix)]
#[path = "workspace_close_control_tests.rs"]
mod supervisor_controls;

#[path = "workspace_failure_tests.rs"]
mod failure;

#[path = "workspace_native_progress_tests.rs"]
mod native_progress;

#[path = "workspace_exit_hold_tests.rs"]
mod exit_hold;

#[path = "workspace_request_access_tests.rs"]
mod request_admission;

#[cfg(unix)]
#[path = "workspace_setup_tests.rs"]
mod setup_commands;

#[cfg(unix)]
#[path = "workspace_protection_access_tests.rs"]
mod protection_status;

#[cfg(all(feature = "encrypted-workspace", unix))]
#[path = "workspace_managed_bootstrap_tests.rs"]
mod managed_bootstrap_tests;

#[cfg(unix)]
#[path = "workspace_recovery_file_tests.rs"]
mod recovery_files;
