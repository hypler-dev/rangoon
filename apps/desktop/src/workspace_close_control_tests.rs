#[cfg(feature = "encrypted-workspace")]
use super::*;

#[cfg(feature = "encrypted-workspace")]
use crate::workspace_close::{View, WorkspaceClose};
#[cfg(feature = "encrypted-workspace")]
use crate::workspace_source_access::{ControlError, ControlKind};
#[cfg(feature = "encrypted-workspace")]
use rangoon_desktop::workspace_session::Owner;
#[cfg(feature = "encrypted-workspace")]
use serde_json::json;
#[cfg(feature = "encrypted-workspace")]
use std::{
    fs,
    sync::{Arc, Mutex, mpsc},
    thread,
};

#[cfg(feature = "encrypted-workspace")]
fn view_json(view: View) -> serde_json::Value {
    serde_json::to_value(view).unwrap()
}

#[cfg(feature = "encrypted-workspace")]
fn attempt(view: &serde_json::Value) -> (String, String) {
    (
        view["attemptId"].as_str().unwrap().to_owned(),
        view["epoch"].as_str().unwrap().to_owned(),
    )
}

#[cfg(feature = "encrypted-workspace")]
fn acknowledgement(id: &str, epoch: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.workspace-purge-ack.v1",
        "attemptId": id,
        "epoch": epoch,
    }))
    .unwrap()
}

#[cfg(feature = "encrypted-workspace")]
fn locked_access(root: &TestRoot, vault: &ThreadVault) -> SourceAccess {
    SourceAccess::managed(
        Gate::new(),
        Owner::acquire(root.0.clone()).unwrap(),
        vault.clone(),
    )
}

#[cfg(feature = "encrypted-workspace")]
fn empty_workspace(root: &TestRoot, vault: &ThreadVault) {
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let preview = owner.prepare_create_empty().unwrap();
    owner.confirm(&preview.id, vault).unwrap();
    owner.lock();
}

#[cfg(feature = "encrypted-workspace")]
type VaultPause = (mpsc::Sender<()>, mpsc::Receiver<()>);

#[cfg(feature = "encrypted-workspace")]
#[derive(Clone)]
struct BlockingVault {
    inner: ThreadVault,
    next_open: Arc<Mutex<Option<VaultPause>>>,
}

#[cfg(feature = "encrypted-workspace")]
impl BlockingVault {
    fn new(inner: ThreadVault) -> Self {
        Self {
            inner,
            next_open: Arc::new(Mutex::new(None)),
        }
    }

    fn block_next_open(&self) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        *self.next_open.lock().unwrap() = Some((entered_tx, release_rx));
        (entered_rx, release_tx)
    }
}

#[cfg(feature = "encrypted-workspace")]
impl rangoon_desktop::workspace_session::Vault for BlockingVault {
    type Slot = ThreadSlot;

    fn open(
        &self,
        identity: &rangoon_desktop::workspace_keys::WorkspaceId,
    ) -> Result<Self::Slot, rangoon_desktop::workspace_keys::Error> {
        if let Some((entered, release)) = self.next_open.lock().unwrap().take() {
            assert!(entered.send(()).is_ok());
            assert!(release.recv().is_ok());
        }
        rangoon_desktop::workspace_session::Vault::open(&self.inner, identity)
    }
}

#[cfg(feature = "encrypted-workspace")]
fn locked_close(access: SourceAccess) -> WorkspaceClose {
    let close = WorkspaceClose::new(access);
    let started = view_json(close.begin(|_| true));
    assert_eq!(started["state"], "closing");
    let (id, epoch) = attempt(&started);
    let inspected = view_json(close.inspect(|_| true));
    assert_eq!(inspected["state"], "closing");
    let locked = view_json(close.acknowledge(&acknowledgement(&id, &epoch), |_| true));
    assert_eq!(locked["state"], "locked");
    close
}

#[cfg(feature = "encrypted-workspace")]
fn control_attempt(close: &WorkspaceClose) -> (String, String) {
    let view = view_json(close.inspect(|_| panic!("live control must prevent native cleanup")));
    assert_eq!(view["state"], "closing");
    attempt(&view)
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn actual_empty_control_retains_takes_confirms_resumes_and_authors() {
    let root = TestRoot::new("supervisor-empty");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let close = locked_close(access.clone());

    let mut flight = close
        .start_control(ControlKind::CreateEmpty, |_| true)
        .unwrap();
    let preview = flight.prepare().unwrap();
    let (id, epoch) = control_attempt(&close);
    assert!(matches!(
        close.take_control("wrong", &epoch, |_| true),
        Err(ControlError::Stale)
    ));
    flight.retain().unwrap();
    let mut resumed = close.take_control(&id, &epoch, |_| true).unwrap();
    assert!(matches!(
        close.take_control(&id, &epoch, |_| true),
        Err(ControlError::Busy)
    ));
    resumed.confirm(&preview.id).unwrap();
    assert_eq!(view_json(resumed.resume().unwrap())["state"], "open");
    assert!(matches!(
        close.take_control(&id, &epoch, |_| true),
        Err(ControlError::Stale)
    ));
    assert_eq!(
        view_json(close.cancel_control(&id, &epoch, |_| true))["state"],
        "open"
    );
    let read = access.begin(Lane::Read).unwrap();
    drop(read);

    let source = report("supervisor-empty.md", b"# Supervisor empty\nExact bytes\n");
    select(&access, &source);
    assert!(matches!(
        tauri::async_runtime::block_on(access.save_analysis(source.source.id.clone())),
        WorkspaceResult::Saved { .. }
    ));
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn cancelled_worker_callback_late_drop_cannot_reuse_its_native_attempt() {
    let root = TestRoot::new("supervisor-worker-cancel");
    let vault = BlockingVault::new(ThreadVault::default());
    let access = SourceAccess::managed(
        Gate::new(),
        Owner::acquire(root.0.clone()).unwrap(),
        vault.clone(),
    );
    let close = locked_close(access.clone());
    let mut flight = close
        .start_control(ControlKind::CreateEmpty, |_| true)
        .unwrap();
    let preview = flight.prepare().unwrap();
    let (old_id, old_epoch) = control_attempt(&close);
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();
    let preview_id = preview.id.clone();
    let (vault_entered, vault_release) = vault.block_next_open();
    let worker = thread::spawn(move || {
        assert!(started_tx.send(()).is_ok());
        assert!(release_rx.recv().is_ok());
        let result = flight.confirm(&preview_id);
        drop(flight);
        assert!(finished_tx.send(result).is_ok());
    });
    drop(worker);

    assert!(started_rx.recv().is_ok());
    assert!(release_tx.send(()).is_ok());
    assert!(vault_entered.recv().is_ok());
    let cancelled = view_json(close.cancel_control(&old_id, &old_epoch, |_| true));
    let (new_id, new_epoch) = attempt(&cancelled);
    assert_ne!(
        (old_id.as_str(), old_epoch.as_str()),
        (new_id.as_str(), new_epoch.as_str())
    );
    let replay_cancel = view_json(close.cancel_control(&old_id, &old_epoch, |_| true));
    assert_eq!(
        replay_cancel["diagnostic"],
        "invalid_workspace_close_request"
    );
    assert_eq!(attempt(&replay_cancel), (new_id.clone(), new_epoch.clone()));
    assert_eq!(
        view_json(close.acknowledge(&acknowledgement(&old_id, &old_epoch), |_| true))["diagnostic"],
        "invalid_workspace_close_request"
    );
    assert_eq!(
        view_json(close.acknowledge(&acknowledgement(&new_id, &new_epoch), |_| true))["state"],
        "closing"
    );
    assert!(vault_release.send(()).is_ok());
    assert!(matches!(
        finished_rx.recv().unwrap(),
        Err(ControlError::Stale)
    ));

    let current = view_json(
        close.inspect(|_| panic!("late flight Drop already finalized current native cleanup")),
    );
    assert_eq!(current["state"], "locked");
    assert_eq!(attempt(&current), (new_id, new_epoch));
    assert!(matches!(
        access.begin(Lane::Read),
        Err(AdmissionError::Unavailable)
    ));
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn control_requires_acknowledged_locked_close_and_fresh_tokens_stay_closing() {
    let root = TestRoot::new("supervisor-tokens");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let close = WorkspaceClose::new(access);
    assert!(matches!(
        close.start_control(ControlKind::CreateEmpty, |_| true),
        Err(ControlError::Invalid)
    ));

    let begun = view_json(close.begin(|_| true));
    let (initial_id, initial_epoch) = attempt(&begun);
    assert!(matches!(
        close.start_control(ControlKind::CreateEmpty, |_| true),
        Err(ControlError::Unavailable)
    ));
    let _ = close.inspect(|_| true);
    assert_eq!(
        view_json(close.acknowledge(&acknowledgement(&initial_id, &initial_epoch), |_| true))["state"],
        "locked"
    );

    let flight = close
        .start_control(ControlKind::CreateEmpty, |_| true)
        .unwrap();
    let (first_id, first_epoch) = control_attempt(&close);
    let early = view_json(close.acknowledge(&acknowledgement(&first_id, &first_epoch), |_| true));
    assert_eq!(early["state"], "closing");
    assert_eq!(
        view_json(close.cancel_control("wrong", &first_epoch, |_| true))["diagnostic"],
        "invalid_workspace_close_request"
    );
    let cancelled = view_json(close.cancel_control(&first_id, &first_epoch, |_| true));
    let (second_id, second_epoch) = attempt(&cancelled);
    assert_ne!(
        (first_id.as_str(), first_epoch.as_str()),
        (second_id.as_str(), second_epoch.as_str())
    );
    assert_eq!(
        view_json(close.acknowledge(&acknowledgement(&first_id, &first_epoch), |_| true))["diagnostic"],
        "invalid_workspace_close_request"
    );
    drop(flight);
    let _ = close.inspect(|_| true);
    assert_eq!(
        view_json(close.acknowledge(&acknowledgement(&second_id, &second_epoch), |_| true))["state"],
        "locked"
    );

    let late = close
        .start_control(ControlKind::CreateEmpty, |_| true)
        .unwrap();
    let (third_id, third_epoch) = control_attempt(&close);
    let early = view_json(close.acknowledge(&acknowledgement(&third_id, &third_epoch), |_| true));
    assert_eq!(early["state"], "closing");
    assert_eq!(
        view_json(close.acknowledge(&acknowledgement(&first_id, &first_epoch), |_| true))["diagnostic"],
        "invalid_workspace_close_request"
    );
    drop(late);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn retained_control_cancellation_drops_custody_and_refuses_old_take() {
    let root = TestRoot::new("supervisor-retained-cancel");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let close = locked_close(access.clone());
    let mut flight = close
        .start_control(ControlKind::CreateEmpty, |_| true)
        .unwrap();
    let _ = flight.prepare().unwrap();
    let (id, epoch) = control_attempt(&close);
    flight.retain().unwrap();

    let cancelled = view_json(close.cancel_control(&id, &epoch, |_| true));
    assert_eq!(cancelled["state"], "closing");
    assert!(matches!(
        close.take_control(&id, &epoch, |_| true),
        Err(ControlError::Stale)
    ));
    assert!(matches!(
        access.begin(Lane::Read),
        Err(AdmissionError::Unavailable)
    ));
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn recovery_control_inspects_reconciles_and_never_resumes() {
    let root = TestRoot::new("supervisor-recovery");
    let vault = ThreadVault::default();
    empty_workspace(&root, &vault);
    fs::write(root.0.join("encrypted-workspace.operation.terminal"), b"").unwrap();
    let access = locked_access(&root, &vault);
    let close = locked_close(access);

    let mut flight = close
        .start_control(ControlKind::Recovery, |_| true)
        .unwrap();
    let preview = flight.inspect_recovery().unwrap();
    let (id, epoch) = control_attempt(&close);
    flight.retain().unwrap();
    let mut flight = close.take_control(&id, &epoch, |_| true).unwrap();
    flight.reconcile(&preview.id).unwrap();
    assert!(matches!(flight.resume(), Err(ControlError::Invalid)));
    let view = view_json(close.inspect(|_| true));
    assert_ne!(view["state"], "open");
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn retained_recovery_inspection_cancel_drops_custody_for_fresh_owner_inspection() {
    let root = TestRoot::new("supervisor-recovery-cancel");
    let vault = ThreadVault::default();
    empty_workspace(&root, &vault);
    fs::write(root.0.join("encrypted-workspace.operation.terminal"), b"").unwrap();
    let access = locked_access(&root, &vault);
    let close = locked_close(access.clone());
    let mut flight = close
        .start_control(ControlKind::Recovery, |_| true)
        .unwrap();
    let preview = flight.inspect_recovery().unwrap();
    let opens = vault.0.lock().unwrap().opens;
    let (id, epoch) = control_attempt(&close);
    flight.retain().unwrap();
    assert_eq!(
        view_json(close.cancel_control(&id, &epoch, |_| true))["state"],
        "closing"
    );
    assert_eq!(vault.0.lock().unwrap().opens, opens);
    drop(close);
    drop(access);

    let mut fresh = Owner::acquire(root.0.clone()).unwrap();
    let observed = fresh.inspect_recovery(&vault).unwrap();
    assert_eq!(observed.workspace_id, preview.workspace_id);
    assert_eq!(observed.outcome, "unknown");
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn poison_active_or_retained_control_only_invalidates_and_closes() {
    for retained in [false, true] {
        let root = TestRoot::new(if retained {
            "supervisor-poison-retained"
        } else {
            "supervisor-poison-active"
        });
        let vault = ThreadVault::default();
        let access = locked_access(&root, &vault);
        let close = locked_close(access.clone());
        let mut flight = close
            .start_control(ControlKind::CreateEmpty, |_| true)
            .unwrap();
        if retained {
            let _ = flight.prepare().unwrap();
            let (id, epoch) = control_attempt(&close);
            flight.retain().unwrap();
            close.poison_control_state_for_test();
            assert_eq!(
                view_json(close.cancel_control(&id, &epoch, |_| true))["state"],
                "recovery_required"
            );
        } else {
            close.poison_control_state_for_test();
            drop(flight);
            assert_eq!(
                view_json(close.begin(|_| true))["state"],
                "recovery_required"
            );
        }
        assert!(matches!(
            close.start_control(ControlKind::CreateEmpty, |_| true),
            Err(ControlError::Unavailable)
        ));
        assert!(matches!(
            access.begin(Lane::Read),
            Err(AdmissionError::Unavailable)
        ));
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn poisoned_progress_for_active_or_retained_control_only_closes() {
    for retained in [false, true] {
        let root = TestRoot::new(if retained {
            "supervisor-progress-retained"
        } else {
            "supervisor-progress-active"
        });
        let vault = ThreadVault::default();
        let access = locked_access(&root, &vault);
        let close = locked_close(access.clone());
        let mut flight = Some(
            close
                .start_control(ControlKind::CreateEmpty, |_| true)
                .unwrap(),
        );
        let preview = flight.as_mut().unwrap().prepare().unwrap();
        let token = control_attempt(&close);
        if retained {
            flight.take().unwrap().retain().unwrap();
        } else {
            flight.as_mut().unwrap().confirm(&preview.id).unwrap();
        }
        close.poison_control_progress_for_test();
        assert_eq!(
            view_json(close.cancel_control(&token.0, &token.1, |_| true))["state"],
            "recovery_required"
        );
        if let Some(flight) = flight {
            assert!(matches!(flight.resume(), Err(ControlError::Stale)));
        } else {
            assert_eq!(
                view_json(close.inspect(|_| true))["state"],
                "recovery_required"
            );
        }
        assert!(matches!(
            close.start_control(ControlKind::CreateEmpty, |_| true),
            Err(ControlError::Unavailable)
        ));
        assert!(matches!(
            access.begin(Lane::Read),
            Err(AdmissionError::Unavailable)
        ));
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn entropy_epoch_and_source_swap_failures_clear_locked_evidence_without_vault_open() {
    for failure in ["entropy", "epoch", "source"] {
        let root = TestRoot::new(&format!("supervisor-failure-{failure}"));
        let vault = ThreadVault::default();
        let access = locked_access(&root, &vault);
        let close = locked_close(access.clone());
        let opens = vault.0.lock().unwrap().opens;
        let result = match failure {
            "entropy" => close.fail_control_entropy_for_test(ControlKind::CreateEmpty),
            "epoch" => {
                close.exhaust_control_epoch_for_test();
                close.start_control(ControlKind::CreateEmpty, |_| true)
            }
            "source" => {
                close.fail_source_swap_for_test();
                close.start_control(ControlKind::CreateEmpty, |_| true)
            }
            _ => unreachable!(),
        };
        assert!(matches!(result, Err(ControlError::Unavailable)));
        assert_eq!(vault.0.lock().unwrap().opens, opens);
        assert_eq!(
            view_json(close.inspect(|_| true))["state"],
            "recovery_required"
        );
        assert!(matches!(
            access.begin(Lane::Read),
            Err(AdmissionError::Unavailable)
        ));
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn panicking_callback_cleanup_and_generic_lock_after_success_stay_closed() {
    let root = TestRoot::new("supervisor-cleanup-panic");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let close = locked_close(access.clone());
    let flight = close
        .start_control(ControlKind::CreateEmpty, |_| {
            panic!("synthetic cleanup panic")
        })
        .unwrap();
    drop(flight);
    assert_eq!(
        view_json(close.inspect(|_| true))["state"],
        "recovery_required"
    );
    assert!(matches!(
        access.begin(Lane::Read),
        Err(AdmissionError::Unavailable)
    ));

    let root = TestRoot::new("supervisor-generic-after-success");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let close = locked_close(access.clone());
    let mut flight = close
        .start_control(ControlKind::CreateEmpty, |_| true)
        .unwrap();
    let preview = flight.prepare().unwrap();
    flight.confirm(&preview.id).unwrap();
    assert_eq!(view_json(flight.resume().unwrap())["state"], "open");
    assert_eq!(view_json(close.begin(|_| true))["state"], "closing");
    assert!(matches!(
        access.begin(Lane::Read),
        Err(AdmissionError::Unavailable)
    ));
}
