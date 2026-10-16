#[cfg(feature = "encrypted-workspace")]
use super::*;

#[cfg(feature = "encrypted-workspace")]
use crate::workspace_close::{
    WorkspaceClose,
    setup::{Preparation, ProtectionResponse, prepare},
};
#[cfg(feature = "encrypted-workspace")]
use crate::workspace_source_access::ControlKind;
#[cfg(feature = "encrypted-workspace")]
use rangoon_desktop::workspace_session::Owner;
#[cfg(feature = "encrypted-workspace")]
use serde_json::json;
#[cfg(feature = "encrypted-workspace")]
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[cfg(feature = "encrypted-workspace")]
use tauri::ipc::InvokeBody;

#[cfg(feature = "encrypted-workspace")]
fn protection_json(response: ProtectionResponse) -> serde_json::Value {
    serde_json::to_value(response).unwrap()
}

#[cfg(feature = "encrypted-workspace")]
fn view_json(view: crate::workspace_close::View) -> serde_json::Value {
    serde_json::to_value(view).unwrap()
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
fn preparation(id: &str, epoch: &str, action: &str) -> Preparation {
    Preparation::parse(&InvokeBody::Raw(
        serde_json::to_vec(&json!({
            "schemaVersion": "rangoon.workspace-control-prepare.v1",
            "attemptId": id,
            "epoch": epoch,
            "action": action,
        }))
        .unwrap(),
    ))
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
fn closed_c0(access: SourceAccess) -> (WorkspaceClose, String, String) {
    let close = WorkspaceClose::new(access);
    let started = view_json(close.begin(|_| true));
    assert_eq!(started["state"], "closing");
    let (id, epoch) = identity(&started);
    let locked = view_json(close.acknowledge(&acknowledgement(&id, &epoch), |_| true));
    assert_eq!(locked["state"], "locked");
    (close, id, epoch)
}

#[cfg(feature = "encrypted-workspace")]
fn make_existing(root: &TestRoot, vault: &ThreadVault) {
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let preview = owner.prepare_create_empty().unwrap();
    owner.confirm(&preview.id, vault).unwrap();
    owner.lock();
}

#[cfg(feature = "encrypted-workspace")]
fn assert_source_free(value: &serde_json::Value) {
    assert_eq!(value.as_object().unwrap().len(), 4);
    assert_eq!(
        value["schemaVersion"],
        "rangoon.workspace-protection-state.v1"
    );
    let rendered = value.to_string();
    for forbidden in [
        "workspaceId",
        "sourceId",
        "preview",
        "clientId",
        "key",
        "sqlite",
    ] {
        assert!(!rendered.contains(forbidden));
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn protection_legacy_is_pure_and_returns_a_finite_source_free_dto() {
    let (access, state, _) = probe(report("protection-legacy.md", b"legacy bytes"));
    let close = WorkspaceClose::new(access);
    let response = protection_json(ProtectionResponse::observe(&close));
    assert_source_free(&response);
    assert_eq!(response["layout"], "legacy");
    assert_eq!(response["diagnostic"], "encrypted_protection_unavailable");
    assert_eq!(state.list_calls.load(Ordering::SeqCst), 0);
    assert_eq!(state.open_calls.load(Ordering::SeqCst), 0);
    assert_eq!(state.save_calls.load(Ordering::SeqCst), 0);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn protection_reads_absent_and_existing_locked_layouts_through_real_closed_c0() {
    let root = TestRoot::new("protection-absent");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let (close, id, epoch) = closed_c0(access);
    let opens = vault.0.lock().unwrap().opens;
    let absent = protection_json(ProtectionResponse::observe(&close));
    assert_source_free(&absent);
    assert_eq!(absent["layout"], "absent");
    assert_eq!(absent["diagnostic"], serde_json::Value::Null);
    assert_eq!(vault.0.lock().unwrap().opens, opens);
    let prepared = prepare(&close, preparation(&id, &epoch, "create_empty"), |_| true);
    assert_eq!(
        serde_json::to_value(prepared).unwrap()["outcome"],
        "prepared"
    );
    assert_eq!(vault.0.lock().unwrap().opens, opens);

    let root = TestRoot::new("protection-locked");
    let vault = ThreadVault::default();
    make_existing(&root, &vault);
    let access = locked_access(&root, &vault);
    let (close, _, _) = closed_c0(access);
    let opens = vault.0.lock().unwrap().opens;
    let locked = protection_json(ProtectionResponse::observe(&close));
    assert_source_free(&locked);
    assert_eq!(locked["layout"], "locked");
    assert_eq!(locked["diagnostic"], serde_json::Value::Null);
    assert_eq!(vault.0.lock().unwrap().opens, opens);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn protection_unacknowledged_active_and_failed_cycles_never_start_metadata_read() {
    let calls = Arc::new(AtomicUsize::new(0));

    let root = TestRoot::new("protection-unacknowledged");
    let vault = ThreadVault::default();
    let close = WorkspaceClose::new(locked_access(&root, &vault));
    let _ = close.begin(|_| true);
    let after = Arc::clone(&calls);
    let response = protection_json(ProtectionResponse::observe_after_for_test(
        &close,
        move || {
            after.fetch_add(1, Ordering::SeqCst);
        },
    ));
    assert_eq!(response["layout"], "busy");
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let root = TestRoot::new("protection-active-control");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let (close, id, epoch) = closed_c0(access);
    let _flight = close
        .start_control(ControlKind::CreateEmpty, |_| true)
        .unwrap();
    let after = Arc::clone(&calls);
    let response = protection_json(ProtectionResponse::observe_after_for_test(
        &close,
        move || {
            after.fetch_add(1, Ordering::SeqCst);
        },
    ));
    assert_eq!(response["layout"], "busy");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    drop(_flight);
    let _ = (id, epoch);

    let root = TestRoot::new("protection-failed-control");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let (close, id, epoch) = closed_c0(access);
    close.fail_source_swap_for_test();
    let _ = prepare(&close, preparation(&id, &epoch, "create_empty"), |_| true);
    let after = Arc::clone(&calls);
    let response = protection_json(ProtectionResponse::observe_after_for_test(
        &close,
        move || {
            after.fetch_add(1, Ordering::SeqCst);
        },
    ));
    assert_eq!(response["layout"], "recovery_required");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn protection_discards_layout_when_exact_c0_becomes_c1_during_observation() {
    let root = TestRoot::new("protection-stale-c0");
    let vault = ThreadVault::default();
    let access = locked_access(&root, &vault);
    let (close, id, epoch) = closed_c0(access);
    let response = protection_json(ProtectionResponse::observe_after_for_test(&close, || {
        let prepared = prepare(&close, preparation(&id, &epoch, "create_empty"), |_| true);
        assert_eq!(
            serde_json::to_value(prepared).unwrap()["outcome"],
            "prepared"
        );
    }));
    assert_source_free(&response);
    assert_eq!(response["layout"], "busy");
    assert_eq!(response["diagnostic"], "workspace_protection_busy");
    assert_eq!(vault.0.lock().unwrap().opens, 0);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn protection_command_and_permission_wiring_stay_local() {
    let main = include_str!("main.rs");
    let build = include_str!("../build.rs");
    let permissions: serde_json::Value =
        serde_json::from_str(include_str!("../capabilities/source-analysis.json")).unwrap();
    assert!(main.contains("get_workspace_protection_state"));
    assert!(build.contains("\"get_workspace_protection_state\""));
    assert!(
        permissions["permissions"]
            .as_array()
            .unwrap()
            .contains(&json!("allow-get-workspace-protection-state"))
    );
    assert_eq!(permissions["windows"], json!(["main"]));
    assert_eq!(permissions["local"], true);
}
