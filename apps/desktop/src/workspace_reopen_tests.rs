#[cfg(feature = "encrypted-workspace")]
use super::super::reopen::{ControlError, Kind};
#[cfg(feature = "encrypted-workspace")]
use super::*;
#[cfg(feature = "encrypted-workspace")]
use rangoon_desktop::workspace_session::Owner;
#[cfg(feature = "encrypted-workspace")]
use std::{
    fs,
    sync::{Arc, Mutex, mpsc},
    thread,
};

#[cfg(feature = "encrypted-workspace")]
fn locked_access(root: &TestRoot, vault: &ThreadVault) -> SourceAccess {
    SourceAccess::managed(
        Gate::new(),
        Owner::acquire(root.0.clone()).unwrap(),
        vault.clone(),
    )
}

#[cfg(feature = "encrypted-workspace")]
fn released_ticket(access: &SourceAccess) -> CloseTicket {
    let ticket = access.request_close().unwrap();
    assert!(ticket.purge());
    assert_eq!(ticket.is_quiescent(), Ok(true));
    assert_eq!(ticket.release_key(), Ok(true));
    ticket
}

#[cfg(feature = "encrypted-workspace")]
fn ready_empty_owner(root: &TestRoot, vault: &ThreadVault) {
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let preview = owner.prepare_create_empty().unwrap();
    owner.confirm(&preview.id, vault).unwrap();
    owner.lock();
}

#[cfg(feature = "encrypted-workspace")]
fn torn_empty_recovery(root: &TestRoot, vault: &ThreadVault) {
    ready_empty_owner(root, vault);
    fs::write(root.0.join("encrypted-workspace.operation.terminal"), b"").unwrap();
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
#[test]
fn empty_control_requires_prior_release_prepares_without_vault_and_resumes_authoring() {
    let root = TestRoot::new("reopen-empty");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let ticket = access.request_close().unwrap();
    assert!(matches!(
        ticket.start_control(Kind::CreateEmpty),
        Err(ControlError::Unavailable)
    ));
    assert!(ticket.purge());
    assert_eq!(ticket.release_key(), Ok(true));

    let opens = vault.0.lock().unwrap().opens;
    let mut control = ticket.start_control(Kind::CreateEmpty).unwrap();
    let preview = control.prepare().unwrap();
    assert_eq!(vault.0.lock().unwrap().opens, opens);
    assert!(matches!(
        access.begin(Lane::Read),
        Err(AdmissionError::Unavailable)
    ));
    control.confirm(&preview.id).unwrap();
    control.resume().unwrap();

    let source = report("reopen-empty.md", b"# Reopened empty\nExact bytes\n");
    select(&access, &source);
    assert!(matches!(
        tauri::async_runtime::block_on(access.save_analysis(source.source.id.clone())),
        WorkspaceResult::Saved { .. }
    ));
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn clean_unlock_control_confirms_once_then_resumes_empty_workspace() {
    let root = TestRoot::new("reopen-unlock");
    let vault = ThreadVault::default();
    ready_empty_owner(&root, &vault);
    let access = locked_access(&root, &vault);
    let ticket = released_ticket(&access);
    let mut control = ticket.start_control(Kind::Unlock).unwrap();
    let preview = control.prepare().unwrap();
    assert!(matches!(
        control.confirm("wrong"),
        Err(ControlError::Owner(_))
    ));
    assert!(matches!(
        control.confirm(&preview.id),
        Err(ControlError::Invalid)
    ));
    drop(control);
    drop(ticket);
    drop(access);

    let access = locked_access(&root, &vault);
    let ticket = released_ticket(&access);
    let mut control = ticket.start_control(Kind::Unlock).unwrap();
    let preview = control.prepare().unwrap();
    control.confirm(&preview.id).unwrap();
    control.resume().unwrap();
    assert!(matches!(
        tauri::async_runtime::block_on(access.list_snapshots()),
        WorkspaceResult::Listed { snapshots } if snapshots.is_empty()
    ));
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn old_ticket_cannot_purge_or_release_after_control_replaces_its_cycle() {
    let root = TestRoot::new("reopen-old-ticket");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let ticket = released_ticket(&access);
    let control = ticket.start_control(Kind::CreateEmpty).unwrap();

    assert!(!ticket.purge());
    assert_eq!(ticket.is_quiescent(), Err(()));
    assert_eq!(ticket.release_key(), Err(()));
    assert!(matches!(
        ticket.start_control(Kind::CreateEmpty),
        Err(ControlError::Stale)
    ));
    drop(control);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn recovery_kind_and_replayed_control_calls_refuse_without_reopen() {
    let root = TestRoot::new("reopen-kind-token");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let ticket = released_ticket(&access);
    let mut control = ticket.start_control(Kind::Recovery).unwrap();
    assert!(matches!(control.prepare(), Err(ControlError::Invalid)));
    assert!(matches!(
        control.inspect_recovery(),
        Err(ControlError::Invalid)
    ));
    drop(control);
    assert!(matches!(
        access.begin(Lane::Read),
        Err(AdmissionError::Unavailable)
    ));
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn recovery_fence_blocks_live_owner_then_fresh_controls_inspect_reconcile_and_drop() {
    let root = TestRoot::new("reopen-recovery");
    let vault = ThreadVault::default();
    torn_empty_recovery(&root, &vault);

    let mut fenced = Owner::acquire(root.0.clone()).unwrap();
    assert!(matches!(
        fenced.prepare_unlock(),
        Err(rangoon_desktop::workspace_session::Error::RecoveryRequired)
    ));
    let retained = fenced.inspect_recovery(&vault).unwrap();
    fs::write(
        root.0.join("encrypted-workspace.operation.terminal"),
        b"stale",
    )
    .unwrap();
    assert!(matches!(
        fenced.confirm_recovery(&retained.id, &vault),
        Err(rangoon_desktop::workspace_session::Error::Stale)
    ));
    let opens = vault.0.lock().unwrap().opens;
    let access = SourceAccess::managed(Gate::new(), fenced, vault.clone());
    let ticket = access.request_close().unwrap();
    assert!(ticket.purge());
    assert_eq!(ticket.release_key(), Ok(false));
    assert!(matches!(
        ticket.start_control(Kind::Recovery),
        Err(ControlError::Unavailable)
    ));
    assert_eq!(vault.0.lock().unwrap().opens, opens);
    drop(ticket);
    drop(access);

    let access = locked_access(&root, &vault);
    let ticket = released_ticket(&access);
    let mut control = ticket.start_control(Kind::Recovery).unwrap();
    let preview = control.inspect_recovery().unwrap();
    assert_eq!(preview.outcome, "unknown");
    assert!(matches!(control.resume(), Err(ControlError::Invalid)));
    drop(ticket);
    drop(access);

    let access = locked_access(&root, &vault);
    let ticket = released_ticket(&access);
    let mut control = ticket.start_control(Kind::Recovery).unwrap();
    let preview = control.inspect_recovery().unwrap();
    control.reconcile(&preview.id).unwrap();
    assert!(matches!(control.resume(), Err(ControlError::Invalid)));
    drop(ticket);
    drop(access);

    let mut reopened = Owner::acquire(root.0.clone()).unwrap();
    let unlock = reopened.prepare_unlock().unwrap();
    reopened.confirm(&unlock.id, &vault).unwrap();
    assert!(reopened.is_unlocked());
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn cancellation_renewal_keeps_old_callback_counted_until_drop() {
    let root = TestRoot::new("reopen-renew");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let ticket = released_ticket(&access);
    let mut control = ticket.start_control(Kind::CreateEmpty).unwrap();
    let current = access.request_close().unwrap();
    let renewed = current.renew_control().unwrap();
    let newest = access.request_close().unwrap().renew_control().unwrap();

    assert!(matches!(control.prepare(), Err(ControlError::Stale)));
    assert!(matches!(
        renewed.start_control(Kind::CreateEmpty),
        Err(ControlError::Stale)
    ));
    assert!(matches!(
        newest.start_control(Kind::CreateEmpty),
        Err(ControlError::Busy)
    ));
    drop(control);
    assert_eq!(newest.is_quiescent(), Ok(true));
    assert_eq!(newest.release_key(), Ok(true));
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn dropped_waiter_does_not_drop_worker_control_custody() {
    let root = TestRoot::new("reopen-worker");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let ticket = released_ticket(&access);
    let control = ticket.start_control(Kind::CreateEmpty).unwrap();
    let current = access.request_close().unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        assert!(started_tx.send(()).is_ok());
        assert!(release_rx.recv().is_ok());
        drop(control);
        assert!(finished_tx.send(()).is_ok());
    });
    drop(worker);
    assert!(started_rx.recv_timeout(WAIT).is_ok());
    assert_eq!(current.is_quiescent(), Ok(false));
    assert!(release_tx.send(()).is_ok());
    assert!(finished_rx.recv_timeout(WAIT).is_ok());
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn resume_opens_gate_before_slot_clear_but_source_admission_waits_for_clear() {
    let root = TestRoot::new("reopen-resume-slot");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let ticket = released_ticket(&access);
    let mut control = ticket.start_control(Kind::CreateEmpty).unwrap();
    let preview = control.prepare().unwrap();
    control.confirm(&preview.id).unwrap();

    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (resumed_tx, resumed_rx) = mpsc::channel();
    let resume_worker = thread::spawn(move || {
        let result = control.resume_paused(entered_tx, release_rx);
        assert!(resumed_tx.send(result).is_ok());
    });
    assert!(entered_rx.recv().is_ok());

    let gate_lease = access.0.gate.begin(Lane::Read).unwrap();
    drop(gate_lease);
    assert!(matches!(
        access.0.closing.try_lock(),
        Err(std::sync::TryLockError::WouldBlock)
    ));

    let waiting_access = access.clone();
    let (started_tx, started_rx) = mpsc::channel();
    let (admitted_tx, admitted_rx) = mpsc::channel();
    let admission_worker = thread::spawn(move || {
        assert!(started_tx.send(()).is_ok());
        assert!(admitted_tx.send(waiting_access.begin(Lane::Read)).is_ok());
    });
    assert!(started_rx.recv().is_ok());
    assert!(release_tx.send(()).is_ok());
    assert_eq!(resumed_rx.recv().unwrap(), Ok(()));
    resume_worker.join().unwrap();

    let admission = admitted_rx.recv().unwrap().unwrap();
    drop(admission);
    admission_worker.join().unwrap();
    assert!(access.0.closing.lock().unwrap().is_none());
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn renewal_during_blocked_confirm_keeps_callback_custody_and_refuses_replay() {
    let root = TestRoot::new("reopen-confirm-race");
    let keys = ThreadVault::default();
    ready_empty_owner(&root, &keys);
    let vault = BlockingVault::new(keys);
    let access = SourceAccess::managed(
        Gate::new(),
        Owner::acquire(root.0.clone()).unwrap(),
        vault.clone(),
    );
    let ticket = released_ticket(&access);
    let mut control = ticket.start_control(Kind::Unlock).unwrap();
    let preview = control.prepare().unwrap();
    let (entered_rx, release_tx) = vault.block_next_open();
    let (result_tx, result_rx) = mpsc::channel();
    let preview_id = preview.id.clone();
    let worker = thread::spawn(move || {
        let result = control.confirm(&preview_id);
        drop(control);
        assert!(result_tx.send(result).is_ok());
    });

    assert!(entered_rx.recv_timeout(WAIT).is_ok());
    let current = access.request_close().unwrap();
    let newest = current.renew_control().unwrap();
    assert_eq!(newest.is_quiescent(), Ok(false));
    assert_eq!(newest.release_key(), Err(()));
    assert!(matches!(
        current.start_control(Kind::Unlock),
        Err(ControlError::Stale)
    ));
    assert!(matches!(
        access.begin(Lane::Read),
        Err(AdmissionError::Unavailable)
    ));

    assert!(release_tx.send(()).is_ok());
    assert!(matches!(
        result_rx.recv_timeout(WAIT),
        Ok(Err(ControlError::Stale))
    ));
    worker.join().unwrap();
    assert_eq!(newest.is_quiescent(), Ok(true));
    assert_eq!(newest.release_key(), Ok(true));
    assert!(matches!(
        access.begin(Lane::Read),
        Err(AdmissionError::Unavailable)
    ));
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn backend_failure_and_poisoned_cleanup_stay_closed() {
    let failed = SourceAccess::close_protocol_fixture(Gate::new());
    let ticket = released_ticket(&failed);
    let mut control = ticket.start_control(Kind::CreateEmpty).unwrap();
    assert!(matches!(control.prepare(), Err(ControlError::Owner(_))));
    drop(control);
    drop(ticket);
    assert!(matches!(
        failed.begin(Lane::Read),
        Err(AdmissionError::Unavailable)
    ));

    let poisoned = SourceAccess::close_protocol_fixture(Gate::new());
    let ticket = released_ticket(&poisoned);
    let control = ticket.start_control(Kind::CreateEmpty).unwrap();
    std::thread::scope(|scope| {
        let backend = &poisoned.0.backend;
        let panic = scope.spawn(move || {
            let _held = backend.lock().unwrap();
            panic!("synthetic control cleanup poison");
        });
        assert!(panic.join().is_err());
    });
    drop(control);
    let current = poisoned.request_close().unwrap();
    assert_eq!(current.release_key(), Ok(false));
    assert!(matches!(
        current.start_control(Kind::CreateEmpty),
        Err(ControlError::Unavailable)
    ));
    assert!(matches!(
        poisoned.begin(Lane::Read),
        Err(AdmissionError::Unavailable)
    ));
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn legacy_refuses_control_without_backend_or_gate_side_effects() {
    let root = TestRoot::new("reopen-legacy");
    let gate = Gate::new();
    let access = SourceAccess::legacy(gate.clone(), Ok(root.0.clone()));
    assert!(access.request_close().is_err());
    assert!(gate.begin(Lane::Read).is_ok());
    assert!(!root.0.join("workspace.sqlite3").exists());
}
