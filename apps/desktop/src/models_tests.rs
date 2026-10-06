use super::*;
use rangoon_model_local::Cancellation;
use serde_json::{Value, json};

fn review() -> Review {
    Review {
        generation: "1".into(),
        run_id: format!("run:{}", "1".repeat(64)),
        request_id: "2".repeat(64),
        origin: "http://127.0.0.1:11434".into(),
        model: "fixture:v1".into(),
        body_bytes: 2,
        body_json: "{}".into(),
        pack_id: format!("pack:{}", "3".repeat(64)),
        inputs: vec![],
    }
}
fn decision(review: &Review) -> Decision {
    Decision::parse(&serde_json::to_vec(&json!({"schemaVersion":"rangoon.local-review-decision.v1","runId":review.run_id,"requestId":review.request_id})).unwrap()).unwrap()
}

#[test]
fn no_approval_from_install_inspection_or_renderer_fields() {
    let consent = Consent::default();
    let value = review();
    let mut receiver = consent
        .install(value.clone(), Cancellation::new(), Arc::new(()))
        .unwrap();
    assert_eq!(consent.inspect().unwrap().body_json, "{}");
    assert_eq!(
        receiver.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    );
    let attack = json!({"schemaVersion":"rangoon.local-review-decision.v1","runId":value.run_id,"requestId":value.request_id,"approved":true});
    assert!(Decision::parse(&serde_json::to_vec(&attack).unwrap()).is_none());
    assert!(consent.remove(&value.run_id));
    assert!(receiver.try_recv().is_err());
}

#[test]
fn prompt_single_claim_and_only_final_affirmative_releases_decision() {
    let consent = Consent::default();
    let value = review();
    let mut receiver = consent
        .install(value.clone(), Cancellation::new(), Arc::new(()))
        .unwrap();
    let input = decision(&value);
    assert!(consent.prompt(&input).is_some());
    assert!(consent.prompt(&input).is_none());
    assert!(receiver.try_recv().is_err());
    assert!(!consent.finish_prompt(&value.run_id, true));
    assert_eq!(receiver.try_recv(), Ok(true));
    assert!(consent.prompt(&input).is_none());
    assert!(consent.inspect().is_none());
}

#[test]
fn cancellation_during_dialog_holds_decision_and_late_yes_cannot_revive() {
    let consent = Consent::default();
    let value = review();
    let cancellation = Cancellation::new();
    let mut receiver = consent
        .install(value.clone(), cancellation.clone(), Arc::new(()))
        .unwrap();
    consent.prompt(&decision(&value)).unwrap();
    assert!(consent.cancel_run(Some(&value.run_id)));
    assert!(cancellation.is_cancelled());
    assert!(receiver.try_recv().is_err());
    assert!(!consent.finish_prompt(&value.run_id, true));
    assert_eq!(receiver.try_recv(), Ok(false));
}

#[test]
fn cleanup_during_prompt_defers_parent_destruction_and_blocks_replacement() {
    let consent = Consent::default();
    let value = review();
    let mut receiver = consent
        .install(value.clone(), Cancellation::new(), Arc::new(()))
        .unwrap();
    consent.prompt(&decision(&value)).unwrap();
    assert!(!consent.remove(&value.run_id));
    assert!(
        consent
            .install(value.clone(), Cancellation::new(), Arc::new(()))
            .is_none()
    );
    assert!(receiver.try_recv().is_err());
    assert!(consent.finish_prompt(&value.run_id, true));
    assert_eq!(receiver.try_recv(), Ok(false));
    assert!(
        consent
            .install(value, Cancellation::new(), Arc::new(()))
            .is_some()
    );
}

#[test]
fn close_before_prompt_cancels_without_affirmative_and_stale_events_are_inert() {
    let consent = Consent::default();
    let value = review();
    let token = Cancellation::new();
    let mut receiver = consent
        .install(value.clone(), token.clone(), Arc::new(()))
        .unwrap();
    assert!(!consent.cancel_run(Some("run:stale")));
    assert!(!token.is_cancelled());
    let mut wrong = decision(&value);
    wrong.request_id = "4".repeat(64);
    assert!(!consent.cancel_decision(&wrong));
    assert!(consent.prompt(&wrong).is_none());
    assert!(!consent.remove("run:stale"));
    assert!(!consent.cancel_run(Some(&value.run_id)));
    assert_eq!(receiver.try_recv(), Ok(false));
    assert!(consent.prompt(&decision(&value)).is_none());
    assert!(consent.remove(&value.run_id));
    let mut next = value.clone();
    next.run_id = format!("run:{}", "5".repeat(64));
    let next_token = Cancellation::new();
    let _next_receiver = consent
        .install(next.clone(), next_token.clone(), Arc::new(()))
        .unwrap();
    assert!(!consent.cancel_run(Some(&value.run_id)));
    assert!(!consent.remove(&value.run_id));
    assert!(!next_token.is_cancelled());
    assert_eq!(consent.inspect().unwrap().run_id, next.run_id);
}

#[test]
fn decision_parser_rejects_duplicate_escaped_keys_trailing_data_and_oversize() {
    let value = review();
    let valid = serde_json::to_string(&json!({"schemaVersion":"rangoon.local-review-decision.v1","runId":value.run_id,"requestId":value.request_id})).unwrap();
    assert!(Decision::parse(valid.as_bytes()).is_some());
    for invalid in [
        format!("{valid} false"),
        valid.replace("\"runId\":", "\"runId\":\"bad\",\"run\\u0049d\":"),
        " ".repeat(513),
        "[]".into(),
    ] {
        assert!(Decision::parse(invalid.as_bytes()).is_none());
    }
}

#[test]
fn handlers_require_actual_window_and_bounded_raw_input() {
    assert!(allowed("main", "main").is_ok());
    assert!(allowed(REVIEW_WINDOW, "main").is_err());
    assert!(allowed("main", REVIEW_WINDOW).is_err());
    assert!(allowed("main-child", "main").is_err());
    assert!(raw(&InvokeBody::Json(json!({"approved":true})), 512).is_err());
    assert!(raw(&InvokeBody::Raw(vec![0; 513]), 512).is_err());
    assert_eq!(raw(&InvokeBody::Raw(vec![0; 512]), 512).unwrap().len(), 512);
    assert!(empty(&InvokeBody::Json(json!({}))));
    assert!(!empty(&InvokeBody::Json(json!({"approved":true}))));
    assert!(!empty(&InvokeBody::Raw(vec![])));
}

#[test]
fn review_navigation_is_exact_and_cannot_load_workbench_or_remote_assets() {
    for url in [
        "tauri://localhost/model-confirmation.html",
        "http://tauri.localhost/model-confirmation.html",
    ] {
        assert!(review_navigation(&url.parse().unwrap()));
    }
    for url in [
        "tauri://localhost/analyze.html",
        "tauri://localhost/model-confirmation.html?approved=true",
        "http://tauri.localhost/model-confirmation.html#yes",
        "http://tauri.localhost:8080/model-confirmation.html",
        "https://example.com/model-confirmation.html",
        "file:///model-confirmation.html",
    ] {
        assert!(!review_navigation(&url.parse().unwrap()));
    }
}

#[test]
fn outcome_envelopes_preserve_closed_fields_and_captured_generation() {
    let model = NativeModel::default();
    let result = serde_json::to_value(profile_result(&model)).unwrap();
    assert_eq!(result["outcome"], "unconfigured");
    assert_eq!(result["generation"], "0");
    assert_eq!(result["runId"], Value::Null);
    assert_eq!(result["authority"], "none");
    assert_eq!(result.as_object().unwrap().len(), 6);
    let stamp = Stamp {
        generation: Some("7".into()),
        run_id: Some(review().run_id),
    };
    model.session.clear().unwrap();
    let result = serde_json::to_value(stamp.error(Code::Session(Diagnostic::Cancelled))).unwrap();
    assert_eq!(result["generation"], "7");
    assert_eq!(result["outcome"], "cancelled");
    assert_eq!(result.as_object().unwrap().len(), 5);
}

#[test]
fn native_capabilities_separate_main_and_review_without_network_permissions() {
    let main: Value =
        serde_json::from_str(include_str!("../capabilities/source-analysis.json")).unwrap();
    let review: Value =
        serde_json::from_str(include_str!("../capabilities/model-confirmation.json")).unwrap();
    assert_eq!(review["windows"], json!([REVIEW_WINDOW]));
    assert_eq!(
        review["permissions"],
        json!([
            "allow-get-local-model-review",
            "allow-confirm-local-model-review",
            "allow-cancel-local-model-review"
        ])
    );
    for permission in review["permissions"].as_array().unwrap() {
        assert!(!main["permissions"].as_array().unwrap().contains(permission));
    }
    let config: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    assert!(
        config["app"]["security"]["csp"]
            .as_str()
            .unwrap()
            .contains("connect-src ipc: http://ipc.localhost;")
    );
}

#[test]
fn prompt_keeps_session_and_shared_flight_after_ipc_owner_disappears() {
    let session = LocalSession::default();
    session.configure(br#"{"schemaVersion":"rangoon.local-profile-request.v1","profileId":"fixture","model":"fixture:v1","host":"127.0.0.1","port":11434,"maxOutputTokens":128}"#).unwrap();
    let operation = session.begin_check().unwrap();
    let cancellation = operation.cancellation().clone();
    let gate = ModelFlight::default();
    let owner = Arc::new((operation, gate.begin().unwrap()));
    let consent = Consent::default();
    let mut value = review();
    value.run_id = owner.0.run_id().to_owned();
    let receiver = consent
        .install(value.clone(), cancellation.clone(), Arc::clone(&owner))
        .unwrap();
    consent.prompt(&decision(&value)).unwrap();
    drop(owner);
    drop(receiver);
    // The native command's RAII cleanup runs when its waiter disappears.
    assert!(!consent.remove(&value.run_id));
    assert!(cancellation.is_cancelled());
    assert!(matches!(session.begin_check(), Err(Diagnostic::Busy)));
    assert!(matches!(gate.begin(), Err(Diagnostic::Busy)));
    // A late affirmative dialog completion cannot revive or retain the run.
    assert!(consent.finish_prompt(&value.run_id, true));
    assert!(session.begin_check().is_ok());
    assert!(gate.begin().is_ok());
}

#[test]
fn ipc_drop_guard_cancels_work_still_owned_by_a_callback() {
    let cancellation = Cancellation::new();
    let callback_token = cancellation.clone();
    let waiter = CancelOnDrop(cancellation);
    assert!(!callback_token.is_cancelled());
    drop(waiter);
    assert!(callback_token.is_cancelled());
}
