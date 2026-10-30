use super::super::{
    Host, HostStatus, Observation, PRODUCT_VERSION, ProposedTarget, ReadKind, ReadOutcome,
    Retention, SyntheticReadClient,
};
use super::*;
use crate::{SourceAccess, workspace_close::WorkspaceClose, workspace_requests::WorkspaceRequest};
use rangoon_desktop::workspace_lifecycle::Gate;
use rangoon_openshell_client::{
    Authenticity, BearerLease, ClientConfig, ClientError, ConnectionCheck, Continuity,
    HealthStatus, LogWindow, OpenShellReadClient, PolicyLoadStatus, PolicyRead, PolicyRevision,
    ProducerKind, WatchControl, WatchEvent, WatchEventKind, WatchWindow,
};
use serde_json::{Value, json};
use std::{
    io::Write,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

const WAIT: Duration = Duration::from_secs(3);

fn target() -> ProposedTarget {
    ProposedTarget::new(
        "https://openshell.test".into(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 443),
        "installation-canary".into(),
        "workspace-canary".into(),
        "sandbox-canary".into(),
        b"root-canary".to_vec(),
        b"leaf-canary".to_vec(),
    )
    .unwrap()
}

fn workspace(gate: &Gate) -> WorkspaceRequest {
    WorkspaceRequest::Managed(Arc::new(gate.observe_open().unwrap()))
}

fn prepare(host: &Host, gate: &Gate, kind: ReadKind) -> super::super::Review {
    host.prepare(&workspace(gate), target(), kind, Retention::SessionOnly)
        .unwrap()
}

fn lease() -> BearerLease {
    BearerLease::new(
        "test-token".into(),
        Instant::now() + Duration::from_secs(30),
    )
    .unwrap()
}

fn check() -> ConnectionCheck {
    ConnectionCheck {
        authenticity: Authenticity::Unqualified,
        installation: "installation-canary".into(),
        workspace: "workspace-canary".into(),
        sandbox: "sandbox-canary".into(),
        reported_product_version: PRODUCT_VERSION.into(),
        health: HealthStatus::Healthy,
        protected_read: PolicyRevision {
            version: 7,
            policy_hash: Some("hash-canary".into()),
            status: PolicyLoadStatus::Loaded,
            created_time_millis: Some(111),
            loaded_time_millis: Some(222),
            losses: vec![Loss::PolicyHashMissing],
        },
        received_time_millis: 333,
    }
}

fn outcome(kind: ReadKind) -> ReadOutcome {
    let protected_check = check();
    let observation = match kind {
        ReadKind::ProtectedCheck => Observation::ProtectedCheck(protected_check.clone()),
        ReadKind::PolicyStatus => Observation::PolicyStatus(PolicyRead {
            authenticity: Authenticity::Unqualified,
            installation: "installation-canary".into(),
            workspace: "workspace-canary".into(),
            sandbox: "sandbox-canary".into(),
            active_version: 7,
            queried: PolicyRevision {
                version: 7,
                policy_hash: Some("policy-hash-canary".into()),
                status: PolicyLoadStatus::Loaded,
                created_time_millis: Some(333),
                loaded_time_millis: Some(444),
                losses: vec![Loss::PolicyBodyRedacted],
            },
            revisions: vec![PolicyRevision {
                version: 7,
                policy_hash: Some("revision-hash-canary".into()),
                status: PolicyLoadStatus::Loaded,
                created_time_millis: Some(555),
                loaded_time_millis: Some(666),
                losses: vec![Loss::PolicyHashInvalid],
            }],
            received_time_millis: 777,
        }),
        ReadKind::RecentLogs => Observation::RecentLogs(LogWindow {
            authenticity: Authenticity::Unqualified,
            installation: "installation-canary".into(),
            workspace: "workspace-canary".into(),
            sandbox: "sandbox-canary".into(),
            line_count: 2,
            buffer_total: 99,
            native_sandbox_id: Some("native-id-canary".into()),
            source_times_millis: vec![Some(1000), Some(2000)],
            producers: vec![ProducerKind::Gateway, ProducerKind::Process],
            received_time_millis: 3000,
            losses: vec![Loss::LogTextRedacted],
        }),
        ReadKind::RecentWatch => Observation::RecentWatch(WatchWindow {
            authenticity: Authenticity::Unqualified,
            installation: "installation-canary".into(),
            workspace: "workspace-canary".into(),
            sandbox: "sandbox-canary".into(),
            events: vec![
                WatchEvent {
                    kind: WatchEventKind::Status,
                    producer: Some(ProducerKind::Gateway),
                    cursor: Some("cursor-canary".into()),
                    native_sandbox_id: Some("native-id-canary".into()),
                    source_time_millis: Some(4000),
                    received_time_millis: 5000,
                    losses: vec![Loss::MissingNativeEventId],
                },
                WatchEvent {
                    kind: WatchEventKind::Warning,
                    producer: None,
                    cursor: Some("cursor-canary-2".into()),
                    native_sandbox_id: None,
                    source_time_millis: None,
                    received_time_millis: 6000,
                    losses: vec![Loss::StreamWarning],
                },
            ],
            continuity: Continuity::Gap,
        }),
    };
    ReadOutcome {
        protected_check,
        observation,
    }
}

struct ResultClient {
    kind: ReadKind,
    result: Result<ReadOutcome, ClientError>,
    entered: Option<mpsc::Sender<()>>,
    release: Option<mpsc::Receiver<()>>,
    returning: Option<mpsc::Sender<()>>,
}

impl SyntheticReadClient for ResultClient {
    fn run(
        mut self: Box<Self>,
        _config: ClientConfig,
        _lease: BearerLease,
        kind: ReadKind,
        _control: WatchControl,
    ) -> Result<ReadOutcome, ClientError> {
        assert_eq!(kind, self.kind);
        if let Some(entered) = self.entered.take() {
            entered.send(()).unwrap();
        }
        if let Some(release) = self.release.take() {
            release.recv_timeout(WAIT).unwrap();
        }
        if let Some(returning) = self.returning.take() {
            returning.send(()).unwrap();
        }
        self.result
    }
}

fn start(host: &Host, review: &super::super::Review, result: Result<ReadOutcome, ClientError>) {
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            lease(),
            Box::new(ResultClient {
                kind: review.kind,
                result,
                entered: None,
                release: None,
                returning: None,
            })
        ),
        HostStatus::Running
    );
}

fn terminal(host: &Host) -> HostStatus {
    let until = Instant::now() + WAIT;
    loop {
        let status = host.status();
        if !matches!(status, HostStatus::Running | HostStatus::Closing) {
            return status;
        }
        assert!(Instant::now() < until, "callback did not terminate");
        std::thread::yield_now();
    }
}

fn value(view: &SessionView) -> Value {
    serde_json::from_slice(&view.encode().unwrap()).unwrap()
}

fn unavailable(view: SessionView) {
    assert_eq!(
        value(&view),
        json!({
            "schema":"rangoon.openshell.session-view.v1", "provider":"openshell",
            "productVersion":"v0.1.4", "phase":"unavailable", "startAvailable":false,
            "retention":"session_only", "provenance": {
                "authenticity":"unqualified", "authority":"none", "execution":"unavailable",
                "actionOutcome":"unknown", "coverage":"bounded_partial", "omissions":[
                    "native_identifiers", "source_timestamps", "policy_hashes", "provider_payloads",
                    "target_and_credential_references"
                ]
            }
        })
    );
}

#[test]
fn unavailable_refuses_no_current_legacy_refused_foreign_stale_draining_and_poisoned_custody() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    unavailable(host.inspect_session(&workspace(&gate)));
    unavailable(host.inspect_session(&WorkspaceRequest::Legacy));
    unavailable(host.inspect_session(&WorkspaceRequest::Refused));
    let foreign = Gate::new();
    unavailable(host.inspect_session(&workspace(&foreign)));
    let current = workspace(&gate);
    let _drain = gate.request_drain().unwrap();
    unavailable(host.inspect_session(&current));

    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let poisoned = host.clone();
    std::thread::spawn(move || {
        let _guard = poisoned.state.lock().unwrap();
        panic!("synthetic session-view poison");
    })
    .join()
    .unwrap_err();
    unavailable(host.inspect_session(&workspace(&gate)));
}

#[test]
fn pending_phases_preserve_kind_and_expiry_and_stale_win_over_expiry() {
    for (confirmed, phase) in [
        (false, "awaiting_consent"),
        (true, "ready_for_native_credential"),
    ] {
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let review = prepare(&host, &gate, ReadKind::PolicyStatus);
        if confirmed {
            assert_eq!(
                host.confirm(&review.id, true),
                HostStatus::ReadyForNativeCredential
            );
        }
        let request = workspace(&gate);
        let current = value(&host.inspect_session(&request));
        assert_eq!(current["phase"], phase);
        assert_eq!(current["readKind"], "policy_status");
        assert!(current.get("observation").is_none());
        let mut state = host.state.lock().unwrap();
        match state.pending.as_mut().unwrap() {
            super::super::Pending::Awaiting { selection, .. }
            | super::super::Pending::Confirmed { selection, .. } => {
                selection.deadline = Instant::now() - Duration::from_secs(1);
            }
        }
        drop(state);
        let expired = value(&host.inspect_session(&request));
        assert_eq!(expired["phase"], "error");
        assert_eq!(expired["error"], "credential_lease_expired");
        assert_eq!(expired["readKind"], "policy_status");
        let _drain = gate.request_drain().unwrap();
        unavailable(host.inspect_session(&request));
    }
}

#[test]
fn pending_snapshot_rechecks_expiry_after_before_final_barrier_for_both_consent_phases() {
    for confirmed in [false, true] {
        let gate = Gate::new();
        let host = Arc::new(Host::new(gate.clone()));
        let review = prepare(&host, &gate, ReadKind::RecentLogs);
        if confirmed {
            assert_eq!(
                host.confirm(&review.id, true),
                HostStatus::ReadyForNativeCredential
            );
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        {
            let mut state = host.state.lock().unwrap();
            match state.pending.as_mut().unwrap() {
                super::super::Pending::Awaiting { selection, .. }
                | super::super::Pending::Confirmed { selection, .. } => {
                    selection.deadline = deadline
                }
            }
        }
        let request = workspace(&gate);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let probe = SnapshotProbe {
            before_final: Some((entered_tx, release_rx)),
            ..Default::default()
        };
        let reader = {
            let host = Arc::clone(&host);
            std::thread::spawn(move || host.inspect_session_inner(&request, Some(&probe)))
        };
        entered_rx.recv_timeout(WAIT).unwrap();
        assert!(Instant::now() < deadline, "barrier must precede expiry");
        while Instant::now() < deadline {
            std::thread::yield_now();
        }
        release_tx.send(()).unwrap();
        let view = value(&reader.join().unwrap());
        assert_eq!(view["phase"], "error");
        assert_eq!(view["error"], "credential_lease_expired");
        assert_eq!(view["readKind"], "recent_logs");
        assert!(view.get("observation").is_none());
    }
}

#[test]
fn active_phases_errors_and_completed_keep_retained_kind() {
    let errors = [
        ClientError::InvalidConfiguration,
        ClientError::CredentialLeaseExpired,
        ClientError::Unreachable,
        ClientError::Tls,
        ClientError::Unauthenticated,
        ClientError::PermissionDenied,
        ClientError::Unsupported,
        ClientError::Cancelled,
        ClientError::Stale,
        ClientError::DeadlineExceeded,
        ClientError::InvalidCursor,
        ClientError::Gap,
        ClientError::Bounds,
        ClientError::Source,
    ];
    for kind in [
        ReadKind::ProtectedCheck,
        ReadKind::PolicyStatus,
        ReadKind::RecentLogs,
        ReadKind::RecentWatch,
    ] {
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let review = prepare(&host, &gate, kind);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        assert_eq!(
            host.confirm(&review.id, true),
            HostStatus::ReadyForNativeCredential
        );
        assert_eq!(
            host.begin_synthetic(
                &review.id,
                lease(),
                Box::new(ResultClient {
                    kind,
                    result: Ok(outcome(kind)),
                    entered: Some(entered_tx),
                    release: Some(release_rx),
                    returning: None,
                })
            ),
            HostStatus::Running
        );
        entered_rx.recv_timeout(WAIT).unwrap();
        let running = value(&host.inspect_session(&workspace(&gate)));
        assert_eq!(running["phase"], "running");
        assert_eq!(running["readKind"], kind_name(kind));
        assert!(running.get("observation").is_none());
        assert_eq!(host.stop(), HostStatus::Closing);
        let closing = value(&host.inspect_session(&workspace(&gate)));
        assert_eq!(closing["phase"], "closing");
        assert_eq!(closing["readKind"], kind_name(kind));
        assert!(closing.get("observation").is_none());
        release_tx.send(()).unwrap();
        assert_eq!(terminal(&host), HostStatus::Cancelled);
        let cancelled = value(&host.inspect_session(&workspace(&gate)));
        assert_eq!(cancelled["phase"], "cancelled");
        assert_eq!(cancelled["readKind"], kind_name(kind));

        for error in errors {
            let gate = Gate::new();
            let host = Host::new(gate.clone());
            let review = prepare(&host, &gate, kind);
            start(&host, &review, Err(error));
            let terminal_status = terminal(&host);
            let expected_status = if error == ClientError::Cancelled {
                HostStatus::Cancelled
            } else {
                HostStatus::Error(error)
            };
            assert_eq!(terminal_status, expected_status);
            let view = value(&host.inspect_session(&workspace(&gate)));
            let expected_phase = if error == ClientError::Cancelled {
                "cancelled"
            } else {
                "error"
            };
            assert_eq!(view["phase"], expected_phase);
            if error == ClientError::Cancelled {
                assert!(view.get("error").is_none());
            } else {
                assert_eq!(view["error"], error.code());
            }
            assert_eq!(view["readKind"], kind_name(kind));
            assert!(view.get("observation").is_none());
        }
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let review = prepare(&host, &gate, kind);
        start(&host, &review, Ok(outcome(kind)));
        assert_eq!(terminal(&host), HostStatus::Completed);
        let done = value(&host.inspect_session(&workspace(&gate)));
        assert_eq!(done["phase"], "completed");
        assert_eq!(done["readKind"], kind_name(kind));
        assert!(done.get("observation").is_some());
    }
}

fn kind_name(kind: ReadKind) -> &'static str {
    match kind {
        ReadKind::ProtectedCheck => "protected_check",
        ReadKind::PolicyStatus => "policy_status",
        ReadKind::RecentLogs => "recent_logs",
        ReadKind::RecentWatch => "recent_watch",
    }
}

#[test]
fn completed_is_historical_after_expiry_but_stop_and_close_purge_before_next_delivery() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::ProtectedCheck);
    start(&host, &review, Ok(outcome(ReadKind::ProtectedCheck)));
    assert_eq!(terminal(&host), HostStatus::Completed);
    {
        let state = host.state.lock().unwrap();
        state
            .active
            .as_ref()
            .unwrap()
            .state
            .lock()
            .unwrap()
            .credential_deadline = Some(Instant::now() - Duration::from_secs(1));
    }
    assert_eq!(
        value(&host.inspect_session(&workspace(&gate)))["phase"],
        "completed"
    );
    assert_eq!(host.stop(), HostStatus::Cancelled);
    unavailable(host.inspect_session(&workspace(&gate)));

    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::RecentLogs);
    start(&host, &review, Ok(outcome(ReadKind::RecentLogs)));
    assert_eq!(terminal(&host), HostStatus::Completed);
    let closed_request = workspace(&gate);
    let close = WorkspaceClose::new(SourceAccess::close_protocol_fixture(gate.clone()));
    let closing = close.begin(|drained| host.purge_for_workspace_close(drained));
    assert_eq!(serde_json::to_value(closing).unwrap()["state"], "closing");
    assert_eq!(
        close.progress_native(|drained| host.purge_for_workspace_close(drained)),
        Ok(true)
    );
    unavailable(host.inspect_session(&closed_request));
}

#[test]
fn encoded_schema_has_exact_allowlist_and_never_leaks_canaries() {
    for kind in [
        ReadKind::ProtectedCheck,
        ReadKind::PolicyStatus,
        ReadKind::RecentLogs,
        ReadKind::RecentWatch,
    ] {
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let review = prepare(&host, &gate, kind);
        let credential_reference = host.credential_inspection(&review.id).unwrap().reference;
        start(&host, &review, Ok(outcome(kind)));
        assert_eq!(terminal(&host), HostStatus::Completed);
        let bytes = host.inspect_session(&workspace(&gate)).encode().unwrap();
        let view: Value = serde_json::from_slice(&bytes).unwrap();
        let observation = match kind {
            ReadKind::ProtectedCheck => json!({
                "kind": "protected_check", "health": "healthy", "policyLoad": "loaded"
            }),
            ReadKind::PolicyStatus => json!({
                "kind": "policy_status", "activeVersion": 7, "queriedVersion": 7,
                "loadState": "loaded", "revisionCount": 1
            }),
            ReadKind::RecentLogs => json!({
                "kind": "recent_logs", "returnedLineCount": 2, "providerBufferTotal": 99
            }),
            ReadKind::RecentWatch => json!({
                "kind": "recent_watch", "eventCount": 2,
                "kinds": {"status": 1, "log": 0, "platform": 0, "warning": 1},
                "producers": {
                    "gateway": 1, "sandbox": 0, "kubernetes": 0,
                    "docker": 0, "process": 0, "unknown": 0
                },
                "producerAbsentCount": 1, "continuity": "gap",
                "lossReasons": ["missing_native_event_id", "stream_warning"]
            }),
        };
        // Full equality fixes every nested key, scalar type and semantic value.
        // It also prevents a timestamp from being substituted for an allowed count.
        assert_eq!(
            view,
            json!({
                "schema": "rangoon.openshell.session-view.v1", "provider": "openshell",
                "productVersion": "v0.1.4", "phase": "completed", "readKind": kind_name(kind),
                "startAvailable": false, "retention": "session_only",
                "provenance": {
                    "authenticity": "unqualified", "authority": "none", "execution": "unavailable",
                    "actionOutcome": "unknown", "coverage": "bounded_partial", "omissions": [
                        "native_identifiers", "source_timestamps", "policy_hashes", "provider_payloads",
                        "target_and_credential_references"
                    ]
                },
                "observation": observation
            })
        );
        let raw = String::from_utf8(bytes).unwrap();
        let peer = review.selected_peer.to_string();
        for forbidden in [
            review.id.as_str(),
            credential_reference.as_str(),
            review.origin.as_str(),
            peer.as_str(),
            review.installation.as_str(),
            review.workspace.as_str(),
            review.sandbox.as_str(),
            review.root_sha256.as_str(),
            review.peer_pin_sha256.as_str(),
            "root-canary",
            "leaf-canary",
            "hash-canary",
            "policy-hash-canary",
            "revision-hash-canary",
            "native-id-canary",
            "cursor-canary",
            "cursor-canary-2",
            "111",
            "222",
            "333",
            "444",
            "555",
            "666",
            "777",
            "1000",
            "2000",
            "3000",
            "4000",
            "5000",
            "6000",
            "test-token",
        ] {
            assert!(!raw.contains(forbidden), "leaked forbidden fixture value");
        }
    }
}

#[test]
fn malformed_kind_mismatch_oversized_lists_and_writer_boundary_refuse_without_truncation() {
    let mismatch = SessionView::completed(ReadKind::RecentLogs, &outcome(ReadKind::ProtectedCheck));
    assert_eq!(value(&mismatch)["error"], "source_error");
    let mut many = outcome(ReadKind::RecentWatch);
    if let Observation::RecentWatch(window) = &mut many.observation {
        window.events = vec![window.events[0].clone(); 129];
    }
    let oversized = SessionView::completed(ReadKind::RecentWatch, &many);
    assert_eq!(value(&oversized)["error"], "bounds");
    assert!(value(&oversized).get("observation").is_none());
    let mut losses = outcome(ReadKind::PolicyStatus);
    losses.protected_check.protected_read.losses = vec![Loss::LogTextRedacted; 13];
    let bad_losses = SessionView::completed(ReadKind::PolicyStatus, &losses);
    assert_eq!(value(&bad_losses)["error"], "bounds");
    let mut revisions = outcome(ReadKind::PolicyStatus);
    if let Observation::PolicyStatus(policy) = &mut revisions.observation {
        policy.revisions = vec![policy.queried.clone(); 65];
    }
    assert_eq!(
        value(&SessionView::completed(ReadKind::PolicyStatus, &revisions))["error"],
        "bounds"
    );
    let mut logs = outcome(ReadKind::RecentLogs);
    if let Observation::RecentLogs(window) = &mut logs.observation {
        window.line_count = 65;
        window.source_times_millis = vec![None; 65];
        window.producers = vec![ProducerKind::Unknown; 65];
    }
    assert_eq!(
        value(&SessionView::completed(ReadKind::RecentLogs, &logs))["error"],
        "bounds"
    );
    let mut metadata = outcome(ReadKind::PolicyStatus);
    if let Observation::PolicyStatus(policy) = &mut metadata.observation {
        policy.workspace = "wrong-workspace".into();
    }
    assert_eq!(
        value(&SessionView::completed(ReadKind::PolicyStatus, &metadata))["error"],
        "source_error"
    );
    let mut writer = BoundedJson(vec![b'x'; MAX_ENCODED_BYTES - 1]);
    assert!(writer.write(b"xx").is_err());
    assert_eq!(writer.0.len(), MAX_ENCODED_BYTES - 1);
}

#[test]
fn probe_orders_snapshot_against_stop_and_generation_fence_without_contradictory_completion() {
    let gate = Gate::new();
    let host = Arc::new(Host::new(gate.clone()));
    let review = prepare(&host, &gate, ReadKind::ProtectedCheck);
    start(&host, &review, Ok(outcome(ReadKind::ProtectedCheck)));
    assert_eq!(terminal(&host), HostStatus::Completed);
    let request = workspace(&gate);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let probe = SnapshotProbe {
        before_final: Some((entered_tx, release_rx)),
        ..Default::default()
    };
    let reader = {
        let host = Arc::clone(&host);
        std::thread::spawn(move || host.inspect_session_inner(&request, Some(&probe)))
    };
    entered_rx.recv_timeout(WAIT).unwrap();
    // Host lock stays held at final witness; Stop waits. Release lets snapshot win historically.
    let stopper = {
        let host = Arc::clone(&host);
        std::thread::spawn(move || host.stop())
    };
    release_tx.send(()).unwrap();
    let snap = reader.join().unwrap();
    assert_eq!(value(&snap)["phase"], "completed");
    assert_eq!(stopper.join().unwrap(), HostStatus::Cancelled);
    unavailable(host.inspect_session(&workspace(&gate)));

    let gate = Gate::new();
    let host = Arc::new(Host::new(gate.clone()));
    let review = prepare(&host, &gate, ReadKind::RecentLogs);
    start(&host, &review, Ok(outcome(ReadKind::RecentLogs)));
    assert_eq!(terminal(&host), HostStatus::Completed);
    let request = workspace(&gate);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let probe = SnapshotProbe {
        before_final: Some((entered_tx, release_rx)),
        ..Default::default()
    };
    let reader = {
        let host = Arc::clone(&host);
        std::thread::spawn(move || host.inspect_session_inner(&request, Some(&probe)))
    };
    entered_rx.recv_timeout(WAIT).unwrap();
    let drain = gate.request_drain().unwrap();
    release_tx.send(()).unwrap();
    unavailable(reader.join().unwrap());
    assert!(host.purge_for_workspace_close(false));
    assert!(drain.is_quiescent().unwrap());
    assert!(host.purge_for_workspace_close(true));
}

#[test]
fn before_lock_stop_wins_and_dropped_inspection_reader_keeps_callback_custody() {
    let gate = Gate::new();
    let host = Arc::new(Host::new(gate.clone()));
    let review = prepare(&host, &gate, ReadKind::ProtectedCheck);
    start(&host, &review, Ok(outcome(ReadKind::ProtectedCheck)));
    assert_eq!(terminal(&host), HostStatus::Completed);
    let request = workspace(&gate);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let probe = SnapshotProbe {
        before_lock: Some((entered_tx, release_rx)),
        ..Default::default()
    };
    let reader = {
        let host = Arc::clone(&host);
        std::thread::spawn(move || host.inspect_session_inner(&request, Some(&probe)))
    };
    entered_rx.recv_timeout(WAIT).unwrap();
    assert_eq!(host.stop(), HostStatus::Cancelled);
    release_tx.send(()).unwrap();
    unavailable(reader.join().unwrap());

    let gate = Gate::new();
    let host = Arc::new(Host::new(gate.clone()));
    let review = prepare(&host, &gate, ReadKind::RecentWatch);
    let (callback_entered_tx, callback_entered_rx) = mpsc::channel();
    let (callback_release_tx, callback_release_rx) = mpsc::channel();
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            lease(),
            Box::new(ResultClient {
                kind: ReadKind::RecentWatch,
                result: Ok(outcome(ReadKind::RecentWatch)),
                entered: Some(callback_entered_tx),
                release: Some(callback_release_rx),
                returning: None
            })
        ),
        HostStatus::Running
    );
    callback_entered_rx.recv_timeout(WAIT).unwrap();
    let request = workspace(&gate);
    let (inspect_entered_tx, inspect_entered_rx) = mpsc::channel();
    let (inspect_release_tx, inspect_release_rx) = mpsc::channel();
    let probe = SnapshotProbe {
        before_lock: Some((inspect_entered_tx, inspect_release_rx)),
        ..Default::default()
    };
    let (reply_tx, reply_rx) = mpsc::channel();
    let reader = {
        let host = Arc::clone(&host);
        std::thread::spawn(move || {
            let snapshot = host.inspect_session_inner(&request, Some(&probe));
            assert!(reply_tx.send(snapshot).is_err(), "waiter must be gone");
        })
    };
    inspect_entered_rx.recv_timeout(WAIT).unwrap();
    drop(reply_rx);
    inspect_release_tx.send(()).unwrap();
    reader.join().unwrap();
    assert_eq!(host.stop(), HostStatus::Closing);
    callback_release_tx.send(()).unwrap();
    assert_eq!(terminal(&host), HostStatus::Cancelled);
    assert!(gate.request_drain().unwrap().is_quiescent().unwrap());
}

#[test]
fn callback_finish_blocked_while_stop_waits_resolves_to_cancelled_without_observation() {
    let gate = Gate::new();
    let host = Arc::new(Host::new(gate.clone()));
    let review = prepare(&host, &gate, ReadKind::RecentLogs);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (returning_tx, returning_rx) = mpsc::channel();
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            lease(),
            Box::new(ResultClient {
                kind: ReadKind::RecentLogs,
                result: Ok(outcome(ReadKind::RecentLogs)),
                entered: Some(entered_tx),
                release: Some(release_rx),
                returning: Some(returning_tx)
            })
        ),
        HostStatus::Running
    );
    entered_rx.recv_timeout(WAIT).unwrap();
    let active = host.state.lock().unwrap().active.as_ref().unwrap().clone();
    let active_guard = active.state.lock().unwrap();
    release_tx.send(()).unwrap();
    returning_rx.recv_timeout(WAIT).unwrap();
    let (stopped_tx, stopped_rx) = mpsc::channel();
    let stopper = {
        let host = Arc::clone(&host);
        std::thread::spawn(move || {
            stopped_tx.send(host.stop()).unwrap();
        })
    };
    // Stop sets cancellation only after obtaining Host custody and before
    // waiting for Active. This flag proves that ordering while both Stop and
    // callback completion remain blocked by the held Active guard.
    let until = Instant::now() + WAIT;
    while !active.cancelled.load(std::sync::atomic::Ordering::Acquire) {
        assert!(
            Instant::now() < until,
            "Stop did not reach Active cancellation"
        );
        std::thread::yield_now();
    }
    assert!(matches!(
        stopped_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    drop(active_guard);
    assert!(matches!(
        stopped_rx.recv_timeout(WAIT).unwrap(),
        HostStatus::Closing | HostStatus::Cancelled
    ));
    stopper.join().unwrap();
    assert!(matches!(
        terminal(&host),
        HostStatus::Cancelled | HostStatus::Unavailable
    ));
    let view = value(&host.inspect_session(&workspace(&gate)));
    assert!(matches!(
        view["phase"].as_str(),
        Some("cancelled") | Some("unavailable")
    ));
    assert!(view.get("observation").is_none());
}

struct RealClient;
impl SyntheticReadClient for RealClient {
    fn run(
        self: Box<Self>,
        config: ClientConfig,
        lease: BearerLease,
        kind: ReadKind,
        control: WatchControl,
    ) -> Result<ReadOutcome, ClientError> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let mut client = OpenShellReadClient::connect(config, lease).await?;
                let protected_check = client.check_protected_read().await?;
                let observation = match kind {
                    ReadKind::ProtectedCheck => {
                        Observation::ProtectedCheck(protected_check.clone())
                    }
                    ReadKind::PolicyStatus => {
                        Observation::PolicyStatus(client.read_policy_status(0).await?)
                    }
                    ReadKind::RecentLogs => {
                        Observation::RecentLogs(client.read_recent_logs().await?)
                    }
                    ReadKind::RecentWatch => Observation::RecentWatch(
                        client
                            .watch_recent(&control, None, Duration::from_secs(30))
                            .await?,
                    ),
                };
                Ok(ReadOutcome {
                    protected_check,
                    observation,
                })
            })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_tls_callbacks_have_exact_ledger_and_repeated_inspect_adds_no_rpc() {
    use rangoon_openshell_client::synthetic_peer::{
        SyntheticPeer, SyntheticRpc, SyntheticScenario,
    };
    for (kind, ledger) in [
        (
            ReadKind::ProtectedCheck,
            vec![SyntheticRpc::Health, SyntheticRpc::GetSandboxPolicyStatus],
        ),
        (
            ReadKind::PolicyStatus,
            vec![
                SyntheticRpc::Health,
                SyntheticRpc::GetSandboxPolicyStatus,
                SyntheticRpc::GetSandboxPolicyStatus,
                SyntheticRpc::ListSandboxPolicies,
            ],
        ),
        (
            ReadKind::RecentLogs,
            vec![
                SyntheticRpc::Health,
                SyntheticRpc::GetSandboxPolicyStatus,
                SyntheticRpc::GetSandboxLogs,
            ],
        ),
        (
            ReadKind::RecentWatch,
            vec![
                SyntheticRpc::Health,
                SyntheticRpc::GetSandboxPolicyStatus,
                SyntheticRpc::WatchSandbox,
            ],
        ),
    ] {
        let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
            .await
            .unwrap();
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let request = workspace(&gate);
        let review = host
            .prepare(
                &request,
                ProposedTarget::new(
                    peer.origin().into(),
                    peer.selected_peer(),
                    "installation-1".into(),
                    "workspace-1".into(),
                    "sandbox-1".into(),
                    peer.root_der().to_vec(),
                    peer.leaf_der().to_vec(),
                )
                .unwrap(),
                kind,
                Retention::SessionOnly,
            )
            .unwrap();
        assert_eq!(
            host.confirm(&review.id, true),
            HostStatus::ReadyForNativeCredential
        );
        assert_eq!(host.begin_production(&review.id), HostStatus::Unavailable);
        assert_eq!(
            host.begin_synthetic(&review.id, lease(), Box::new(RealClient)),
            HostStatus::Running
        );
        let deadline = Instant::now() + WAIT;
        while matches!(host.status(), HostStatus::Running | HostStatus::Closing) {
            assert!(Instant::now() < deadline);
            tokio::task::yield_now().await;
        }
        assert_eq!(host.status(), HostStatus::Completed);
        assert_eq!(peer.calls().unwrap(), ledger);
        for _ in 0..3 {
            let view = value(&host.inspect_session(&request));
            assert_eq!(view["phase"], "completed");
            assert_eq!(view["readKind"], kind_name(kind));
        }
        assert_eq!(peer.calls().unwrap(), ledger);
        peer.shutdown().await.unwrap();
    }
}
