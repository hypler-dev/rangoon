use super::*;

use crate::{
    capabilities::CapabilityResult,
    workspace_close::{
        WorkspaceClose,
        setup::{self, Cycle, Preparation},
    },
    workspace_requests::WorkspaceRequest,
};
use rangoon_desktop::{
    workspace_keys::{Error as KeyError, Store, WorkspaceId},
    workspace_session::{Error as OwnerError, Vault},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    sync::{Arc, Mutex},
};
use tauri::ipc::InvokeBody;
use zeroize::Zeroizing;

#[derive(Default)]
struct CountingVaultState {
    slots: BTreeMap<String, Option<Vec<u8>>>,
    opens: usize,
    reads: usize,
    writes: usize,
}

#[derive(Clone, Default)]
struct CountingVault(Arc<Mutex<CountingVaultState>>);

struct CountingSlot {
    identity: WorkspaceId,
    lookup: String,
    state: Arc<Mutex<CountingVaultState>>,
}

impl CountingVault {
    fn counts(&self) -> (usize, usize, usize) {
        let state = self.0.lock().unwrap();
        (state.opens, state.reads, state.writes)
    }

    fn reset_counts(&self) {
        let mut state = self.0.lock().unwrap();
        state.opens = 0;
        state.reads = 0;
        state.writes = 0;
    }
}

impl Vault for CountingVault {
    type Slot = CountingSlot;

    fn open(&self, identity: &WorkspaceId) -> Result<Self::Slot, KeyError> {
        let mut state = self.0.lock().unwrap();
        state.opens += 1;
        let lookup = identity.as_hex();
        state.slots.entry(lookup.clone()).or_insert(None);
        Ok(CountingSlot {
            identity: identity.clone(),
            lookup,
            state: Arc::clone(&self.0),
        })
    }
}

impl Store for CountingSlot {
    fn identity(&self) -> &WorkspaceId {
        &self.identity
    }

    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, KeyError> {
        let mut state = self.state.lock().unwrap();
        state.reads += 1;
        Ok(state
            .slots
            .get(&self.lookup)
            .cloned()
            .flatten()
            .map(Zeroizing::new))
    }

    fn write(&self, envelope: &[u8]) -> Result<(), KeyError> {
        let mut state = self.state.lock().unwrap();
        state.writes += 1;
        state
            .slots
            .insert(self.lookup.clone(), Some(envelope.to_vec()));
        Ok(())
    }
}

fn managed_locked(root: &TestRoot, vault: &CountingVault) -> SourceAccess {
    SourceAccess::managed_locked(Gate::new(), root.0.clone(), vault.clone()).unwrap()
}

#[test]
fn deferred_managed_refuses_prebind_content_without_latching_failure_or_touching_storage() {
    let root = TestRoot::new("deferred-prebind");
    let roots = crate::workspace_startup::StartupRoots::new(root.0.clone());
    let vault = CountingVault::default();
    let access = SourceAccess::managed_deferred(Gate::new(), roots.managed(), vault.clone());
    assert!(access.encrypted_close_available());
    assert!(!roots.managed().exists());
    assert_unavailable(&access);
    let called = std::cell::Cell::new(false);
    let result = access.call_backend(
        || true,
        |_| {
            called.set(true);
            Ok(())
        },
        Error::Store(StoreError::Unavailable),
    );
    assert!(result.is_err());
    assert!(!called.get());
    assert!(!access.failure_latched());
    assert!(!roots.managed().exists());
    assert_eq!(vault.counts(), (0, 0, 0));
}

#[test]
fn deferred_binding_uses_real_closed_custody_then_existing_native_confirmation() {
    let root = TestRoot::new("deferred-confirm");
    let roots = crate::workspace_startup::StartupRoots::new(root.0.clone());
    let vault = CountingVault::default();
    let access = SourceAccess::managed_deferred(Gate::new(), roots.managed(), vault.clone());
    let close = WorkspaceClose::new(access.clone());
    let (id, epoch) = complete_c0(&close);
    let mut flight = close
        .start_control_for(&id, &epoch, ControlKind::CreateEmpty, synthetic_cleanup_ok)
        .unwrap();
    let proof = roots.fresh_binding(roots.inspect()).unwrap();
    flight.bind_locked_owner(proof).unwrap();
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_unavailable(&access);
    assert_eq!(fs::read_dir(roots.managed()).unwrap().count(), 1);
    flight.retain().unwrap();
    let current = serde_json::to_value(close.inspect_control().0).unwrap();
    let (id, epoch) = identity(&current);
    close.acknowledge(&acknowledgement(&id, &epoch), synthetic_cleanup_ok);
    assert_eq!(consume_c1(&close, &id, &epoch)["outcome"], "opened");
    assert_eq!(vault.counts().2, 1);
    assert!(!client_id(&access).is_empty());
    assert!(!roots.legacy().exists());
    close_open_access(&access, &close);
}

#[test]
fn deferred_bind_twice_marks_cycle_unhealthy_without_any_credential_use() {
    let root = TestRoot::new("deferred-twice");
    let roots = crate::workspace_startup::StartupRoots::new(root.0.clone());
    let vault = CountingVault::default();
    let access = SourceAccess::managed_deferred(Gate::new(), roots.managed(), vault.clone());
    let close = WorkspaceClose::new(access.clone());
    let (id, epoch) = complete_c0(&close);
    let mut flight = close
        .start_control_for(&id, &epoch, ControlKind::CreateEmpty, synthetic_cleanup_ok)
        .unwrap();
    flight
        .bind_locked_owner(roots.fresh_binding(roots.inspect()).unwrap())
        .unwrap();
    assert!(
        flight
            .bind_locked_owner(roots.setup_residue_binding().unwrap())
            .is_err()
    );
    drop(flight);
    assert_eq!(
        serde_json::to_value(close.inspect_control().0).unwrap()["state"],
        "recovery_required"
    );
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_unavailable(&access);
}

#[test]
fn deferred_bind_rejects_foreign_native_root_before_owner_or_credential_access() {
    let root = TestRoot::new("deferred-fixed");
    let foreign = TestRoot::new("deferred-foreign");
    let roots = crate::workspace_startup::StartupRoots::new(root.0.clone());
    let other = crate::workspace_startup::StartupRoots::new(foreign.0.clone());
    let vault = CountingVault::default();
    let access = SourceAccess::managed_deferred(Gate::new(), roots.managed(), vault.clone());
    let close = WorkspaceClose::new(access.clone());
    let (id, epoch) = complete_c0(&close);
    let mut flight = close
        .start_control_for(&id, &epoch, ControlKind::CreateEmpty, synthetic_cleanup_ok)
        .unwrap();
    assert!(
        flight
            .bind_locked_owner(other.fresh_binding(other.inspect()).unwrap())
            .is_err()
    );
    drop(flight);
    assert!(!roots.managed().exists());
    assert_eq!(fs::read_dir(other.managed()).unwrap().count(), 0);
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_unavailable(&access);
}

#[test]
fn deferred_cancelled_setup_can_bind_exact_existing_control_after_restart() {
    let root = TestRoot::new("deferred-cancel-restart");
    let roots = crate::workspace_startup::StartupRoots::new(root.0.clone());
    let vault = CountingVault::default();
    {
        let access = SourceAccess::managed_deferred(Gate::new(), roots.managed(), vault.clone());
        let close = WorkspaceClose::new(access.clone());
        let (id, epoch) = complete_c0(&close);
        let mut flight = close
            .start_control_for(&id, &epoch, ControlKind::CreateEmpty, synthetic_cleanup_ok)
            .unwrap();
        flight
            .bind_locked_owner(roots.fresh_binding(roots.inspect()).unwrap())
            .unwrap();
        flight.retain().unwrap();
        let current = serde_json::to_value(close.inspect_control().0).unwrap();
        let (id, epoch) = identity(&current);
        close.acknowledge(&acknowledgement(&id, &epoch), synthetic_cleanup_ok);
        let response = setup::confirm(
            &close,
            cycle(&id, &epoch),
            synthetic_cleanup_ok,
            |_| setup::Consent::Cancelled,
            || true,
        );
        assert_eq!(
            serde_json::to_value(response).unwrap()["outcome"],
            "cancelled"
        );
        assert_unavailable(&access);
    }
    assert_eq!(vault.counts(), (0, 0, 0));
    let access = SourceAccess::managed_deferred(Gate::new(), roots.managed(), vault.clone());
    let close = WorkspaceClose::new(access.clone());
    let (id, epoch) = complete_c0(&close);
    let mut flight = close
        .start_control_for(&id, &epoch, ControlKind::CreateEmpty, synthetic_cleanup_ok)
        .unwrap();
    flight
        .bind_locked_owner(roots.setup_residue_binding().unwrap())
        .unwrap();
    drop(flight);
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_eq!(fs::read_dir(roots.managed()).unwrap().count(), 1);
    assert_unavailable(&access);
}

fn synthetic_cleanup_ok(_: bool) -> bool {
    // Synthetic cleanup double only. It is not native-window or OS-consent proof.
    true
}

fn protection_json(close: &WorkspaceClose) -> serde_json::Value {
    serde_json::to_value(setup::ProtectionResponse::observe(close)).unwrap()
}

fn identity(value: &serde_json::Value) -> (String, String) {
    (
        value["attemptId"].as_str().unwrap().to_owned(),
        value["epoch"].as_str().unwrap().to_owned(),
    )
}

fn body(value: serde_json::Value) -> InvokeBody {
    InvokeBody::Raw(serde_json::to_vec(&value).unwrap())
}

fn preparation(id: &str, epoch: &str, action: &str) -> Preparation {
    Preparation::parse(&body(json!({
        "schemaVersion": "rangoon.workspace-control-prepare.v1",
        "attemptId": id,
        "epoch": epoch,
        "action": action,
    })))
    .unwrap()
}

fn cycle(id: &str, epoch: &str) -> Cycle {
    Cycle::parse(&body(json!({
        "schemaVersion": "rangoon.workspace-control-cycle.v1",
        "attemptId": id,
        "epoch": epoch,
    })))
    .unwrap()
}

fn acknowledgement(id: &str, epoch: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.workspace-purge-ack.v1",
        "attemptId": id,
        "epoch": epoch,
    }))
    .unwrap()
}

fn complete_c0(close: &WorkspaceClose) -> (String, String) {
    let started = serde_json::to_value(close.begin(synthetic_cleanup_ok)).unwrap();
    assert_eq!(started["state"], "closing");
    let (id, epoch) = identity(&started);
    let locked = serde_json::to_value(
        close.acknowledge(&acknowledgement(&id, &epoch), synthetic_cleanup_ok),
    )
    .unwrap();
    assert_eq!(locked["state"], "locked");
    (id, epoch)
}

fn prepare_c1(
    close: &WorkspaceClose,
    c0_id: &str,
    c0_epoch: &str,
    action: &str,
) -> (String, String) {
    let response = serde_json::to_value(setup::prepare(
        close,
        preparation(c0_id, c0_epoch, action),
        synthetic_cleanup_ok,
    ))
    .unwrap();
    assert_eq!(response["outcome"], "prepared");
    let (id, epoch) = identity(&response["close"]);
    let acknowledged = serde_json::to_value(
        close.acknowledge(&acknowledgement(&id, &epoch), synthetic_cleanup_ok),
    )
    .unwrap();
    assert_eq!(acknowledged["state"], "closing");
    (id, epoch)
}

fn consume_c1(close: &WorkspaceClose, id: &str, epoch: &str) -> serde_json::Value {
    serde_json::to_value(setup::confirm(
        close,
        cycle(id, epoch),
        synthetic_cleanup_ok,
        |_| setup::Consent::Approved,
        || true,
    ))
    .unwrap()
}

fn client_id(access: &SourceAccess) -> String {
    serde_json::to_value(access.request_view()).unwrap()["clientId"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn assert_unavailable(access: &SourceAccess) {
    assert_eq!(
        serde_json::to_value(access.request_view()).unwrap()["state"],
        "unavailable"
    );
    assert!(matches!(
        access.request_context("main", Some(&"a".repeat(64))),
        WorkspaceRequest::Refused
    ));
    assert!(matches!(
        access.request_context("main", None),
        WorkspaceRequest::Refused
    ));
}

fn close_open_access(access: &SourceAccess, close: &WorkspaceClose) {
    complete_c0(close);
    assert!(!access.0.backend.lock().unwrap().control_is_open());
}

#[test]
fn managed_locked_cold_absent_and_existing_roots_do_not_touch_vault_or_issue_requests() {
    let root = TestRoot::new("managed-bootstrap-cold-absent");
    let vault = CountingVault::default();
    let access = managed_locked(&root, &vault);
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_unavailable(&access);
    assert_eq!(
        workspace_code(tauri::async_runtime::block_on(access.list_snapshots())),
        "workspace_unavailable"
    );
    assert_eq!(vault.counts(), (0, 0, 0));

    let close = WorkspaceClose::new(access);
    let (c0_id, c0_epoch) = complete_c0(&close);
    assert_eq!(protection_json(&close)["layout"], "absent");
    let (c1_id, c1_epoch) = prepare_c1(&close, &c0_id, &c0_epoch, "create_empty");
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_eq!(consume_c1(&close, &c1_id, &c1_epoch)["outcome"], "opened");
    assert!(vault.counts().0 > 0);
    drop(close);

    vault.reset_counts();
    let reopened = managed_locked(&root, &vault);
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_unavailable(&reopened);
    drop(reopened);
}

#[test]
fn managed_locked_c0_c1_consumption_binds_real_requests_and_reopen_preserves_source() {
    let root = TestRoot::new("managed-bootstrap-round-trip");
    let vault = CountingVault::default();
    let source = report(
        "managed-bootstrap.md",
        b"# Managed bootstrap\nsecret data\n",
    );
    let source_id = source.source.id.clone();

    let old_header = {
        let access = managed_locked(&root, &vault);
        assert_unavailable(&access);
        let close = WorkspaceClose::new(access.clone());
        let (c0_id, c0_epoch) = complete_c0(&close);
        assert_eq!(protection_json(&close)["layout"], "absent");
        let (c1_id, c1_epoch) = prepare_c1(&close, &c0_id, &c0_epoch, "create_empty");
        let opened = consume_c1(&close, &c1_id, &c1_epoch);
        assert_eq!(opened["outcome"], "opened");
        assert_eq!(opened["reload"], true);

        let header = client_id(&access);
        assert!(matches!(
            access.request_context("main", None),
            WorkspaceRequest::Refused
        ));
        let request = access.request_context("main", Some(&header));
        assert!(matches!(request, WorkspaceRequest::Managed(_)));
        let bound = access.for_request(&request);
        select(&bound, &source);
        assert!(matches!(
            tauri::async_runtime::block_on(bound.save_analysis(source_id.clone())),
            WorkspaceResult::Saved { .. }
        ));
        match tauri::async_runtime::block_on(bound.open_snapshot(source_id.clone())) {
            SelectionResult::Analyzed { report } => assert_eq!(*report, source),
            _ => panic!("expected header-bound encrypted source"),
        }
        assert!(matches!(
            tauri::async_runtime::block_on(bound.list_capabilities()),
            CapabilityResult::Listed { .. }
        ));
        assert!(vault.counts().1 > 0);

        let database = fs::read_dir(root.0.join("encrypted-workspaces"))
            .unwrap()
            .find_map(Result::ok)
            .map(|entry| entry.path().join("workspace.sqlite3"))
            .unwrap();
        let bytes = fs::read(database).unwrap();
        assert!(!bytes.starts_with(b"SQLite format 3\0"));

        close_open_access(&access, &close);
        assert!(matches!(
            access.request_context("main", Some(&header)),
            WorkspaceRequest::Refused
        ));
        assert!(matches!(
            tauri::async_runtime::block_on(bound.list_snapshots()),
            WorkspaceResult::Failed { .. }
        ));
        drop(bound);
        drop(request);
        drop(close);
        drop(access);
        header
    };

    let reopened = managed_locked(&root, &vault);
    let close = WorkspaceClose::new(reopened.clone());
    let (c0_id, c0_epoch) = complete_c0(&close);
    assert_eq!(protection_json(&close)["layout"], "locked");
    let (c1_id, c1_epoch) = prepare_c1(&close, &c0_id, &c0_epoch, "unlock");
    assert_eq!(consume_c1(&close, &c1_id, &c1_epoch)["outcome"], "opened");
    let header = client_id(&reopened);
    assert_ne!(header, old_header);
    assert!(matches!(
        reopened.request_context("main", Some(&old_header)),
        WorkspaceRequest::Refused
    ));
    assert!(matches!(
        reopened.request_context("main", None),
        WorkspaceRequest::Refused
    ));
    let request = reopened.request_context("main", Some(&header));
    let bound = reopened.for_request(&request);
    match tauri::async_runtime::block_on(bound.open_snapshot(source_id)) {
        SelectionResult::Analyzed { report } => assert_eq!(*report, source),
        _ => panic!("expected source after same-vault unlock"),
    }
}

#[test]
fn managed_locked_missing_key_refuses_unlock_without_regeneration_or_client() {
    let root = TestRoot::new("managed-bootstrap-missing-key");
    let vault = CountingVault::default();

    {
        let access = managed_locked(&root, &vault);
        let close = WorkspaceClose::new(access.clone());
        let (c0_id, c0_epoch) = complete_c0(&close);
        let (c1_id, c1_epoch) = prepare_c1(&close, &c0_id, &c0_epoch, "create_empty");
        assert_eq!(consume_c1(&close, &c1_id, &c1_epoch)["outcome"], "opened");
        close_open_access(&access, &close);
    }

    let key = vault
        .0
        .lock()
        .unwrap()
        .slots
        .keys()
        .next()
        .cloned()
        .unwrap();
    vault.0.lock().unwrap().slots.remove(&key);
    vault.reset_counts();

    let access = managed_locked(&root, &vault);
    let close = WorkspaceClose::new(access.clone());
    let (c0_id, c0_epoch) = complete_c0(&close);
    assert_eq!(protection_json(&close)["layout"], "locked");
    let (c1_id, c1_epoch) = prepare_c1(&close, &c0_id, &c0_epoch, "unlock");
    let refused = consume_c1(&close, &c1_id, &c1_epoch);
    assert_eq!(refused["outcome"], "closed");
    assert_unavailable(&access);
    let (opens, reads, writes) = vault.counts();
    assert!(opens > 0 && reads > 0);
    assert_eq!(writes, 0);
    assert_eq!(vault.0.lock().unwrap().slots.get(&key), Some(&None));
}

#[test]
fn managed_locked_busy_orphan_and_torn_roots_remain_closed_without_key_regeneration() {
    let busy_root = TestRoot::new("managed-bootstrap-busy");
    let held = rangoon_desktop::workspace_session::Owner::acquire(busy_root.0.clone()).unwrap();
    let vault = CountingVault::default();
    assert!(matches!(
        SourceAccess::managed_locked(Gate::new(), busy_root.0.clone(), vault.clone()),
        Err(OwnerError::Busy)
    ));
    assert_eq!(vault.counts(), (0, 0, 0));
    drop(held);

    for (label, seed) in [
        ("orphan", "encrypted-workspaces"),
        ("torn", "encrypted-workspace.binding"),
    ] {
        let root = TestRoot::new(&format!("managed-bootstrap-{label}"));
        let vault = CountingVault::default();
        if seed == "encrypted-workspaces" {
            fs::create_dir(root.0.join(seed)).unwrap();
        } else {
            fs::write(root.0.join(seed), b"torn binding").unwrap();
        }
        let access = managed_locked(&root, &vault);
        let close = WorkspaceClose::new(access.clone());
        let (c0_id, c0_epoch) = complete_c0(&close);
        if label == "orphan" {
            assert_eq!(protection_json(&close)["layout"], "recovery_required");
        }
        let (c1_id, c1_epoch) = prepare_c1(&close, &c0_id, &c0_epoch, "create_empty");
        let result = consume_c1(&close, &c1_id, &c1_epoch);
        assert_eq!(result["outcome"], "closed", "{label}");
        assert_unavailable(&access);
        assert_eq!(vault.counts(), (0, 0, 0), "{label}");
    }
}

#[test]
fn managed_locked_cancel_stale_dropped_and_failed_cleanup_never_publish_client() {
    let root = TestRoot::new("managed-bootstrap-cancel");
    let vault = CountingVault::default();
    let access = managed_locked(&root, &vault);
    let close = WorkspaceClose::new(access.clone());
    let (c0_id, c0_epoch) = complete_c0(&close);
    let refused = serde_json::to_value(setup::prepare(
        &close,
        preparation(&"f".repeat(64), &c0_epoch, "create_empty"),
        synthetic_cleanup_ok,
    ))
    .unwrap();
    assert_eq!(refused["outcome"], "refused");
    let (c1_id, c1_epoch) = prepare_c1(&close, &c0_id, &c0_epoch, "create_empty");
    let cancelled = serde_json::to_value(setup::cancel(
        &close,
        cycle(&c1_id, &c1_epoch),
        synthetic_cleanup_ok,
    ))
    .unwrap();
    assert_eq!(cancelled["outcome"], "cancelled");
    assert_unavailable(&access);
    assert_eq!(vault.counts(), (0, 0, 0));

    let root = TestRoot::new("managed-bootstrap-dropped-result");
    let vault = CountingVault::default();
    let access = managed_locked(&root, &vault);
    let close = WorkspaceClose::new(access.clone());
    let (c0_id, c0_epoch) = complete_c0(&close);
    let (c1_id, c1_epoch) = prepare_c1(&close, &c0_id, &c0_epoch, "create_empty");
    let dropped = serde_json::to_value(setup::confirm(
        &close,
        cycle(&c1_id, &c1_epoch),
        synthetic_cleanup_ok,
        |_| setup::Consent::Approved,
        || false,
    ))
    .unwrap();
    assert_eq!(dropped["outcome"], "closed");
    assert_unavailable(&access);

    let root = TestRoot::new("managed-bootstrap-failed-cleanup");
    let vault = CountingVault::default();
    let access = managed_locked(&root, &vault);
    let close = WorkspaceClose::new(access.clone());
    let closing = serde_json::to_value(close.begin(|_| false)).unwrap();
    let (id, epoch) = identity(&closing);
    let failed = serde_json::to_value(
        close.acknowledge(&acknowledgement(&id, &epoch), synthetic_cleanup_ok),
    )
    .unwrap();
    assert_eq!(failed["state"], "recovery_required");
    assert_unavailable(&access);
    assert_eq!(vault.counts(), (0, 0, 0));
}
