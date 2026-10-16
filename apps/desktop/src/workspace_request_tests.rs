use super::*;
use rangoon_desktop::workspace_lifecycle::{Error, Gate, Lane};

fn drain(gate: &Gate) -> Drain {
    gate.request_drain().expect("fixture drain")
}

fn view(registry: &Registry) -> serde_json::Value {
    serde_json::to_value(registry.view()).expect("serializable fixed view")
}

fn publish(registry: &Registry, gate: &Gate, drain: &Drain, byte: u8) {
    registry
        .resume_with(gate, drain, |bytes| {
            bytes.fill(byte);
            Ok(())
        })
        .expect("fixture publication")
}

#[test]
fn managed_registry_starts_closed_and_entropy_failure_never_mints_or_resumes() {
    let gate = Gate::new();
    let registry = Registry::new(true);
    assert_eq!(
        view(&registry),
        serde_json::json!({
            "schemaVersion": "rangoon.workspace-request-state.v1",
            "storage": "managed_encrypted",
            "state": "unavailable",
            "clientId": null,
        })
    );
    assert!(matches!(
        registry.request("main", Some(&"a".repeat(64))),
        WorkspaceRequest::Refused
    ));

    let closing = drain(&gate);
    assert!(registry.resume_with(&gate, &closing, |_| Err(())).is_err());
    assert_eq!(view(&registry)["state"], "unavailable");
    assert!(matches!(gate.begin(Lane::Read), Err(Error::Closed)));
    assert!(gate.resume(&closing).is_ok());
}

#[test]
fn exact_resume_publishes_fresh_count_free_request_and_revoke_closes_it() {
    let gate = Gate::new();
    let registry = Registry::new(true);
    let first_drain = drain(&gate);
    publish(&registry, &gate, &first_drain, 0x1a);
    let first = "1a".repeat(32);
    assert_eq!(
        view(&registry),
        serde_json::json!({
            "schemaVersion": "rangoon.workspace-request-state.v1",
            "storage": "managed_encrypted",
            "state": "open",
            "clientId": first,
        })
    );
    let request = registry.request("main", Some(&first));
    let lease = request
        .admit(&gate, Lane::Read)
        .expect("current request admission");
    drop(lease);

    assert!(registry.revoke().is_ok());
    assert!(matches!(
        registry.request("main", Some(&first)),
        WorkspaceRequest::Refused
    ));
    let second_drain = drain(&gate);
    publish(&registry, &gate, &second_drain, 0x2b);
    let second = "2b".repeat(32);
    assert_ne!(first, second);
    assert!(matches!(
        registry.request("main", Some(&first)),
        WorkspaceRequest::Refused
    ));
    assert!(matches!(
        registry.request("main", Some(&second)),
        WorkspaceRequest::Managed(_)
    ));
}

#[test]
fn request_rejects_missing_malformed_duplicate_and_foreign_window_headers() {
    let gate = Gate::new();
    let registry = Registry::new(true);
    let closing = drain(&gate);
    publish(&registry, &gate, &closing, 0x3c);
    let current = "3c".repeat(32);
    for malformed in ["", "3c", &"3C".repeat(32), &"g0".repeat(32)] {
        assert!(matches!(
            registry.request("main", Some(malformed)),
            WorkspaceRequest::Refused
        ));
    }
    assert!(matches!(
        registry.request("main", None),
        WorkspaceRequest::Refused
    ));
    assert!(matches!(
        registry.request("main", Some(&"4d".repeat(32))),
        WorkspaceRequest::Refused
    ));
    assert!(matches!(
        registry.request("other", Some(&current)),
        WorkspaceRequest::Refused
    ));
    assert!(matches!(
        registry.request("main", Some(&current)),
        WorkspaceRequest::Managed(_)
    ));

    let mut headers = tauri::http::HeaderMap::new();
    headers.append(HEADER, tauri::http::HeaderValue::from_static("aa"));
    assert_eq!(header(&headers), Some("aa"));
    headers.append(HEADER, tauri::http::HeaderValue::from_static("bb"));
    assert_eq!(header(&headers), None);
}

#[test]
fn legacy_registry_preserves_legacy_request_without_client_identifier() {
    let registry = Registry::new(false);
    assert_eq!(
        view(&registry),
        serde_json::json!({
            "schemaVersion": "rangoon.workspace-request-state.v1",
            "storage": "legacy_plaintext",
            "state": "legacy",
            "clientId": null,
        })
    );
    assert!(matches!(
        registry.request("main", None),
        WorkspaceRequest::Legacy
    ));
    assert!(matches!(
        registry.request("other", None),
        WorkspaceRequest::Refused
    ));
}

#[test]
fn poisoned_registry_stays_closed_without_resuming_or_publishing() {
    let gate = Gate::new();
    let registry = Registry::new(true);
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            let _held = registry.open.lock().expect("fixture registry mutex");
            panic!("synthetic registry poison");
        });
        assert!(worker.join().is_err());
    });
    let closing = drain(&gate);
    assert!(
        registry
            .resume_with(&gate, &closing, |bytes| {
                bytes.fill(0x4d);
                Ok(())
            })
            .is_err()
    );
    assert_eq!(view(&registry)["state"], "unavailable");
    assert!(matches!(gate.begin(Lane::Read), Err(Error::Closed)));
}
