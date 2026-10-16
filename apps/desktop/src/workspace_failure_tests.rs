#[cfg(feature = "encrypted-workspace")]
use super::*;

#[cfg(feature = "encrypted-workspace")]
use crate::cloud_custody::{Custody, Phase as CredentialPhase};
#[cfg(feature = "encrypted-workspace")]
use crate::workspace_close::{
    View, WorkspaceClose,
    failure::{Host, Monitor, Progress, wait_for_progress},
};
#[cfg(feature = "encrypted-workspace")]
use rangoon_desktop::workspace_session::Owner;
#[cfg(feature = "encrypted-workspace")]
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex, TryLockError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Instant,
};

#[cfg(feature = "encrypted-workspace")]
fn view_json(view: View) -> serde_json::Value {
    serde_json::to_value(view).unwrap()
}

#[cfg(feature = "encrypted-workspace")]
fn locked_managed(root: &TestRoot, vault: &ThreadVault) -> SourceAccess {
    SourceAccess::managed(
        Gate::new(),
        Owner::acquire(root.0.clone()).unwrap(),
        vault.clone(),
    )
}

#[cfg(feature = "encrypted-workspace")]
fn assert_terminal(access: &SourceAccess) {
    assert!(access.failure_latched());
    assert!(matches!(
        access.begin(Lane::Read),
        Err(AdmissionError::Unavailable)
    ));
    let ticket = access.request_close().unwrap();
    assert_eq!(ticket.is_quiescent(), Ok(true));
    assert!(!ticket.purge());
    assert_eq!(ticket.release_key(), Ok(false));
}

#[cfg(feature = "encrypted-workspace")]
fn local_profile() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "schemaVersion": "rangoon.local-profile-request.v1",
        "profileId": "failure.fixture.local",
        "host": "127.0.0.1",
        "port": 11434,
        "model": "fixture:v1",
        "maxOutputTokens": 256,
    }))
    .unwrap()
}

#[cfg(feature = "encrypted-workspace")]
fn local_selection(source_id: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "schemaVersion": "rangoon.local-selection.v1",
        "task": "classify_v1",
        "inputs": [{"kind": "source", "sourceId": source_id}],
    }))
    .unwrap()
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn locked_actual_owner_latches_every_source_boundary_and_blocks_model_inputs() {
    for boundary in ["list", "open", "save"] {
        let root = TestRoot::new(&format!("failure-owner-{boundary}"));
        let vault = ThreadVault::default();
        let access = locked_managed(&root, &vault);
        let source = report("failure.md", b"# Failure\nExact bytes\n");
        let result = match boundary {
            "list" => tauri::async_runtime::block_on(access.list_snapshots()),
            "open" => {
                let result =
                    tauri::async_runtime::block_on(access.open_snapshot(source.source.id.clone()));
                assert!(matches!(result, SelectionResult::Failed { .. }));
                assert_terminal(&access);
                assert!(access.begin_model_inputs().is_err());
                continue;
            }
            "save" => {
                select(&access, &source);
                tauri::async_runtime::block_on(access.save_analysis(source.source.id.clone()))
            }
            _ => unreachable!(),
        };
        assert!(matches!(result, WorkspaceResult::Failed { .. }));
        assert_terminal(&access);
        assert!(access.begin_model_inputs().is_err());
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn healthy_validation_refusal_keeps_actual_owner_open() {
    let root = TestRoot::new("failure-healthy-validation");
    let vault = ThreadVault::default();
    let (access, _) = encrypted_access(&root, &vault);
    let invalid = tauri::async_runtime::block_on(access.open_snapshot("invalid".into()));
    assert!(matches!(invalid, SelectionResult::Failed { .. }));
    assert!(!access.failure_latched());
    assert!(matches!(
        access
            .begin_capability(Lane::Read)
            .unwrap()
            .call(|backend| {
                assert!(backend.control_is_open());
                Err::<(), _>(Error::Store(StoreError::InvalidId))
            }),
        Err(Error::Store(StoreError::InvalidId))
    ));
    assert!(!access.failure_latched());
    assert!(matches!(
        tauri::async_runtime::block_on(access.list_snapshots()),
        WorkspaceResult::Listed { .. }
    ));
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn open_owner_integrity_failure_purges_selection_through_actual_model_reader() {
    let root = TestRoot::new("failure-open-owner-model-reader");
    let vault = ThreadVault::default();
    let (access, identity) = encrypted_access(&root, &vault);
    let source = report("actual-reader.md", b"# Actual reader\nRetained content\n");
    select(&access, &source);
    assert!(matches!(
        tauri::async_runtime::block_on(access.save_analysis(source.source.id.clone())),
        WorkspaceResult::Saved { .. }
    ));
    assert!(access.0.session.snapshot(&source.source.id).is_some());

    vault.0.lock().unwrap().slots.remove(&identity);
    let session = rangoon_model_session::LocalSession::default();
    session.configure(&local_profile()).unwrap();
    let raw = local_selection(&source.source.id);
    assert!(matches!(
        access
            .begin_model_inputs()
            .unwrap()
            .with_inputs(|reader| session.begin_prepare(&raw)?.prepare_with_inputs(reader))
            .deliver(),
        Err(rangoon_model_session::Diagnostic::InputUnavailable)
    ));
    assert_terminal(&access);
    assert!(access.0.session.snapshot(&source.source.id).is_none());
    assert!(matches!(
        tauri::async_runtime::block_on(access.save_analysis(source.source.id.clone())),
        WorkspaceResult::Failed { .. }
    ));
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn terminal_failure_notifies_once_and_close_progress_never_claims_locked() {
    let root = TestRoot::new("failure-notify-close");
    let vault = ThreadVault::default();
    let access = locked_managed(&root, &vault);
    let notifications = Arc::new(AtomicUsize::new(0));
    access
        .set_failure_notifier({
            let notifications = Arc::clone(&notifications);
            Arc::new(move || {
                notifications.fetch_add(1, Ordering::SeqCst);
            })
        })
        .unwrap();
    assert!(access.set_failure_notifier(Arc::new(|| {})).is_err());

    let _ = tauri::async_runtime::block_on(access.list_snapshots());
    assert_terminal(&access);
    assert_eq!(notifications.load(Ordering::SeqCst), 1);
    let close = WorkspaceClose::new(access);
    let cleanup_calls = AtomicUsize::new(0);
    let first = view_json(close.inspect(|_| {
        cleanup_calls.fetch_add(1, Ordering::SeqCst);
        true
    }));
    assert_eq!(first["state"], "recovery_required");
    assert!(first["attemptId"].is_string());
    assert_eq!(cleanup_calls.load(Ordering::SeqCst), 1);
    assert_eq!(close.progress_failure(|_| true), Ok(true));
    let view = view_json(close.inspect(|_| true));
    assert_eq!(view["state"], "recovery_required");
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn backend_panic_or_poison_latches_after_the_backend_guard_path() {
    for poisoned in [false, true] {
        let gate = Gate::new();
        let state = Arc::new(ProbeState::default());
        let access = SourceAccess::with_mode(
            gate,
            Box::new(ProbeBackend::new(
                Arc::clone(&state),
                report("failure-probe.md", b"# Probe\n"),
            )),
            true,
        );
        let post_guard = Arc::new(AtomicUsize::new(0));
        access
            .set_failure_notifier({
                let backend = Arc::downgrade(&access.0);
                let post_guard = Arc::clone(&post_guard);
                Arc::new(move || {
                    let inner = backend.upgrade().unwrap();
                    assert!(!matches!(
                        inner.backend.try_lock(),
                        Err(TryLockError::WouldBlock)
                    ));
                    post_guard.fetch_add(1, Ordering::SeqCst);
                })
            })
            .unwrap();
        if poisoned {
            std::thread::scope(|scope| {
                let backend = &access.0.backend;
                let worker = scope.spawn(move || {
                    let _held = backend.lock().unwrap();
                    panic!("synthetic backend poison");
                });
                assert!(worker.join().is_err());
            });
        } else {
            *state.panic.lock().unwrap() = Some(Operation::List);
        }
        assert!(matches!(
            tauri::async_runtime::block_on(access.list_snapshots()),
            WorkspaceResult::Failed { .. }
        ));
        assert_terminal(&access);
        assert_eq!(post_guard.load(Ordering::SeqCst), 1);
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn successful_backend_payloads_never_cross_the_fatal_failure_fence() {
    for boundary in ["list", "open"] {
        let state = Arc::new(ProbeState::default());
        let source = report("payload.md", b"# Payload\n");
        let access = SourceAccess::with_mode(
            Gate::new(),
            Box::new(ProbeBackend::new(Arc::clone(&state), source.clone())),
            true,
        );
        select(&access, &source);
        assert!(access.0.session.snapshot(&source.source.id).is_some());
        match boundary {
            "list" => assert!(matches!(
                tauri::async_runtime::block_on(access.list_snapshots()),
                WorkspaceResult::Failed { .. }
            )),
            "open" => assert!(matches!(
                tauri::async_runtime::block_on(access.open_snapshot(source.source.id.clone())),
                SelectionResult::Failed { .. }
            )),
            _ => unreachable!(),
        }
        assert!(access.failure_latched());
        assert!(access.0.session.snapshot(&source.source.id).is_none());
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn held_model_or_credential_lease_keeps_fatal_close_drain_nonquiescent() {
    for credential in [false, true] {
        let root = TestRoot::new("failure-held-lane");
        let vault = ThreadVault::default();
        let gate = Gate::new();
        let access =
            SourceAccess::managed(gate.clone(), Owner::acquire(root.0.clone()).unwrap(), vault);
        let credential_lease = if credential {
            let custody = Custody::with_gate(gate.clone());
            Some(custody.begin(CredentialPhase::Reading).unwrap().0)
        } else {
            None
        };
        let model_lease = (!credential).then(|| gate.begin(Lane::Model).unwrap());
        let _ = tauri::async_runtime::block_on(access.list_snapshots());
        assert!(access.failure_latched());
        let close = WorkspaceClose::new(access.clone());
        assert_eq!(close.progress_failure(|_| true), Ok(false));
        assert_eq!(
            view_json(close.inspect(|_| true))["state"],
            "recovery_required"
        );
        let ticket = access.request_close().unwrap();
        assert_eq!(ticket.is_quiescent(), Ok(false));
        drop(model_lease);
        drop(credential_lease);
        assert_eq!(ticket.is_quiescent(), Ok(true));
        assert_eq!(close.progress_failure(|_| true), Ok(true));
        assert_eq!(
            view_json(close.inspect(|_| true))["state"],
            "recovery_required"
        );
        assert_eq!(ticket.release_key(), Ok(false));
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn aborted_ipc_waiter_does_not_cancel_backend_failure_finalization() {
    let gate = Gate::new();
    let state = Arc::new(ProbeState::default());
    let source = report("worker.md", b"# Worker\n");
    let access = SourceAccess::with_mode(
        gate,
        Box::new(ProbeBackend::new(Arc::clone(&state), source.clone())),
        true,
    );
    select(&access, &source);
    let (started, release) = state.block(Operation::List);
    let (latched_tx, latched_rx) = mpsc::channel();
    access
        .set_failure_notifier(Arc::new(move || {
            let _ = latched_tx.send(());
        }))
        .unwrap();
    let worker_access = access.clone();
    let waiter = tauri::async_runtime::spawn(async move { worker_access.list_snapshots().await });
    assert!(started.recv_timeout(WAIT).is_ok());
    waiter.abort();
    assert!(tauri::async_runtime::block_on(waiter).is_err());
    assert!(release.send(()).is_ok());
    assert!(latched_rx.recv_timeout(WAIT).is_ok());
    let ticket = access.request_close().unwrap();
    let deadline = Instant::now() + WAIT;
    loop {
        match ticket.is_quiescent() {
            Ok(true) => break,
            Ok(false) => {
                assert!(
                    Instant::now() < deadline,
                    "backend callback lease did not drop"
                );
                thread::yield_now();
            }
            Err(()) => panic!("failure close drain became unobservable"),
        }
    }
    assert_terminal(&access);
    assert!(access.0.session.snapshot(&source.source.id).is_none());
}

#[cfg(feature = "encrypted-workspace")]
struct ProgressHost {
    calls: AtomicUsize,
    values: Mutex<VecDeque<Progress>>,
    entered: mpsc::Sender<()>,
    first_release: Mutex<Option<mpsc::Receiver<()>>>,
}

#[cfg(feature = "encrypted-workspace")]
impl Host for ProgressHost {
    fn progress(&self) -> Progress {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let _ = self.entered.send(());
        if call == 0 {
            assert!(
                self.first_release
                    .lock()
                    .unwrap()
                    .take()
                    .unwrap()
                    .recv()
                    .is_ok()
            );
        }
        self.values
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Progress::Stopped)
    }
}

#[cfg(feature = "encrypted-workspace")]
struct FailureProgressHost {
    close: WorkspaceClose,
    calls: Arc<AtomicUsize>,
}

#[cfg(feature = "encrypted-workspace")]
impl Host for FailureProgressHost {
    fn progress(&self) -> Progress {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.close.progress_failure(|_| true) {
            Ok(true) => Progress::Finished,
            Ok(false) => Progress::Pending,
            Err(()) => Progress::Stopped,
        }
    }
}

#[cfg(feature = "encrypted-workspace")]
fn unobservable_failed_close() -> (SourceAccess, WorkspaceClose) {
    let access = SourceAccess::close_protocol_fixture(Gate::new());
    access.fail_closed();
    access.0.drain_unavailable.store(true, Ordering::SeqCst);
    let close = WorkspaceClose::new(access.clone());
    (access, close)
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn unobservable_fatal_drain_stops_native_monitor_without_repolling() {
    let (access, close) = unobservable_failed_close();
    assert_eq!(close.progress_failure(|_| true), Err(()));
    assert_eq!(
        view_json(close.inspect(|_| true))["state"],
        "recovery_required"
    );
    assert!(access.failure_latched());

    let (_access, close) = unobservable_failed_close();
    let calls = Arc::new(AtomicUsize::new(0));
    let host: Arc<dyn Host> = Arc::new(FailureProgressHost {
        close,
        calls: Arc::clone(&calls),
    });
    let monitor = Monitor::new(Arc::downgrade(&host));
    monitor.wake_joined_for_test().unwrap().join().unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn weak_monitor_coalesces_wakes_and_stops_after_finished_or_host_drop() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let state = Arc::new(ProgressHost {
        calls: AtomicUsize::new(0),
        values: Mutex::new(VecDeque::from([Progress::Pending, Progress::Finished])),
        entered: entered_tx,
        first_release: Mutex::new(Some(release_rx)),
    });
    let host: Arc<dyn Host> = state.clone();
    let monitor = Monitor::new(Arc::downgrade(&host));
    let joined = monitor.wake_joined_for_test().unwrap();
    assert!(entered_rx.recv_timeout(WAIT).is_ok());
    monitor.wake();
    assert!(release_tx.send(()).is_ok());
    assert!(entered_rx.recv_timeout(WAIT).is_ok());
    joined.join().unwrap();
    assert_eq!(state.calls.load(Ordering::SeqCst), 2);
    drop(host);
    drop(state);
    monitor.wake_joined_for_test().unwrap().join().unwrap();
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn native_progress_wait_stops_for_exit_or_disconnected_completion() {
    let alive = std::sync::atomic::AtomicBool::new(false);
    let (_sender, receiver) = mpsc::channel();
    assert_eq!(wait_for_progress(receiver, &alive), Progress::Stopped);

    let alive = std::sync::atomic::AtomicBool::new(true);
    let (sender, receiver) = mpsc::channel();
    drop(sender);
    assert_eq!(wait_for_progress(receiver, &alive), Progress::Stopped);

    let alive = std::sync::atomic::AtomicBool::new(true);
    let (sender, receiver) = mpsc::channel();
    sender.send(Progress::Finished).unwrap();
    assert_eq!(wait_for_progress(receiver, &alive), Progress::Finished);

    let alive = Arc::new(AtomicBool::new(true));
    let (started_tx, started_rx) = mpsc::channel();
    let (sender, receiver) = mpsc::channel();
    let waiting_alive = Arc::clone(&alive);
    let waiting = thread::spawn(move || {
        started_tx.send(()).unwrap();
        wait_for_progress(receiver, &waiting_alive)
    });
    assert!(started_rx.recv_timeout(WAIT).is_ok());
    alive.store(false, Ordering::SeqCst);
    assert_eq!(waiting.join().unwrap(), Progress::Stopped);
    drop(sender);
}
