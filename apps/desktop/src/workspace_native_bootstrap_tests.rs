use super::*;

use crate::workspace_close::WorkspaceClose;
#[cfg(feature = "encrypted-workspace")]
use crate::workspace_requests::WorkspaceRequest;
use rangoon_desktop::workspace_lifecycle::Lane;
#[cfg(all(feature = "encrypted-workspace", unix))]
use rangoon_desktop::workspace_session::Owner;
#[cfg(feature = "encrypted-workspace")]
use rangoon_desktop::{
    workspace_keys::{Error as KeyError, Store, WorkspaceId},
    workspace_session::Vault,
};
#[cfg(all(feature = "encrypted-workspace", unix))]
use rangoon_store::Workspace;
#[cfg(all(feature = "encrypted-workspace", unix))]
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
#[cfg(all(feature = "encrypted-workspace", unix))]
use std::{
    io::{Read, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
#[cfg(all(feature = "encrypted-workspace", unix))]
use tauri::ipc::InvokeBody;
#[cfg(feature = "encrypted-workspace")]
use zeroize::Zeroizing;

struct TestRoot(PathBuf);

#[cfg(all(feature = "encrypted-workspace", unix))]
struct ChildGuard(Child);

#[cfg(all(feature = "encrypted-workspace", unix))]
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl TestRoot {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rangoon-native-bootstrap-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
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

#[test]
fn unavailable_backend_still_closes_before_presenting_a_source_free_window() {
    let gate = Gate::new();
    let source = SourceAccess::unavailable(gate.clone());
    let close = WorkspaceClose::new(source.clone());
    let calls = Arc::new(Mutex::new(Vec::new()));
    let result = prepare_workbench(
        &gate,
        &source,
        &close,
        true,
        {
            let calls = Arc::clone(&calls);
            move |_| {
                calls.lock().unwrap().push("cleanup");
                true
            }
        },
        {
            let calls = Arc::clone(&calls);
            let source = source.clone();
            let gate = gate.clone();
            let close = close.clone();
            move || {
                assert!(gate.begin(Lane::Read).is_err());
                assert!(serde_json::to_value(source.request_view()).unwrap()["clientId"].is_null());
                assert_eq!(
                    serde_json::to_value(close.inspect(|_| true)).unwrap()["state"],
                    "closing"
                );
                calls.lock().unwrap().push("window");
                Ok(())
            }
        },
    );
    assert_eq!(result, Ok(()));
    assert_eq!(*calls.lock().unwrap(), vec!["cleanup", "window"]);
    assert!(!source.failure_latched());
}

#[cfg(feature = "encrypted-workspace")]
fn view(close: &WorkspaceClose) -> serde_json::Value {
    serde_json::to_value(close.inspect(|_| true)).unwrap()
}

#[cfg(all(feature = "encrypted-workspace", unix))]
fn acknowledgement(view: &serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.workspace-purge-ack.v1",
        "attemptId": view["attemptId"].as_str().unwrap(),
        "epoch": view["epoch"].as_str().unwrap(),
    }))
    .unwrap()
}

#[cfg(all(feature = "encrypted-workspace", unix))]
fn action_request(
    view: &serde_json::Value,
    action: &str,
) -> crate::workspace_startup_host::ActionRequest {
    crate::workspace_startup_host::ActionRequest::parse(
        &serde_json::to_vec(&json!({
            "schemaVersion": "rangoon.workspace-startup-action.v1",
            "startupId": view["startupId"].as_str().unwrap(),
            "attemptId": view["attemptId"].as_str().unwrap(),
            "epoch": view["epoch"].as_str().unwrap(),
            "action": action,
        }))
        .unwrap(),
    )
    .unwrap()
}

#[cfg(all(feature = "encrypted-workspace", unix))]
fn completed_c1(close: &WorkspaceClose) -> serde_json::Value {
    let closing = view(close);
    assert_eq!(closing["state"], "closing");
    serde_json::to_value(close.acknowledge(&acknowledgement(&closing), |_| true)).unwrap()
}

#[cfg(all(feature = "encrypted-workspace", unix))]
fn closed_startup_cycle(close: &WorkspaceClose) {
    let started = serde_json::to_value(close.begin(|_| true)).unwrap();
    assert_eq!(started["state"], "closing");
    assert_eq!(completed_c1(close)["state"], "locked");
}

#[cfg(all(feature = "encrypted-workspace", unix))]
fn control_preparation(
    view: &serde_json::Value,
    action: &str,
) -> crate::workspace_close::setup::Preparation {
    crate::workspace_close::setup::Preparation::parse(&InvokeBody::Raw(
        serde_json::to_vec(&json!({
            "schemaVersion": "rangoon.workspace-control-prepare.v1",
            "attemptId": view["attemptId"].as_str().unwrap(),
            "epoch": view["epoch"].as_str().unwrap(),
            "action": action,
        }))
        .unwrap(),
    ))
    .unwrap()
}

#[cfg(all(feature = "encrypted-workspace", unix))]
fn control_cycle(view: &serde_json::Value) -> crate::workspace_close::setup::Cycle {
    crate::workspace_close::setup::Cycle::parse(&InvokeBody::Raw(
        serde_json::to_vec(&json!({
            "schemaVersion": "rangoon.workspace-control-cycle.v1",
            "attemptId": view["attemptId"].as_str().unwrap(),
            "epoch": view["epoch"].as_str().unwrap(),
        }))
        .unwrap(),
    ))
    .unwrap()
}

#[cfg(all(feature = "encrypted-workspace", unix))]
fn confirmed_native_setup_action(close: &WorkspaceClose, action: &str) {
    let locked = view(close);
    let prepared = serde_json::to_value(crate::workspace_close::setup::prepare(
        close,
        control_preparation(&locked, action),
        |_| true,
    ))
    .unwrap();
    assert_eq!(prepared["outcome"], "prepared");
    let control = &prepared["close"];
    assert_eq!(
        serde_json::to_value(close.acknowledge(&acknowledgement(control), |_| true)).unwrap()["state"],
        "closing"
    );
    let opened = serde_json::to_value(crate::workspace_close::setup::confirm(
        close,
        control_cycle(control),
        |_| true,
        |_| crate::workspace_close::setup::Consent::Approved,
        || true,
    ))
    .unwrap();
    assert_eq!(opened["outcome"], "opened");
    assert_eq!(opened["reload"], true);
}

#[cfg(all(feature = "encrypted-workspace", unix))]
fn confirmed_create_empty(close: &WorkspaceClose) {
    confirmed_native_setup_action(close, "create_empty");
}

#[cfg(all(feature = "encrypted-workspace", unix))]
fn confirmed_unlock(close: &WorkspaceClose) {
    confirmed_native_setup_action(close, "unlock");
}

#[cfg(all(feature = "encrypted-workspace", unix))]
fn ordinary_lock(close: &WorkspaceClose, host: &crate::workspace_startup_host::Host) {
    let retired = host.retire_for_ordinary_close();
    assert!(retired.is_none());
    let closing = serde_json::to_value(close.begin(|_| true)).unwrap();
    assert_eq!(closing["state"], "closing");
    assert_eq!(
        serde_json::to_value(close.acknowledge(&acknowledgement(&closing), |_| true)).unwrap()["state"],
        "locked"
    );
}

#[cfg(all(feature = "encrypted-workspace", unix))]
fn status_with_current_c1(host: &crate::workspace_startup_host::Host) -> serde_json::Value {
    serde_json::to_value(host.status(
        |_| true,
        |current| {
            let closing = view(current);
            assert_eq!(closing["state"], "closing");
            let _ = current.acknowledge(&acknowledgement(&closing), |_| true);
            true
        },
    ))
    .unwrap()
}

#[test]
fn legacy_startup_presents_without_cleanup_or_admission_fence() {
    let root = TestRoot::new("legacy-success");
    let gate = Gate::new();
    let source = SourceAccess::legacy(gate.clone(), Ok(root.0.clone()));
    let close = WorkspaceClose::new(source.clone());
    let cleanup_calls = Arc::new(Mutex::new(0_usize));
    let present_calls = Arc::new(Mutex::new(0_usize));

    assert_eq!(
        prepare_workbench(
            &gate,
            &source,
            &close,
            false,
            {
                let cleanup_calls = Arc::clone(&cleanup_calls);
                move |_| {
                    *cleanup_calls.lock().unwrap() += 1;
                    true
                }
            },
            {
                let present_calls = Arc::clone(&present_calls);
                move || {
                    *present_calls.lock().unwrap() += 1;
                    Ok(())
                }
            },
        ),
        Ok(())
    );
    assert_eq!(*cleanup_calls.lock().unwrap(), 0);
    assert_eq!(*present_calls.lock().unwrap(), 1);
    assert!(gate.begin(Lane::Read).is_ok());
    assert!(!source.failure_latched());
}

#[test]
fn legacy_window_failure_fences_shared_gate_without_cleanup() {
    let root = TestRoot::new("legacy-window-failure");
    let gate = Gate::new();
    let source = SourceAccess::legacy(gate.clone(), Ok(root.0.clone()));
    let close = WorkspaceClose::new(source.clone());
    let cleanup_calls = Arc::new(Mutex::new(0_usize));

    assert_eq!(
        prepare_workbench(
            &gate,
            &source,
            &close,
            false,
            {
                let cleanup_calls = Arc::clone(&cleanup_calls);
                move |_| {
                    *cleanup_calls.lock().unwrap() += 1;
                    true
                }
            },
            || Err(()),
        ),
        Err(StartupFailure::WindowCreation)
    );
    assert_eq!(*cleanup_calls.lock().unwrap(), 0);
    assert!(gate.begin(Lane::Read).is_err());
    assert!(!source.failure_latched());
}

#[cfg(feature = "encrypted-workspace")]
#[derive(Default)]
struct CountingVaultState {
    slots: std::collections::BTreeMap<String, Option<Vec<u8>>>,
    opens: usize,
    reads: usize,
    writes: usize,
}

#[cfg(feature = "encrypted-workspace")]
#[derive(Clone, Default)]
struct CountingVault(Arc<Mutex<CountingVaultState>>);

#[cfg(feature = "encrypted-workspace")]
struct CountingSlot {
    identity: WorkspaceId,
    lookup: String,
    state: Arc<Mutex<CountingVaultState>>,
}

#[cfg(feature = "encrypted-workspace")]
impl CountingVault {
    fn counts(&self) -> (usize, usize, usize) {
        let state = self.0.lock().unwrap();
        (state.opens, state.reads, state.writes)
    }
}

#[cfg(feature = "encrypted-workspace")]
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

#[cfg(feature = "encrypted-workspace")]
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

#[cfg(all(feature = "encrypted-workspace", unix))]
fn managed_locked(root: &TestRoot, gate: Gate, vault: &CountingVault) -> SourceAccess {
    SourceAccess::managed_locked(gate, root.0.clone(), vault.clone()).unwrap()
}

#[cfg(feature = "encrypted-workspace")]
fn assert_no_client(access: &SourceAccess) {
    assert_eq!(
        serde_json::to_value(access.request_view()).unwrap()["state"],
        "unavailable"
    );
    assert!(matches!(
        access.request_context("main", Some(&"a".repeat(64))),
        WorkspaceRequest::Refused
    ));
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn managed_startup_closes_before_present_and_exact_ack_locks_without_client() {
    let root = TestRoot::new("managed-order");
    let gate = Gate::new();
    let vault = CountingVault::default();
    let source = managed_locked(&root, gate.clone(), &vault);
    let close = WorkspaceClose::new(source.clone());
    let events = Arc::new(Mutex::new(Vec::new()));

    assert_eq!(
        prepare_workbench(
            &gate,
            &source,
            &close,
            true,
            {
                let events = Arc::clone(&events);
                move |drained| {
                    events.lock().unwrap().push(format!("cleanup:{drained}"));
                    true
                }
            },
            {
                let events = Arc::clone(&events);
                move || {
                    events.lock().unwrap().push("present".into());
                    Ok(())
                }
            },
        ),
        Ok(())
    );
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_no_client(&source);
    assert_eq!(
        &*events.lock().unwrap(),
        &[String::from("cleanup:false"), String::from("present")]
    );

    let closing = view(&close);
    assert_eq!(closing["state"], "closing");
    assert_no_client(&source);
    let locked =
        serde_json::to_value(close.acknowledge(&acknowledgement(&closing), |_| true)).unwrap();
    assert_eq!(locked["state"], "locked");
    assert_no_client(&source);
    assert_eq!(vault.counts(), (0, 0, 0));
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn managed_missing_anchor_refuses_before_cleanup_or_window() {
    let root = TestRoot::new("managed-anchor");
    let gate = Gate::new();
    let vault = CountingVault::default();
    let source = managed_locked(&root, gate.clone(), &vault);
    let close = WorkspaceClose::new(source.clone());
    let cleanup_calls = Arc::new(Mutex::new(0_usize));
    let present_calls = Arc::new(Mutex::new(0_usize));

    assert_eq!(
        prepare_workbench(
            &gate,
            &source,
            &close,
            false,
            {
                let cleanup_calls = Arc::clone(&cleanup_calls);
                move |_| {
                    *cleanup_calls.lock().unwrap() += 1;
                    true
                }
            },
            {
                let present_calls = Arc::clone(&present_calls);
                move || {
                    *present_calls.lock().unwrap() += 1;
                    Ok(())
                }
            },
        ),
        Err(StartupFailure::MissingAnchor)
    );
    assert_eq!(*cleanup_calls.lock().unwrap(), 0);
    assert_eq!(*present_calls.lock().unwrap(), 0);
    assert!(source.failure_latched());
    assert_no_client(&source);
    assert!(gate.begin(Lane::Read).is_err());
    assert_eq!(vault.counts(), (0, 0, 0));
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn managed_failed_cleanup_or_window_never_claims_clean_startup() {
    for panic_cleanup in [false, true] {
        let root = TestRoot::new(if panic_cleanup {
            "managed-cleanup-panic"
        } else {
            "managed-cleanup-false"
        });
        let gate = Gate::new();
        let vault = CountingVault::default();
        let source = managed_locked(&root, gate.clone(), &vault);
        let close = WorkspaceClose::new(source.clone());
        let present_calls = Arc::new(Mutex::new(0_usize));

        let result = if panic_cleanup {
            prepare_workbench(&gate, &source, &close, true, |_| panic!("cleanup panic"), {
                let present_calls = Arc::clone(&present_calls);
                move || {
                    *present_calls.lock().unwrap() += 1;
                    Ok(())
                }
            })
        } else {
            prepare_workbench(&gate, &source, &close, true, |_| false, {
                let present_calls = Arc::clone(&present_calls);
                move || {
                    *present_calls.lock().unwrap() += 1;
                    Ok(())
                }
            })
        };
        assert_eq!(result, Err(StartupFailure::InitialClose));
        assert_eq!(*present_calls.lock().unwrap(), 0);
        assert!(source.failure_latched());
        assert!(!close.clean_native_completion());
        let _ = close.progress_native(|_| true);
        assert!(!close.clean_native_completion());
        assert_no_client(&source);
        assert!(gate.begin(Lane::Read).is_err());
        assert_eq!(vault.counts(), (0, 0, 0));
    }

    let root = TestRoot::new("managed-window-failure");
    let gate = Gate::new();
    let vault = CountingVault::default();
    let source = managed_locked(&root, gate.clone(), &vault);
    let close = WorkspaceClose::new(source.clone());
    let cleanup_calls = Arc::new(Mutex::new(0_usize));

    assert_eq!(
        prepare_workbench(
            &gate,
            &source,
            &close,
            true,
            {
                let cleanup_calls = Arc::clone(&cleanup_calls);
                move |_| {
                    *cleanup_calls.lock().unwrap() += 1;
                    true
                }
            },
            || Err(()),
        ),
        Err(StartupFailure::WindowCreation)
    );
    assert_eq!(*cleanup_calls.lock().unwrap(), 1);
    assert!(source.failure_latched());
    assert!(!close.clean_native_completion());
    let _ = close.progress_native(|_| true);
    assert!(!close.clean_native_completion());
    assert_no_client(&source);
    assert!(gate.begin(Lane::Read).is_err());
    assert_eq!(vault.counts(), (0, 0, 0));
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn managed_duplicate_or_exit_latched_startup_refuses() {
    let root = TestRoot::new("managed-duplicate");
    let gate = Gate::new();
    let vault = CountingVault::default();
    let source = managed_locked(&root, gate.clone(), &vault);
    let close = WorkspaceClose::new(source.clone());
    let present_calls = Arc::new(Mutex::new(0_usize));
    assert_eq!(
        prepare_workbench(&gate, &source, &close, true, |_| true, {
            let present_calls = Arc::clone(&present_calls);
            move || {
                *present_calls.lock().unwrap() += 1;
                Ok(())
            }
        }),
        Ok(())
    );
    assert_eq!(
        prepare_workbench(&gate, &source, &close, true, |_| true, {
            let present_calls = Arc::clone(&present_calls);
            move || {
                *present_calls.lock().unwrap() += 1;
                Ok(())
            }
        }),
        Err(StartupFailure::InitialClose)
    );
    assert_eq!(*present_calls.lock().unwrap(), 1);
    assert!(source.failure_latched());
    assert_no_client(&source);
    assert!(gate.begin(Lane::Read).is_err());
    assert_eq!(vault.counts(), (0, 0, 0));

    let root = TestRoot::new("managed-exit-latched");
    let gate = Gate::new();
    let vault = CountingVault::default();
    let source = managed_locked(&root, gate.clone(), &vault);
    let close = WorkspaceClose::new(source.clone());
    close.fence_admission_for_exit();
    let present_calls = Arc::new(Mutex::new(0_usize));
    assert_eq!(
        prepare_workbench(&gate, &source, &close, true, |_| true, {
            let present_calls = Arc::clone(&present_calls);
            move || {
                *present_calls.lock().unwrap() += 1;
                Ok(())
            }
        },),
        Err(StartupFailure::InitialClose)
    );
    assert_eq!(*present_calls.lock().unwrap(), 0);
    assert!(source.failure_latched());
    assert_no_client(&source);
    assert!(gate.begin(Lane::Read).is_err());
    assert_eq!(vault.counts(), (0, 0, 0));
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn deferred_factory_has_no_metadata_key_or_client_before_a_closed_cycle() {
    let root = TestRoot::new("deferred-factory");
    let gate = Gate::new();
    let vault = CountingVault::default();
    let managed = root.0.join("managed-workspace");

    let (source, close, _host) = startup_state(&gate, root.0.clone(), vault.clone());

    assert!(!managed.exists());
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_no_client(&source);
    assert_eq!(view(&close)["state"], "open");
    assert!(!close.clean_native_completion());
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn startup_status_requires_completed_c0_and_current_c1_without_side_effects() {
    let root = TestRoot::new("startup-c0-c1");
    let gate = Gate::new();
    let vault = CountingVault::default();
    let managed = root.0.join("managed-workspace");
    let (source, close, host) = startup_state(&gate, root.0.clone(), vault.clone());

    let c0 = serde_json::to_value(close.begin(|_| true)).unwrap();
    assert_eq!(c0["state"], "closing");
    let refused = serde_json::to_value(host.status(|_| true, |_| true)).unwrap();
    assert_eq!(refused["state"], "unavailable");
    assert!(!managed.exists());
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_no_client(&source);

    assert_eq!(completed_c1(&close)["state"], "locked");
    let fresh = status_with_current_c1(&host);
    assert_eq!(fresh["state"], "fresh");
    assert_eq!(fresh["retainedOriginal"], "not_applicable");
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_no_client(&source);
    assert_eq!(view(&close)["state"], "closing");
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn fresh_setup_binds_only_locked_owner_then_hands_off_without_key_or_client() {
    let root = TestRoot::new("fresh-setup");
    let gate = Gate::new();
    let vault = CountingVault::default();
    let (source, close, host) = startup_state(&gate, root.0.clone(), vault.clone());

    closed_startup_cycle(&close);
    let fresh = status_with_current_c1(&host);
    assert_eq!(fresh["state"], "fresh");
    let handoff =
        serde_json::to_value(host.action(action_request(&fresh, "setup"), &vault, |_| {
            crate::workspace_close::setup::Consent::Approved
        }))
        .unwrap();

    assert_eq!(handoff["state"], "handoff");
    assert!(handoff["startupId"].is_null());
    assert!(handoff["attemptId"].is_null());
    assert!(handoff["epoch"].is_null());
    assert_eq!(handoff["actions"], json!([]));
    assert!(
        root.0
            .join("managed-workspace/encrypted-workspace.lock")
            .is_file()
    );
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_no_client(&source);
    assert_eq!(view(&close)["state"], "locked");
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn bound_existing_owner_rejects_wrong_selected_identity_before_vault_or_client_access() {
    use crate::{workspace_source_access::ControlKind, workspace_startup::StartupRoots};

    let root = TestRoot::new("selected-identity");
    let managed = root.0.join("managed-workspace");
    fs::create_dir(&managed).unwrap();
    let fixture_vault = CountingVault::default();
    let mut owner = Owner::acquire(managed).unwrap();
    let preview = owner.prepare_create_empty().unwrap();
    owner.confirm(&preview.id, &fixture_vault).unwrap();
    owner.lock();
    drop(owner);
    let gate = Gate::new();
    let vault = CountingVault::default();
    let (source, close, _host) = startup_state(&gate, root.0.clone(), vault.clone());

    closed_startup_cycle(&close);
    assert!(close.startup_acknowledged().is_err());
    let locked = view(&close);
    let mut flight = close
        .start_control_for(
            locked["attemptId"].as_str().unwrap(),
            locked["epoch"].as_str().unwrap(),
            ControlKind::Startup,
            |_| true,
        )
        .unwrap();
    assert_eq!(close.startup_acknowledged(), Ok(false));
    assert_eq!(completed_c1(&close)["state"], "closing");
    assert_eq!(close.startup_acknowledged(), Ok(true));
    flight.startup_permit().unwrap();
    flight
        .bind_locked_owner(
            StartupRoots::new(root.0.clone())
                .existing_binding()
                .unwrap(),
        )
        .unwrap();
    assert!(flight.check_bound_identity(&"f".repeat(64)).is_err());
    assert!(flight.startup_permit().is_err());
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_no_client(&source);
    assert!(
        close
            .start_control_for(
                locked["attemptId"].as_str().unwrap(),
                locked["epoch"].as_str().unwrap(),
                ControlKind::CreateEmpty,
                |_| true,
            )
            .is_err()
    );
    close.fence_admission_for_exit();
    assert!(close.startup_acknowledged().is_err());
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn legacy_review_stale_copy_cancel_and_verified_adoption_keep_native_custody_across_locks() {
    let root = TestRoot::new("legacy-router");
    let legacy = Workspace::new(root.0.join("source-workspace"));
    legacy
        .save(&rangoon_import::analyze("AGENTS.md", b"# Existing\nExact bytes\n").unwrap())
        .unwrap();
    let original = fs::read(root.0.join("source-workspace/workspace.sqlite3")).unwrap();
    let gate = Gate::new();
    let vault = CountingVault::default();
    let (source, close, host) = startup_state(&gate, root.0.clone(), vault.clone());

    closed_startup_cycle(&close);
    let legacy_found = status_with_current_c1(&host);
    assert_eq!(legacy_found["state"], "legacy_found");
    assert_eq!(legacy_found["retainedOriginal"], "unassessed");
    let review = serde_json::to_value(host.action(
        action_request(&legacy_found, "review_conversion"),
        &vault,
        |_| crate::workspace_close::setup::Consent::Approved,
    ))
    .unwrap();
    assert_eq!(review["state"], "conversion_review");
    assert_eq!(review["retainedOriginal"], "present_unencrypted");
    assert_ne!(review["startupId"], legacy_found["startupId"]);

    let stale = serde_json::to_value(host.action(
        action_request(&legacy_found, "copy"),
        &vault,
        |_| crate::workspace_close::setup::Consent::Approved,
    ))
    .unwrap();
    assert_eq!(stale["state"], "conversion_review");
    assert!(!root.0.join("managed-workspace").exists());
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_no_client(&source);

    let cancelled =
        serde_json::to_value(host.action(action_request(&review, "cancel"), &vault, |_| {
            crate::workspace_close::setup::Consent::Approved
        }))
        .unwrap();
    assert_eq!(cancelled["state"], "legacy_found");
    assert_ne!(cancelled["startupId"], review["startupId"]);
    assert!(!root.0.join("managed-workspace").exists());
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_eq!(
        fs::read(root.0.join("source-workspace/workspace.sqlite3")).unwrap(),
        original
    );

    let reviewed_again = serde_json::to_value(host.action(
        action_request(&cancelled, "review_conversion"),
        &vault,
        |_| crate::workspace_close::setup::Consent::Approved,
    ))
    .unwrap();
    let restarted = serde_json::to_value(host.action(
        action_request(&reviewed_again, "copy"),
        &vault,
        |_| crate::workspace_close::setup::Consent::Approved,
    ))
    .unwrap();
    assert_eq!(restarted["state"], "restart_required");
    assert_eq!(restarted["retainedOriginal"], "present_unencrypted");
    assert_eq!(
        fs::read(root.0.join("source-workspace/workspace.sqlite3")).unwrap(),
        original
    );
    assert!(root.0.join("managed-workspace").exists());
    assert!(vault.counts().0 > 0);
    assert_no_client(&source);
    assert_eq!(view(&close)["state"], "locked");

    let (slots, writes) = {
        let state = vault.0.lock().unwrap();
        let slots: std::collections::BTreeMap<_, _> = state
            .slots
            .iter()
            .map(|(identity, envelope)| {
                assert!(WorkspaceId::parse(identity).is_ok());
                let envelope = envelope.as_ref().expect("copy stores one fixture key");
                assert!(!envelope.is_empty());
                (identity.clone(), envelope.clone())
            })
            .collect();
        assert_eq!(slots.len(), 1);
        (serde_json::to_vec(&slots).unwrap(), state.writes)
    };
    assert!(slots.len() <= 4096);
    drop(host);
    drop(close);
    drop(source);
    let mut child = ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "workspace_native_bootstrap::tests::legacy_adoption_process_child",
                "--ignored",
                "--nocapture",
            ])
            .env("RANGOON_E1C5E_TEST_ROOT", &root.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    child.0.stdin.take().unwrap().write_all(&slots).unwrap();
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() >= Duration::from_secs(20) {
            let _ = child.0.kill();
            let _ = child.0.wait();
            panic!("legacy adoption child timeout");
        }
        thread::sleep(Duration::from_millis(5));
    };
    let mut diagnostics = String::new();
    child
        .0
        .stderr
        .take()
        .unwrap()
        .take(8192)
        .read_to_string(&mut diagnostics)
        .unwrap();
    assert!(
        status.success(),
        "legacy adoption child failed: {diagnostics}"
    );
    assert_eq!(vault.counts().2, writes);
    assert_eq!(
        fs::read(root.0.join("source-workspace/workspace.sqlite3")).unwrap(),
        original
    );
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn close_begin_cancels_ready_startup_then_retired_plan_keeps_real_drain_until_drop() {
    let root = TestRoot::new("retired-plan");
    let gate = Gate::new();
    let vault = CountingVault::default();
    let (source, close, host) = startup_state(&gate, root.0.clone(), vault.clone());

    closed_startup_cycle(&close);
    let fresh = status_with_current_c1(&host);
    assert_eq!(fresh["state"], "fresh");
    let retired = host.retire_for_ordinary_close().unwrap();
    assert_eq!(
        serde_json::to_value(close.begin(|_| true)).unwrap()["state"],
        "closing"
    );
    assert_eq!(view(&close)["state"], "closing");
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_no_client(&source);
    assert_eq!(
        serde_json::to_value(host.action(action_request(&fresh, "setup"), &vault, |_| {
            crate::workspace_close::setup::Consent::Approved
        }))
        .unwrap()["state"],
        "unavailable"
    );
    drop(retired);
    assert_eq!(
        serde_json::to_value(close.inspect(|_| true)).unwrap()["state"],
        "closing"
    );
    assert_eq!(completed_c1(&close)["state"], "locked");
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn fenced_native_copy_cannot_create_destination_or_key_after_blocked_confirmation_returns() {
    let root = TestRoot::new("blocked-native-copy");
    Workspace::new(root.0.join("source-workspace"))
        .save(&rangoon_import::analyze("AGENTS.md", b"# Existing\nExact bytes\n").unwrap())
        .unwrap();
    let gate = Gate::new();
    let vault = CountingVault::default();
    let (source, close, host) = startup_state(&gate, root.0.clone(), vault.clone());
    closed_startup_cycle(&close);
    let legacy = status_with_current_c1(&host);
    let review = serde_json::to_value(host.action(
        action_request(&legacy, "review_conversion"),
        &vault,
        |_| crate::workspace_close::setup::Consent::Approved,
    ))
    .unwrap();
    assert_eq!(review["state"], "conversion_review");

    let copy = action_request(&review, "copy");
    let duplicate = action_request(&review, "copy");
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (returned_tx, returned_rx) = mpsc::channel();
    let worker_host = host.clone();
    let worker_vault = vault.clone();
    let worker = thread::spawn(move || {
        let result = worker_host.action(copy, &worker_vault, move |_| {
            entered_tx.send(()).unwrap();
            if release_rx.recv_timeout(Duration::from_secs(5)).is_ok() {
                crate::workspace_close::setup::Consent::Approved
            } else {
                crate::workspace_close::setup::Consent::Unavailable
            }
        });
        returned_tx.send(result).unwrap();
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();

    close.fence_admission_for_exit();
    assert!(host.retire().is_none());
    assert_eq!(
        serde_json::to_value(host.status(|_| true, |_| true)).unwrap()["state"],
        "unavailable"
    );
    assert_eq!(
        serde_json::to_value(host.action(duplicate, &vault, |_| {
            crate::workspace_close::setup::Consent::Approved
        }))
        .unwrap()["state"],
        "unavailable"
    );
    assert!(!close.clean_native_completion());
    assert!(!host.cleanup_consistent(true));
    assert!(matches!(
        returned_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert!(!root.0.join("managed-workspace").exists());
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_no_client(&source);

    release_tx.send(()).unwrap();
    let result = returned_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    worker.join().unwrap();
    assert_eq!(
        serde_json::to_value(result).unwrap()["state"],
        "unavailable"
    );
    assert!(!root.0.join("managed-workspace").exists());
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_no_client(&source);
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn fatal_source_failure_retires_ready_startup_without_resumable_client() {
    let root = TestRoot::new("fatal-ready-startup");
    let gate = Gate::new();
    let vault = CountingVault::default();
    let (source, close, host) = startup_state(&gate, root.0.clone(), vault.clone());

    closed_startup_cycle(&close);
    assert_eq!(status_with_current_c1(&host)["state"], "fresh");
    source.fail_closed();
    let retired = host.retire().unwrap();
    assert!(source.failure_latched());
    assert!(!close.clean_native_completion());
    assert_no_client(&source);
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_eq!(
        serde_json::to_value(host.status(|_| true, |_| true)).unwrap()["state"],
        "unavailable"
    );

    drop(retired);
    assert!(source.failure_latched());
    assert_no_client(&source);
    assert!(!close.clean_native_completion());
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn ordinary_lock_preserves_only_completed_handoff_across_confirmed_unlock_cycles() {
    let root = TestRoot::new("ordinary-handoff");
    let gate = Gate::new();
    let vault = CountingVault::default();
    let (source, close, host) = startup_state(&gate, root.0.clone(), vault.clone());

    closed_startup_cycle(&close);
    let fresh = status_with_current_c1(&host);
    let handoff =
        serde_json::to_value(host.action(action_request(&fresh, "setup"), &vault, |_| {
            crate::workspace_close::setup::Consent::Approved
        }))
        .unwrap();
    assert_eq!(handoff["state"], "handoff");
    assert!(handoff["startupId"].is_null());
    assert!(handoff["attemptId"].is_null());
    assert!(handoff["epoch"].is_null());

    confirmed_create_empty(&close);
    assert!(serde_json::to_value(source.request_view()).unwrap()["clientId"].is_string());

    for cycle in 0..2 {
        ordinary_lock(&close, &host);
        assert_no_client(&source);
        let before_unlock = vault.counts();
        confirmed_unlock(&close);
        assert_eq!(vault.counts().2, before_unlock.2);
        assert!(serde_json::to_value(source.request_view()).unwrap()["clientId"].is_string());

        let retained = serde_json::to_value(host.status(
            |_| true,
            |_| panic!("completed handoff must not create a startup cycle"),
        ))
        .unwrap();
        assert_eq!(retained["state"], "handoff", "cycle {cycle}");
        assert!(retained["startupId"].is_null());
        assert!(retained["attemptId"].is_null());
        assert!(retained["epoch"].is_null());
        assert_eq!(retained["actions"], json!([]));
    }
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn ordinary_lock_retires_busy_copy_before_result_or_custody_can_revive() {
    let root = TestRoot::new("ordinary-busy-copy");
    Workspace::new(root.0.join("source-workspace"))
        .save(&rangoon_import::analyze("AGENTS.md", b"# Existing\nExact bytes\n").unwrap())
        .unwrap();
    let gate = Gate::new();
    let vault = CountingVault::default();
    let (source, close, host) = startup_state(&gate, root.0.clone(), vault.clone());
    closed_startup_cycle(&close);
    let legacy = status_with_current_c1(&host);
    let review = serde_json::to_value(host.action(
        action_request(&legacy, "review_conversion"),
        &vault,
        |_| crate::workspace_close::setup::Consent::Approved,
    ))
    .unwrap();
    assert_eq!(review["state"], "conversion_review");

    let copy = action_request(&review, "copy");
    let stale = action_request(&review, "copy");
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (returned_tx, returned_rx) = mpsc::channel();
    let worker_host = host.clone();
    let worker_vault = vault.clone();
    let worker = thread::spawn(move || {
        let result = worker_host.action(copy, &worker_vault, move |_| {
            entered_tx.send(()).unwrap();
            if release_rx.recv_timeout(Duration::from_secs(5)).is_ok() {
                crate::workspace_close::setup::Consent::Approved
            } else {
                crate::workspace_close::setup::Consent::Unavailable
            }
        });
        returned_tx.send(result).unwrap();
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();

    assert!(host.retire_for_ordinary_close().is_none());
    let closing = serde_json::to_value(close.begin(|_| true)).unwrap();
    assert_eq!(closing["state"], "closing");
    assert_eq!(
        serde_json::to_value(close.acknowledge(&acknowledgement(&closing), |_| true)).unwrap()["state"],
        "closing"
    );
    assert_eq!(
        serde_json::to_value(host.action(stale, &vault, |_| {
            crate::workspace_close::setup::Consent::Approved
        }))
        .unwrap()["state"],
        "unavailable"
    );
    assert!(!close.clean_native_completion());
    assert!(!host.cleanup_consistent(true));
    assert!(matches!(
        returned_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert!(!root.0.join("managed-workspace").exists());
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_no_client(&source);

    release_tx.send(()).unwrap();
    let result = returned_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    worker.join().unwrap();
    assert_eq!(
        serde_json::to_value(result).unwrap()["state"],
        "unavailable"
    );
    assert!(!root.0.join("managed-workspace").exists());
    assert_eq!(vault.counts(), (0, 0, 0));
    assert_no_client(&source);
    assert!(host.cleanup_consistent(true));
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn terminal_exit_overrides_handoff_and_repeated_ordinary_lock_cannot_revive_it() {
    let root = TestRoot::new("terminal-handoff");
    let gate = Gate::new();
    let vault = CountingVault::default();
    let (source, close, host) = startup_state(&gate, root.0.clone(), vault.clone());

    closed_startup_cycle(&close);
    let fresh = status_with_current_c1(&host);
    assert_eq!(
        serde_json::to_value(host.action(action_request(&fresh, "setup"), &vault, |_| {
            crate::workspace_close::setup::Consent::Approved
        },))
        .unwrap()["state"],
        "handoff"
    );

    close.fence_admission_for_exit();
    assert!(host.retire().is_none());
    for _ in 0..2 {
        assert!(host.retire_for_ordinary_close().is_none());
        assert_eq!(
            serde_json::to_value(host.status(|_| true, |_| true)).unwrap()["state"],
            "unavailable"
        );
        assert_no_client(&source);
    }
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
#[ignore = "Disposable process child; only the Legacy fixture supplies its root and fake vault"]
fn legacy_adoption_process_child() {
    let Some(root) = std::env::var_os("RANGOON_E1C5E_TEST_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let temporary = std::env::temp_dir().canonicalize().unwrap();
    assert!(root.starts_with(&temporary));
    assert_eq!(root, root.canonicalize().unwrap());
    assert!(
        root.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("rangoon-native-bootstrap-legacy-router-"))
    );

    let mut encoded_slots = Vec::new();
    std::io::stdin()
        .take(4097)
        .read_to_end(&mut encoded_slots)
        .unwrap();
    assert!(encoded_slots.len() <= 4096);
    let mut slots: std::collections::BTreeMap<String, Vec<u8>> =
        serde_json::from_slice(&encoded_slots).unwrap();
    assert_eq!(slots.len(), 1);
    let (identity, envelope) = slots.pop_first().unwrap();
    assert!(WorkspaceId::parse(&identity).is_ok());
    assert!(!envelope.is_empty());
    let vault = CountingVault::default();
    vault
        .0
        .lock()
        .unwrap()
        .slots
        .insert(identity, Some(envelope));

    let original = fs::read(root.join("source-workspace/workspace.sqlite3")).unwrap();
    let gate = Gate::new();
    let (source, close, host) = startup_state(&gate, root.clone(), vault.clone());
    closed_startup_cycle(&close);
    let verify = status_with_current_c1(&host);
    assert_eq!(verify["state"], "verify_required");
    assert_eq!(verify["retainedOriginal"], "present_unencrypted");
    let handoff =
        serde_json::to_value(host.action(action_request(&verify, "verify"), &vault, |_| {
            crate::workspace_close::setup::Consent::Approved
        }))
        .unwrap();
    assert_eq!(handoff["state"], "handoff");
    assert_eq!(handoff["retainedOriginal"], "present_unencrypted");
    assert!(handoff["startupId"].is_null());
    assert!(handoff["attemptId"].is_null());
    assert!(handoff["epoch"].is_null());
    assert_no_client(&source);
    let writes_after_verify = vault.counts().2;

    confirmed_unlock(&close);
    let mut previous_client = serde_json::to_value(source.request_view()).unwrap()["clientId"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(vault.counts().2, writes_after_verify);
    for cycle in 0..2 {
        ordinary_lock(&close, &host);
        assert_no_client(&source);
        assert_eq!(vault.counts().2, writes_after_verify);

        confirmed_unlock(&close);
        let client = serde_json::to_value(source.request_view()).unwrap()["clientId"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_ne!(client, previous_client, "cycle {cycle}");
        previous_client = client;
        assert_eq!(vault.counts().2, writes_after_verify);

        let retained = serde_json::to_value(host.status(
            |_| true,
            |_| panic!("verified handoff must not create a startup cycle"),
        ))
        .unwrap();
        assert_eq!(retained["state"], "handoff", "cycle {cycle}");
        assert_eq!(retained["retainedOriginal"], "present_unencrypted");
        assert!(retained["startupId"].is_null());
        assert!(retained["attemptId"].is_null());
        assert!(retained["epoch"].is_null());
        assert_eq!(retained["actions"], json!([]));
    }
    assert_eq!(
        fs::read(root.join("source-workspace/workspace.sqlite3")).unwrap(),
        original
    );
}
