use super::*;
use serde_json::{Value, json};

const ACTION_SCHEMA: &str = "rangoon.workspace-startup-action.v1";
const VIEW_SCHEMA: &str = "rangoon.workspace-startup.v1";

#[test]
fn startup_commands_are_limited_to_the_exact_main_window_label() {
    assert!(main_window("main").is_ok());
    for label in ["", "Main", "main/child", "model-review", "splash"] {
        assert!(main_window(label).is_err());
    }
}

#[test]
fn source_free_quit_requires_main_and_empty_json_before_requesting_exit() {
    use std::cell::Cell;
    use tauri::ipc::InvokeBody;
    let exits = Cell::new(0);
    for label in ["", "Main", "main/child", "model-review", "splash"] {
        assert!(
            request_quit(label, &InvokeBody::Json(json!({})), || exits
                .set(exits.get() + 1))
            .is_err()
        );
    }
    for body in [
        InvokeBody::Raw(b"{}".to_vec()),
        InvokeBody::Json(json!(null)),
        InvokeBody::Json(json!([])),
        InvokeBody::Json(json!({"action": "quit"})),
        InvokeBody::Json(json!({"path": "not-accepted"})),
        InvokeBody::Json(json!({"exitCode": 0})),
    ] {
        assert!(request_quit("main", &body, || exits.set(exits.get() + 1)).is_err());
    }
    assert_eq!(exits.get(), 0);
    request_quit("main", &InvokeBody::Json(json!({})), || {
        exits.set(exits.get() + 1)
    })
    .unwrap();
    assert_eq!(exits.get(), 1);
}

fn attempt() -> String {
    "a".repeat(64)
}

fn action_body(
    schema_version: &str,
    startup_id: &str,
    attempt_id: &str,
    epoch: &str,
    action: &str,
) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": schema_version,
        "startupId": startup_id,
        "attemptId": attempt_id,
        "epoch": epoch,
        "action": action,
    }))
    .unwrap()
}

fn identity() -> Identity {
    Identity::new(&attempt(), "1").unwrap()
}

fn request_for(identity: &Identity, action: &str) -> ActionRequest {
    ActionRequest::parse(&action_body(
        ACTION_SCHEMA,
        &identity.startup_id,
        &identity.attempt_id,
        &identity.epoch,
        action,
    ))
    .unwrap()
}

#[test]
fn action_request_requires_the_exact_closed_schema_and_all_fields_in_any_order() {
    let identity = identity();
    let reordered = format!(
        r#"{{"action":"quit","epoch":"{}","attemptId":"{}","startupId":"{}","schemaVersion":"{}"}}"#,
        identity.epoch, identity.attempt_id, identity.startup_id, ACTION_SCHEMA
    );
    let request = ActionRequest::parse(reordered.as_bytes()).unwrap();
    assert!(request.matches(&identity));
    assert_eq!(request.action(), Action::Quit);
    assert_eq!(format!("{request:?}"), "StartupAction(<redacted>)");

    let valid = json!({
        "schemaVersion": ACTION_SCHEMA,
        "startupId": &identity.startup_id,
        "attemptId": &identity.attempt_id,
        "epoch": &identity.epoch,
        "action": "quit",
    });
    for field in ["schemaVersion", "startupId", "attemptId", "epoch", "action"] {
        let mut missing = valid.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(ActionRequest::parse(&serde_json::to_vec(&missing).unwrap()).is_err());
    }
    for invalid in [
        json!({"schemaVersion": "other", "startupId": &identity.startup_id, "attemptId": &identity.attempt_id, "epoch": &identity.epoch, "action": "quit"}),
        json!({"schemaVersion": ACTION_SCHEMA, "startupId": &identity.startup_id, "attemptId": &identity.attempt_id, "epoch": &identity.epoch, "action": "quit", "extra": true}),
    ] {
        assert!(ActionRequest::parse(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
}

#[test]
fn action_request_rejects_duplicate_unknown_malformed_and_wrongly_typed_values() {
    let identity = identity();
    for duplicate in [
        format!(
            r#"{{"schemaVersion":"{ACTION_SCHEMA}","startupId":"{}","startupId":"{}","attemptId":"{}","epoch":"1","action":"quit"}}"#,
            identity.startup_id, identity.startup_id, identity.attempt_id
        ),
        format!(
            r#"{{"schemaVersion":"{ACTION_SCHEMA}","startupId":"{}","attemptId":"{}","epoch":"1","action":"quit","unexpected":true,"unexpected":false}}"#,
            identity.startup_id, identity.attempt_id
        ),
    ] {
        assert!(ActionRequest::parse(duplicate.as_bytes()).is_err());
    }
    assert!(ActionRequest::parse(b"\xff").is_err());
    for value in [
        json!({"schemaVersion": ACTION_SCHEMA, "startupId": null, "attemptId": &identity.attempt_id, "epoch": "1", "action": "quit"}),
        json!({"schemaVersion": ACTION_SCHEMA, "startupId": &identity.startup_id, "attemptId": [], "epoch": "1", "action": "quit"}),
        json!({"schemaVersion": ACTION_SCHEMA, "startupId": &identity.startup_id, "attemptId": &identity.attempt_id, "epoch": 1, "action": "quit"}),
        json!({"schemaVersion": ACTION_SCHEMA, "startupId": &identity.startup_id, "attemptId": &identity.attempt_id, "epoch": "1", "action": null}),
    ] {
        assert!(ActionRequest::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}

#[test]
fn action_request_accepts_all_closed_actions_and_only_canonical_identity_fields() {
    let startup_id = "b".repeat(64);
    let attempt_id = attempt();
    for action in [
        "setup",
        "review_conversion",
        "copy",
        "cancel",
        "verify",
        "recover",
        "quit",
    ] {
        assert!(
            ActionRequest::parse(&action_body(
                ACTION_SCHEMA,
                &startup_id,
                &attempt_id,
                "1",
                action,
            ))
            .is_ok()
        );
    }
    for invalid in [
        "a".repeat(63),
        "A".repeat(64),
        format!("{}g", "a".repeat(63)),
    ] {
        assert!(
            ActionRequest::parse(&action_body(
                ACTION_SCHEMA,
                &invalid,
                &attempt_id,
                "1",
                "quit",
            ))
            .is_err()
        );
        assert!(
            ActionRequest::parse(&action_body(
                ACTION_SCHEMA,
                &startup_id,
                &invalid,
                "1",
                "quit",
            ))
            .is_err()
        );
    }
    for invalid in ["0", "00", "01", "+1", "18446744073709551616"] {
        assert!(
            ActionRequest::parse(&action_body(
                ACTION_SCHEMA,
                &startup_id,
                &attempt_id,
                invalid,
                "quit",
            ))
            .is_err()
        );
    }
    assert!(
        ActionRequest::parse(&action_body(
            ACTION_SCHEMA,
            &startup_id,
            &attempt_id,
            &u64::MAX.to_string(),
            "quit",
        ))
        .is_ok()
    );
}

#[test]
fn action_request_accepts_exactly_512_raw_bytes_and_rejects_513() {
    let identity = identity();
    let mut raw = action_body(
        ACTION_SCHEMA,
        &identity.startup_id,
        &identity.attempt_id,
        &identity.epoch,
        "quit",
    );
    assert!(raw.len() < 512);
    raw.resize(512, b' ');
    assert!(ActionRequest::parse(&raw).is_ok());
    raw.push(b' ');
    assert!(ActionRequest::parse(&raw).is_err());
}

#[test]
fn identity_rotation_and_request_matching_compare_all_opaque_bookkeeping_fields() {
    let first = identity();
    let second = Identity::new(&attempt(), "1").unwrap();
    assert_ne!(first.startup_id, second.startup_id);
    assert_eq!(first.attempt_id, second.attempt_id);
    assert_eq!(first.epoch, second.epoch);
    assert_eq!(format!("{first:?}"), "StartupIdentity(<redacted>)");

    assert!(request_for(&first, "quit").matches(&first));
    for request in [
        action_body(
            ACTION_SCHEMA,
            &second.startup_id,
            &first.attempt_id,
            &first.epoch,
            "quit",
        ),
        action_body(
            ACTION_SCHEMA,
            &first.startup_id,
            &"c".repeat(64),
            &first.epoch,
            "quit",
        ),
        action_body(
            ACTION_SCHEMA,
            &first.startup_id,
            &first.attempt_id,
            "2",
            "quit",
        ),
    ] {
        assert!(!ActionRequest::parse(&request).unwrap().matches(&first));
    }
}

#[test]
fn views_allow_only_ordered_state_actions_originals_and_copying_phases() {
    let identity = identity();
    let cases = [
        (
            Stage::Fresh,
            Original::NotApplicable,
            &[Action::Setup, Action::Quit][..],
            None,
        ),
        (
            Stage::LegacyFound,
            Original::Unassessed,
            &[Action::ReviewConversion, Action::Quit][..],
            None,
        ),
        (
            Stage::ConversionReview,
            Original::PresentUnencrypted,
            &[Action::Copy, Action::Cancel, Action::Quit][..],
            None,
        ),
        (
            Stage::Copying,
            Original::PresentUnencrypted,
            &[Action::Quit][..],
            Some(CopyPhase::CopyingRecords),
        ),
        (
            Stage::RestartRequired,
            Original::PresentUnencrypted,
            &[Action::Quit][..],
            None,
        ),
        (
            Stage::VerifyRequired,
            Original::NotVerified,
            &[Action::Verify, Action::Quit][..],
            None,
        ),
        (
            Stage::RecoveryRequired,
            Original::NotVerified,
            &[Action::Quit][..],
            None,
        ),
        (Stage::Handoff, Original::NotApplicable, &[][..], None),
        (
            Stage::Unavailable,
            Original::NotApplicable,
            &[Action::Quit][..],
            None,
        ),
    ];
    for (stage, original, actions, phase) in cases {
        let view = View::for_cycle(&identity, stage, original, actions, phase, None).unwrap();
        let value: Value = serde_json::from_slice(&view.encode().unwrap()).unwrap();
        assert_eq!(value["state"], serde_json::to_value(stage).unwrap());
        assert_eq!(value["actions"], serde_json::to_value(actions).unwrap());
    }
    for (stage, original, actions, phase) in [
        (
            Stage::Fresh,
            Original::NotApplicable,
            &[Action::Quit, Action::Setup][..],
            None,
        ),
        (
            Stage::Fresh,
            Original::NotApplicable,
            &[Action::Setup, Action::Setup][..],
            None,
        ),
        (
            Stage::Fresh,
            Original::NotApplicable,
            &[Action::Copy][..],
            None,
        ),
        (
            Stage::Fresh,
            Original::Unassessed,
            &[Action::Setup][..],
            None,
        ),
        (
            Stage::Copying,
            Original::PresentUnencrypted,
            &[Action::Quit][..],
            None,
        ),
        (
            Stage::Fresh,
            Original::NotApplicable,
            &[Action::Quit][..],
            Some(CopyPhase::CheckingCopy),
        ),
    ] {
        assert!(View::for_cycle(&identity, stage, original, actions, phase, None).is_err());
    }
}

#[test]
fn unavailable_view_has_no_identity_or_actions_and_views_encode_only_closed_fields() {
    let identity = identity();
    let unavailable = View::unavailable(Diagnostic::BackendUnavailable);
    let unavailable_value: Value = serde_json::from_slice(&unavailable.encode().unwrap()).unwrap();
    assert_eq!(unavailable_value["schemaVersion"], VIEW_SCHEMA);
    assert_eq!(unavailable_value["startupId"], Value::Null);
    assert_eq!(unavailable_value["attemptId"], Value::Null);
    assert_eq!(unavailable_value["epoch"], Value::Null);
    assert_eq!(unavailable_value["actions"], json!([]));

    let view = View::for_cycle(
        &identity,
        Stage::Copying,
        Original::PresentUnencrypted,
        &[Action::Quit],
        Some(CopyPhase::CheckingCopy),
        Some(Diagnostic::Busy),
    )
    .unwrap();
    let encoded = view.encode().unwrap();
    assert!(encoded.len() <= 2048);
    let value: Value = serde_json::from_slice(&encoded).unwrap();
    let mut keys = value
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    assert_eq!(
        keys,
        [
            "actions",
            "attemptId",
            "copyPhase",
            "diagnostic",
            "epoch",
            "retainedOriginal",
            "schemaVersion",
            "startupId",
            "state",
        ]
    );
    for forbidden in ["root", "source", "key", "customPayload"] {
        assert!(value.get(forbidden).is_none());
    }
}

#[test]
fn terminal_handoff_has_no_cycle_or_effectful_fields() {
    for original in [
        Original::NotApplicable,
        Original::PresentUnencrypted,
        Original::NotVerified,
    ] {
        let handoff = View::handoff(original).unwrap();
        let value: Value = serde_json::from_slice(&handoff.encode().unwrap()).unwrap();
        assert_eq!(value["state"], "handoff");
        assert_eq!(value["startupId"], Value::Null);
        assert_eq!(value["attemptId"], Value::Null);
        assert_eq!(value["epoch"], Value::Null);
        assert_eq!(value["actions"], json!([]));
        assert_eq!(value["copyPhase"], Value::Null);
        assert_eq!(
            value["retainedOriginal"],
            serde_json::to_value(original).unwrap()
        );
        assert_eq!(value["diagnostic"], Value::Null);
    }
    assert_eq!(
        View::handoff(Original::Unassessed).err(),
        Some(Error::Invalid)
    );
}

#[test]
fn view_encoding_cap_rejects_test_mutated_output_without_claiming_valid_input_reachability() {
    let identity = identity();
    let mut view = View::for_cycle(
        &identity,
        Stage::Fresh,
        Original::NotApplicable,
        &[Action::Setup],
        None,
        None,
    )
    .unwrap();
    view.startup_id = Some("x".repeat(2049));
    assert_eq!(view.encode(), Err(Error::Invalid));
}
