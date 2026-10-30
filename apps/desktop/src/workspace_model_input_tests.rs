use super::*;

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

fn local_profile() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "schemaVersion": "rangoon.local-profile-request.v1",
        "profileId": "fixture.local",
        "host": "127.0.0.1",
        "port": 11434,
        "model": "fixture:v1",
        "maxOutputTokens": 256,
    }))
    .unwrap()
}

fn cloud_profile() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "schemaVersion": "rangoon.cloud-profile-request.v1",
        "profileId": "fixture.cloud",
        "model": "fixture-v1",
        "maxOutputTokens": 256,
    }))
    .unwrap()
}

fn local_selection(source_id: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "schemaVersion": "rangoon.local-selection.v1",
        "task": "classify_v1",
        "inputs": [{"kind": "source", "sourceId": source_id}],
    }))
    .unwrap()
}

fn cloud_selection(inputs: Vec<serde_json::Value>) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "schemaVersion": "rangoon.cloud-selection.v1",
        "task": "compare_v1",
        "inputs": inputs,
    }))
    .unwrap()
}

fn revision(byte: char) -> String {
    byte.to_string().repeat(64)
}

fn assert_local_equivalent(
    expected: &rangoon_model_session::PreparedView,
    routed: &rangoon_model_session::PreparedView,
) {
    assert_eq!(routed.body_json, expected.body_json);
    assert_eq!(routed.body_bytes, expected.body_bytes);
    assert_eq!(
        serde_json::to_value(&routed.inputs).unwrap(),
        serde_json::to_value(&expected.inputs).unwrap(),
    );
    assert_eq!(
        serde_json::to_value(&routed.pack).unwrap(),
        serde_json::to_value(&expected.pack).unwrap(),
    );
}

fn assert_cloud_equivalent(
    expected: &rangoon_model_session::cloud::PreparedView,
    routed: &rangoon_model_session::cloud::PreparedView,
) {
    assert_eq!(routed.body_json, expected.body_json);
    assert_eq!(routed.body_bytes, expected.body_bytes);
    assert_eq!(routed.body_sha256, expected.body_sha256);
    assert_eq!(
        serde_json::to_value(&routed.inputs).unwrap(),
        serde_json::to_value(&expected.inputs).unwrap(),
    );
    assert_eq!(
        serde_json::to_value(&routed.pack).unwrap(),
        serde_json::to_value(&expected.pack).unwrap(),
    );
}

fn route_local(
    access: &SourceAccess,
    session: &rangoon_model_session::LocalSession,
    raw: &[u8],
) -> Result<rangoon_model_session::RetainedPreparation, rangoon_model_session::Diagnostic> {
    access
        .begin_model_inputs()
        .unwrap()
        .with_inputs(|reader| session.begin_prepare(raw)?.prepare_with_inputs(reader))
        .deliver()
}

fn route_cloud(
    access: &SourceAccess,
    session: &rangoon_model_session::cloud::CloudSession,
    raw: &[u8],
) -> Result<rangoon_model_session::cloud::RetainedPreparation, rangoon_model_session::Diagnostic> {
    access
        .begin_model_inputs()
        .unwrap()
        .with_inputs(|reader| {
            session
                .begin_prepare(raw, &revision('a'))?
                .prepare_retained_with_inputs(reader)
        })
        .deliver()
}

#[cfg(all(feature = "encrypted-workspace", unix))]
fn save_through_access(access: &SourceAccess, source: &AnalysisReport) {
    select(access, source);
    assert!(matches!(
        tauri::async_runtime::block_on(access.save_analysis(source.source.id.clone())),
        WorkspaceResult::Saved { .. }
    ));
}

#[test]
fn legacy_typed_reader_preserves_unicode_source_pack_and_sqlite_bytes() {
    let root = TestRoot::new("model-input-legacy-local");
    let workspace = rangoon_store::Workspace::new(root.0.clone());
    let source = report(
        "exact.md",
        "\u{feff}# Exact\r\nCrab \u{1f980}\r\n".as_bytes(),
    );
    workspace.save_v1(&source).unwrap();
    let database = root.0.join("workspace.sqlite3");
    let before = std::fs::read(&database).unwrap();
    let raw = local_selection(&source.source.id);

    let expected_session = rangoon_model_session::LocalSession::default();
    expected_session.configure(&local_profile()).unwrap();
    let expected = expected_session
        .begin_prepare(&raw)
        .unwrap()
        .prepare(&workspace)
        .unwrap();

    let access = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));
    let routed_session = rangoon_model_session::LocalSession::default();
    routed_session.configure(&local_profile()).unwrap();
    let mut routed = route_local(&access, &routed_session, &raw).unwrap();
    assert_local_equivalent(&expected, routed.view());
    assert!(routed.view().body_json.contains("Crab 🦀"));
    routed.publish().unwrap();
    drop(routed);
    assert_eq!(std::fs::read(database).unwrap(), before);
}

#[test]
fn legacy_typed_reader_preserves_current_and_historical_capability_body_and_manifest() {
    let root = TestRoot::new("model-input-legacy-cloud");
    let workspace = rangoon_store::Workspace::new(root.0.clone());
    let source = report("rules.md", b"# Rules\nCapability history.\n");
    workspace.save_v1(&source).unwrap();
    let first = workspace
        .create_capability_v1(&source.source.id, &source.fragments[0].id, "Rules")
        .unwrap()
        .capability;
    let current = workspace
        .revise_capability_v1(
            &first.id,
            &first.latest_revision_id,
            "Rules current",
            "Current \u{1f980} capability body.\n",
        )
        .unwrap()
        .capability;
    let historical = workspace
        .open_capability_v1(&first.id, Some(&first.revision.id))
        .unwrap();
    let raw = cloud_selection(vec![
        serde_json::json!({"kind":"capability","capabilityId":historical.id,"revisionId":historical.revision.id}),
        serde_json::json!({"kind":"capability","capabilityId":current.id,"revisionId":current.revision.id}),
    ]);
    let database = root.0.join("workspace.sqlite3");
    let before = std::fs::read(&database).unwrap();

    let expected_session = rangoon_model_session::cloud::CloudSession::default();
    expected_session.configure(&cloud_profile()).unwrap();
    let expected = expected_session
        .begin_prepare(&raw, &revision('a'))
        .unwrap()
        .prepare(&workspace)
        .unwrap();

    let access = SourceAccess::legacy(Gate::new(), Ok(root.0.clone()));
    let routed_session = rangoon_model_session::cloud::CloudSession::default();
    routed_session.configure(&cloud_profile()).unwrap();
    let mut routed = route_cloud(&access, &routed_session, &raw).unwrap();
    assert_cloud_equivalent(&expected, routed.view());
    assert!(
        routed
            .view()
            .body_json
            .contains("Current 🦀 capability body.")
    );
    routed.publish().unwrap();
    drop(routed);
    assert_eq!(std::fs::read(database).unwrap(), before);
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn managed_owner_typed_reader_packs_unicode_source_and_capability_history_without_plaintext() {
    let root = TestRoot::new("model-input-managed");
    let vault = ThreadVault::default();
    let (access, identity) = encrypted_access(&root, &vault);
    let source = report(
        "managed.md",
        "# Managed\nExact \u{1f980} bytes.\n".as_bytes(),
    );
    save_through_access(&access, &source);
    let first = match tauri::async_runtime::block_on(access.create_capability(
        source.source.id.clone(),
        source.fragments[0].id.clone(),
        "Managed rules".to_owned(),
    )) {
        crate::capabilities::CapabilityResult::Opened { capability, .. } => *capability,
        _ => panic!("expected managed capability"),
    };
    let current = match tauri::async_runtime::block_on(access.revise_capability(
        first.id.clone(),
        first.latest_revision_id.clone(),
        "Managed current".to_owned(),
        "Current managed \u{1f980} text.\n".to_owned(),
    )) {
        crate::capabilities::CapabilityResult::Opened { capability, .. } => *capability,
        _ => panic!("expected managed revision"),
    };
    let raw = cloud_selection(vec![
        serde_json::json!({"kind":"capability","capabilityId":first.id,"revisionId":first.revision.id}),
        serde_json::json!({"kind":"capability","capabilityId":current.id,"revisionId":current.revision.id}),
    ]);
    let database = root
        .0
        .join("encrypted-workspaces")
        .join(identity)
        .join("workspace.sqlite3");
    let before = std::fs::read(&database).unwrap();
    assert!(
        !before
            .windows("Exact 🦀 bytes.".len())
            .any(|bytes| bytes == "Exact 🦀 bytes.".as_bytes())
    );

    let session = rangoon_model_session::cloud::CloudSession::default();
    session.configure(&cloud_profile()).unwrap();
    let mut routed = route_cloud(&access, &session, &raw).unwrap();
    assert!(routed.view().body_json.contains("Current managed 🦀 text."));
    assert_eq!(routed.view().inputs.len(), 2);
    routed.publish().unwrap();
    drop(routed);
    assert_eq!(std::fs::read(&database).unwrap(), before);
    assert!(!root.0.join("workspace.sqlite3").exists());
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn managed_locked_or_missing_source_never_falls_back_to_legacy_workspace() {
    use rangoon_desktop::workspace_session::Owner;

    let locked_root = TestRoot::new("model-input-locked");
    let locked_vault = ThreadVault::default();
    let locked = SourceAccess::managed(
        Gate::new(),
        Owner::acquire(locked_root.0.clone()).unwrap(),
        locked_vault,
    );
    let local = rangoon_model_session::LocalSession::default();
    local.configure(&local_profile()).unwrap();
    let raw =
        local_selection("source:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    assert!(matches!(
        route_local(&locked, &local, &raw),
        Err(rangoon_model_session::Diagnostic::InputUnavailable)
    ));
    assert!(!locked_root.0.join("workspace.sqlite3").exists());

    let missing_root = TestRoot::new("model-input-missing");
    let missing_vault = ThreadVault::default();
    let (missing, _) = encrypted_access(&missing_root, &missing_vault);
    assert!(matches!(
        route_local(&missing, &local, &raw),
        Err(rangoon_model_session::Diagnostic::InputUnavailable)
    ));
    assert!(!missing_root.0.join("workspace.sqlite3").exists());
}

#[test]
fn closed_gate_refuses_before_reader_callback() {
    let root = TestRoot::new("model-input-closed");
    let gate = Gate::new();
    let access = SourceAccess::legacy(gate.clone(), Ok(root.0.clone()));
    let callback = AtomicBool::new(false);
    let _drain = gate.request_drain().unwrap();
    assert!(matches!(
        access.begin_model_inputs(),
        Err(AdmissionError::Unavailable)
    ));
    assert!(!callback.load(Ordering::SeqCst));
}

#[test]
fn held_or_late_model_completion_holds_drain_and_drops_unpublished_stage() {
    let root = TestRoot::new("model-input-completion");
    let workspace = rangoon_store::Workspace::new(root.0.clone());
    let source = report("stage.md", b"# Stage\nRetained custody.\n");
    workspace.save_v1(&source).unwrap();
    let raw = local_selection(&source.source.id);
    let gate = Gate::new();
    let access = SourceAccess::legacy(gate.clone(), Ok(root.0.clone()));
    let session = rangoon_model_session::LocalSession::default();
    session.configure(&local_profile()).unwrap();
    let prepared = Arc::new(Mutex::new(None));
    let captured = Arc::clone(&prepared);
    let completion = access.begin_model_inputs().unwrap().with_inputs(|reader| {
        let retained = session.begin_prepare(&raw)?.prepare_with_inputs(reader)?;
        *captured.lock().unwrap() = Some(retained.view().clone());
        Ok(retained)
    });
    let drain = gate.request_drain().unwrap();
    assert!(!drain.is_quiescent().unwrap());
    assert!(matches!(
        completion.deliver(),
        Err(rangoon_model_session::Diagnostic::Cancelled)
    ));
    assert!(drain.is_quiescent().unwrap());
    let view = prepared.lock().unwrap().take().unwrap();
    let send = serde_json::to_vec(&serde_json::json!({
        "schemaVersion": "rangoon.local-send.v1",
        "preparedId": view.prepared_id,
        "requestId": view.request_id,
    }))
    .unwrap();
    assert!(matches!(
        session.begin_send(&send),
        Err(rangoon_model_session::Diagnostic::StalePrepared)
    ));
}

#[test]
fn callback_panic_releases_model_custody_and_drops_retained_stage() {
    let root = TestRoot::new("model-input-panic");
    let workspace = rangoon_store::Workspace::new(root.0.clone());
    let source = report("panic.md", b"# Panic\nCleanup custody.\n");
    workspace.save_v1(&source).unwrap();
    let raw = local_selection(&source.source.id);
    let gate = Gate::new();
    let access = SourceAccess::legacy(gate.clone(), Ok(root.0.clone()));
    let session = rangoon_model_session::LocalSession::default();
    session.configure(&local_profile()).unwrap();
    let prepared = Arc::new(Mutex::new(None));
    let captured = Arc::clone(&prepared);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = access.begin_model_inputs().unwrap().with_inputs(
                |reader| -> Result<rangoon_model_session::RetainedPreparation, _> {
                    let retained = session.begin_prepare(&raw)?.prepare_with_inputs(reader)?;
                    *captured.lock().unwrap() = Some(retained.view().clone());
                    panic!("synthetic model callback panic");
                },
            );
        }))
        .is_err()
    );
    let drain = gate.request_drain().unwrap();
    assert!(drain.is_quiescent().unwrap());
    let view = prepared.lock().unwrap().take().unwrap();
    let send = serde_json::to_vec(&serde_json::json!({
        "schemaVersion": "rangoon.local-send.v1",
        "preparedId": view.prepared_id,
        "requestId": view.request_id,
    }))
    .unwrap();
    assert!(matches!(
        session.begin_send(&send),
        Err(rangoon_model_session::Diagnostic::StalePrepared)
    ));
}

#[test]
fn native_model_wrappers_use_typed_inputs_without_raw_workspace_resolution() {
    for source in [include_str!("models.rs"), include_str!("cloud_models.rs")] {
        assert!(source.contains("begin_model_inputs"));
        assert!(!source.contains("workspace(&app)"));
        assert!(!source.contains("begin_operation"));
    }
}

#[test]
fn dropped_ipc_waiter_does_not_release_blocking_model_input_or_unpublished_payload() {
    let root = TestRoot::new("model-input-waiter");
    let source = report("waiter.md", b"# Waiter\nExact selected bytes\n");
    rangoon_store::Workspace::new(root.0.clone())
        .save_v1(&source)
        .unwrap();
    let gate = Gate::new();
    let access = SourceAccess::legacy(gate.clone(), Ok(root.0.clone()));
    let session = rangoon_model_session::LocalSession::default();
    session.configure(&local_profile()).unwrap();
    let worker_session = session.clone();
    let raw = local_selection(&source.source.id);
    let operation = access.begin_model_inputs().unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let prepared = Arc::new(Mutex::new(None));
    let captured = Arc::clone(&prepared);
    let waiter = tauri::async_runtime::spawn_blocking(move || {
        let completion = operation.with_inputs(|reader| {
            let retained = worker_session
                .begin_prepare(&raw)?
                .prepare_with_inputs(reader)?;
            *captured.lock().unwrap() = Some(retained.view().clone());
            started_tx.send(()).unwrap();
            release_rx.recv_timeout(WAIT).unwrap();
            Ok(retained)
        });
        assert!(done_tx.send(completion.deliver()).is_ok());
    });
    started_rx.recv_timeout(WAIT).unwrap();
    drop(waiter);
    let drain = gate.request_drain().unwrap();
    assert!(!drain.is_quiescent().unwrap());
    release_tx.send(()).unwrap();
    assert!(matches!(
        done_rx.recv_timeout(WAIT).unwrap(),
        Err(rangoon_model_session::Diagnostic::Cancelled)
    ));
    assert!(drain.is_quiescent().unwrap());
    let view = prepared.lock().unwrap().take().unwrap();
    let send = serde_json::to_vec(&serde_json::json!({"schemaVersion":"rangoon.local-send.v1","preparedId":view.prepared_id,"requestId":view.request_id})).unwrap();
    assert!(matches!(
        session.begin_send(&send),
        Err(rangoon_model_session::Diagnostic::StalePrepared)
    ));
}
