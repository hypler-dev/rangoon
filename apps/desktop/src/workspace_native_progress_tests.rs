#[cfg(feature = "encrypted-workspace")]
use super::*;

#[cfg(feature = "encrypted-workspace")]
use crate::workspace_close::{
    View, WorkspaceClose,
    failure::{Host, Monitor, Progress},
    handle_main_window_event, main_window_destroyed,
};
#[cfg(all(feature = "encrypted-workspace", unix))]
use rangoon_desktop::workspace_session::Owner;
#[cfg(feature = "encrypted-workspace")]
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};

#[cfg(feature = "encrypted-workspace")]
fn view_json(view: View) -> serde_json::Value {
    serde_json::to_value(view).unwrap()
}

#[cfg(all(feature = "encrypted-workspace", unix))]
fn managed_access(root: &TestRoot, vault: &ThreadVault) -> SourceAccess {
    SourceAccess::managed(
        Gate::new(),
        Owner::acquire(root.0.clone()).unwrap(),
        vault.clone(),
    )
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn explicit_close_waits_for_held_callback_without_renderer_ack_then_releases_key() {
    let root = TestRoot::new("native-progress-held-callback");
    let vault = ThreadVault::default();
    let access = managed_access(&root, &vault);
    let held = access.begin(Lane::Read).unwrap();
    let close = WorkspaceClose::new(access.clone());
    assert_eq!(view_json(close.begin(|_| true))["state"], "closing");
    let ticket = access.request_close().unwrap();

    let cleanup_calls = AtomicUsize::new(0);
    assert_eq!(
        close.progress_native(|drained| {
            cleanup_calls.fetch_add(usize::from(drained), Ordering::SeqCst);
            true
        }),
        Ok(false)
    );
    assert_eq!(cleanup_calls.load(Ordering::SeqCst), 0);
    assert_eq!(view_json(close.inspect_control().0)["state"], "closing");
    assert_eq!(ticket.is_quiescent(), Ok(false));
    assert_eq!(ticket.release_key(), Err(()));

    drop(held);
    assert_eq!(
        close.progress_native(|drained| {
            cleanup_calls.fetch_add(usize::from(drained), Ordering::SeqCst);
            true
        }),
        Ok(true)
    );
    assert_eq!(cleanup_calls.load(Ordering::SeqCst), 1);
    // Native completion does not fabricate the renderer acknowledgment.
    assert_eq!(view_json(close.inspect_control().0)["state"], "closing");
    assert_eq!(ticket.is_quiescent(), Ok(true));
    assert_eq!(ticket.release_key(), Ok(true));
    assert!(!access.0.backend.lock().unwrap().control_is_open());
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn native_progress_is_inert_for_legacy_and_preserves_fatal_recovery() {
    let (legacy, state, _) = probe(report("native-progress-legacy.md", b"legacy"));
    let close = WorkspaceClose::new(legacy);
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

    let access = SourceAccess::close_protocol_fixture(Gate::new());
    access.fail_closed();
    let close = WorkspaceClose::new(access.clone());
    assert!(matches!(
        close.progress_native(|_| true),
        Ok(true) | Ok(false)
    ));
    assert!(access.failure_latched());
    assert_eq!(
        view_json(close.inspect_control().0)["state"],
        "recovery_required"
    );
    assert!(matches!(
        access.begin(Lane::Read),
        Err(AdmissionError::Unavailable)
    ));
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn main_window_destruction_classifier_is_exact_and_source_free() {
    assert!(main_window_destroyed(
        "main",
        &tauri::WindowEvent::Destroyed
    ));
    assert!(!main_window_destroyed(
        "secondary",
        &tauri::WindowEvent::Destroyed
    ));
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn main_window_destroyed_dispatches_close_and_wake_only_for_main_destroyed() {
    let root = TestRoot::new("native-window-destroyed");
    let vault = ThreadVault::default();
    let access = managed_access(&root, &vault);
    let close = WorkspaceClose::new(access.clone());
    let wakes = AtomicUsize::new(0);
    let cleanup = AtomicUsize::new(0);

    handle_main_window_event(
        "secondary",
        &tauri::WindowEvent::Destroyed,
        &close,
        |_| {
            cleanup.fetch_add(1, Ordering::SeqCst);
            true
        },
        || {
            wakes.fetch_add(1, Ordering::SeqCst);
        },
    );
    handle_main_window_event(
        "main",
        &tauri::WindowEvent::Focused(false),
        &close,
        |_| {
            cleanup.fetch_add(1, Ordering::SeqCst);
            true
        },
        || {
            wakes.fetch_add(1, Ordering::SeqCst);
        },
    );
    assert_eq!(wakes.load(Ordering::SeqCst), 0);
    assert_eq!(cleanup.load(Ordering::SeqCst), 0);
    let held = access.begin(Lane::Read).unwrap();

    handle_main_window_event(
        "main",
        &tauri::WindowEvent::Destroyed,
        &close,
        |_| {
            cleanup.fetch_add(1, Ordering::SeqCst);
            true
        },
        || {
            wakes.fetch_add(1, Ordering::SeqCst);
        },
    );
    assert_eq!(wakes.load(Ordering::SeqCst), 1);
    assert_eq!(cleanup.load(Ordering::SeqCst), 1);
    assert!(matches!(
        access.begin(Lane::Read),
        Err(AdmissionError::Unavailable)
    ));
    assert_eq!(close.progress_native(|_| true), Ok(false));
    drop(held);
    assert_eq!(close.progress_native(|_| true), Ok(true));
}

#[cfg(feature = "encrypted-workspace")]
struct FinishingWakeHost {
    calls: AtomicUsize,
    entered: mpsc::Sender<()>,
    release: Mutex<Option<mpsc::Receiver<()>>>,
}

#[cfg(feature = "encrypted-workspace")]
impl Host for FinishingWakeHost {
    fn progress(&self) -> Progress {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            self.entered.send(()).unwrap();
            self.release.lock().unwrap().take().unwrap().recv().unwrap();
        }
        Progress::Finished
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn monitor_does_not_lose_second_wake_while_first_finishes() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let state = Arc::new(FinishingWakeHost {
        calls: AtomicUsize::new(0),
        entered: entered_tx,
        release: Mutex::new(Some(release_rx)),
    });
    let host: Arc<dyn Host> = state.clone();
    let monitor = Monitor::new(Arc::downgrade(&host));
    let joined = monitor.wake_joined_for_test().unwrap();
    assert!(entered_rx.recv_timeout(WAIT).is_ok());
    monitor.wake();
    release_tx.send(()).unwrap();
    joined.join().unwrap();
    assert_eq!(state.calls.load(Ordering::SeqCst), 2);
}
