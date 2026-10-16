#[cfg(feature = "encrypted-workspace")]
use super::*;

#[cfg(feature = "encrypted-workspace")]
use crate::workspace_close::{
    View, WorkspaceClose,
    failure::{
        CLEAN_EXIT_CODE, ExitAction, ExitCoordinator, Host, Monitor, Progress, RECOVERY_EXIT_CODE,
    },
};
#[cfg(feature = "encrypted-workspace")]
use rangoon_desktop::workspace_session::Owner;
#[cfg(feature = "encrypted-workspace")]
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};

#[cfg(feature = "encrypted-workspace")]
fn managed_access(root: &TestRoot, vault: &ThreadVault) -> SourceAccess {
    SourceAccess::managed(
        Gate::new(),
        Owner::acquire(root.0.clone()).unwrap(),
        vault.clone(),
    )
}

#[cfg(feature = "encrypted-workspace")]
fn view_json(view: View) -> serde_json::Value {
    serde_json::to_value(view).unwrap()
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn exit_coordinator_holds_repeated_requests_and_consumes_one_matching_exit() {
    let coordinator = ExitCoordinator::new();
    assert!(!coordinator.is_holding());
    assert_eq!(coordinator.request(None), ExitAction::PreventAndStart);
    assert!(coordinator.is_holding());
    assert_eq!(coordinator.request(None), ExitAction::Prevent);
    assert_eq!(coordinator.request(Some(91)), ExitAction::Prevent);
    assert!(coordinator.send(CLEAN_EXIT_CODE));
    assert_eq!(coordinator.request(None), ExitAction::Prevent);
    assert_eq!(coordinator.request(Some(91)), ExitAction::Prevent);
    assert_eq!(coordinator.request(Some(CLEAN_EXIT_CODE)), ExitAction::Pass);
    assert_eq!(
        coordinator.request(Some(CLEAN_EXIT_CODE)),
        ExitAction::Prevent
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn restart_code_passes_through_without_creating_an_exit_hold() {
    let coordinator = ExitCoordinator::new();
    assert_eq!(
        coordinator.request(Some(tauri::RESTART_EXIT_CODE)),
        ExitAction::Pass
    );
    assert!(!coordinator.is_holding());
    assert!(!coordinator.send(CLEAN_EXIT_CODE));
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn held_callback_allows_clean_exit_only_after_actual_native_completion() {
    let root = TestRoot::new("exit-hold-held-callback");
    let vault = ThreadVault::default();
    let access = managed_access(&root, &vault);
    let held = access.begin(Lane::Read).unwrap();
    let close = WorkspaceClose::new(access.clone());
    let coordinator = ExitCoordinator::new();

    assert_eq!(coordinator.request(None), ExitAction::PreventAndStart);
    assert_eq!(view_json(close.begin(|_| true))["state"], "closing");
    assert!(!close.clean_native_completion());
    assert_eq!(close.progress_native(|_| true), Ok(false));
    assert!(!close.clean_native_completion());

    drop(held);
    assert_eq!(close.progress_native(|_| true), Ok(true));
    assert!(close.clean_native_completion());
    // A native completion is not a renderer purge acknowledgment.
    assert_eq!(view_json(close.inspect_control().0)["state"], "closing");
    assert!(coordinator.send(CLEAN_EXIT_CODE));
    assert_eq!(coordinator.request(Some(CLEAN_EXIT_CODE)), ExitAction::Pass);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn unhealthy_native_completion_never_qualifies_for_clean_exit() {
    let access = SourceAccess::close_protocol_fixture(Gate::new());
    let close = WorkspaceClose::new(access);
    let coordinator = ExitCoordinator::new();

    assert_eq!(coordinator.request(None), ExitAction::PreventAndStart);
    assert_eq!(
        view_json(close.begin(|_| false))["state"],
        "recovery_required"
    );
    assert_eq!(close.progress_native(|_| false), Ok(true));
    assert!(!close.clean_native_completion());
    assert!(coordinator.send(RECOVERY_EXIT_CODE));
    assert_eq!(
        coordinator.request(Some(RECOVERY_EXIT_CODE)),
        ExitAction::Pass
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn legacy_native_progress_is_inert_and_cannot_qualify_an_exit() {
    let (access, state, _) = probe(report("exit-hold-legacy.md", b"legacy"));
    let close = WorkspaceClose::new(access);
    let calls = AtomicUsize::new(0);

    assert_eq!(
        close.progress_native(|_| {
            calls.fetch_add(1, Ordering::SeqCst);
            true
        }),
        Ok(true)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(state.list_calls.load(Ordering::SeqCst), 0);
    assert!(!close.clean_native_completion());
}

#[cfg(feature = "encrypted-workspace")]
struct TerminalExitHost {
    coordinator: Arc<ExitCoordinator>,
    code: i32,
    terminal: Mutex<Vec<(Progress, bool)>>,
}

#[cfg(feature = "encrypted-workspace")]
impl Host for TerminalExitHost {
    fn progress(&self) -> Progress {
        Progress::Finished
    }

    fn terminal(&self, result: Progress) {
        self.terminal
            .lock()
            .unwrap()
            .push((result, self.coordinator.send(self.code)));
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn failed_monitor_dispatch_reports_stopped_and_allows_only_recovery_exit() {
    let coordinator = Arc::new(ExitCoordinator::new());
    assert_eq!(coordinator.request(None), ExitAction::PreventAndStart);
    let terminal = Arc::new(TerminalExitHost {
        coordinator: Arc::clone(&coordinator),
        code: RECOVERY_EXIT_CODE,
        terminal: Mutex::new(Vec::new()),
    });
    let host: Arc<dyn Host> = terminal.clone();
    let monitor = Monitor::new(Arc::downgrade(&host));

    monitor.dispatch_failed_for_test();
    assert_eq!(
        terminal.terminal.lock().unwrap().as_slice(),
        &[(Progress::Stopped, true)]
    );
    assert_eq!(
        coordinator.request(Some(CLEAN_EXIT_CODE)),
        ExitAction::Prevent
    );
    assert_eq!(
        coordinator.request(Some(RECOVERY_EXIT_CODE)),
        ExitAction::Pass
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn finished_monitor_when_idle_does_not_request_an_exit() {
    let coordinator = Arc::new(ExitCoordinator::new());
    let terminal = Arc::new(TerminalExitHost {
        coordinator: Arc::clone(&coordinator),
        code: CLEAN_EXIT_CODE,
        terminal: Mutex::new(Vec::new()),
    });
    let host: Arc<dyn Host> = terminal.clone();
    let monitor = Monitor::new(Arc::downgrade(&host));

    let worker = monitor.wake_joined_for_test().unwrap();
    worker.join().unwrap();
    assert_eq!(
        terminal.terminal.lock().unwrap().as_slice(),
        &[(Progress::Finished, false)]
    );
    assert!(!coordinator.is_holding());
}

#[cfg(feature = "encrypted-workspace")]
struct SettlingTerminalHost {
    calls: AtomicUsize,
    terminal: Mutex<Vec<Progress>>,
    entered: mpsc::Sender<()>,
    release: Mutex<Option<mpsc::Receiver<()>>>,
}

#[cfg(feature = "encrypted-workspace")]
impl Host for SettlingTerminalHost {
    fn progress(&self) -> Progress {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            self.entered.send(()).unwrap();
            self.release
                .lock()
                .unwrap()
                .take()
                .unwrap()
                .recv_timeout(WAIT)
                .unwrap();
        }
        Progress::Finished
    }

    fn terminal(&self, result: Progress) {
        self.terminal.lock().unwrap().push(result);
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn monitor_settles_queued_wake_before_sending_one_terminal_result() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let terminal = Arc::new(SettlingTerminalHost {
        calls: AtomicUsize::new(0),
        terminal: Mutex::new(Vec::new()),
        entered: entered_tx,
        release: Mutex::new(Some(release_rx)),
    });
    let host: Arc<dyn Host> = terminal.clone();
    let monitor = Monitor::new(Arc::downgrade(&host));

    let worker = monitor.wake_joined_for_test().unwrap();
    assert!(entered_rx.recv_timeout(WAIT).is_ok());
    monitor.wake();
    release_tx.send(()).unwrap();
    worker.join().unwrap();

    assert_eq!(terminal.calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        terminal.terminal.lock().unwrap().as_slice(),
        &[Progress::Finished]
    );
}
