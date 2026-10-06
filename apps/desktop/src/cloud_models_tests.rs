use super::*;
use serde_json::{Value, json};

const PROFILE: &[u8] = br#"{"schemaVersion":"rangoon.cloud-profile-request.v1","profileId":"fixture","model":"synthetic-v1","maxOutputTokens":128}"#;
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
                "rangoon-cloud-native-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )))
    }
    fn store(&self) -> rangoon_store::Workspace {
        rangoon_store::Workspace::new(self.0.clone())
    }
    fn selection(&self) -> Vec<u8> {
        let source =
            rangoon_import::analyze("AGENTS.md", b"# Rules\nPreserve evidence.\n").unwrap();
        self.store().save_v1(&source).unwrap();
        serde_json::to_vec(&json!({"schemaVersion":"rangoon.cloud-selection.v1","task":"classify_v1","inputs":[{"kind":"source","sourceId":source.source.id}]})).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn model() -> NativeCloud {
    let model = NativeCloud::default();
    model.configure(PROFILE).unwrap();
    model
}
fn staged(model: &NativeCloud, fixture: &Fixture) -> PreparedView {
    let secret = Secret::new(b"synthetic-native-key").unwrap();
    let ready = model
        .resolution(&fixture.selection())
        .unwrap()
        .with_credential_revision(&secret.revision())
        .unwrap()
        .prepare_retained(&fixture.store())
        .unwrap();
    model.stage(ready, secret).unwrap()
}
fn claim(view: &PreparedView) -> Vec<u8> {
    serde_json::to_vec(&json!({"schemaVersion":"rangoon.cloud-send.v1","preparedId":view.prepared_id,"requestId":view.request_id})).unwrap()
}
fn check_flow(gate: &ModelFlight, custody: &Custody) -> Arc<Flow> {
    let model = model();
    let operation = model.lock().unwrap().session.begin_check().unwrap();
    let secret = Secret::new(b"synthetic-check-key").unwrap();
    let request = CheckRequest::new(operation.profile(), &secret.revision()).unwrap();
    let (credential, receiver) = custody.begin(Phase::Using).unwrap();
    drop(receiver);
    Arc::new(Flow {
        work: Work::Check {
            operation,
            request: Box::new(request),
        },
        secret,
        resources: Arc::new(Resources {
            _flight: gate.begin().unwrap(),
            credential,
        }),
    })
}
fn decision(review: &Review) -> Decision {
    Decision::parse(&serde_json::to_vec(&json!({"schemaVersion":"rangoon.cloud-review-decision.v1","runId":review.run_id,"requestId":review.request_id})).unwrap()).unwrap()
}

#[test]
fn staged_envelope_is_exact_one_shot_and_never_rendered() {
    let fixture = Fixture::new();
    let model = model();
    let prepared = staged(&model, &fixture);
    assert!(
        !serde_json::to_string(&prepared)
            .unwrap()
            .contains("synthetic-native-key")
    );
    let (send, secret) = model.claim(&claim(&prepared)).unwrap();
    assert_eq!(send.request().request_id(), prepared.request_id);
    assert_eq!(secret.revision(), prepared.credential_revision);
    drop(send);
    assert!(matches!(
        model.claim(&claim(&prepared)),
        Err(Diagnostic::StalePrepared)
    ));
    let prepared = staged(&model, &fixture);
    model.lock().unwrap().staged.as_mut().unwrap().prepared_id =
        format!("cloud-prepared:{}", "b".repeat(64));
    assert!(matches!(
        model.claim(&claim(&prepared)),
        Err(Diagnostic::StalePrepared)
    ));
    assert!(model.lock().unwrap().staged.is_none());
    assert!(matches!(
        model.claim(&claim(&prepared)),
        Err(Diagnostic::StalePrepared)
    ));
}
#[test]
fn late_pairing_after_clear_or_competing_prepare_is_refused() {
    let fixture = Fixture::new();
    let model = model();
    let raw = fixture.selection();
    let secret = Secret::new(b"synthetic-native-key").unwrap();
    let ready = model
        .resolution(&raw)
        .unwrap()
        .with_credential_revision(&secret.revision())
        .unwrap()
        .prepare_retained(&fixture.store())
        .unwrap();
    assert!(model.inspect().unwrap().active.is_some());
    assert!(matches!(model.resolution(&raw), Err(Diagnostic::Busy)));
    assert!(matches!(
        model.stage(ready, secret),
        Err(Diagnostic::StalePrepared)
    ));
    assert!(model.lock().unwrap().staged.is_none());
    let secret = Secret::new(b"synthetic-native-key").unwrap();
    let ready = model
        .resolution(&raw)
        .unwrap()
        .with_credential_revision(&secret.revision())
        .unwrap()
        .prepare_retained(&fixture.store())
        .unwrap();
    model.clear().unwrap();
    assert!(matches!(
        model.stage(ready, secret),
        Err(Diagnostic::Cancelled)
    ));
    assert!(model.lock().unwrap().staged.is_none());
}
#[test]
fn invalid_profile_preserves_pair_but_new_selection_and_cancel_remove_it() {
    let fixture = Fixture::new();
    let model = model();
    let view = staged(&model, &fixture);
    assert!(model.configure(b"{}").is_err());
    drop(model.claim(&claim(&view)).unwrap());
    let view = staged(&model, &fixture);
    assert!(model.resolution(b"{}").is_err());
    assert!(matches!(
        model.claim(&claim(&view)),
        Err(Diagnostic::StalePrepared)
    ));
    staged(&model, &fixture);
    let operation = model.lock().unwrap().session.begin_check().unwrap();
    let raw = serde_json::to_vec(
        &json!({"schemaVersion":"rangoon.cloud-cancel.v1","runId":operation.run_id()}),
    )
    .unwrap();
    let result = serde_json::to_value(cancel_result(&model, &raw)).unwrap();
    assert_eq!(result["outcome"], "cancel_requested");
    assert_eq!(result["runId"], operation.run_id());
    assert_eq!(result["session"]["active"]["runId"], operation.run_id());
    assert_eq!(result["generation"], operation.generation());
    assert!(model.lock().unwrap().staged.is_none());
    assert_eq!(operation.ensure_current(), Err(Diagnostic::Cancelled));
}
#[test]
fn source_free_check_requires_exact_one_shot_consent_and_retains_both_leases() {
    let gate = ModelFlight::default();
    let custody = Custody::default();
    let flow = check_flow(&gate, &custody);
    let review = flow.review();
    assert_eq!(review.body_json, "");
    assert_eq!(review.body_bytes, 0);
    assert!(review.inputs.is_empty() && review.pack_id.is_none());
    assert!(review.question().contains("stored credential"));
    let consent = Consent::default();
    let token = flow.operation().cancellation().clone();
    let mut receiver = consent
        .install(review.clone(), token.clone(), Arc::clone(&flow))
        .unwrap();
    assert!(receiver.try_recv().is_err());
    assert!(consent.prompt(&decision(&review)).is_some());
    assert!(consent.prompt(&decision(&review)).is_none());
    drop(flow);
    drop(receiver);
    assert!(!consent.remove(&review.run_id));
    assert!(token.is_cancelled());
    assert!(matches!(gate.begin(), Err(Diagnostic::Busy)));
    assert!(matches!(
        custody.begin(Phase::Reading),
        Err(cloud_store::Error::Busy)
    ));
    assert!(consent.finish_prompt(&review.run_id, true));
    assert!(gate.begin().is_ok());
    assert!(custody.begin(Phase::Reading).is_ok());
}
#[test]
fn cloud_decisions_cannot_use_local_handles_unknown_fields_or_stale_identity() {
    let gate = ModelFlight::default();
    let custody = Custody::default();
    let flow = check_flow(&gate, &custody);
    let review = flow.review();
    let valid = serde_json::to_string(&json!({"schemaVersion":"rangoon.cloud-review-decision.v1","runId":review.run_id,"requestId":review.request_id})).unwrap();
    for bad in [
        valid.replace("cloud-run:", "run:"),
        valid.replace("cloud-review", "local-review"),
        valid.replace("\"runId\":", "\"approved\":true,\"runId\":"),
        format!("{valid} false"),
        " ".repeat(513),
        valid.replace("\"runId\":", "\"runId\":\"bad\",\"run\\u0049d\":"),
    ] {
        assert!(Decision::parse(bad.as_bytes()).is_none());
    }
    let consent = Consent::default();
    let token = flow.operation().cancellation().clone();
    let _receiver = consent
        .install(review.clone(), token.clone(), Arc::clone(&flow))
        .unwrap();
    let mut wrong = decision(&review);
    wrong.request_id = "c".repeat(64);
    assert!(consent.prompt(&wrong).is_none());
    assert!(!consent.cancel_decision(&wrong));
    assert!(!token.is_cancelled());
    let waiter = CancelOnDrop(token.clone());
    drop(waiter);
    assert!(token.is_cancelled());
    assert!(consent.prompt(&decision(&review)).is_none());
}
#[test]
fn final_os_affirmative_is_one_shot_and_cleanup_preserves_transport_custody() {
    let gate = ModelFlight::default();
    let custody = Custody::default();
    let flow = check_flow(&gate, &custody);
    let review = flow.review();
    let token = flow.operation().cancellation().clone();
    let consent = Consent::default();
    let mut receiver = consent
        .install(review.clone(), token.clone(), Arc::clone(&flow))
        .unwrap();
    assert!(consent.inspect().is_some());
    assert!(receiver.try_recv().is_err());
    // Completion without a claimed native prompt cannot create consent.
    assert!(!consent.finish_prompt(&review.run_id, true));
    assert!(receiver.try_recv().is_err());
    consent.prompt(&decision(&review)).unwrap();
    assert!(!consent.finish_prompt(&review.run_id, true));
    assert!(receiver.try_recv().unwrap());
    assert!(consent.prompt(&decision(&review)).is_none());
    assert!(consent.remove(&review.run_id));
    assert!(!token.is_cancelled());
    assert!(matches!(gate.begin(), Err(Diagnostic::Busy)));
    assert!(matches!(
        custody.begin(Phase::Editing),
        Err(cloud_store::Error::Busy)
    ));
    // The review closes, but the caller still owns the resources for re-read,
    // freshness and transport. A later cancellation still invalidates that work.
    token.cancel();
    assert!(flow.ensure_current().is_err());
    drop(flow);
    assert!(gate.begin().is_ok());
    assert!(custody.begin(Phase::Editing).is_ok());
}
#[test]
fn commands_labels_navigation_and_capabilities_are_closed() {
    assert!(allowed("main", REVIEW_WINDOW).is_err());
    assert!(allowed(REVIEW_WINDOW, "main").is_err());
    assert!(raw(&InvokeBody::Json(json!({})), 512).is_err());
    assert!(raw(&InvokeBody::Raw(vec![0; 513]), 512).is_err());
    assert!(empty(&InvokeBody::Json(json!({}))));
    assert!(!empty(&InvokeBody::Json(json!({"approved":true}))));
    for url in [
        "tauri://localhost/cloud-model-confirmation.html",
        "http://tauri.localhost/cloud-model-confirmation.html",
    ] {
        assert!(review_navigation(&url.parse().unwrap()));
    }
    for url in [
        "tauri://localhost/model-confirmation.html",
        "tauri://localhost/cloud-model-confirmation.html?yes=true",
        "http://tauri.localhost:80/cloud-model-confirmation.html#yes",
        "https://example.com/cloud-model-confirmation.html",
    ] {
        assert!(!review_navigation(&url.parse().unwrap()));
    }
    let main: Value =
        serde_json::from_str(include_str!("../capabilities/source-analysis.json")).unwrap();
    let review: Value = serde_json::from_str(include_str!(
        "../capabilities/cloud-model-confirmation.json"
    ))
    .unwrap();
    assert_eq!(review["windows"], json!([REVIEW_WINDOW]));
    let config: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    assert!(
        config["app"]["security"]["capabilities"]
            .as_array()
            .unwrap()
            .contains(&json!(REVIEW_WINDOW))
    );
    assert_eq!(
        review["permissions"],
        json!([
            "allow-get-cloud-model-review",
            "allow-confirm-cloud-model-review",
            "allow-cancel-cloud-model-review"
        ])
    );
    for permission in review["permissions"].as_array().unwrap() {
        assert!(!main["permissions"].as_array().unwrap().contains(permission));
    }
    let body = serde_json::to_value(profile_result(&NativeCloud::default())).unwrap();
    assert_eq!(body["schemaVersion"], "rangoon.cloud-session-result.v1");
    assert_eq!(body["authority"], "none");
    assert_eq!(body["outcome"], "unconfigured");
    assert_eq!(body.as_object().unwrap().len(), 6);
}
