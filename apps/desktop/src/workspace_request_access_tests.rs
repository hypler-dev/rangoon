#[cfg(all(feature = "encrypted-workspace", unix))]
use super::super::reopen::Kind;
use super::*;
use crate::{
    cloud_custody::{Custody, Phase as CredentialPhase},
    model_flight::ModelFlight,
    workspace_requests::WorkspaceRequest,
};

fn client_id(access: &SourceAccess) -> String {
    serde_json::to_value(access.request_view()).unwrap()["clientId"]
        .as_str()
        .expect("issued client identifier")
        .to_owned()
}

fn issue(access: &SourceAccess, gate: &Gate) -> WorkspaceRequest {
    let drain = gate.request_drain().expect("fixture drain");
    access
        .0
        .clients
        .resume(gate, &drain)
        .expect("fixture issue");
    access.request_context("main", Some(&client_id(access)))
}

fn managed_probe() -> (SourceAccess, Arc<ProbeState>, Gate, AnalysisReport) {
    let gate = Gate::new();
    let state = Arc::new(ProbeState::default());
    let source = report("request.md", b"# Request\n");
    let access = SourceAccess::with_mode(
        gate.clone(),
        Box::new(ProbeBackend::new(Arc::clone(&state), source.clone())),
        true,
    );
    (access, state, gate, source)
}

#[test]
fn refused_or_stale_source_request_cannot_reach_backend_or_mutate_selected_session() {
    let (access, state, gate, source) = managed_probe();
    assert!(matches!(
        tauri::async_runtime::block_on(access.list_snapshots()),
        WorkspaceResult::Failed { .. }
    ));
    assert_eq!(state.list_calls.load(Ordering::SeqCst), 0);

    let old = issue(&access, &gate);
    let bound = access.for_request(&old);
    select(&bound, &source);
    assert!(access.0.session.snapshot(&source.source.id).is_some());
    let _fresh = issue(&access, &gate);
    assert!(matches!(
        tauri::async_runtime::block_on(bound.save_analysis(source.source.id.clone())),
        WorkspaceResult::Failed { .. }
    ));
    assert_eq!(state.save_calls.load(Ordering::SeqCst), 0);
    assert!(access.0.session.snapshot(&source.source.id).is_some());
}

#[test]
fn stale_request_refuses_model_and_credential_admission_before_their_first_mutation() {
    let (access, _state, gate, _source) = managed_probe();
    let old = issue(&access, &gate);
    let fresh = issue(&access, &gate);

    let model = ModelFlight::with_gate(gate.clone());
    assert!(matches!(
        model.begin_for(&old),
        Err(rangoon_model_session::Diagnostic::SessionUnavailable)
    ));
    let model_lease = model.begin_for(&fresh).expect("fresh model lease");

    let custody = Custody::with_gate(gate);
    assert!(matches!(
        custody.begin_for(&old, CredentialPhase::Reading),
        Err(crate::cloud_store::Error::Unavailable)
    ));
    let (credential_lease, _receiver) = custody
        .begin_for(&fresh, CredentialPhase::Reading)
        .expect("fresh credential lease");
    drop(credential_lease);
    drop(model_lease);
}

#[test]
fn rejected_request_does_not_consume_any_of_sixty_four_reader_slots() {
    let (access, _state, gate, _source) = managed_probe();
    let stale = issue(&access, &gate);
    let fresh = issue(&access, &gate);
    let flight = ModelFlight::with_gate(gate);
    assert!(matches!(
        flight.read_for(&stale),
        Err(rangoon_model_session::Diagnostic::SessionUnavailable)
    ));
    let mut readers = Vec::new();
    for _ in 0..64 {
        readers.push(flight.read_for(&fresh).expect("current reader lease"));
    }
    assert!(matches!(
        flight.read_for(&fresh),
        Err(rangoon_model_session::Diagnostic::Busy)
    ));
    drop(readers);
}

#[test]
fn callback_custody_survives_waiter_drop_for_a_current_request() {
    let (access, _state, gate, _source) = managed_probe();
    let request = issue(&access, &gate);
    let flight = ModelFlight::with_gate(gate.clone());
    let waiter = Arc::new(flight.begin_for(&request).expect("fixture model lease"));
    let callback = Arc::clone(&waiter);
    drop(waiter);

    let drain = gate.request_drain().expect("fixture drain");
    assert_eq!(drain.is_quiescent(), Ok(false));
    drop(callback);
    assert_eq!(drain.is_quiescent(), Ok(true));
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn source_control_revoke_and_unlock_resume_rotate_the_actual_managed_request() {
    let root = TestRoot::new("request-source-control");
    let vault = ThreadVault::default();
    let (access, _) = encrypted_access(&root, &vault);

    let first_ticket = access.request_close().unwrap();
    assert!(first_ticket.purge());
    assert_eq!(first_ticket.is_quiescent(), Ok(true));
    assert_eq!(first_ticket.release_key(), Ok(true));
    let mut first_control = first_ticket.start_control(Kind::Unlock).unwrap();
    let first_preview = first_control.prepare().unwrap();
    first_control.confirm(&first_preview.id).unwrap();
    first_control.resume().unwrap();

    let old_id = client_id(&access);
    assert!(matches!(
        access.request_context("main", Some(&old_id)),
        WorkspaceRequest::Managed(_)
    ));
    let second_ticket = access.request_close().unwrap();
    let view = serde_json::to_value(access.request_view()).unwrap();
    assert_eq!(view["state"], "unavailable");
    assert_eq!(view["clientId"], serde_json::Value::Null);
    assert!(matches!(
        access.request_context("main", Some(&old_id)),
        WorkspaceRequest::Refused
    ));

    assert!(second_ticket.purge());
    assert_eq!(second_ticket.is_quiescent(), Ok(true));
    assert_eq!(second_ticket.release_key(), Ok(true));
    let mut second_control = second_ticket.start_control(Kind::Unlock).unwrap();
    let second_preview = second_control.prepare().unwrap();
    second_control.confirm(&second_preview.id).unwrap();
    second_control.resume().unwrap();

    let fresh_id = client_id(&access);
    assert_ne!(old_id, fresh_id);
    assert!(matches!(
        access.request_context("main", Some(&old_id)),
        WorkspaceRequest::Refused
    ));
    let fresh = access.request_context("main", Some(&fresh_id));
    let bound = access.for_request(&fresh);
    let lease = bound.begin(Lane::Read).expect("fresh source admission");
    drop(lease);
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn managed_close_discards_retained_read_only_picker_result() {
    let root = TestRoot::new("operations-managed-close");
    let vault = ThreadVault::default();
    let (access, _) = encrypted_access(&root, &vault);

    let initial = access.request_close().unwrap();
    assert!(initial.purge());
    assert_eq!(initial.release_key(), Ok(true));
    let mut control = initial.start_control(Kind::Unlock).unwrap();
    let preview = control.prepare().unwrap();
    control.confirm(&preview.id).unwrap();
    control.resume().unwrap();

    let request = access.request_context("main", Some(&client_id(&access)));
    let picker = access.for_request(&request).begin_selection().unwrap();
    let completion = picker.complete_read_only("safe-view");
    let closing = access.request_close().unwrap();
    assert_eq!(closing.is_quiescent(), Ok(false));
    assert!(closing.purge());
    assert_eq!(completion.deliver(|| "stale"), "stale");
    assert_eq!(closing.is_quiescent(), Ok(true));
}

#[test]
fn ordinary_command_surface_has_exact_request_binding_coverage() {
    let sources = [
        include_str!("main.rs"),
        include_str!("capabilities.rs"),
        include_str!("workflows.rs"),
        include_str!("composition.rs"),
        include_str!("data_controls.rs"),
        include_str!("compilation.rs"),
        include_str!("instruction_bundles.rs"),
        include_str!("models.rs"),
        include_str!("cloud_models.rs"),
        include_str!("cloud_credentials.rs"),
        include_str!("operations.rs"),
    ];
    let commands = [
        (0, "select_and_analyze", "for_request(&workspace)"),
        (
            10,
            "select_and_inspect_operations",
            "for_request(&workspace)",
        ),
        (0, "list_snapshots", "for_request(&workspace)"),
        (0, "save_analysis", "for_request(&workspace)"),
        (0, "open_snapshot", "for_request(&workspace)"),
        (0, "read_composition_source", "for_request(&workspace)"),
        (0, "clear_analysis", "for_request(&workspace)"),
        (1, "list_capabilities", "for_request(&workspace)"),
        (1, "open_capability", "for_request(&workspace)"),
        (1, "create_capability", "for_request(&workspace)"),
        (1, "revise_capability", "for_request(&workspace)"),
        (1, "review_capability", "for_request(&workspace)"),
        (2, "list_workflows", "for_request(&workspace)"),
        (2, "open_workflow", "for_request(&workspace)"),
        (2, "begin_workflow_draft", "for_request(&workspace)"),
        (2, "inspect_workflow_save", "for_request(&workspace)"),
        (2, "commit_workflow_save", "for_request(&workspace)"),
        (2, "clear_workflow_draft", "for_request(&workspace)"),
        (3, "preview_composition", "for_request(&workspace)"),
        (3, "commit_composition", "for_request(&workspace)"),
        (4, "get_workspace_data", "for_request(&workspace)"),
        (4, "export_workspace_backup", "for_request(&workspace)"),
        (4, "prepare_workspace_restore", "for_request(&workspace)"),
        (4, "restore_workspace_backup", "for_request(&workspace)"),
        (4, "inspect_workspace_deletion", "for_request(&workspace)"),
        (4, "delete_workspace_record", "for_request(&workspace)"),
        (5, "compile_capability", "for_request(&workspace)"),
        (6, "export_instruction_bundle", "for_request(&workspace)"),
        (6, "inspect_instruction_bundle", "for_request(&workspace)"),
        (7, "get_local_model_profile", "read_for(&workspace)"),
        (7, "configure_local_model", "begin_for(&workspace)"),
        (7, "clear_local_model", "read_for(&workspace)"),
        (7, "cancel_local_model", "read_for(&workspace)"),
        (7, "prepare_local_model", "begin_for(&workspace)"),
        (7, "check_local_model", "begin_for(&workspace)"),
        (7, "send_local_model", "begin_for(&workspace)"),
        (8, "get_cloud_model_profile", "read_for(&workspace)"),
        (8, "configure_cloud_model", "begin_for(&workspace)"),
        (8, "clear_cloud_model", "read_for(&workspace)"),
        (8, "cancel_cloud_model", "read_for(&workspace)"),
        (
            8,
            "prepare_cloud_model",
            "Resources::acquire(&app, &workspace)",
        ),
        (
            8,
            "check_cloud_model",
            "Resources::acquire(&app, &workspace)",
        ),
        (
            8,
            "send_cloud_model",
            "Resources::acquire(&app, &workspace)",
        ),
        (
            9,
            "inspect_cloud_credential",
            "begin_for(&workspace, Phase::Reading)",
        ),
        (
            9,
            "edit_cloud_credential",
            "begin_for(&workspace, Phase::Reading)",
        ),
        (
            9,
            "remove_cloud_credential",
            "begin_for(&workspace, Phase::Reading)",
        ),
    ];
    for (source, command, binding) in commands {
        let signature = format!("fn {command}(");
        let start = sources[source]
            .find(&signature)
            .expect("named command exists");
        assert!(
            sources[source][..start]
                .rfind("#[tauri::command]")
                .is_some()
        );
        let body = &sources[source][start..];
        let body = &body[..body.find("\n#[tauri::command]").unwrap_or(body.len())];
        let argument_end = body.find(") ->").expect("command signature closes");
        assert!(
            body[..argument_end].contains("workspace: crate::workspace_requests::WorkspaceRequest")
        );
        let binding_at = body.find(binding).expect("request binds before work");
        assert!(binding_at < 1_500, "binding must precede command work");
    }
    assert_eq!(commands.len(), 46);
    for (source, command) in [
        (0, "get_engine_status"),
        (7, "get_local_model_review"),
        (7, "cancel_local_model_review"),
        (7, "confirm_local_model_review"),
        (8, "get_cloud_model_review"),
        (8, "cancel_cloud_model_review"),
        (8, "confirm_cloud_model_review"),
        (9, "get_cloud_credential_edit"),
        (9, "submit_cloud_credential"),
        (9, "cancel_cloud_credential_edit"),
    ] {
        let signature = format!("fn {command}(");
        let start = sources[source]
            .find(&signature)
            .expect("excluded command exists");
        let body = &sources[source][start..];
        let argument_end = body.find(") ->").expect("command signature closes");
        assert!(!body[..argument_end].contains("WorkspaceRequest"));
    }
    for source in [
        include_str!("workspace_close.rs"),
        include_str!("workspace_reopen.rs"),
        include_str!("workspace_close_control.rs"),
    ] {
        assert!(!source.contains("WorkspaceRequest"));
    }
}
