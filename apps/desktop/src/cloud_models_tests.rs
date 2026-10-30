use super::*;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};

const PROFILE: &[u8] = br#"{"schemaVersion":"rangoon.cloud-profile-request.v1","profileId":"fixture","model":"synthetic-v1","maxOutputTokens":128}"#;
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let sequence = NEXT_FIXTURE
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .expect("cloud fixture sequence exhausted");
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rangoon-cloud-native-{}-{}-{sequence}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        Self::reserve(path).expect("could not reserve a fresh cloud fixture directory")
    }
    fn reserve(path: std::path::PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir(&path)?;
        Ok(Self(path))
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
#[track_caller]
fn staged(model: &NativeCloud, fixture: &Fixture) -> PreparedView {
    let secret = Secret::new(b"synthetic-native-key").unwrap();
    let selected = fixture.selection();
    let prepared = model
        .resolution(&selected)
        .unwrap()
        .with_credential_revision(&secret.revision())
        .unwrap()
        .prepare_retained(&fixture.store());
    let ready = match prepared {
        Ok(ready) => ready,
        Err(error) => {
            let selection: Value = serde_json::from_slice(&selected).unwrap();
            let id = selection["inputs"][0]["sourceId"].as_str().unwrap();
            // A later read is diagnostic evidence, not the original error or a retry.
            // Only a closed error class is printed, never content, paths or secrets.
            let read = fixture.store().open(id).map(|_| ());
            panic!("cloud fixture preparation failed: {error:?}; post-failure read: {read:?}");
        }
    };
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
fn fixture_reservation_refuses_existing_directory_without_adoption_or_cleanup() {
    let fixture = Fixture::new();
    let selection: Value = serde_json::from_slice(&fixture.selection()).unwrap();
    let id = selection["inputs"][0]["sourceId"].as_str().unwrap();
    let marker = fixture.0.join("keep-existing");
    std::fs::write(&marker, b"synthetic marker").unwrap();
    assert!(Fixture::reserve(fixture.0.clone()).is_err());
    assert_eq!(std::fs::read(&marker).unwrap(), b"synthetic marker");
    assert!(fixture.store().open(id).is_ok());
}

#[test]
fn parallel_fixture_lifetimes_preserve_the_surviving_saved_input() {
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let barrier = std::sync::Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                let fixture = Fixture::new();
                fixture.selection();
                fixture
            })
        })
        .collect();
    let mut fixtures: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    let paths: std::collections::HashSet<_> = fixtures.iter().map(|f| f.0.clone()).collect();
    assert_eq!(paths.len(), fixtures.len());
    let survivor = fixtures.pop().unwrap();
    drop(fixtures);
    let view = staged(&model(), &survivor);
    assert_eq!(view.inputs.len(), 1);
}

#[test]
fn unavailable_saved_input_refuses_preparation_and_cannot_revive_an_old_pair() {
    for missing in [true, false] {
        let fixture = Fixture::new();
        let model = model();
        let previous = staged(&model, &fixture);
        let selected = fixture.selection();
        let selection: Value = serde_json::from_slice(&selected).unwrap();
        let id = selection["inputs"][0]["sourceId"].as_str().unwrap();
        let database = fixture.0.join("workspace.sqlite3");
        if missing {
            std::fs::remove_file(&database).unwrap();
        } else {
            std::fs::write(&database, b"synthetic invalid database").unwrap();
        }
        assert_eq!(
            fixture.store().open(id).err(),
            Some(if missing {
                rangoon_store::StoreError::NotFound
            } else {
                rangoon_store::StoreError::Unavailable
            })
        );
        let secret = Secret::new(b"synthetic-native-key").unwrap();
        let preparation = model
            .resolution(&selected)
            .unwrap()
            .with_credential_revision(&secret.revision())
            .unwrap();
        assert!(matches!(
            preparation.prepare_retained(&fixture.store()),
            Err(Diagnostic::InputUnavailable)
        ));
        assert!(model.lock().unwrap().staged.is_none());
        assert!(matches!(
            model.claim(&claim(&previous)),
            Err(Diagnostic::StalePrepared)
        ));
        // Refusal must not recreate a missing database or repair an invalid one.
        if missing {
            assert!(!database.exists());
        } else {
            assert_eq!(
                std::fs::read(&database).unwrap(),
                b"synthetic invalid database"
            );
        }
    }
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
