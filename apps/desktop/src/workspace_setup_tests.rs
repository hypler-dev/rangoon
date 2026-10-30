#[cfg(feature = "encrypted-workspace")]
use super::*;

#[cfg(feature = "encrypted-workspace")]
use crate::workspace_close::{
    WorkspaceClose,
    setup::{Consent, Cycle, Preparation, Response, cancel, confirm, prepare},
};
#[cfg(feature = "encrypted-workspace")]
use rangoon_desktop::workspace_session::{Action as NativeAction, Owner};
#[cfg(feature = "encrypted-workspace")]
use serde_json::json;
#[cfg(feature = "encrypted-workspace")]
use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::{Arc, Mutex, mpsc},
    thread,
};
#[cfg(feature = "encrypted-workspace")]
use tauri::ipc::InvokeBody;

#[cfg(feature = "encrypted-workspace")]
fn response_json(response: Response) -> serde_json::Value {
    serde_json::to_value(response).unwrap()
}

#[cfg(feature = "encrypted-workspace")]
fn close_json(close: &WorkspaceClose) -> serde_json::Value {
    response_json(Response::observe(close))["close"].clone()
}

#[cfg(feature = "encrypted-workspace")]
fn request_json(access: &SourceAccess) -> serde_json::Value {
    serde_json::to_value(access.request_view()).unwrap()
}

#[cfg(feature = "encrypted-workspace")]
fn assert_owner_closed(access: &SourceAccess) {
    assert!(!access.0.backend.lock().unwrap().control_is_open());
}

#[cfg(feature = "encrypted-workspace")]
fn identity(view: &serde_json::Value) -> (String, String) {
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
fn body(value: serde_json::Value) -> InvokeBody {
    InvokeBody::Raw(serde_json::to_vec(&value).unwrap())
}

#[cfg(feature = "encrypted-workspace")]
fn preparation(id: &str, epoch: &str, action: &str) -> Preparation {
    Preparation::parse(&body(json!({
        "schemaVersion": "rangoon.workspace-control-prepare.v1",
        "attemptId": id,
        "epoch": epoch,
        "action": action,
    })))
    .unwrap()
}

#[cfg(feature = "encrypted-workspace")]
fn cycle(id: &str, epoch: &str) -> Cycle {
    Cycle::parse(&body(json!({
        "schemaVersion": "rangoon.workspace-control-cycle.v1",
        "attemptId": id,
        "epoch": epoch,
    })))
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
fn locked_close(access: SourceAccess) -> (WorkspaceClose, String, String) {
    let close = WorkspaceClose::new(access);
    let started = close_json(&close);
    assert_eq!(started["state"], "open");
    let initial = serde_json::to_value(close.begin(|_| true)).unwrap();
    assert_eq!(initial["state"], "closing");
    let (id, epoch) = identity(&initial);
    let locked =
        serde_json::to_value(close.acknowledge(&acknowledgement(&id, &epoch), |_| true)).unwrap();
    assert_eq!(locked["state"], "locked");
    (close, id, epoch)
}

#[cfg(feature = "encrypted-workspace")]
fn prepared_empty(
    close: &WorkspaceClose,
    old_id: &str,
    old_epoch: &str,
) -> (serde_json::Value, String, String) {
    prepared_action(close, old_id, old_epoch, "create_empty")
}

#[cfg(feature = "encrypted-workspace")]
fn prepared_action(
    close: &WorkspaceClose,
    old_id: &str,
    old_epoch: &str,
    action: &str,
) -> (serde_json::Value, String, String) {
    let prepared = response_json(prepare(
        close,
        preparation(old_id, old_epoch, action),
        |_| true,
    ));
    assert_eq!(prepared["outcome"], "prepared");
    assert_eq!(prepared["phase"], "awaiting_confirmation");
    let (id, epoch) = identity(&prepared["close"]);
    (prepared, id, epoch)
}

#[cfg(feature = "encrypted-workspace")]
fn acknowledge_current(close: &WorkspaceClose, id: &str, epoch: &str) {
    let view =
        serde_json::to_value(close.acknowledge(&acknowledgement(id, epoch), |_| true)).unwrap();
    assert_eq!(view["state"], "closing");
    assert_eq!(identity(&view), (id.to_owned(), epoch.to_owned()));
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn setup_prepare_requires_clean_c0_and_keeps_c1_owner_and_vault_inert() {
    let root = TestRoot::new("setup-prepare-c1");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let (close, c0_id, c0_epoch) = locked_close(access);

    let wrong = response_json(prepare(
        &close,
        preparation("0".repeat(64).as_str(), &c0_epoch, "create_empty"),
        |_| true,
    ));
    assert_eq!(wrong["outcome"], "refused");
    assert_eq!(identity(&wrong["close"]), (c0_id.clone(), c0_epoch.clone()));
    assert_eq!(vault.0.lock().unwrap().opens, 0);

    let (prepared, c1_id, c1_epoch) = prepared_empty(&close, &c0_id, &c0_epoch);
    assert_eq!(prepared["close"]["state"], "closing");
    assert_eq!(vault.0.lock().unwrap().opens, 0);

    let duplicate = response_json(prepare(
        &close,
        preparation(&c0_id, &c0_epoch, "create_empty"),
        |_| true,
    ));
    assert_eq!(duplicate["outcome"], "refused");
    assert_eq!(identity(&duplicate["close"]), (c1_id, c1_epoch));
    assert_eq!(vault.0.lock().unwrap().opens, 0);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn setup_rejects_incomplete_c0_before_c1_or_owner_preparation() {
    let root = TestRoot::new("setup-incomplete-c0");
    let vault = ThreadVault::default();
    let close = WorkspaceClose::new(locked_access(&root, &vault));
    let c0 = serde_json::to_value(close.begin(|_| true)).unwrap();
    let (id, epoch) = identity(&c0);
    let refused = response_json(prepare(
        &close,
        preparation(&id, &epoch, "create_empty"),
        |_| true,
    ));
    assert_eq!(refused["outcome"], "refused");
    assert_eq!(identity(&refused["close"]), (id, epoch));
    assert_eq!(vault.0.lock().unwrap().opens, 0);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn setup_confirm_acknowledges_c1_then_holds_real_owner_until_consumed() {
    let root = TestRoot::new("setup-confirm-consume");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let (close, c0_id, c0_epoch) = locked_close(access.clone());
    let (prepared, c1_id, c1_epoch) = prepared_empty(&close, &c0_id, &c0_epoch);
    assert_eq!(prepared.as_object().unwrap().len(), 7);
    assert!(prepared.get("previewId").is_none());
    acknowledge_current(&close, &c1_id, &c1_epoch);

    let (confirmed_tx, confirmed_rx) = mpsc::channel();
    let (consume_tx, consume_rx) = mpsc::channel();
    let worker_close = close.clone();
    let worker = thread::spawn(move || {
        response_json(confirm(
            &worker_close,
            cycle(&c1_id, &c1_epoch),
            |_| true,
            |preview| {
                assert!(preview.source_id.is_none());
                Consent::Approved
            },
            || confirmed_tx.send(()).is_ok() && consume_rx.recv().is_ok(),
        ))
    });
    confirmed_rx.recv_timeout(WAIT).unwrap();
    assert_eq!(close_json(&close)["state"], "closing");
    assert_eq!(request_json(&access)["state"], "unavailable");
    assert!(vault.0.lock().unwrap().opens >= 1);

    consume_tx.send(()).unwrap();
    let opened = worker.join().unwrap();
    assert_eq!(opened["outcome"], "opened");
    assert_eq!(opened["reload"], true);
    assert_eq!(opened["close"]["state"], "open");
    assert_eq!(request_json(&access)["state"], "open");
    assert!(opened.get("clientId").is_none());
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn setup_refuses_unacknowledged_wrong_and_replayed_c1_without_losing_retained_control() {
    let root = TestRoot::new("setup-c1-exact");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let (close, c0_id, c0_epoch) = locked_close(access.clone());
    let (_, c1_id, c1_epoch) = prepared_empty(&close, &c0_id, &c0_epoch);

    let before_ack = response_json(confirm(
        &close,
        cycle(&c1_id, &c1_epoch),
        |_| true,
        |_| panic!("unacknowledged C1 must not prepare Owner"),
        || true,
    ));
    assert_eq!(before_ack["outcome"], "refused");
    let wrong = response_json(confirm(
        &close,
        cycle(&"f".repeat(64), &c1_epoch),
        |_| true,
        |_| panic!("wrong C1 must not prepare Owner"),
        || true,
    ));
    assert_eq!(wrong["outcome"], "refused");
    assert_eq!(vault.0.lock().unwrap().opens, 0);

    acknowledge_current(&close, &c1_id, &c1_epoch);
    let declined = response_json(confirm(
        &close,
        cycle(&c1_id, &c1_epoch),
        |_| true,
        |_| Consent::Unavailable,
        || true,
    ));
    assert_eq!(declined["outcome"], "closed");
    assert_eq!(declined["diagnostic"], "workspace_control_not_completed");
    assert_owner_closed(&access);
    let replay = response_json(confirm(
        &close,
        cycle(&c1_id, &c1_epoch),
        |_| true,
        |_| panic!("replayed C1 must not prepare Owner"),
        || true,
    ));
    assert_eq!(replay["outcome"], "refused");
    assert_eq!(close_json(&close)["state"], "locked");
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn setup_unlock_confirms_existing_workspace_and_publishes_fresh_client() {
    let root = TestRoot::new("setup-unlock-success");
    let vault = ThreadVault::default();
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let initial = owner.prepare_create_empty().unwrap();
    owner.confirm(&initial.id, &vault).unwrap();
    owner.lock();
    drop(owner);

    let access = locked_access(&root, &vault);
    let (close, c0_id, c0_epoch) = locked_close(access.clone());
    let (_, c1_id, c1_epoch) = prepared_action(&close, &c0_id, &c0_epoch, "unlock");
    acknowledge_current(&close, &c1_id, &c1_epoch);
    let opened = response_json(confirm(
        &close,
        cycle(&c1_id, &c1_epoch),
        |_| true,
        |preview| {
            assert!(preview.kind == NativeAction::Unlock && preview.source_id.is_none());
            Consent::Approved
        },
        || true,
    ));
    assert_eq!(opened["outcome"], "opened");
    assert_eq!(opened["reload"], true);
    let request = request_json(&access);
    let client = request["clientId"].as_str().unwrap();
    assert_eq!(client.len(), 64);
    assert!(client.bytes().all(|byte| byte.is_ascii_hexdigit()));
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn setup_failed_consent_prepare_or_consumption_never_publishes_a_client() {
    let root = TestRoot::new("setup-failed-callbacks");
    let vault = ThreadVault::default();

    let access = locked_access(&root, &vault);
    let (close, c0_id, c0_epoch) = locked_close(access.clone());
    let (_, c1_id, c1_epoch) = prepared_empty(&close, &c0_id, &c0_epoch);
    acknowledge_current(&close, &c1_id, &c1_epoch);
    let declined = response_json(confirm(
        &close,
        cycle(&c1_id, &c1_epoch),
        |_| true,
        |_| panic!("native consent panic is closed"),
        || true,
    ));
    assert_eq!(declined["outcome"], "closed");
    assert_owner_closed(&access);
    assert_eq!(request_json(&access)["state"], "unavailable");

    let root = TestRoot::new("setup-unconsumed");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let (close, c0_id, c0_epoch) = locked_close(access.clone());
    let (_, c1_id, c1_epoch) = prepared_empty(&close, &c0_id, &c0_epoch);
    acknowledge_current(&close, &c1_id, &c1_epoch);
    let unconsumed = response_json(confirm(
        &close,
        cycle(&c1_id, &c1_epoch),
        |_| true,
        |_| Consent::Approved,
        || false,
    ));
    assert_eq!(unconsumed["outcome"], "closed");
    assert!(vault.0.lock().unwrap().opens >= 1);
    assert_owner_closed(&access);
    assert_eq!(request_json(&access)["state"], "unavailable");

    let root = TestRoot::new("setup-consumption-panic");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let (close, c0_id, c0_epoch) = locked_close(access.clone());
    let (_, c1_id, c1_epoch) = prepared_empty(&close, &c0_id, &c0_epoch);
    acknowledge_current(&close, &c1_id, &c1_epoch);
    let panicked_consumption = response_json(confirm(
        &close,
        cycle(&c1_id, &c1_epoch),
        |_| true,
        |_| Consent::Approved,
        || panic!("dropped completion is closed"),
    ));
    assert_eq!(panicked_consumption["outcome"], "closed");
    assert!(vault.0.lock().unwrap().opens >= 1);
    assert_owner_closed(&access);
    assert_eq!(request_json(&access)["state"], "unavailable");

    let root = TestRoot::new("setup-unlock-prepare-failure");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let (close, c0_id, c0_epoch) = locked_close(access.clone());
    let (_, c1_id, c1_epoch) = prepared_action(&close, &c0_id, &c0_epoch, "unlock");
    acknowledge_current(&close, &c1_id, &c1_epoch);
    let failed_prepare = response_json(confirm(
        &close,
        cycle(&c1_id, &c1_epoch),
        |_| true,
        |_| panic!("failed preparation must not ask consent"),
        || true,
    ));
    assert_eq!(failed_prepare["outcome"], "closed");
    assert_eq!(vault.0.lock().unwrap().opens, 0);
    assert_owner_closed(&access);
    assert_eq!(request_json(&access)["state"], "unavailable");
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn setup_parse_is_bounded_canonical_and_side_effect_free() {
    let valid_id = "a".repeat(64);
    for body in [
        InvokeBody::Raw(Vec::new()),
        body(json!({
            "schemaVersion": "rangoon.workspace-control-prepare.v1",
            "attemptId": valid_id.clone(),
            "epoch": "01",
            "action": "unlock",
        })),
        body(json!({
            "schemaVersion": "rangoon.workspace-control-prepare.v1",
            "attemptId": "A".repeat(64),
            "epoch": "1",
            "action": "unknown",
        })),
        InvokeBody::Raw(vec![b'x'; 513]),
        InvokeBody::Raw(
            br#"{"schemaVersion":"rangoon.workspace-control-cycle.v1","attemptId":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","epoch":"1","epoch":"1"}"#.to_vec(),
        ),
    ] {
        assert!(Preparation::parse(&body).is_none());
        assert!(Cycle::parse(&body).is_none());
    }
    assert!(
        Cycle::parse(&body(json!({
            "schemaVersion": "rangoon.workspace-control-cycle.v1",
        "attemptId": valid_id,
            "epoch": "1",
        })))
        .is_some()
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn setup_observation_and_legacy_mode_are_pure_local_and_source_free() {
    let (legacy, state, _) = probe(report("setup-legacy.md", b"legacy bytes"));
    let close = WorkspaceClose::new(legacy);
    let observed = response_json(Response::observe(&close));
    let fields = observed.as_object().unwrap();
    assert_eq!(fields.len(), 7);
    for field in [
        "schemaVersion",
        "close",
        "phase",
        "action",
        "outcome",
        "diagnostic",
        "reload",
    ] {
        assert!(fields.contains_key(field));
    }
    assert_eq!(observed["close"]["state"], "unavailable");
    assert_eq!(observed["phase"], "unavailable");
    assert_eq!(observed["outcome"], "observed");
    assert_eq!(observed["action"], serde_json::Value::Null);
    assert_eq!(state.list_calls.load(Ordering::SeqCst), 0);
    assert_eq!(state.open_calls.load(Ordering::SeqCst), 0);
    assert_eq!(state.save_calls.load(Ordering::SeqCst), 0);

    let main = include_str!("main.rs");
    let build = include_str!("../build.rs");
    let permissions: serde_json::Value =
        serde_json::from_str(include_str!("../capabilities/source-analysis.json")).unwrap();
    let granted = permissions["permissions"].as_array().unwrap();
    for command in [
        "get_workspace_control_state",
        "prepare_workspace_control",
        "confirm_workspace_control",
        "cancel_workspace_control",
    ] {
        assert!(main.contains(command));
        assert!(build.contains(&format!("\"{command}\"")));
        assert!(granted.iter().any(|value| {
            value == &serde_json::Value::String(format!("allow-{}", command.replace('_', "-")))
        }));
    }
    assert_eq!(permissions["windows"], json!(["main"]));
    assert_eq!(permissions["local"], true);
    for private in [
        include_str!("../capabilities/cloud-credential-entry.json"),
        include_str!("../capabilities/cloud-model-confirmation.json"),
        include_str!("../capabilities/model-confirmation.json"),
    ] {
        for command in [
            "get-workspace-control-state",
            "prepare-workspace-control",
            "confirm-workspace-control",
            "cancel-workspace-control",
        ] {
            assert!(!private.contains(command));
        }
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn setup_worker_result_wait_stops_on_shutdown_or_disconnected_completion() {
    let (held_sender, receiver) = mpsc::channel::<()>();
    let alive = AtomicBool::new(false);
    assert_eq!(
        crate::workspace_close::failure::wait_for_result(receiver, &alive),
        None
    );
    drop(held_sender);

    let (_sender, receiver) = mpsc::channel::<()>();
    let alive = AtomicBool::new(true);
    drop(_sender);
    assert_eq!(
        crate::workspace_close::failure::wait_for_result(receiver, &alive),
        None
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn setup_cancelled_blocked_callback_stays_closed_and_late_result_cannot_resume() {
    let root = TestRoot::new("setup-cancel-worker");
    let vault = BlockingVault::new(ThreadVault::default());
    let access = SourceAccess::managed(
        Gate::new(),
        Owner::acquire(root.0.clone()).unwrap(),
        vault.clone(),
    );
    let (close, c0_id, c0_epoch) = locked_close(access.clone());
    let (_, c1_id, c1_epoch) = prepared_empty(&close, &c0_id, &c0_epoch);
    acknowledge_current(&close, &c1_id, &c1_epoch);

    let (entered, release) = vault.block_next_open();
    let worker_close = close.clone();
    let callback_id = c1_id.clone();
    let callback_epoch = c1_epoch.clone();
    let worker = thread::spawn(move || {
        response_json(confirm(
            &worker_close,
            cycle(&callback_id, &callback_epoch),
            |_| true,
            |_| Consent::Approved,
            || true,
        ))
    });
    entered.recv_timeout(WAIT).unwrap();
    let cancelled = response_json(cancel(&close, cycle(&c1_id, &c1_epoch), |_| true));
    assert_eq!(cancelled["outcome"], "cancelled");
    assert_eq!(cancelled["close"]["state"], "closing");
    release.send(()).unwrap();
    let late = worker.join().unwrap();
    assert_eq!(late["outcome"], "closed");
    assert_eq!(late["reload"], false);
    assert_owner_closed(&access);
    assert_eq!(request_json(&access)["state"], "unavailable");
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn setup_registry_poison_after_owner_confirmation_fails_closed() {
    let root = TestRoot::new("setup-registry-poison");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let (close, c0_id, c0_epoch) = locked_close(access.clone());
    let (_, c1_id, c1_epoch) = prepared_empty(&close, &c0_id, &c0_epoch);
    acknowledge_current(&close, &c1_id, &c1_epoch);

    let result = response_json(confirm(
        &close,
        cycle(&c1_id, &c1_epoch),
        |_| true,
        |_| Consent::Approved,
        || {
            access.0.clients.poison_for_test();
            true
        },
    ));
    assert_eq!(result["outcome"], "closed");
    assert_eq!(result["diagnostic"], "workspace_control_not_completed");
    assert_eq!(result["reload"], false);
    assert_owner_closed(&access);
    assert_eq!(request_json(&access)["state"], "unavailable");
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
            assert!(release.recv_timeout(WAIT).is_ok());
        }
        rangoon_desktop::workspace_session::Vault::open(&self.inner, identity)
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn setup_explicit_cancel_releases_custody_and_allows_fresh_setup_or_unlock() {
    for action in ["create_empty", "unlock"] {
        let root = TestRoot::new("setup-explicit-cancel");
        let vault = ThreadVault::default();
        if action == "unlock" {
            let mut owner = Owner::acquire(root.0.clone()).unwrap();
            let preview = owner.prepare_create_empty().unwrap();
            owner.confirm(&preview.id, &vault).unwrap();
            owner.lock();
        }
        let opens_before = vault.0.lock().unwrap().opens;
        let access = locked_access(&root, &vault);
        let (close, mut id, mut epoch) = locked_close(access.clone());
        for _ in 0..2 {
            let (_, next_id, next_epoch) = prepared_action(&close, &id, &epoch, action);
            assert_ne!(id, next_id);
            assert_ne!(epoch, next_epoch);
            acknowledge_current(&close, &next_id, &next_epoch);
            let cleanup_ran = Arc::new(AtomicBool::new(false));
            let cleaned = cleanup_ran.clone();
            let cancelled = response_json(confirm(
                &close,
                cycle(&next_id, &next_epoch),
                move |drained| {
                    cleaned.store(drained, Ordering::SeqCst);
                    drained
                },
                |_| Consent::Cancelled,
                || panic!("cancel must not consume a successful Owner confirmation"),
            ));
            assert!(cleanup_ran.load(Ordering::SeqCst));
            assert_eq!(cancelled["outcome"], "cancelled");
            assert_eq!(cancelled["phase"], "locked");
            assert_eq!(cancelled["diagnostic"], serde_json::Value::Null);
            assert_eq!(cancelled["action"], serde_json::Value::Null);
            assert_eq!(cancelled["reload"], false);
            assert_eq!(
                identity(&cancelled["close"]),
                (next_id.clone(), next_epoch.clone())
            );
            assert_owner_closed(&access);
            assert_eq!(request_json(&access)["state"], "unavailable");
            assert_eq!(vault.0.lock().unwrap().opens, opens_before);
            let protection = serde_json::to_value(
                crate::workspace_close::setup::ProtectionResponse::observe(&close),
            )
            .unwrap();
            assert_eq!(
                protection["layout"],
                if action == "unlock" {
                    "locked"
                } else {
                    "absent"
                }
            );
            id = next_id;
            epoch = next_epoch;
        }
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn setup_cancel_with_failed_or_panicked_cleanup_cannot_report_retryable_cancellation() {
    for panic_cleanup in [false, true] {
        let root = TestRoot::new("setup-cancel-cleanup-failure");
        let vault = ThreadVault::default();
        let access = locked_access(&root, &vault);
        let (close, id, epoch) = locked_close(access.clone());
        let (_, id, epoch) = prepared_empty(&close, &id, &epoch);
        acknowledge_current(&close, &id, &epoch);
        let cancelled = response_json(confirm(
            &close,
            cycle(&id, &epoch),
            move |_| {
                assert!(!panic_cleanup, "synthetic cleanup failure");
                false
            },
            |_| Consent::Cancelled,
            || panic!("cancel never consumes confirmation"),
        ));
        assert_eq!(cancelled["outcome"], "closed");
        assert_eq!(cancelled["diagnostic"], "workspace_control_not_completed");
        assert_eq!(cancelled["phase"], "recovery_required");
        assert_eq!(cancelled["reload"], false);
        assert_owner_closed(&access);
        assert_eq!(request_json(&access)["state"], "unavailable");
        assert_eq!(vault.0.lock().unwrap().opens, 0);
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn setup_late_cancel_after_concurrent_close_or_exit_cannot_offer_retry() {
    for exit in [false, true] {
        let root = TestRoot::new("setup-late-native-cancel");
        let vault = ThreadVault::default();
        let access = locked_access(&root, &vault);
        let (close, id, epoch) = locked_close(access.clone());
        let (_, id, epoch) = prepared_empty(&close, &id, &epoch);
        acknowledge_current(&close, &id, &epoch);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (return_tx, return_rx) = mpsc::channel();
        let worker_close = close.clone();
        let worker_id = id.clone();
        let worker_epoch = epoch.clone();
        let worker = thread::spawn(move || {
            response_json(confirm(
                &worker_close,
                cycle(&worker_id, &worker_epoch),
                |_| true,
                |_| {
                    entered_tx.send(()).unwrap();
                    return_rx.recv_timeout(WAIT).unwrap();
                    Consent::Cancelled
                },
                || panic!("late cancel never consumes confirmation"),
            ))
        });
        entered_rx.recv_timeout(WAIT).unwrap();
        if exit {
            close.fence_admission_for_exit();
        } else {
            cancel(&close, cycle(&id, &epoch), |_| true);
        }
        return_tx.send(()).unwrap();
        let late = worker.join().unwrap();
        assert_eq!(late["outcome"], "closed");
        assert_eq!(late["reload"], false);
        assert_owner_closed(&access);
        assert_eq!(request_json(&access)["state"], "unavailable");
        assert_eq!(vault.0.lock().unwrap().opens, 0);
    }
}
