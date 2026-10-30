use super::*;

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum CapabilityOperation {
    List,
    Open,
    Create,
    Revise,
    Review,
}

struct CapabilityBlock {
    operation: CapabilityOperation,
    started: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}

#[derive(Default)]
struct CapabilityProbeState {
    list_calls: AtomicUsize,
    open_calls: AtomicUsize,
    create_calls: AtomicUsize,
    revise_calls: AtomicUsize,
    review_calls: AtomicUsize,
    block: Mutex<Option<CapabilityBlock>>,
    panic: Mutex<Option<CapabilityOperation>>,
    error: Mutex<Option<(CapabilityOperation, Error)>>,
}

impl CapabilityProbeState {
    fn block(&self, operation: CapabilityOperation) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        *self.block.lock().unwrap() = Some(CapabilityBlock {
            operation,
            started: started_tx,
            release: release_rx,
        });
        (started_rx, release_tx)
    }

    fn before(&self, operation: CapabilityOperation) -> Result<(), Error> {
        match operation {
            CapabilityOperation::List => self.list_calls.fetch_add(1, Ordering::SeqCst),
            CapabilityOperation::Open => self.open_calls.fetch_add(1, Ordering::SeqCst),
            CapabilityOperation::Create => self.create_calls.fetch_add(1, Ordering::SeqCst),
            CapabilityOperation::Revise => self.revise_calls.fetch_add(1, Ordering::SeqCst),
            CapabilityOperation::Review => self.review_calls.fetch_add(1, Ordering::SeqCst),
        };
        if self.panic.lock().unwrap().as_ref() == Some(&operation) {
            panic!("synthetic capability backend panic");
        }
        if let Some((_, error)) = self
            .error
            .lock()
            .unwrap()
            .as_ref()
            .filter(|(target, _)| *target == operation)
        {
            return Err(*error);
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
        Ok(())
    }
}

struct CapabilityProbeBackend {
    state: Arc<CapabilityProbeState>,
    root: PathBuf,
}

impl CapabilityProbeBackend {
    fn workspace(&self) -> Workspace {
        Workspace::new(self.root.clone())
    }
}

impl Backend for CapabilityProbeBackend {
    fn list(&mut self) -> Result<Vec<SnapshotMetadata>, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }

    fn open(&mut self, _source_id: &str) -> Result<AnalysisReport, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }

    fn save(&mut self, _report: &AnalysisReport) -> Result<SaveReceipt, Error> {
        Err(Error::Store(StoreError::Unavailable))
    }

    fn list_capabilities(&mut self) -> Result<Vec<CapabilitySummary>, Error> {
        self.state.before(CapabilityOperation::List)?;
        self.workspace()
            .list_capabilities_v1()
            .map_err(Error::Store)
    }

    fn open_capability(
        &mut self,
        id: &str,
        revision: Option<&str>,
    ) -> Result<CapabilityDetail, Error> {
        self.state.before(CapabilityOperation::Open)?;
        self.workspace()
            .open_capability_v1(id, revision)
            .map_err(Error::Store)
    }

    fn create_capability(
        &mut self,
        source: &str,
        fragment: &str,
        title: &str,
    ) -> Result<CapabilityReceiptV1, Error> {
        self.state.before(CapabilityOperation::Create)?;
        self.workspace()
            .create_capability_v1(source, fragment, title)
            .map_err(Error::Store)
    }

    fn revise_capability(
        &mut self,
        id: &str,
        expected: &str,
        title: &str,
        content: &str,
    ) -> Result<CapabilityReceiptV1, Error> {
        self.state.before(CapabilityOperation::Revise)?;
        self.workspace()
            .revise_capability_v1(id, expected, title, content)
            .map_err(Error::Store)
    }

    fn review_capability(
        &mut self,
        id: &str,
        expected: &str,
    ) -> Result<CapabilityReceiptV1, Error> {
        self.state.before(CapabilityOperation::Review)?;
        self.workspace()
            .review_capability_v1(id, expected)
            .map_err(Error::Store)
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

fn capability_probe(root: &TestRoot) -> (SourceAccess, Arc<CapabilityProbeState>, Gate) {
    let gate = Gate::new();
    let state = Arc::new(CapabilityProbeState::default());
    let access = SourceAccess::new(
        gate.clone(),
        Box::new(CapabilityProbeBackend {
            state: Arc::clone(&state),
            root: root.0.clone(),
        }),
    );
    (access, state, gate)
}

fn capability_code(result: CapabilityResult) -> &'static str {
    match result {
        CapabilityResult::Failed { error } => error.code,
        _ => panic!("expected failed capability result"),
    }
}

fn opened(result: CapabilityResult) -> (CapabilityDetail, bool) {
    match result {
        CapabilityResult::Opened {
            capability,
            already_applied,
        } => (*capability, already_applied),
        _ => panic!("expected opened capability"),
    }
}

fn save_source(root: &TestRoot) -> AnalysisReport {
    let source = report(
        "capability-access.md",
        "\u{feff}# Capability\r\nExact source \u{1f980}\r\n".as_bytes(),
    );
    Workspace::new(root.0.clone()).save_v1(&source).unwrap();
    source
}

#[test]
fn legacy_capability_routing_preserves_records_history_provenance_and_import_clear_independence() {
    let root = TestRoot::new("capability-legacy");
    let source = save_source(&root);
    let access = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));

    let (created, duplicate) = opened(tauri::async_runtime::block_on(access.create_capability(
        source.source.id.clone(),
        source.fragments[0].id.clone(),
        "Exact capability \u{1f980}".into(),
    )));
    assert!(!duplicate);
    assert_eq!(
        created.revision.content,
        "\u{feff}# Capability\r\nExact source \u{1f980}\r\n"
    );
    assert_eq!(created.history.len(), 1);
    assert!(matches!(
        created.origin,
        rangoon_domain::capability_v1::Origin::Source { .. }
    ));
    assert!(
        opened(tauri::async_runtime::block_on(access.create_capability(
            source.source.id.clone(),
            source.fragments[0].id.clone(),
            "Exact capability \u{1f980}".into(),
        )))
        .1
    );

    select(&access, &source);
    assert!(matches!(access.clear_analysis(), WorkspaceResult::Cleared));
    let listed = tauri::async_runtime::block_on(access.list_capabilities());
    match listed {
        CapabilityResult::Listed { capabilities } => {
            assert_eq!(capabilities.len(), 1);
            assert_eq!(capabilities[0].id, created.id);
            assert_eq!(
                capabilities[0].latest_revision_id,
                created.latest_revision_id
            );
        }
        _ => panic!("expected capability list"),
    }

    let historical = created.latest_revision_id.clone();
    let (revised, already_applied) =
        opened(tauri::async_runtime::block_on(access.revise_capability(
            created.id.clone(),
            historical.clone(),
            "Exact revision".into(),
            "Changed exact content \u{1f980}\n".into(),
        )));
    assert!(!already_applied);
    assert_eq!(revised.history.len(), 2);
    assert_eq!(revised.history[0].id, historical);
    assert!(
        opened(tauri::async_runtime::block_on(access.revise_capability(
            revised.id.clone(),
            created.latest_revision_id.clone(),
            "Exact revision".into(),
            "Changed exact content \u{1f980}\n".into(),
        )))
        .1
    );
    let (reviewed, reviewed_again) = opened(tauri::async_runtime::block_on(
        access.review_capability(revised.id.clone(), revised.latest_revision_id.clone()),
    ));
    assert!(!reviewed_again);
    assert!(reviewed.revision.review.is_some());
    assert!(
        opened(tauri::async_runtime::block_on(access.review_capability(
            reviewed.id.clone(),
            reviewed.latest_revision_id.clone(),
        )))
        .1
    );
    let (old, _) = opened(tauri::async_runtime::block_on(
        access.open_capability(reviewed.id.clone(), Some(historical)),
    ));
    assert_eq!(
        old.revision.content,
        "\u{feff}# Capability\r\nExact source \u{1f980}\r\n"
    );
}

#[test]
fn capability_access_refuses_stale_preaction_and_redacts_late_read_metadata() {
    let root = TestRoot::new("capability-stale-read");
    save_source(&root);
    let (access, state, gate) = capability_probe(&root);
    let ticket = access.begin_capability(Lane::Read).unwrap();
    let drain = gate.request_drain().unwrap();
    let completed = ticket.read(|backend| backend.list_capabilities());
    assert!(matches!(
        completed.deliver(),
        Err(Error::Store(StoreError::Unavailable))
    ));
    assert_eq!(state.list_calls.load(Ordering::SeqCst), 0);
    assert!(drain.is_quiescent().unwrap());

    let (access, state, gate) = capability_probe(&root);
    let (started, release) = state.block(CapabilityOperation::List);
    let ticket = access.begin_capability(Lane::Read).unwrap();
    let (completion_tx, completion_rx) = mpsc::channel();
    let waiter = tauri::async_runtime::spawn_blocking(move || {
        let completion = ticket.read(|backend| backend.list_capabilities());
        assert!(completion_tx.send(completion).is_ok());
    });
    assert!(started.recv_timeout(WAIT).is_ok());
    let drain = gate.request_drain().unwrap();
    assert!(!drain.is_quiescent().unwrap());
    assert!(release.send(()).is_ok());
    let completion = completion_rx.recv_timeout(WAIT).unwrap();
    drop(waiter);
    assert!(matches!(
        completion.deliver(),
        Err(Error::Store(StoreError::Unavailable))
    ));
    assert!(drain.is_quiescent().unwrap());
}

#[test]
fn capability_callback_and_retained_result_hold_drain_through_dropped_tauri_waiter() {
    let root = TestRoot::new("capability-drain");
    save_source(&root);
    let (access, state, gate) = capability_probe(&root);
    let (started, release) = state.block(CapabilityOperation::List);
    let ticket = access.begin_capability(Lane::Read).unwrap();
    let (completion_tx, completion_rx) = mpsc::channel();
    let waiter = tauri::async_runtime::spawn_blocking(move || {
        let completion = ticket.read(|backend| backend.list_capabilities());
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
fn admitted_read_refuses_while_backend_busy_and_gate_closes() {
    let root = TestRoot::new("capability-queued-stale");
    save_source(&root);
    let (access, state, gate) = capability_probe(&root);
    let (started, release) = state.block(CapabilityOperation::List);
    let first = access.begin_capability(Lane::Read).unwrap();
    let second = access.begin_capability(Lane::Read).unwrap();
    let (first_tx, first_rx) = mpsc::channel();
    let (second_tx, second_rx) = mpsc::channel();
    tauri::async_runtime::spawn_blocking(move || {
        assert!(
            first_tx
                .send(first.read(|backend| backend.list_capabilities()))
                .is_ok()
        );
    });
    assert!(started.recv_timeout(WAIT).is_ok());
    tauri::async_runtime::spawn_blocking(move || {
        assert!(
            second_tx
                .send(second.read(|backend| backend.list_capabilities()))
                .is_ok()
        );
    });
    let drain = gate.request_drain().unwrap();
    assert!(release.send(()).is_ok());
    let first = first_rx.recv_timeout(WAIT).unwrap();
    let second = second_rx.recv_timeout(WAIT).unwrap();
    assert!(matches!(
        first.deliver(),
        Err(Error::Store(StoreError::Unavailable))
    ));
    assert!(matches!(
        second.deliver(),
        Err(Error::Store(StoreError::Unavailable))
    ));
    assert_eq!(state.list_calls.load(Ordering::SeqCst), 1);
    assert!(drain.is_quiescent().unwrap());
}

#[test]
fn capability_stale_preaction_mutation_refuses_without_backend_call() {
    let root = TestRoot::new("capability-stale-mutation");
    let source = save_source(&root);
    let (access, state, gate) = capability_probe(&root);
    let ticket = access.begin_capability(Lane::Exclusive).unwrap();
    let drain = gate.request_drain().unwrap();
    let completion = ticket.mutate(|backend| {
        backend.create_capability(
            &source.source.id,
            &source.fragments[0].id,
            "must not reach backend",
        )
    });
    assert_eq!(
        capability_code(completion.deliver()),
        "workspace_unavailable"
    );
    assert_eq!(state.create_calls.load(Ordering::SeqCst), 0);
    assert!(drain.is_quiescent().unwrap());
}

#[test]
fn capability_stale_mutation_reports_committed_and_uncertain_mutation_never_retries() {
    let root = TestRoot::new("capability-stale-write");
    let source = save_source(&root);
    let (access, state, gate) = capability_probe(&root);
    let (started, release) = state.block(CapabilityOperation::Create);
    let ticket = access.begin_capability(Lane::Exclusive).unwrap();
    let source_id = source.source.id.clone();
    let fragment_id = source.fragments[0].id.clone();
    let (completion_tx, completion_rx) = mpsc::channel();
    tauri::async_runtime::spawn_blocking(move || {
        let completion = ticket.mutate(|backend| {
            backend.create_capability(&source_id, &fragment_id, "Late committed")
        });
        assert!(completion_tx.send(completion).is_ok());
    });
    assert!(started.recv_timeout(WAIT).is_ok());
    let drain = gate.request_drain().unwrap();
    assert!(release.send(()).is_ok());
    let completed = completion_rx.recv_timeout(WAIT).unwrap().deliver();
    let serialized = serde_json::to_string(&completed).unwrap();
    assert_eq!(capability_code(completed), "workspace_write_completed");
    for forbidden in [
        source.source.id.as_str(),
        source.source.display_name.as_str(),
        source.source.sha256.as_str(),
        source.source.content.as_str(),
        "Late committed",
    ] {
        assert!(!serialized.contains(forbidden));
    }
    assert_eq!(state.create_calls.load(Ordering::SeqCst), 1);
    assert!(drain.is_quiescent().unwrap());
    let stored = Workspace::new(root.0.clone())
        .list_capabilities_v1()
        .unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].title, "Late committed");

    let (access, state, _) = capability_probe(&root);
    *state.error.lock().unwrap() = Some((CapabilityOperation::Review, Error::WriteUncertain));
    assert_eq!(
        capability_code(tauri::async_runtime::block_on(access.review_capability(
            "capability:invalid".into(),
            "revision:invalid".into(),
        ))),
        "workspace_write_uncertain"
    );
    assert_eq!(state.review_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn capability_read_and_mutation_panics_preserve_their_distinct_public_outcomes() {
    let root = TestRoot::new("capability-panics");
    let source = save_source(&root);
    let (read_access, read_state, _) = capability_probe(&root);
    *read_state.panic.lock().unwrap() = Some(CapabilityOperation::List);
    assert_eq!(
        capability_code(tauri::async_runtime::block_on(
            read_access.list_capabilities()
        )),
        "workspace_unavailable"
    );
    assert_eq!(read_state.list_calls.load(Ordering::SeqCst), 1);

    let (write_access, write_state, _) = capability_probe(&root);
    *write_state.panic.lock().unwrap() = Some(CapabilityOperation::Create);
    assert_eq!(
        capability_code(tauri::async_runtime::block_on(
            write_access.create_capability(
                source.source.id,
                source.fragments[0].id.clone(),
                "Panic".into(),
            )
        )),
        "workspace_write_uncertain"
    );
    assert_eq!(write_state.create_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn capability_legacy_malformed_arguments_stay_bounded_and_do_not_create_records() {
    let root = TestRoot::new("capability-malformed");
    let source = save_source(&root);
    let access = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));
    let invalid_titles = vec![
        String::new(),
        " leading".into(),
        "trailing ".into(),
        "line\nfeed".into(),
        "separator\u{2028}".into(),
        "é".repeat(81),
    ];
    for title in invalid_titles {
        assert_eq!(
            capability_code(tauri::async_runtime::block_on(access.create_capability(
                source.source.id.clone(),
                source.fragments[0].id.clone(),
                title,
            ))),
            "capability_invalid"
        );
    }
    for (source_id, fragment_id) in [
        ("source:not-a-canonical-id", source.fragments[0].id.as_str()),
        (source.source.id.as_str(), "fragment:not-a-canonical-id"),
    ] {
        assert_eq!(
            capability_code(tauri::async_runtime::block_on(access.create_capability(
                source_id.into(),
                fragment_id.into(),
                "Valid selector test".into(),
            ))),
            "capability_invalid"
        );
    }
    let (created, _) = opened(tauri::async_runtime::block_on(access.create_capability(
        source.source.id.clone(),
        source.fragments[0].id.clone(),
        "Valid bounded capability".into(),
    )));
    for (title, content) in [
        (String::new(), "valid content".into()),
        (" leading".into(), "valid content".into()),
        ("line\nfeed".into(), "valid content".into()),
        ("Valid".into(), String::new()),
        ("Valid".into(), " \r\n".into()),
        ("Valid".into(), "nul\0content".into()),
    ] {
        assert_eq!(
            capability_code(tauri::async_runtime::block_on(access.revise_capability(
                created.id.clone(),
                created.latest_revision_id.clone(),
                title,
                content,
            ))),
            "capability_invalid"
        );
    }
    for (capability_id, revision_id) in [
        (
            "capability:not-a-canonical-id",
            created.latest_revision_id.as_str(),
        ),
        (created.id.as_str(), "revision:not-a-canonical-id"),
    ] {
        assert_eq!(
            capability_code(tauri::async_runtime::block_on(
                access.open_capability(capability_id.into(), Some(revision_id.into()),)
            )),
            "capability_invalid"
        );
        assert_eq!(
            capability_code(tauri::async_runtime::block_on(access.revise_capability(
                capability_id.into(),
                revision_id.into(),
                "Valid".into(),
                "valid content".into(),
            ))),
            "capability_invalid"
        );
        assert_eq!(
            capability_code(tauri::async_runtime::block_on(
                access.review_capability(capability_id.into(), revision_id.into(),)
            )),
            "capability_invalid"
        );
    }
    match tauri::async_runtime::block_on(access.list_capabilities()) {
        CapabilityResult::Listed { capabilities } => {
            assert_eq!(capabilities.len(), 1);
            assert_eq!(capabilities[0].id, created.id);
        }
        _ => panic!("expected bounded capability list"),
    }
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn managed_capability_access_encrypts_reopens_and_refuses_locked_wrong_slot_and_binding() {
    use rangoon_desktop::{workspace_keys::WorkspaceId, workspace_session::Owner};

    let root = TestRoot::new("managed-capability-access");
    let source = report(
        "managed-capability.md",
        b"# Managed capability\nExact secret \xF0\x9F\xA6\x80\n",
    );
    let vault = ThreadVault::default();
    let locked = SourceAccess::managed(
        Gate::new(),
        Owner::acquire(root.0.clone()).unwrap(),
        vault.clone(),
    );
    assert_eq!(
        capability_code(tauri::async_runtime::block_on(locked.list_capabilities())),
        "workspace_locked"
    );
    drop(locked);
    let (access, identity) = encrypted_access(&root, &vault);
    select(&access, &source);
    assert!(matches!(
        tauri::async_runtime::block_on(access.save_analysis(source.source.id.clone())),
        WorkspaceResult::Saved { .. }
    ));
    let (created, _) = opened(tauri::async_runtime::block_on(access.create_capability(
        source.source.id.clone(),
        source.fragments[0].id.clone(),
        "Managed capability".into(),
    )));
    let (revised, _) = opened(tauri::async_runtime::block_on(access.revise_capability(
        created.id.clone(),
        created.latest_revision_id.clone(),
        "Managed revision".into(),
        "Exact managed revision \u{1f980}\n".into(),
    )));
    let (reviewed, _) = opened(tauri::async_runtime::block_on(
        access.review_capability(revised.id.clone(), revised.latest_revision_id.clone()),
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
            .windows(b"Exact secret \xF0\x9F\xA6\x80".len())
            .any(|bytes| bytes == b"Exact secret \xF0\x9F\xA6\x80")
    );
    drop(access);

    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let unlock = owner.prepare_unlock().unwrap();
    owner.confirm(&unlock.id, &vault).unwrap();
    let reopened = SourceAccess::managed(Gate::new(), owner, vault.clone());
    match tauri::async_runtime::block_on(reopened.list_capabilities()) {
        CapabilityResult::Listed { capabilities } => {
            assert_eq!(capabilities.len(), 1);
            assert_eq!(capabilities[0].id, reviewed.id);
            assert_eq!(
                capabilities[0].latest_revision_id,
                reviewed.latest_revision_id
            );
        }
        _ => panic!("expected reopened managed list"),
    }
    let (latest, _) = opened(tauri::async_runtime::block_on(
        reopened.open_capability(reviewed.id.clone(), None),
    ));
    assert_eq!(latest, reviewed);
    let (restored, _) = opened(tauri::async_runtime::block_on(reopened.open_capability(
        reviewed.id.clone(),
        Some(created.latest_revision_id.clone()),
    )));
    assert_eq!(
        restored.revision.content,
        "# Managed capability\nExact secret \u{1f980}\n"
    );
    vault.substitute_identity(WorkspaceId::parse(&"ab".repeat(32)).unwrap());
    assert_eq!(
        capability_code(tauri::async_runtime::block_on(reopened.list_capabilities())),
        "workspace_unavailable"
    );
    assert_eq!(std::fs::read(&database).unwrap(), ciphertext);
    assert!(!root.0.join("workspace.sqlite3").exists());
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn managed_capability_malformed_attempt_fences_recovery_without_retry_or_fallback() {
    let root = TestRoot::new("managed-capability-recovery");
    let source = report("managed-recovery.md", b"# Managed recovery\nExact bytes\n");
    let vault = ThreadVault::default();
    let (access, identity) = encrypted_access(&root, &vault);
    select(&access, &source);
    assert!(matches!(
        tauri::async_runtime::block_on(access.save_analysis(source.source.id.clone())),
        WorkspaceResult::Saved { .. }
    ));
    let database = root
        .0
        .join("encrypted-workspaces")
        .join(identity)
        .join("workspace.sqlite3");
    let before = std::fs::read(&database).unwrap();
    assert_eq!(
        capability_code(tauri::async_runtime::block_on(access.create_capability(
            source.source.id.clone(),
            source.fragments[0].id.clone(),
            " malformed".into(),
        ))),
        "workspace_write_uncertain"
    );
    assert_eq!(std::fs::read(&database).unwrap(), before);
    let opens = vault.0.lock().unwrap().opens;
    assert_eq!(
        capability_code(tauri::async_runtime::block_on(access.list_capabilities())),
        "workspace_recovery_required"
    );
    assert_eq!(
        capability_code(tauri::async_runtime::block_on(
            access.open_capability("capability:invalid".into(), None,)
        )),
        "workspace_recovery_required"
    );
    assert_eq!(
        capability_code(tauri::async_runtime::block_on(access.create_capability(
            source.source.id.clone(),
            source.fragments[0].id.clone(),
            "Another".into(),
        ))),
        "workspace_recovery_required"
    );
    assert_eq!(
        capability_code(tauri::async_runtime::block_on(access.revise_capability(
            "capability:invalid".into(),
            "revision:invalid".into(),
            "Another".into(),
            "content".into(),
        ))),
        "workspace_recovery_required"
    );
    assert_eq!(
        capability_code(tauri::async_runtime::block_on(access.review_capability(
            "capability:invalid".into(),
            "revision:invalid".into(),
        ))),
        "workspace_recovery_required"
    );
    assert_eq!(vault.0.lock().unwrap().opens, opens);
    assert_eq!(std::fs::read(&database).unwrap(), before);
    assert!(!root.0.join("workspace.sqlite3").exists());
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn managed_capability_bad_binding_refuses_without_legacy_fallback() {
    let root = TestRoot::new("managed-capability-binding");
    let source = report("managed-binding.md", b"# Managed binding\nExact bytes\n");
    let vault = ThreadVault::default();
    let (access, identity) = encrypted_access(&root, &vault);
    select(&access, &source);
    assert!(matches!(
        tauri::async_runtime::block_on(access.save_analysis(source.source.id.clone())),
        WorkspaceResult::Saved { .. }
    ));
    assert!(matches!(
        tauri::async_runtime::block_on(access.create_capability(
            source.source.id.clone(),
            source.fragments[0].id.clone(),
            "Binding fixture".into(),
        )),
        CapabilityResult::Opened { .. }
    ));
    let database = root
        .0
        .join("encrypted-workspaces")
        .join(identity)
        .join("workspace.sqlite3");
    let ciphertext = std::fs::read(&database).unwrap();
    std::fs::write(root.0.join("encrypted-workspace.binding"), b"bad binding").unwrap();
    assert_eq!(
        capability_code(tauri::async_runtime::block_on(access.list_capabilities())),
        "workspace_unavailable"
    );
    assert_eq!(std::fs::read(&database).unwrap(), ciphertext);
    assert!(!root.0.join("workspace.sqlite3").exists());
}

#[test]
fn capability_commands_bind_only_to_typed_source_access_forwarders() {
    let source = include_str!("capabilities.rs");
    let normalized: String = source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert_eq!(normalized.matches("app.state::<SourceAccess>()").count(), 5);
    for forwarder in [
        "pubasyncfnlist_capabilities(workspace:crate::workspace_requests::WorkspaceRequest,app:AppHandle,)->CapabilityResult{letaccess=app.state::<SourceAccess>().for_request(&workspace);access.list_capabilities().await}",
        "pubasyncfnopen_capability(workspace:crate::workspace_requests::WorkspaceRequest,app:AppHandle,capability_id:String,revision_id:Option<String>,)->CapabilityResult{letaccess=app.state::<SourceAccess>().for_request(&workspace);access.open_capability(capability_id,revision_id).await}",
        "pubasyncfncreate_capability(workspace:crate::workspace_requests::WorkspaceRequest,app:AppHandle,source_id:String,fragment_id:String,title:String,)->CapabilityResult{letaccess=app.state::<SourceAccess>().for_request(&workspace);access.create_capability(source_id,fragment_id,title).await}",
        "pubasyncfnrevise_capability(workspace:crate::workspace_requests::WorkspaceRequest,app:AppHandle,capability_id:String,expected_revision_id:String,title:String,content:String,)->CapabilityResult{letaccess=app.state::<SourceAccess>().for_request(&workspace);access.revise_capability(capability_id,expected_revision_id,title,content).await}",
        "pubasyncfnreview_capability(workspace:crate::workspace_requests::WorkspaceRequest,app:AppHandle,capability_id:String,expected_revision_id:String,)->CapabilityResult{letaccess=app.state::<SourceAccess>().for_request(&workspace);access.review_capability(capability_id,expected_revision_id).await}",
    ] {
        assert!(normalized.contains(forwarder));
    }
    for forbidden in [
        "workspace(",
        "Workspace::",
        "dispatch_read",
        "begin_operation",
    ] {
        assert!(!source.contains(forbidden));
    }
}
