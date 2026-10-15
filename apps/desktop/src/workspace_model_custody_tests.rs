use crate::model_flight::{ModelFlight, ModelLease};
use rangoon_desktop::workspace_lifecycle::Gate;
use std::sync::Arc;

macro_rules! consent_custody_fixtures {
    () => {
        #[test]
        fn guarded_install_inspection_and_prompt_refuse_closed_witness() {
            let gate = Gate::new();
            let flight = ModelFlight::with_gate(gate.clone());
            let owner = Arc::new(flight.begin().expect("fixture model lease"));
            let rejected = Consent::<ModelLease>::default();
            let closed = gate.request_drain().expect("fixture drain");

            assert!(!owner.is_current());
            assert!(
                rejected
                    .install_guarded(
                        review('a', 'b'),
                        Cancellation::new(),
                        Arc::clone(&owner),
                        owner.witness(),
                    )
                    .is_none()
            );
            assert!(!closed.is_quiescent().expect("fixture drain state"));
            drop(owner);
            assert!(closed.is_quiescent().expect("fixture drain state"));

            let gate = Gate::new();
            let flight = ModelFlight::with_gate(gate.clone());
            let owner = Arc::new(flight.begin().expect("fixture model lease"));
            let consent = Consent::<ModelLease>::default();
            let value = review('c', 'd');
            let mut receiver = consent
                .install_guarded(
                    value.clone(),
                    Cancellation::new(),
                    Arc::clone(&owner),
                    owner.witness(),
                )
                .expect("fixture guarded install");
            let closed = gate.request_drain().expect("fixture drain");

            assert!(consent.inspect().is_none());
            assert!(consent.prompt(&decision(&value)).is_none());
            assert!(receiver.try_recv().is_err());
            consent.purge();
            assert_eq!(receiver.try_recv(), Ok(false));
            drop(owner);
            assert!(closed.is_quiescent().expect("fixture drain state"));
        }

        #[test]
        fn close_during_prompt_denies_late_affirmative() {
            let gate = Gate::new();
            let flight = ModelFlight::with_gate(gate.clone());
            let owner = Arc::new(flight.begin().expect("fixture model lease"));
            let consent = Consent::<ModelLease>::default();
            let cancellation = Cancellation::new();
            let value = review('e', 'f');
            let mut receiver = consent
                .install_guarded(
                    value.clone(),
                    cancellation.clone(),
                    Arc::clone(&owner),
                    owner.witness(),
                )
                .expect("fixture guarded install");

            assert!(consent.prompt(&decision(&value)).is_some());
            let closed = gate.request_drain().expect("fixture drain");
            assert!(!consent.finish_prompt(&value.run_id, true));
            assert_eq!(receiver.try_recv(), Ok(false));
            assert!(cancellation.is_cancelled());
            consent.purge();
            drop(owner);
            assert!(closed.is_quiescent().expect("fixture drain state"));
        }

        #[test]
        fn idle_purge_clears_body_owner_and_receiver() {
            let gate = Gate::new();
            let flight = ModelFlight::with_gate(gate);
            let owner = Arc::new(flight.begin().expect("fixture model lease"));
            let consent = Consent::<ModelLease>::default();
            let cancellation = Cancellation::new();
            let value = review('0', '1');
            let mut receiver = consent
                .install_guarded(
                    value,
                    cancellation.clone(),
                    Arc::clone(&owner),
                    owner.witness(),
                )
                .expect("fixture guarded install");

            drop(owner);
            consent.purge();
            assert!(consent.inspect().is_none());
            assert_eq!(receiver.try_recv(), Ok(false));
            assert!(cancellation.is_cancelled());
            assert!(flight.begin().is_ok());
        }

        #[test]
        fn purge_during_prompt_keeps_callback_owner_until_prompt_exits() {
            let gate = Gate::new();
            let flight = ModelFlight::with_gate(gate.clone());
            let owner = Arc::new(flight.begin().expect("fixture model lease"));
            let consent = Consent::<ModelLease>::default();
            let cancellation = Cancellation::new();
            let value = review('2', '3');
            let receiver = consent
                .install_guarded(
                    value.clone(),
                    cancellation.clone(),
                    Arc::clone(&owner),
                    owner.witness(),
                )
                .expect("fixture guarded install");

            assert!(consent.prompt(&decision(&value)).is_some());
            drop(receiver);
            drop(owner);
            let closed = gate.request_drain().expect("fixture drain");
            assert!(!closed.is_quiescent().expect("fixture drain state"));

            consent.purge();
            assert!(cancellation.is_cancelled());
            assert!(consent.inspect().is_none());
            assert!(!closed.is_quiescent().expect("fixture drain state"));
            assert!(consent.finish_prompt(&value.run_id, true));
            assert!(closed.is_quiescent().expect("fixture drain state"));
        }

        #[test]
        fn poisoned_prompt_cleanup_denies_and_releases_only_after_finish() {
            let gate = Gate::new();
            let flight = ModelFlight::with_gate(gate.clone());
            let owner = Arc::new(flight.begin().expect("fixture model lease"));
            let consent = Consent::<ModelLease>::default();
            let value = review('8', '9');
            let mut receiver = consent
                .install_guarded(
                    value.clone(),
                    Cancellation::new(),
                    Arc::clone(&owner),
                    owner.witness(),
                )
                .expect("fixture guarded install");

            assert!(consent.prompt(&decision(&value)).is_some());
            drop(owner);
            let closed = gate.request_drain().expect("fixture drain");
            consent.poison_for_test();
            consent.purge();
            assert!(!closed.is_quiescent().expect("fixture drain state"));
            assert!(consent.finish_prompt(&value.run_id, true));
            assert_eq!(receiver.try_recv(), Ok(false));
            assert!(closed.is_quiescent().expect("fixture drain state"));

            assert!(consent.inspect().is_none());
            gate.resume(&closed).expect("fixture resume");
            let next_owner = Arc::new(flight.begin().expect("replacement model lease"));
            assert!(
                consent
                    .install_guarded(
                        review('a', 'b'),
                        Cancellation::new(),
                        Arc::clone(&next_owner),
                        next_owner.witness(),
                    )
                    .is_none()
            );
        }

        #[test]
        fn stale_cleanup_cannot_remove_new_run() {
            let gate = Gate::new();
            let flight = ModelFlight::with_gate(gate);
            let owner = Arc::new(flight.begin().expect("fixture model lease"));
            let consent = Consent::<ModelLease>::default();
            let old = review('4', '5');
            let mut old_receiver = consent
                .install_guarded(
                    old.clone(),
                    Cancellation::new(),
                    Arc::clone(&owner),
                    owner.witness(),
                )
                .expect("fixture guarded install");
            consent.purge();
            assert_eq!(old_receiver.try_recv(), Ok(false));

            let current = review('6', '7');
            let current_cancellation = Cancellation::new();
            let mut current_receiver = consent
                .install_guarded(
                    current.clone(),
                    current_cancellation.clone(),
                    Arc::clone(&owner),
                    owner.witness(),
                )
                .expect("replacement guarded install");
            assert!(!consent.cancel_run(Some(&old.run_id)));
            assert!(!consent.remove(&old.run_id));
            assert!(!current_cancellation.is_cancelled());
            assert_eq!(
                consent.inspect().expect("current review").run_id,
                current.run_id
            );

            consent.purge();
            assert_eq!(current_receiver.try_recv(), Ok(false));
        }
    };
}

mod local {
    use super::*;
    use crate::model_consent::{Consent, Decision, Review};
    use rangoon_model_local::Cancellation;
    use serde_json::json;

    fn review(run: char, request: char) -> Review {
        Review {
            generation: "1".into(),
            run_id: format!("run:{}", run.to_string().repeat(64)),
            request_id: request.to_string().repeat(64),
            origin: "http://127.0.0.1:11434".into(),
            model: "fixture:v1".into(),
            body_bytes: 2,
            body_json: "{}".into(),
            pack_id: format!("pack:{}", "a".repeat(64)),
            inputs: vec![],
        }
    }

    fn decision(review: &Review) -> Decision {
        let raw = json!({
            "schemaVersion": "rangoon.local-review-decision.v1",
            "runId": review.run_id,
            "requestId": review.request_id,
        });
        Decision::parse(&serde_json::to_vec(&raw).expect("fixture decision bytes"))
            .expect("fixture decision")
    }

    consent_custody_fixtures!();
}

mod cloud {
    use super::*;
    use crate::cloud_model_consent::{Consent, Decision, Purpose, Review};
    use rangoon_model_cloud::Cancellation;
    use serde_json::json;

    fn review(run: char, request: char) -> Review {
        Review {
            generation: "1".into(),
            run_id: format!("cloud-run:{}", run.to_string().repeat(64)),
            request_id: request.to_string().repeat(64),
            origin: "https://api.openai.com".into(),
            model: "fixture-cloud-v1".into(),
            purpose: Purpose::ModelCheck,
            method: "GET".into(),
            path: "/v1/models/fixture-cloud-v1".into(),
            credential_revision: "a".repeat(64),
            body_sha256: "b".repeat(64),
            body_bytes: 0,
            body_json: "".into(),
            pack_id: None,
            inputs: vec![],
        }
    }

    fn decision(review: &Review) -> Decision {
        let raw = json!({
            "schemaVersion": "rangoon.cloud-review-decision.v1",
            "runId": review.run_id,
            "requestId": review.request_id,
        });
        Decision::parse(&serde_json::to_vec(&raw).expect("fixture decision bytes"))
            .expect("fixture decision")
    }

    consent_custody_fixtures!();
}

#[test]
fn resume_never_revives_an_old_model_witness() {
    let gate = Gate::new();
    let flight = ModelFlight::with_gate(gate.clone());
    let old = flight.begin().expect("fixture model lease");
    let old_witness = old.witness();

    let drain = gate.request_drain().expect("fixture drain");
    assert!(!old.is_current());
    assert!(!old_witness.is_current().expect("fixture witness"));
    drop(old);
    assert!(drain.is_quiescent().expect("fixture drain state"));
    gate.resume(&drain).expect("fixture resume");
    assert!(!old_witness.is_current().expect("old witness stays stale"));

    let replacement = flight.begin().expect("replacement model lease");
    assert!(replacement.is_current());
}

#[test]
fn profile_reads_share_admission_with_the_exclusive_model_lane() {
    let flight = ModelFlight::default();
    let profile_read = flight.read().expect("profile read lease");
    let model = flight.begin().expect("exclusive model lease");
    let second_profile_read = flight.read().expect("second profile read lease");

    assert!(profile_read.is_current());
    assert!(model.is_current());
    assert!(second_profile_read.is_current());
    assert!(flight.begin().is_err());
}
