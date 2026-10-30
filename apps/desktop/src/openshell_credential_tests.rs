use super::super::credential::{CredentialAcquirer, CredentialClaim, CredentialInspection};
use super::*;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

macro_rules! assert_not_impl {
    ($owner:ty: $trait:path) => {
        const _: fn() = || {
            trait AmbiguousIfImpl<Marker> {
                fn marker() {}
            }
            impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
            struct Invalid;
            impl<T: ?Sized + $trait> AmbiguousIfImpl<Invalid> for T {}
            let _ = <$owner as AmbiguousIfImpl<_>>::marker;
        };
    };
}

assert_not_impl!(super::super::credential::CredentialSelection: Clone);
assert_not_impl!(super::super::credential::CredentialSelection: std::fmt::Debug);
assert_not_impl!(super::super::credential::CredentialSelection: serde::Serialize);
assert_not_impl!(super::super::credential::CredentialClaim: Clone);
assert_not_impl!(super::super::credential::CredentialClaim: std::fmt::Debug);
assert_not_impl!(super::super::credential::CredentialClaim: serde::Serialize);
assert_not_impl!(super::super::credential::AcquiredCredential: Clone);
assert_not_impl!(super::super::credential::AcquiredCredential: std::fmt::Debug);
assert_not_impl!(super::super::credential::AcquiredCredential: serde::Serialize);
assert_not_impl!(rangoon_native_credential_input::SecretInput: serde::Serialize);

struct SealingAcquirer {
    value: &'static str,
    source_expiry: Option<Instant>,
    called: Option<mpsc::Sender<Instant>>,
}

impl CredentialAcquirer for SealingAcquirer {
    fn acquire(
        self: Box<Self>,
        claim: CredentialClaim,
    ) -> Result<super::super::credential::AcquiredCredential, ClientError> {
        let deadline = claim.deadline();
        if let Some(called) = self.called {
            let _ = called.send(deadline);
        }
        claim.seal(Zeroizing::new(self.value.to_owned()), self.source_expiry)
    }
}

struct ReportingAcquirer {
    source_expiry: Instant,
    acquired: mpsc::Sender<Instant>,
}

impl CredentialAcquirer for ReportingAcquirer {
    fn acquire(
        self: Box<Self>,
        claim: CredentialClaim,
    ) -> Result<super::super::credential::AcquiredCredential, ClientError> {
        let acquired = claim.seal(
            Zeroizing::new("test-token".to_owned()),
            Some(self.source_expiry),
        )?;
        self.acquired.send(acquired.deadline()).unwrap();
        Ok(acquired)
    }
}

struct FailingAcquirer {
    called: mpsc::Sender<()>,
    error: ClientError,
}

impl CredentialAcquirer for FailingAcquirer {
    fn acquire(
        self: Box<Self>,
        _claim: CredentialClaim,
    ) -> Result<super::super::credential::AcquiredCredential, ClientError> {
        let _ = self.called.send(());
        Err(self.error)
    }
}

struct HeldAcquirer {
    started: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}

impl CredentialAcquirer for HeldAcquirer {
    fn acquire(
        self: Box<Self>,
        claim: CredentialClaim,
    ) -> Result<super::super::credential::AcquiredCredential, ClientError> {
        self.started.send(()).unwrap();
        let _ = self.release.recv();
        claim.seal(Zeroizing::new("test-token".to_owned()), None)
    }
}

struct SealedHeldAcquirer {
    source_expiry: Instant,
    sealed: mpsc::Sender<Instant>,
    release: mpsc::Receiver<()>,
}

impl CredentialAcquirer for SealedHeldAcquirer {
    fn acquire(
        self: Box<Self>,
        claim: CredentialClaim,
    ) -> Result<super::super::credential::AcquiredCredential, ClientError> {
        let acquired = claim.seal(
            Zeroizing::new("test-token".to_owned()),
            Some(self.source_expiry),
        )?;
        self.sealed.send(acquired.deadline()).unwrap();
        let _ = self.release.recv();
        Ok(acquired)
    }
}

struct PanickingAcquirer {
    started: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}

impl CredentialAcquirer for PanickingAcquirer {
    fn acquire(
        self: Box<Self>,
        _claim: CredentialClaim,
    ) -> Result<super::super::credential::AcquiredCredential, ClientError> {
        self.started.send(()).unwrap();
        let _ = self.release.recv();
        panic!("synthetic credential callback failure");
    }
}

fn inspection(host: &Host, review: &Review) -> CredentialInspection {
    host.credential_inspection(&review.id)
        .expect("selection inspection must exist with pending review")
}

fn confirm(host: &Host, review: &Review) -> CredentialInspection {
    let inspection = inspection(host, review);
    let before_confirm = {
        let state = host.state.lock().unwrap();
        match state.pending.as_ref().unwrap() {
            Pending::Awaiting { selection, .. } => selection.deadline,
            Pending::Confirmed { .. } => unreachable!(),
        }
    };
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    let after_confirm = {
        let state = host.state.lock().unwrap();
        match state.pending.as_ref().unwrap() {
            Pending::Confirmed { selection, .. } => selection.deadline,
            Pending::Awaiting { .. } => unreachable!(),
        }
    };
    assert_eq!(after_confirm, before_confirm);
    inspection
}

fn no_run_client(kind: ReadKind) -> Box<dyn SyntheticReadClient> {
    Box::new(SyntheticClient {
        expected_kind: kind,
        result: Err(ClientError::Source),
        started: None,
        release: None,
    })
}

#[test]
fn inspection_is_context_only_and_cannot_acquire_before_consent() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::ProtectedCheck);
    let selected = inspection(&host, &review);
    assert_eq!(selected.review, review);
    assert_ne!(selected.reference, review.id);
    assert_eq!(
        host.begin_production(&review.id),
        HostStatus::AwaitingConsent
    );

    let (called, received) = mpsc::channel();
    assert_eq!(
        host.begin_acquisition(
            &selected,
            Some(Box::new(SealingAcquirer {
                value: "test-token",
                source_expiry: None,
                called: Some(called),
            })),
            no_run_client(ReadKind::ProtectedCheck),
        ),
        HostStatus::AwaitingConsent
    );
    assert!(received.recv_timeout(Duration::from_millis(50)).is_err());
    assert_eq!(host.inspect_review(), Some(review.clone()));
    assert_eq!(host.stop(), HostStatus::Cancelled);
    assert!(gate.request_drain().unwrap().is_quiescent().unwrap());
}

#[test]
fn denied_native_confirmation_never_acquires_from_prior_inspection() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::ProtectedCheck);
    let selected = inspection(&host, &review);
    assert_eq!(host.confirm(&review.id, false), HostStatus::Cancelled);
    let (called, received) = mpsc::channel();
    assert_eq!(
        host.begin_acquisition(
            &selected,
            Some(Box::new(SealingAcquirer {
                value: "test-token",
                source_expiry: None,
                called: Some(called),
            })),
            no_run_client(ReadKind::ProtectedCheck),
        ),
        HostStatus::Unavailable
    );
    assert!(received.recv_timeout(Duration::from_millis(50)).is_err());
    assert!(gate.request_drain().unwrap().is_quiescent().unwrap());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn acquisition_uses_exact_inspection_and_real_tls_client_path() {
    let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let (workspace, review) = real_review(&host, &gate, &peer, ReadKind::ProtectedCheck);
    let selected = confirm(&host, &review);
    assert_eq!(
        host.begin_acquisition(
            &selected,
            Some(Box::new(SealingAcquirer {
                value: "test-token",
                source_expiry: None,
                called: None,
            })),
            Box::<RealReadClient>::default(),
        ),
        HostStatus::Running
    );
    assert_eq!(real_terminal_status(&host).await, HostStatus::Completed);
    assert_eq!(
        peer.calls().unwrap(),
        vec![SyntheticRpc::Health, SyntheticRpc::GetSandboxPolicyStatus]
    );
    assert!(host.completed_observation(&workspace, &review.id).is_some());
    assert_eq!(host.begin_production(&review.id), HostStatus::Unavailable);
    assert_eq!(host.stop(), HostStatus::Cancelled);
    peer.shutdown().await.unwrap();
}

#[test]
fn changed_inspection_context_refuses_without_consuming_valid_selection() {
    type InspectionChange = Box<dyn Fn(&mut CredentialInspection)>;
    let changes: Vec<InspectionChange> = vec![
        Box::new(|candidate| candidate.reference.push('x')),
        Box::new(|candidate| candidate.review.id.push('x')),
        Box::new(|candidate| candidate.review.origin.push('x')),
        Box::new(|candidate| candidate.review.selected_peer.set_port(444)),
        Box::new(|candidate| candidate.review.installation.push('x')),
        Box::new(|candidate| candidate.review.workspace.push('x')),
        Box::new(|candidate| candidate.review.sandbox.push('x')),
        Box::new(|candidate| candidate.review.product_version = "v0.1.4-changed"),
        Box::new(|candidate| candidate.review.root_sha256.push('x')),
        Box::new(|candidate| candidate.review.peer_pin_sha256.push('x')),
        Box::new(|candidate| candidate.review.kind = ReadKind::RecentLogs),
        Box::new(|candidate| candidate.review.data_classes.push(DataClass::RedactedLog)),
    ];

    for change in changes {
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let review = prepare(&host, &gate, ReadKind::ProtectedCheck);
        let selected = confirm(&host, &review);
        let mut candidate = selected.clone();
        change(&mut candidate);
        let (called, received) = mpsc::channel();
        assert_eq!(
            host.begin_acquisition(
                &candidate,
                Some(Box::new(SealingAcquirer {
                    value: "test-token",
                    source_expiry: None,
                    called: Some(called),
                })),
                no_run_client(ReadKind::ProtectedCheck),
            ),
            HostStatus::Unavailable
        );
        assert!(received.recv_timeout(Duration::from_millis(50)).is_err());
        assert!(
            host.credential_inspection(&review.id)
                .is_some_and(|current| current == selected)
        );
        assert_eq!(host.stop(), HostStatus::Cancelled);
    }
}

#[test]
fn foreign_host_and_stale_generation_refuse_without_acquisition() {
    let first_gate = Gate::new();
    let first = Host::new(first_gate.clone());
    let first_review = prepare(&first, &first_gate, ReadKind::ProtectedCheck);
    let first_selection = confirm(&first, &first_review);
    let second_gate = Gate::new();
    let second = Host::new(second_gate.clone());
    let second_review = prepare(&second, &second_gate, ReadKind::ProtectedCheck);
    let _second_selection = confirm(&second, &second_review);
    let (called, received) = mpsc::channel();
    assert_eq!(
        second.begin_acquisition(
            &first_selection,
            Some(Box::new(SealingAcquirer {
                value: "test-token",
                source_expiry: None,
                called: Some(called),
            })),
            no_run_client(ReadKind::ProtectedCheck),
        ),
        HostStatus::Unavailable
    );
    assert!(received.recv_timeout(Duration::from_millis(50)).is_err());
    assert!(second.credential_inspection(&second_review.id).is_some());
    let _drain = first_gate.request_drain().unwrap();
    let (called, received) = mpsc::channel();
    assert_eq!(
        first.begin_acquisition(
            &first_selection,
            Some(Box::new(SealingAcquirer {
                value: "test-token",
                source_expiry: None,
                called: Some(called),
            })),
            no_run_client(ReadKind::ProtectedCheck),
        ),
        HostStatus::Unavailable
    );
    assert!(received.recv_timeout(Duration::from_millis(50)).is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn success_and_failure_consume_same_inspection_once() {
    let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
        .await
        .unwrap();
    for succeeds in [true, false] {
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let (_, review) = real_review(&host, &gate, &peer, ReadKind::ProtectedCheck);
        let selected = confirm(&host, &review);
        let acquirer: Box<dyn CredentialAcquirer> = if succeeds {
            Box::new(SealingAcquirer {
                value: "test-token",
                source_expiry: None,
                called: None,
            })
        } else {
            let (called, received) = mpsc::channel();
            let acquirer = Box::new(FailingAcquirer {
                called,
                error: ClientError::PermissionDenied,
            });
            drop(received);
            acquirer
        };
        assert_eq!(
            host.begin_acquisition(&selected, Some(acquirer), Box::<RealReadClient>::default()),
            HostStatus::Running
        );
        assert_eq!(
            real_terminal_status(&host).await,
            if succeeds {
                HostStatus::Completed
            } else {
                HostStatus::Error(ClientError::Source)
            }
        );
        assert_eq!(
            host.begin_acquisition(
                &selected,
                Some(Box::new(SealingAcquirer {
                    value: "test-token",
                    source_expiry: None,
                    called: None,
                })),
                Box::<RealReadClient>::default(),
            ),
            HostStatus::Unavailable
        );
    }
    assert_eq!(
        peer.calls().unwrap(),
        vec![SyntheticRpc::Health, SyntheticRpc::GetSandboxPolicyStatus]
    );
    peer.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn absent_backend_maps_to_fixed_source_error_without_rpc() {
    let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let (_, review) = real_review(&host, &gate, &peer, ReadKind::ProtectedCheck);
    let selected = confirm(&host, &review);
    assert_eq!(
        host.begin_acquisition(&selected, None, Box::<RealReadClient>::default()),
        HostStatus::Running
    );
    assert_eq!(
        real_terminal_status(&host).await,
        HostStatus::Error(ClientError::Source)
    );
    assert!(peer.calls().unwrap().is_empty());
    assert_eq!(ClientError::Source.code(), "source_error");
    peer.shutdown().await.unwrap();
}

async fn start_held_acquisition() -> (SyntheticPeer, Gate, Host, Review, mpsc::Sender<()>) {
    let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let (_, review) = real_review(&host, &gate, &peer, ReadKind::ProtectedCheck);
    let selected = confirm(&host, &review);
    let (started, entered) = mpsc::channel();
    let (release, held) = mpsc::channel();
    assert_eq!(
        host.begin_acquisition(
            &selected,
            Some(Box::new(HeldAcquirer {
                started,
                release: held,
            })),
            Box::<RealReadClient>::default(),
        ),
        HostStatus::Running
    );
    entered.recv_timeout(Duration::from_secs(1)).unwrap();
    (peer, gate, host, review, release)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn held_acquisition_stop_retains_admission_then_drops_late_secret_without_rpc() {
    let (peer, gate, host, _review, release) = start_held_acquisition().await;
    assert_eq!(host.stop(), HostStatus::Closing);
    let drain = gate.request_drain().unwrap();
    assert!(!drain.is_quiescent().unwrap());
    release.send(()).unwrap();
    assert_eq!(real_terminal_status(&host).await, HostStatus::Cancelled);
    assert!(drain.is_quiescent().unwrap());
    assert!(peer.calls().unwrap().is_empty());
    peer.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn held_acquisition_workspace_close_waits_for_actual_callback_return() {
    let (peer, gate, host, _review, release) = start_held_acquisition().await;
    let close = WorkspaceClose::new(SourceAccess::close_protocol_fixture(gate.clone()));
    let closing = close.begin(|drained| host.purge_for_workspace_close(drained));
    assert_eq!(serde_json::to_value(closing).unwrap()["state"], "closing");
    assert_eq!(
        close.progress_native(|drained| host.purge_for_workspace_close(drained)),
        Ok(false)
    );
    release.send(()).unwrap();
    assert_eq!(real_terminal_status(&host).await, HostStatus::Cancelled);
    assert_eq!(
        close.progress_native(|drained| host.purge_for_workspace_close(drained)),
        Ok(true)
    );
    assert!(peer.calls().unwrap().is_empty());
    peer.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sealed_credential_expiry_after_acquisition_blocks_client_without_early_fence() {
    let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let (_, review) = real_review(&host, &gate, &peer, ReadKind::ProtectedCheck);
    let selected = confirm(&host, &review);
    let source_expiry = Instant::now() + Duration::from_secs(1);
    let (sealed, acquired_deadline) = mpsc::channel();
    let (release, held) = mpsc::channel();
    assert_eq!(
        host.begin_acquisition(
            &selected,
            Some(Box::new(SealedHeldAcquirer {
                source_expiry,
                sealed,
                release: held,
            })),
            Box::<RealReadClient>::default(),
        ),
        HostStatus::Running
    );
    let deadline = acquired_deadline
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    assert_eq!(deadline, source_expiry);
    assert!(
        host.state
            .lock()
            .unwrap()
            .active
            .as_ref()
            .is_some_and(|active| { active.state.lock().unwrap().admission.is_some() })
    );
    tokio::time::sleep_until(tokio::time::Instant::from_std(deadline) + Duration::from_millis(20))
        .await;
    assert_eq!(host.status(), HostStatus::Running);
    assert!(
        host.state
            .lock()
            .unwrap()
            .active
            .as_ref()
            .is_some_and(|active| { active.state.lock().unwrap().admission.is_some() })
    );
    release.send(()).unwrap();
    assert_eq!(
        real_terminal_status(&host).await,
        HostStatus::Error(ClientError::CredentialLeaseExpired)
    );
    assert!(gate.request_drain().unwrap().is_quiescent().unwrap());
    assert!(peer.calls().unwrap().is_empty());
    peer.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropped_host_handles_do_not_release_held_acquisition_admission() {
    let (peer, gate, host, _review, release) = start_held_acquisition().await;
    let clone = host.clone();
    drop(host);
    drop(clone);
    let drain = gate.request_drain().unwrap();
    assert!(!drain.is_quiescent().unwrap());
    release.send(()).unwrap();
    for _ in 0..100 {
        if drain.is_quiescent().unwrap() {
            assert!(peer.calls().unwrap().is_empty());
            peer.shutdown().await.unwrap();
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("dropped host handles must not detach acquisition callback custody");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn held_acquisition_native_exit_coordinator_waits_for_callback_return() {
    use crate::workspace_close::failure::{ExitAction, ExitCoordinator, route_exit_request};

    let (peer, gate, host, _review, release) = start_held_acquisition().await;
    let source = SourceAccess::close_protocol_fixture(gate.clone());
    let close = WorkspaceClose::new(source.clone());
    let exit = ExitCoordinator::new();
    assert_eq!(
        route_exit_request(Some(71), Some((&close, &exit)), || {
            assert!(gate.observe_open().is_err());
            host.cancel_and_check_exit()
        }),
        ExitAction::PreventAndWait
    );
    assert!(!source.request_close().unwrap().is_quiescent().unwrap());
    release.send(()).unwrap();
    assert_eq!(real_terminal_status(&host).await, HostStatus::Cancelled);
    assert!(source.request_close().unwrap().is_quiescent().unwrap());
    assert_eq!(
        route_exit_request(Some(71), Some((&close, &exit)), || host
            .cancel_and_check_exit()),
        ExitAction::PreventAndStart
    );
    close.begin(|drained| host.purge_for_workspace_close(drained));
    assert_eq!(
        close.progress_native(|drained| host.purge_for_workspace_close(drained)),
        Ok(true)
    );
    assert!(exit.send(0));
    assert_eq!(
        route_exit_request(Some(0), Some((&close, &exit)), || host
            .cancel_and_check_exit()),
        ExitAction::Pass
    );
    assert!(peer.calls().unwrap().is_empty());
    peer.shutdown().await.unwrap();
}

#[test]
fn expired_selection_calls_neither_acquirer_nor_client_and_cannot_renew() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::ProtectedCheck);
    let selected = confirm(&host, &review);
    {
        let mut state = host.state.lock().unwrap();
        let Some(Pending::Confirmed { selection, .. }) = state.pending.as_mut() else {
            panic!("confirmed pending selection expected");
        };
        selection.deadline = Instant::now() - Duration::from_millis(1);
    }
    let (called, received) = mpsc::channel();
    assert_eq!(
        host.begin_acquisition(
            &selected,
            Some(Box::new(SealingAcquirer {
                value: "test-token",
                source_expiry: Some(Instant::now() + Duration::from_secs(30)),
                called: Some(called),
            })),
            no_run_client(ReadKind::ProtectedCheck),
        ),
        HostStatus::Error(ClientError::CredentialLeaseExpired)
    );
    assert!(received.recv_timeout(Duration::from_millis(50)).is_err());
    assert_eq!(host.status(), HostStatus::Unavailable);
}

#[test]
fn acquisition_expiry_only_shortens_claim_deadline() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::ProtectedCheck);
    let selected = confirm(&host, &review);
    let expected = {
        let state = host.state.lock().unwrap();
        match state.pending.as_ref().unwrap() {
            Pending::Confirmed { selection, .. } => selection.deadline,
            Pending::Awaiting { .. } => unreachable!(),
        }
    };
    let (reported, received) = mpsc::channel();
    assert_eq!(
        host.begin_acquisition(
            &selected,
            Some(Box::new(ReportingAcquirer {
                source_expiry: expected + Duration::from_secs(10),
                acquired: reported,
            })),
            no_run_client(ReadKind::ProtectedCheck),
        ),
        HostStatus::Running
    );
    assert_eq!(
        received.recv_timeout(Duration::from_secs(1)).unwrap(),
        expected
    );
    assert_eq!(finish_status(&host), HostStatus::Error(ClientError::Source));
}

fn confirmed_selection_for_seal() -> super::super::credential::CredentialSelection {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::ProtectedCheck);
    let _selected = confirm(&host, &review);
    let mut state = host.state.lock().unwrap();
    let Some(Pending::Confirmed { selection, .. }) = state.pending.take() else {
        panic!("confirmed pending selection expected");
    };
    selection
}

#[test]
fn credential_claim_seal_enforces_exact_secret_byte_bound() {
    assert!(matches!(
        confirmed_selection_for_seal()
            .claim()
            .seal(Zeroizing::new(String::new()), None),
        Err(ClientError::Source)
    ));
    assert!(matches!(
        confirmed_selection_for_seal()
            .claim()
            .seal(Zeroizing::new("x".repeat(16 * 1024 + 1)), None),
        Err(ClientError::Source)
    ));
    assert!(
        confirmed_selection_for_seal()
            .claim()
            .seal(Zeroizing::new("x".repeat(16 * 1024)), None)
            .is_ok()
    );
    for value in [
        "non-ascii-ñ".to_owned(),
        "line\nbreak".to_owned(),
        "tab\tvalue".to_owned(),
        "delete\u{7f}value".to_owned(),
    ] {
        assert!(matches!(
            confirmed_selection_for_seal()
                .claim()
                .seal(Zeroizing::new(value), None),
            Err(ClientError::Source)
        ));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expiry_after_real_client_result_rejects_late_observation() {
    let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let (workspace, review) = real_review(&host, &gate, &peer, ReadKind::ProtectedCheck);
    let selected = confirm(&host, &review);
    let (returned, result_received) = tokio::sync::oneshot::channel();
    let (reported, acquired_deadline) = mpsc::channel();
    let (release, held) = mpsc::channel();
    let source_expiry = Instant::now() + Duration::from_secs(1);
    assert_eq!(
        host.begin_acquisition(
            &selected,
            Some(Box::new(ReportingAcquirer {
                source_expiry,
                acquired: reported,
            })),
            Box::new(RealReadClient {
                returned: Some(returned),
                release: Some(held),
            }),
        ),
        HostStatus::Running
    );
    assert!(
        tokio::time::timeout(Duration::from_secs(5), result_received)
            .await
            .expect("real client result must arrive within harness bound")
            .unwrap()
            .is_ok()
    );
    let deadline = acquired_deadline
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    assert_eq!(deadline, source_expiry);
    tokio::time::sleep_until(tokio::time::Instant::from_std(deadline) + Duration::from_millis(20))
        .await;
    release.send(()).unwrap();
    assert_eq!(
        real_terminal_status(&host).await,
        HostStatus::Error(ClientError::CredentialLeaseExpired)
    );
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    peer.shutdown().await.unwrap();
}

#[test]
fn sealed_lease_debug_is_fixed_redaction() {
    let lease = BearerLease::new(
        "c0-test-secret-must-not-project".to_owned(),
        Instant::now() + Duration::from_secs(1),
    )
    .unwrap();
    let debug = format!("{lease:?}");
    assert_eq!(debug, "BearerLease([redacted])");
    assert!(!debug.contains("c0-test-secret-must-not-project"));
}

#[test]
fn panic_in_acquirer_keeps_drain_until_callback_unwinds() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::ProtectedCheck);
    let selected = confirm(&host, &review);
    let (started, entered) = mpsc::channel();
    let (release, held) = mpsc::channel();
    assert_eq!(
        host.begin_acquisition(
            &selected,
            Some(Box::new(PanickingAcquirer {
                started,
                release: held,
            })),
            no_run_client(ReadKind::ProtectedCheck),
        ),
        HostStatus::Running
    );
    entered.recv_timeout(Duration::from_secs(1)).unwrap();
    let drain = gate.request_drain().unwrap();
    assert!(!drain.is_quiescent().unwrap());
    release.send(()).unwrap();
    assert_eq!(finish_status(&host), HostStatus::Error(ClientError::Source));
    assert!(drain.is_quiescent().unwrap());
}
