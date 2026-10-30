use super::*;
use crate::workspace_requests::WorkspaceRequest;
use crate::{SourceAccess, workspace_close::WorkspaceClose};
use rangoon_desktop::workspace_lifecycle::{Gate, Lane};
use rangoon_openshell_client::{HealthStatus, PolicyLoadStatus, PolicyRevision};
use rangoon_openshell_client::{
    OpenShellReadClient,
    synthetic_peer::{SyntheticPeer, SyntheticRpc, SyntheticScenario},
};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

struct SyntheticClient {
    expected_kind: ReadKind,
    result: Result<ReadOutcome, ClientError>,
    started: Option<mpsc::Sender<(ReadKind, String)>>,
    release: Option<mpsc::Receiver<()>>,
}

impl SyntheticReadClient for SyntheticClient {
    fn run(
        mut self: Box<Self>,
        config: ClientConfig,
        lease: BearerLease,
        kind: ReadKind,
        _control: WatchControl,
    ) -> Result<ReadOutcome, ClientError> {
        assert_eq!(kind, self.expected_kind);
        let debug = format!("{config:?} {lease:?}");
        assert!(!debug.contains("dummy-bearer-secret"));
        if let Some(started) = self.started.take() {
            let _ = started.send((kind, debug));
        }
        if let Some(release) = self.release.take() {
            // Sender drop also releases this wait if the test exits early.
            let _ = release.recv();
        }
        self.result
    }
}

struct PanickingClient {
    started: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}

impl SyntheticReadClient for PanickingClient {
    fn run(
        self: Box<Self>,
        _config: ClientConfig,
        _lease: BearerLease,
        _kind: ReadKind,
        _control: WatchControl,
    ) -> Result<ReadOutcome, ClientError> {
        self.started.send(()).unwrap();
        let _ = self.release.recv();
        panic!("synthetic callback failure");
    }
}

fn begin_panicking_callback(host: &Host, gate: &Gate) -> (Review, mpsc::Sender<()>) {
    let review = prepare(host, gate, ReadKind::RecentWatch);
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    let (started, received) = mpsc::channel();
    let (release, release_received) = mpsc::channel();
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            dummy_lease(),
            Box::new(PanickingClient {
                started,
                release: release_received,
            }),
        ),
        HostStatus::Running
    );
    received.recv_timeout(Duration::from_secs(1)).unwrap();
    (review, release)
}

#[test]
fn callback_panic_finishes_without_observation_or_abandoned_admission() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let (review, release) = begin_panicking_callback(&host, &gate);
    let workspace = WorkspaceRequest::Managed(Arc::new(gate.observe_open().unwrap()));
    let drain = gate.request_drain().unwrap();
    assert!(!drain.is_quiescent().unwrap());
    release.send(()).unwrap();
    assert_eq!(finish_status(&host), HostStatus::Error(ClientError::Source));
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    assert!(drain.is_quiescent().unwrap());
    assert!(host.cancel_and_check_exit());
    assert_eq!(host.status(), HostStatus::Unavailable);
}

#[test]
fn callback_panic_during_workspace_close_preserves_cancel_and_drains() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let close = WorkspaceClose::new(SourceAccess::close_protocol_fixture(gate.clone()));
    let (_, release) = begin_panicking_callback(&host, &gate);
    let closing = close.begin(|drained| host.purge_for_workspace_close(drained));
    assert_eq!(serde_json::to_value(closing).unwrap()["state"], "closing");
    assert_eq!(
        close.progress_native(|drained| host.purge_for_workspace_close(drained)),
        Ok(false)
    );
    release.send(()).unwrap();
    assert_eq!(finish_status(&host), HostStatus::Cancelled);
    assert_eq!(
        close.progress_native(|drained| host.purge_for_workspace_close(drained)),
        Ok(true)
    );
    assert_eq!(host.status(), HostStatus::Unavailable);
    assert!(gate.observe_open().is_err());
}

fn target() -> ProposedTarget {
    let certificate = b"synthetic-root".to_vec();
    ProposedTarget::new(
        "https://openshell.test".to_owned(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 443),
        "installation-1".to_owned(),
        "workspace-1".to_owned(),
        "sandbox-1".to_owned(),
        certificate,
        b"synthetic-leaf".to_vec(),
    )
    .unwrap()
}

#[test]
fn oversized_origin_refuses_before_review_allocation() {
    assert!(matches!(
        ProposedTarget::new(
            format!("https://{}.test", "a".repeat(MAX_ORIGIN_BYTES)),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 443),
            "installation-1".to_owned(),
            "workspace-1".to_owned(),
            "sandbox-1".to_owned(),
            b"synthetic-root".to_vec(),
            b"synthetic-leaf".to_vec(),
        ),
        Err(ClientError::InvalidConfiguration)
    ));
}

fn dummy_lease() -> BearerLease {
    BearerLease::new(
        "dummy-bearer-secret".to_owned(),
        Instant::now() + Duration::from_secs(30),
    )
    .unwrap()
}

fn protected_check(workspace: &str) -> ConnectionCheck {
    ConnectionCheck {
        authenticity: Authenticity::Unqualified,
        installation: "installation-1".to_owned(),
        workspace: workspace.to_owned(),
        sandbox: "sandbox-1".to_owned(),
        reported_product_version: PRODUCT_VERSION.to_owned(),
        health: HealthStatus::Healthy,
        protected_read: PolicyRevision {
            version: 7,
            policy_hash: None,
            status: PolicyLoadStatus::Loaded,
            created_time_millis: None,
            loaded_time_millis: None,
            losses: Vec::new(),
        },
        received_time_millis: 12,
    }
}

fn protected_observation(workspace: &str) -> ReadOutcome {
    let check = protected_check(workspace);
    ReadOutcome {
        protected_check: check.clone(),
        observation: Observation::ProtectedCheck(check),
    }
}

fn prepare(host: &Host, gate: &Gate, kind: ReadKind) -> Review {
    let witness = gate.observe_open().unwrap();
    host.prepare(
        &WorkspaceRequest::Managed(Arc::new(witness)),
        target(),
        kind,
        Retention::SessionOnly,
    )
    .unwrap()
}

fn finish_status(host: &Host) -> HostStatus {
    for _ in 0..100 {
        let status = host.status();
        if !matches!(status, HostStatus::Running | HostStatus::Closing) {
            return status;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    host.status()
}

// This adapter exists only inside the host test module. It uses the actual
// pinned read client; neither the result nor a cancellation error is canned.
#[derive(Default)]
struct RealReadClient {
    returned: Option<tokio::sync::oneshot::Sender<Result<ReadOutcome, ClientError>>>,
    release: Option<mpsc::Receiver<()>>,
}

impl SyntheticReadClient for RealReadClient {
    fn run(
        mut self: Box<Self>,
        config: ClientConfig,
        lease: BearerLease,
        kind: ReadKind,
        control: WatchControl,
    ) -> Result<ReadOutcome, ClientError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("synthetic client runtime");
        let result = runtime.block_on(async move {
            let mut client = OpenShellReadClient::connect(config, lease).await?;
            let protected_check = client.check_protected_read().await?;
            let observation = match kind {
                ReadKind::ProtectedCheck => Observation::ProtectedCheck(protected_check.clone()),
                ReadKind::PolicyStatus => {
                    Observation::PolicyStatus(client.read_policy_status(0).await?)
                }
                ReadKind::RecentLogs => Observation::RecentLogs(client.read_recent_logs().await?),
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
        });
        // Drop the client task runtime before reporting the real transport
        // result. A separate hold demonstrates host admission remains owned
        // until the callback itself returns, not merely until cancellation.
        drop(runtime);
        if let Some(returned) = self.returned.take() {
            let _ = returned.send(result.clone());
        }
        if let Some(release) = self.release.take() {
            let _ = release.recv();
        }
        result
    }
}

fn peer_target(peer: &SyntheticPeer, workspace: &str, leaf: &[u8]) -> ProposedTarget {
    ProposedTarget::new(
        peer.origin().to_owned(),
        peer.selected_peer(),
        "installation-1".to_owned(),
        workspace.to_owned(),
        "sandbox-1".to_owned(),
        peer.root_der().to_vec(),
        leaf.to_vec(),
    )
    .unwrap()
}

fn real_review(
    host: &Host,
    gate: &Gate,
    peer: &SyntheticPeer,
    kind: ReadKind,
) -> (WorkspaceRequest, Review) {
    let workspace = WorkspaceRequest::Managed(Arc::new(gate.observe_open().unwrap()));
    let review = host
        .prepare(
            &workspace,
            peer_target(peer, "workspace-1", peer.leaf_der()),
            kind,
            Retention::SessionOnly,
        )
        .unwrap();
    (workspace, review)
}

fn peer_lease(token: &str) -> BearerLease {
    BearerLease::new(token.to_owned(), Instant::now() + Duration::from_secs(30)).unwrap()
}

fn start_real_read(host: &Host, review: &Review, token: &str) {
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            peer_lease(token),
            Box::<RealReadClient>::default(),
        ),
        HostStatus::Running
    );
}

async fn real_terminal_status(host: &Host) -> HostStatus {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let status = host.status();
            if !matches!(status, HostStatus::Running | HostStatus::Closing) {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("real client callback must finish within harness bound")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_selected_reads_bind_review_and_exact_allowlisted_calls() {
    use SyntheticRpc::*;
    for (kind, expected_calls) in [
        (
            ReadKind::ProtectedCheck,
            vec![Health, GetSandboxPolicyStatus],
        ),
        (
            ReadKind::PolicyStatus,
            vec![
                Health,
                GetSandboxPolicyStatus,
                GetSandboxPolicyStatus,
                ListSandboxPolicies,
            ],
        ),
        (
            ReadKind::RecentLogs,
            vec![Health, GetSandboxPolicyStatus, GetSandboxLogs],
        ),
        (
            ReadKind::RecentWatch,
            vec![Health, GetSandboxPolicyStatus, WatchSandbox],
        ),
    ] {
        let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
            .await
            .unwrap();
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let (workspace, review) = real_review(&host, &gate, &peer, kind);
        assert_eq!(host.inspect_review(), Some(review.clone()));
        assert_eq!(review.origin, peer.origin());
        assert_eq!(review.selected_peer, peer.selected_peer());
        assert_eq!(review.root_sha256, sha256_hex(peer.root_der()));
        assert_eq!(review.peer_pin_sha256, sha256_hex(peer.leaf_der()));
        assert_eq!(review.retention, Retention::SessionOnly);
        assert_eq!(host.completed_observation(&workspace, &review.id), None);
        start_real_read(&host, &review, "test-token");
        assert_eq!(real_terminal_status(&host).await, HostStatus::Completed);
        assert_eq!(peer.calls().unwrap(), expected_calls);
        let outcome = host.completed_observation(&workspace, &review.id).unwrap();
        assert!(outcome.matches_review(&review));
        assert_eq!(
            outcome.protected_check.authenticity,
            Authenticity::Unqualified
        );
        assert_eq!(
            outcome.protected_check.reported_product_version,
            PRODUCT_VERSION
        );
        assert_eq!(outcome.protected_check.protected_read.version, 7);
        match (&outcome.observation, kind) {
            (Observation::ProtectedCheck(check), ReadKind::ProtectedCheck) => {
                assert_eq!(check, &outcome.protected_check);
            }
            (Observation::PolicyStatus(policy), ReadKind::PolicyStatus) => {
                assert_eq!(policy.active_version, 7);
                assert_eq!(policy.revisions.len(), 1);
            }
            (Observation::RecentLogs(logs), ReadKind::RecentLogs) => {
                assert_eq!(logs.line_count, 1);
                assert!(!logs.losses.is_empty());
            }
            (Observation::RecentWatch(watch), ReadKind::RecentWatch) => {
                assert!(!watch.events.is_empty());
                assert_eq!(watch.continuity, rangoon_openshell_client::Continuity::Gap);
            }
            _ => panic!("selected read kind changed"),
        }
        let projected = format!("{review:?} {outcome:?}");
        for secret in [
            "test-token",
            "secret-token",
            "secret-target",
            "secret-like-provider-error",
            "not-for-projection",
            "secret-like provider text",
        ] {
            assert!(!projected.contains(secret), "raw fixture text leaked");
        }
        assert_eq!(host.completed_observation(&workspace, "wrong-review"), None);
        let drain = gate.request_drain().unwrap();
        assert!(drain.is_quiescent().unwrap());
        assert_eq!(host.completed_observation(&workspace, &review.id), None);
        assert!(host.cancel_and_check_exit());
        peer.shutdown().await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_public_health_never_substitutes_for_protected_access() {
    let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let (workspace, review) = real_review(&host, &gate, &peer, ReadKind::RecentLogs);
    start_real_read(&host, &review, "denied-dummy-token");
    assert_eq!(
        real_terminal_status(&host).await,
        HostStatus::Error(ClientError::PermissionDenied)
    );
    assert_eq!(
        peer.calls().unwrap(),
        vec![SyntheticRpc::Health, SyntheticRpc::GetSandboxPolicyStatus]
    );
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    assert!(gate.request_drain().unwrap().is_quiescent().unwrap());
    peer.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_selected_read_failure_discards_earlier_protected_check() {
    let peer = SyntheticPeer::start(SyntheticScenario::SelectedReadFailure)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let (workspace, review) = real_review(&host, &gate, &peer, ReadKind::RecentLogs);
    start_real_read(&host, &review, "test-token");
    assert_eq!(
        real_terminal_status(&host).await,
        HostStatus::Error(ClientError::Unreachable)
    );
    assert_eq!(
        peer.calls().unwrap(),
        vec![
            SyntheticRpc::Health,
            SyntheticRpc::GetSandboxPolicyStatus,
            SyntheticRpc::GetSandboxLogs
        ]
    );
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    assert!(gate.request_drain().unwrap().is_quiescent().unwrap());
    peer.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_changed_reported_version_refuses_before_protected_rpc() {
    let peer = SyntheticPeer::start(SyntheticScenario::WrongReportedVersion)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let (workspace, review) = real_review(&host, &gate, &peer, ReadKind::PolicyStatus);
    start_real_read(&host, &review, "test-token");
    assert_eq!(
        real_terminal_status(&host).await,
        HostStatus::Error(ClientError::Unsupported)
    );
    assert_eq!(peer.calls().unwrap(), vec![SyntheticRpc::Health]);
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    peer.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_wrong_scope_refuses_without_completed_observation() {
    let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let workspace = WorkspaceRequest::Managed(Arc::new(gate.observe_open().unwrap()));
    let review = host
        .prepare(
            &workspace,
            peer_target(&peer, "wrong-workspace", peer.leaf_der()),
            ReadKind::RecentLogs,
            Retention::SessionOnly,
        )
        .unwrap();
    start_real_read(&host, &review, "test-token");
    assert_eq!(
        real_terminal_status(&host).await,
        HostStatus::Error(ClientError::Source)
    );
    assert_eq!(
        peer.calls().unwrap(),
        vec![SyntheticRpc::Health, SyntheticRpc::GetSandboxPolicyStatus]
    );
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    peer.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_unhealthy_peer_refuses_before_protected_rpc() {
    let peer = SyntheticPeer::start(SyntheticScenario::Unhealthy)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let (workspace, review) = real_review(&host, &gate, &peer, ReadKind::RecentLogs);
    start_real_read(&host, &review, "test-token");
    assert_eq!(
        real_terminal_status(&host).await,
        HostStatus::Error(ClientError::Source)
    );
    assert_eq!(peer.calls().unwrap(), vec![SyntheticRpc::Health]);
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    peer.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_changed_leaf_refuses_before_any_rpc() {
    let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let workspace = WorkspaceRequest::Managed(Arc::new(gate.observe_open().unwrap()));
    let mut changed_leaf = peer.leaf_der().to_vec();
    changed_leaf[0] ^= 1;
    let review = host
        .prepare(
            &workspace,
            peer_target(&peer, "workspace-1", &changed_leaf),
            ReadKind::ProtectedCheck,
            Retention::SessionOnly,
        )
        .unwrap();
    start_real_read(&host, &review, "test-token");
    assert_eq!(
        real_terminal_status(&host).await,
        HostStatus::Error(ClientError::Tls)
    );
    assert!(peer.calls().unwrap().is_empty());
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    peer.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_denied_unconfirmed_and_stale_consent_make_no_rpc() {
    for state in ["denied", "unconfirmed", "stale-pending", "stale-confirmed"] {
        let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
            .await
            .unwrap();
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let (workspace, review) = real_review(&host, &gate, &peer, ReadKind::ProtectedCheck);
        match state {
            "denied" => assert_eq!(host.confirm(&review.id, false), HostStatus::Cancelled),
            "stale-pending" => {
                let _drain = gate.request_drain().unwrap();
                assert_eq!(host.inspect_review(), None);
                assert_eq!(host.confirm(&review.id, true), HostStatus::Unavailable);
            }
            "stale-confirmed" => {
                assert_eq!(
                    host.confirm(&review.id, true),
                    HostStatus::ReadyForNativeCredential
                );
                let _drain = gate.request_drain().unwrap();
            }
            "unconfirmed" => {}
            _ => unreachable!(),
        }
        let expected = if state == "unconfirmed" {
            HostStatus::AwaitingConsent
        } else {
            HostStatus::Unavailable
        };
        assert_eq!(
            host.begin_synthetic(
                &review.id,
                peer_lease("test-token"),
                Box::<RealReadClient>::default()
            ),
            expected
        );
        assert_eq!(host.completed_observation(&workspace, &review.id), None);
        assert!(host.cancel_and_check_exit());
        assert!(peer.calls().unwrap().is_empty());
        peer.shutdown().await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_changed_target_and_replayed_consent_never_dispatch_twice() {
    let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let (workspace, review) = real_review(&host, &gate, &peer, ReadKind::ProtectedCheck);
    assert_eq!(
        host.prepare(
            &workspace,
            peer_target(&peer, "different-workspace", peer.leaf_der()),
            ReadKind::RecentLogs,
            Retention::SessionOnly
        ),
        Err(HostError::Busy)
    );
    assert_eq!(
        host.confirm("different-review", true),
        HostStatus::Unavailable
    );
    assert_eq!(
        host.begin_synthetic(
            "different-review",
            peer_lease("test-token"),
            Box::<RealReadClient>::default()
        ),
        HostStatus::Unavailable
    );
    assert!(peer.calls().unwrap().is_empty());
    assert_eq!(host.inspect_review(), Some(review.clone()));
    start_real_read(&host, &review, "test-token");
    assert_eq!(real_terminal_status(&host).await, HostStatus::Completed);
    assert_eq!(host.confirm(&review.id, true), HostStatus::Unavailable);
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            peer_lease("test-token"),
            Box::<RealReadClient>::default()
        ),
        HostStatus::Unavailable
    );
    assert_eq!(
        peer.calls().unwrap(),
        vec![SyntheticRpc::Health, SyntheticRpc::GetSandboxPolicyStatus]
    );
    assert!(host.completed_observation(&workspace, &review.id).is_some());
    assert_eq!(host.stop(), HostStatus::Cancelled);
    let (_, next) = real_review(&host, &gate, &peer, ReadKind::RecentLogs);
    assert_ne!(next.id, review.id);
    assert_eq!(host.confirm(&review.id, true), HostStatus::Unavailable);
    assert_eq!(host.inspect_review(), Some(next));
    assert!(host.cancel_and_check_exit());
    assert_eq!(
        peer.calls().unwrap(),
        vec![SyntheticRpc::Health, SyntheticRpc::GetSandboxPolicyStatus]
    );
    peer.shutdown().await.unwrap();
}

#[derive(Clone, Copy)]
enum CancellationPath {
    Stop,
    WorkspaceClose,
    ExitHelper,
    NativeExit,
}

struct HeldSuccessfulRead {
    peer: SyntheticPeer,
    gate: Gate,
    host: Host,
    workspace: WorkspaceRequest,
    review: Review,
    release: mpsc::Sender<()>,
    transport_result: ReadOutcome,
}

impl HeldSuccessfulRead {
    async fn start() -> Self {
        let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
            .await
            .unwrap();
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let (workspace, review) = real_review(&host, &gate, &peer, ReadKind::RecentLogs);
        assert_eq!(
            host.confirm(&review.id, true),
            HostStatus::ReadyForNativeCredential
        );
        let (returned, result_received) = tokio::sync::oneshot::channel();
        let (release, held) = mpsc::channel();
        assert_eq!(
            host.begin_synthetic(
                &review.id,
                peer_lease("test-token"),
                Box::new(RealReadClient {
                    returned: Some(returned),
                    release: Some(held),
                }),
            ),
            HostStatus::Running
        );
        // This is the real client's successful result, after its runtime has
        // dropped but before the host callback can return through the barrier.
        let transport_result = tokio::time::timeout(Duration::from_secs(5), result_received)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(transport_result.matches_review(&review));
        assert!(matches!(
            transport_result.observation,
            Observation::RecentLogs(_)
        ));
        assert_eq!(
            peer.calls().unwrap(),
            vec![
                SyntheticRpc::Health,
                SyntheticRpc::GetSandboxPolicyStatus,
                SyntheticRpc::GetSandboxLogs,
            ]
        );
        assert_eq!(host.status(), HostStatus::Running);
        assert_eq!(host.completed_observation(&workspace, &review.id), None);
        assert!(!host.cancellation_signalled());
        Self {
            peer,
            gate,
            host,
            workspace,
            review,
            release,
            transport_result,
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_success_waits_for_callback_before_publication() {
    let HeldSuccessfulRead {
        peer,
        gate,
        host,
        workspace,
        review,
        release,
        transport_result,
    } = HeldSuccessfulRead::start().await;
    release.send(()).unwrap();
    assert_eq!(real_terminal_status(&host).await, HostStatus::Completed);
    assert_eq!(
        host.completed_observation(&workspace, &review.id),
        Some(transport_result)
    );
    assert_eq!(host.confirm(&review.id, true), HostStatus::Unavailable);
    assert_eq!(host.stop(), HostStatus::Cancelled);
    assert_eq!(host.completed_observation(&workspace, &review.id), None);

    // Stop permits a fresh review; it does not resurrect the consumed consent.
    let (_, next) = real_review(&host, &gate, &peer, ReadKind::RecentLogs);
    assert_ne!(next.id, review.id);
    assert_eq!(host.confirm(&review.id, true), HostStatus::Unavailable);
    assert_eq!(host.inspect_review(), Some(next.clone()));
    assert_eq!(host.confirm(&next.id, false), HostStatus::Cancelled);
    assert!(gate.request_drain().unwrap().is_quiescent().unwrap());
    assert_eq!(
        peer.calls().unwrap(),
        vec![
            SyntheticRpc::Health,
            SyntheticRpc::GetSandboxPolicyStatus,
            SyntheticRpc::GetSandboxLogs,
        ]
    );
    peer.shutdown().await.unwrap();
}

async fn real_success_cancellation(path: CancellationPath) {
    let HeldSuccessfulRead {
        peer,
        gate,
        host,
        workspace,
        review,
        release,
        transport_result: _,
    } = HeldSuccessfulRead::start().await;
    let source = SourceAccess::close_protocol_fixture(gate.clone());
    let close = WorkspaceClose::new(source.clone());
    use crate::workspace_close::failure::{ExitAction, ExitCoordinator, route_exit_request};
    let exit = ExitCoordinator::new();
    assert_eq!(
        host.prepare(
            &workspace,
            peer_target(&peer, "workspace-1", peer.leaf_der()),
            ReadKind::RecentLogs,
            Retention::SessionOnly,
        ),
        Err(HostError::Busy)
    );
    assert_eq!(host.confirm(&review.id, true), HostStatus::Unavailable);
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            peer_lease("test-token"),
            Box::<RealReadClient>::default(),
        ),
        HostStatus::Unavailable
    );

    let drain = match path {
        CancellationPath::Stop => {
            assert_eq!(host.stop(), HostStatus::Closing);
            // The test closes Gate solely to measure outstanding custody.
            // Stop itself does not claim to close workspace admission.
            Some(gate.request_drain().unwrap())
        }
        CancellationPath::ExitHelper => {
            assert!(!host.cancel_and_check_exit());
            Some(gate.request_drain().unwrap())
        }
        CancellationPath::WorkspaceClose => {
            let closing = close.begin(|drained| host.purge_for_workspace_close(drained));
            assert_eq!(serde_json::to_value(closing).unwrap()["state"], "closing");
            None
        }
        CancellationPath::NativeExit => {
            assert_eq!(
                route_exit_request(Some(71), Some((&close, &exit)), || {
                    assert!(gate.observe_open().is_err());
                    host.cancel_and_check_exit()
                }),
                ExitAction::PreventAndWait
            );
            assert!(!source.request_close().unwrap().is_quiescent().unwrap());
            None
        }
    };
    assert!(host.cancellation_signalled());
    assert_eq!(host.status(), HostStatus::Closing);
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    if let Some(drain) = &drain {
        assert!(!drain.is_quiescent().unwrap());
    }
    if matches!(path, CancellationPath::WorkspaceClose) {
        assert_eq!(
            close.progress_native(|drained| host.purge_for_workspace_close(drained)),
            Ok(false)
        );
    }
    release.send(()).unwrap();
    assert_eq!(real_terminal_status(&host).await, HostStatus::Cancelled);
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    assert_eq!(host.confirm(&review.id, true), HostStatus::Unavailable);
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            peer_lease("test-token"),
            Box::<RealReadClient>::default(),
        ),
        HostStatus::Unavailable
    );
    if let Some(drain) = &drain {
        assert!(drain.is_quiescent().unwrap());
    }
    match path {
        CancellationPath::WorkspaceClose => assert_eq!(
            close.progress_native(|drained| host.purge_for_workspace_close(drained)),
            Ok(true)
        ),
        CancellationPath::NativeExit => {
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
            assert!(close.clean_native_completion());
            assert!(exit.send(0));
            assert_eq!(
                route_exit_request(Some(0), Some((&close, &exit)), || host
                    .cancel_and_check_exit()),
                ExitAction::Pass
            );
            assert_eq!(
                route_exit_request(Some(0), Some((&close, &exit)), || host
                    .cancel_and_check_exit()),
                ExitAction::Prevent
            );
        }
        _ => assert!(host.cancel_and_check_exit()),
    }
    assert_eq!(host.status(), HostStatus::Unavailable);
    assert!(gate.observe_open().is_err());
    assert_eq!(
        peer.calls().unwrap(),
        vec![
            SyntheticRpc::Health,
            SyntheticRpc::GetSandboxPolicyStatus,
            SyntheticRpc::GetSandboxLogs,
        ]
    );
    peer.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_success_stop_discards_late_observation() {
    real_success_cancellation(CancellationPath::Stop).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_success_workspace_close_discards_late_observation() {
    real_success_cancellation(CancellationPath::WorkspaceClose).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_success_exit_helper_discards_late_observation() {
    real_success_cancellation(CancellationPath::ExitHelper).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_success_native_exit_coordinator_discards_late_observation() {
    real_success_cancellation(CancellationPath::NativeExit).await;
}

async fn real_watch_cancellation(path: CancellationPath) {
    let peer = SyntheticPeer::start(SyntheticScenario::StalledWatch)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let source = SourceAccess::close_protocol_fixture(gate.clone());
    let close = WorkspaceClose::new(source.clone());
    use crate::workspace_close::failure::{ExitAction, ExitCoordinator, route_exit_request};
    let exit = ExitCoordinator::new();
    let (workspace, review) = real_review(&host, &gate, &peer, ReadKind::RecentWatch);
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    let (returned, result_received) = tokio::sync::oneshot::channel();
    let (release, held) = mpsc::channel();
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            peer_lease("test-token"),
            Box::new(RealReadClient {
                returned: Some(returned),
                release: Some(held)
            })
        ),
        HostStatus::Running
    );
    peer.wait_watch_started().await.unwrap();
    assert_eq!(
        peer.calls().unwrap(),
        vec![
            SyntheticRpc::Health,
            SyntheticRpc::GetSandboxPolicyStatus,
            SyntheticRpc::WatchSandbox
        ]
    );
    assert_eq!(host.status(), HostStatus::Running);
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    let drain = match path {
        CancellationPath::Stop => {
            assert_eq!(host.stop(), HostStatus::Closing);
            Some(gate.request_drain().unwrap())
        }
        CancellationPath::WorkspaceClose => {
            let closing = close.begin(|drained| host.purge_for_workspace_close(drained));
            assert_eq!(serde_json::to_value(closing).unwrap()["state"], "closing");
            None
        }
        CancellationPath::ExitHelper => {
            assert!(!host.cancel_and_check_exit());
            Some(gate.request_drain().unwrap())
        }
        CancellationPath::NativeExit => {
            assert_eq!(
                route_exit_request(Some(71), Some((&close, &exit)), || {
                    assert!(
                        gate.observe_open().is_err(),
                        "admission must close before cancellation"
                    );
                    host.cancel_and_check_exit()
                }),
                ExitAction::PreventAndWait
            );
            assert!(!source.request_close().unwrap().is_quiescent().unwrap());
            None
        }
    };
    // This value comes from the actual transport call, before Host maps its
    // cancellation flag. A fake host-only error could not satisfy this check.
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), result_received)
            .await
            .unwrap()
            .unwrap(),
        Err(ClientError::Cancelled)
    );
    assert_eq!(host.status(), HostStatus::Closing);
    if let Some(drain) = &drain {
        assert!(!drain.is_quiescent().unwrap());
    }
    assert!(!host.cancel_and_check_exit());
    if matches!(path, CancellationPath::NativeExit) {
        assert!(!source.request_close().unwrap().is_quiescent().unwrap());
        assert_eq!(
            route_exit_request(Some(71), Some((&close, &exit)), || host
                .cancel_and_check_exit()),
            ExitAction::PreventAndWait
        );
    }
    if matches!(path, CancellationPath::WorkspaceClose) {
        assert_eq!(
            close.progress_native(|drained| host.purge_for_workspace_close(drained)),
            Ok(false)
        );
    }
    release.send(()).unwrap();
    assert_eq!(real_terminal_status(&host).await, HostStatus::Cancelled);
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    if let Some(drain) = &drain {
        assert!(drain.is_quiescent().unwrap());
    }
    match path {
        CancellationPath::WorkspaceClose => {
            assert_eq!(
                close.progress_native(|drained| host.purge_for_workspace_close(drained)),
                Ok(true)
            );
            assert!(gate.observe_open().is_err());
        }
        CancellationPath::NativeExit => {
            assert!(source.request_close().unwrap().is_quiescent().unwrap());
            assert!(gate.observe_open().is_err());
            assert!(
                host.prepare(
                    &workspace,
                    peer_target(&peer, "workspace-1", peer.leaf_der()),
                    ReadKind::RecentWatch,
                    Retention::SessionOnly
                )
                .is_err()
            );
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
            assert!(close.clean_native_completion());
            assert!(exit.send(0));
            assert_eq!(
                route_exit_request(Some(71), Some((&close, &exit)), || host
                    .cancel_and_check_exit()),
                ExitAction::Prevent
            );
            assert_eq!(
                route_exit_request(Some(0), Some((&close, &exit)), || host
                    .cancel_and_check_exit()),
                ExitAction::Pass
            );
            assert_eq!(
                route_exit_request(Some(0), Some((&close, &exit)), || host
                    .cancel_and_check_exit()),
                ExitAction::Prevent
            );
            assert!(gate.observe_open().is_err());
        }
        _ => assert!(host.cancel_and_check_exit()),
    }
    assert_eq!(host.status(), HostStatus::Unavailable);
    assert_eq!(
        peer.calls().unwrap(),
        vec![
            SyntheticRpc::Health,
            SyntheticRpc::GetSandboxPolicyStatus,
            SyntheticRpc::WatchSandbox
        ]
    );
    peer.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_stop_cancels_watch_and_retains_callback_until_return() {
    real_watch_cancellation(CancellationPath::Stop).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_workspace_close_cancels_watch_then_finishes_native_drain() {
    real_watch_cancellation(CancellationPath::WorkspaceClose).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_exit_helper_cancels_watch_then_allows_drained_exit() {
    real_watch_cancellation(CancellationPath::ExitHelper).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_native_exit_fences_before_cancel_and_waits_for_callback_return() {
    real_watch_cancellation(CancellationPath::NativeExit).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_production_start_without_anchor_or_after_source_failure_makes_no_rpc() {
    let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
        .await
        .unwrap();
    for failed in [false, true] {
        let gate = Gate::new();
        let source = SourceAccess::close_protocol_fixture(gate.clone());
        let host = Host::new(gate.clone());
        let (_, review) = real_review(&host, &gate, &peer, ReadKind::ProtectedCheck);
        assert_eq!(
            host.confirm(&review.id, true),
            HostStatus::ReadyForNativeCredential
        );
        if failed {
            // The installation failure branch latches this same source failure.
            // No AppHandle or actual native anchor is installed by this fixture.
            source.fail_closed();
            assert!(gate.observe_open().is_err());
        }
        assert_eq!(host.begin_production(&review.id), HostStatus::Unavailable);
        assert!(peer.calls().unwrap().is_empty());
    }
    peer.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_expired_before_dispatch_consumes_consent_and_allows_fresh_review() {
    let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let (workspace, review) = real_review(&host, &gate, &peer, ReadKind::ProtectedCheck);
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    let expires = Instant::now() + Duration::from_millis(100);
    let lease = BearerLease::new("test-token".to_owned(), expires).unwrap();
    tokio::time::sleep_until(tokio::time::Instant::from_std(expires) + Duration::from_millis(5))
        .await;
    assert!(Instant::now() >= expires);
    let (returned, result_received) = tokio::sync::oneshot::channel();
    let (release, held) = mpsc::channel();
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            lease,
            Box::new(RealReadClient {
                returned: Some(returned),
                release: Some(held),
            })
        ),
        HostStatus::Running
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), result_received)
            .await
            .unwrap()
            .unwrap(),
        Err(ClientError::CredentialLeaseExpired)
    );
    assert_eq!(host.status(), HostStatus::Running);
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    assert_eq!(host.confirm(&review.id, true), HostStatus::Unavailable);
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            peer_lease("test-token"),
            Box::<RealReadClient>::default()
        ),
        HostStatus::Unavailable
    );
    assert!(peer.calls().unwrap().is_empty());
    release.send(()).unwrap();
    assert_eq!(
        real_terminal_status(&host).await,
        HostStatus::Error(ClientError::CredentialLeaseExpired)
    );
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    assert!(peer.calls().unwrap().is_empty());

    // An expired attempt consumes consent. Recovery is an explicit new review
    // and a newly supplied lease, never retry or refresh of the failed attempt.
    assert_eq!(host.stop(), HostStatus::Cancelled);
    let (_, fresh) = real_review(&host, &gate, &peer, ReadKind::ProtectedCheck);
    assert_ne!(fresh.id, review.id);
    assert_eq!(host.confirm(&review.id, true), HostStatus::Unavailable);
    start_real_read(&host, &fresh, "test-token");
    assert_eq!(real_terminal_status(&host).await, HostStatus::Completed);
    assert!(host.completed_observation(&workspace, &fresh.id).is_some());
    assert_eq!(
        peer.calls().unwrap(),
        vec![SyntheticRpc::Health, SyntheticRpc::GetSandboxPolicyStatus]
    );
    assert_eq!(host.stop(), HostStatus::Cancelled);
    peer.shutdown().await.unwrap();
}

#[derive(Clone, Copy)]
enum ExpiredCallbackPath {
    ReturnError,
    Stop,
    WorkspaceClose,
    ExitHelper,
}

async fn real_watch_expiry_retains_callback(path: ExpiredCallbackPath) {
    let peer = SyntheticPeer::start(SyntheticScenario::StalledWatch)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let close = WorkspaceClose::new(SourceAccess::close_protocol_fixture(gate.clone()));
    let (workspace, review) = real_review(&host, &gate, &peer, ReadKind::RecentWatch);
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    let expires = Instant::now() + Duration::from_secs(2);
    let lease = BearerLease::new("test-token".to_owned(), expires).unwrap();
    let (returned, result_received) = tokio::sync::oneshot::channel();
    let (release, held) = mpsc::channel();
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            lease,
            Box::new(RealReadClient {
                returned: Some(returned),
                release: Some(held),
            })
        ),
        HostStatus::Running
    );
    peer.wait_watch_started().await.unwrap();
    assert!(
        Instant::now() < expires,
        "real watch must start while lease valid"
    );
    let expected_calls = vec![
        SyntheticRpc::Health,
        SyntheticRpc::GetSandboxPolicyStatus,
        SyntheticRpc::WatchSandbox,
    ];
    assert_eq!(peer.calls().unwrap(), expected_calls);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), result_received)
            .await
            .unwrap()
            .unwrap(),
        Err(ClientError::CredentialLeaseExpired)
    );
    assert!(Instant::now() >= expires);

    // The real read and its runtime have ended, but callback custody is still
    // held. Expiry must not release admission early or retain the earlier check.
    assert_eq!(host.status(), HostStatus::Running);
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    assert!(matches!(
        host.prepare(
            &workspace,
            peer_target(&peer, "workspace-1", peer.leaf_der()),
            ReadKind::RecentLogs,
            Retention::SessionOnly
        ),
        Err(HostError::Busy)
    ));
    assert_eq!(host.confirm(&review.id, true), HostStatus::Unavailable);
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            peer_lease("test-token"),
            Box::<RealReadClient>::default()
        ),
        HostStatus::Unavailable
    );
    assert_eq!(peer.calls().unwrap(), expected_calls);

    let drain = match path {
        ExpiredCallbackPath::ReturnError => Some(gate.request_drain().unwrap()),
        ExpiredCallbackPath::Stop => {
            assert_eq!(host.stop(), HostStatus::Closing);
            Some(gate.request_drain().unwrap())
        }
        ExpiredCallbackPath::WorkspaceClose => {
            let closing = close.begin(|drained| host.purge_for_workspace_close(drained));
            assert_eq!(serde_json::to_value(closing).unwrap()["state"], "closing");
            assert_eq!(
                close.progress_native(|drained| host.purge_for_workspace_close(drained)),
                Ok(false)
            );
            None
        }
        ExpiredCallbackPath::ExitHelper => {
            assert!(!host.cancel_and_check_exit());
            Some(gate.request_drain().unwrap())
        }
    };
    assert_eq!(
        host.status(),
        if matches!(path, ExpiredCallbackPath::ReturnError) {
            HostStatus::Running
        } else {
            HostStatus::Closing
        }
    );
    if let Some(drain) = &drain {
        assert!(!drain.is_quiescent().unwrap());
    }
    release.send(()).unwrap();
    let expected_status = match path {
        ExpiredCallbackPath::ReturnError => HostStatus::Error(ClientError::CredentialLeaseExpired),
        _ => HostStatus::Cancelled,
    };
    assert_eq!(real_terminal_status(&host).await, expected_status);
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    if let Some(drain) = &drain {
        assert!(drain.is_quiescent().unwrap());
    }
    if matches!(path, ExpiredCallbackPath::WorkspaceClose) {
        assert_eq!(
            close.progress_native(|drained| host.purge_for_workspace_close(drained)),
            Ok(true)
        );
        assert!(gate.observe_open().is_err());
    } else {
        assert!(host.cancel_and_check_exit());
    }
    assert_eq!(host.status(), HostStatus::Unavailable);
    assert_eq!(peer.calls().unwrap(), expected_calls);
    peer.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_expiry_discards_check_and_retains_admission_until_callback_return() {
    real_watch_expiry_retains_callback(ExpiredCallbackPath::ReturnError).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_stop_after_expiry_waits_for_callback_return() {
    real_watch_expiry_retains_callback(ExpiredCallbackPath::Stop).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_workspace_close_after_expiry_waits_for_callback_return() {
    real_watch_expiry_retains_callback(ExpiredCallbackPath::WorkspaceClose).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_client_exit_helper_after_expiry_waits_for_callback_return() {
    real_watch_expiry_retains_callback(ExpiredCallbackPath::ExitHelper).await;
}

#[test]
fn review_binds_exact_target_kind_and_session_only_retention() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::PolicyStatus);
    assert_eq!(review.origin, "https://openshell.test");
    assert_eq!(review.selected_peer.port(), 443);
    assert_eq!(review.installation, "installation-1");
    assert_eq!(review.workspace, "workspace-1");
    assert_eq!(review.sandbox, "sandbox-1");
    assert_eq!(review.product_version, PRODUCT_VERSION);
    assert_eq!(review.root_sha256.len(), 64);
    assert_eq!(review.peer_pin_sha256.len(), 64);
    assert_eq!(review.kind, ReadKind::PolicyStatus);
    assert_eq!(review.retention, Retention::SessionOnly);
    assert_eq!(review.data_classes, vec![DataClass::PolicyRevision]);
    assert_eq!(host.inspect_review(), Some(review));
}

#[test]
fn legacy_and_refused_workspace_requests_cannot_prepare_observation() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    for request in [WorkspaceRequest::Legacy, WorkspaceRequest::Refused] {
        assert_eq!(
            host.prepare(
                &request,
                target(),
                ReadKind::PolicyStatus,
                Retention::SessionOnly,
            ),
            Err(HostError::Unavailable)
        );
    }
    assert!(gate.begin(Lane::Read).is_ok());
}

#[test]
fn changed_trust_material_requires_a_distinct_review() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let first = prepare(&host, &gate, ReadKind::RecentWatch);
    assert_eq!(host.confirm(&first.id, false), HostStatus::Cancelled);

    let certificate = b"synthetic-root".to_vec();
    let changed = ProposedTarget::new(
        "https://openshell.test".to_owned(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 443),
        "installation-1".to_owned(),
        "workspace-1".to_owned(),
        "sandbox-1".to_owned(),
        certificate,
        b"replacement-leaf".to_vec(),
    )
    .unwrap();
    let next = host
        .prepare(
            &WorkspaceRequest::Managed(Arc::new(gate.observe_open().unwrap())),
            changed,
            ReadKind::RecentWatch,
            Retention::SessionOnly,
        )
        .unwrap();
    assert_eq!(first.root_sha256, next.root_sha256);
    assert_ne!(first.peer_pin_sha256, next.peer_pin_sha256);
    assert_ne!(first.id, next.id);
}

#[test]
fn consent_is_one_shot_and_production_start_stays_closed() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::ProtectedCheck);
    assert_eq!(host.confirm("wrong-review", true), HostStatus::Unavailable);
    assert_eq!(host.inspect_review(), Some(review.clone()));
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            dummy_lease(),
            Box::new(SyntheticClient {
                expected_kind: ReadKind::ProtectedCheck,
                result: Err(ClientError::Cancelled),
                started: None,
                release: None,
            }),
        ),
        HostStatus::AwaitingConsent
    );
    assert_eq!(host.inspect_review(), Some(review.clone()));
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    assert_eq!(host.begin_production(&review.id), HostStatus::Unavailable);
    assert_eq!(host.confirm(&review.id, true), HostStatus::Unavailable);
    assert_eq!(host.inspect_review(), Some(review));
}

#[test]
fn pending_status_tracks_confirmation_without_consuming_or_dispatching() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::RecentLogs);
    assert_eq!(host.status(), HostStatus::AwaitingConsent);
    assert_eq!(
        host.confirm("foreign-review", true),
        HostStatus::Unavailable
    );
    assert_eq!(host.status(), HostStatus::AwaitingConsent);
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    for _ in 0..3 {
        assert_eq!(host.status(), HostStatus::ReadyForNativeCredential);
        assert_eq!(host.inspect_review(), Some(review.clone()));
        assert_eq!(host.begin_production(&review.id), HostStatus::Unavailable);
    }
    assert_eq!(host.confirm(&review.id, true), HostStatus::Unavailable);
    assert_eq!(host.stop(), HostStatus::Cancelled);
    assert_eq!(host.status(), HostStatus::Unavailable);
    assert_eq!(host.inspect_review(), None);
}

#[test]
fn pending_status_reports_expiry_without_releasing_retained_custody() {
    for confirmed in [false, true] {
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let review = prepare(&host, &gate, ReadKind::RecentLogs);
        if confirmed {
            assert_eq!(
                host.confirm(&review.id, true),
                HostStatus::ReadyForNativeCredential
            );
        }
        {
            let mut state = host.state.lock().unwrap();
            let selection = match state.pending.as_mut().unwrap() {
                Pending::Awaiting { selection, .. } | Pending::Confirmed { selection, .. } => {
                    selection
                }
            };
            selection.deadline = Instant::now() - Duration::from_secs(1);
        }
        for _ in 0..3 {
            assert_eq!(
                host.status(),
                HostStatus::Error(ClientError::CredentialLeaseExpired)
            );
            assert!(host.state.lock().unwrap().pending.is_some());
            assert!(host.state.lock().unwrap().active.is_none());
        }
        let drain = gate.request_drain().unwrap();
        assert!(!drain.is_quiescent().unwrap());
        assert_eq!(host.status(), HostStatus::Unavailable);
        assert_eq!(host.stop(), HostStatus::Cancelled);
        assert!(drain.is_quiescent().unwrap());
        assert_eq!(host.status(), HostStatus::Unavailable);
    }
}

#[test]
fn pending_status_refuses_stale_generation_in_both_consent_phases() {
    for confirmed in [false, true] {
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let review = prepare(&host, &gate, ReadKind::PolicyStatus);
        if confirmed {
            assert_eq!(
                host.confirm(&review.id, true),
                HostStatus::ReadyForNativeCredential
            );
        }
        let drain = gate.request_drain().unwrap();
        assert_eq!(host.status(), HostStatus::Unavailable);
        assert_eq!(host.inspect_review(), None);
        assert!(host.state.lock().unwrap().pending.is_some());
        assert!(!drain.is_quiescent().unwrap());
        assert_eq!(host.stop(), HostStatus::Cancelled);
        assert!(drain.is_quiescent().unwrap());
    }
}

#[test]
fn refusal_and_replayed_review_release_admission() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::RecentLogs);
    assert_eq!(host.confirm("wrong-review", true), HostStatus::Unavailable);
    assert_eq!(host.inspect_review(), Some(review.clone()));
    assert_eq!(host.confirm(&review.id, false), HostStatus::Cancelled);
    assert_eq!(host.inspect_review(), None);
    assert!(gate.begin(Lane::Read).is_ok());

    let review = prepare(&host, &gate, ReadKind::RecentLogs);
    assert_eq!(host.confirm(&review.id, false), HostStatus::Cancelled);
    assert_eq!(host.confirm(&review.id, true), HostStatus::Unavailable);
}

#[test]
fn preparation_publication_after_close_purge_releases_stale_admission() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let close = WorkspaceClose::new(SourceAccess::close_protocol_fixture(gate.clone()));
    let workspace = WorkspaceRequest::Managed(Arc::new(gate.observe_open().unwrap()));
    let admission = workspace.admit(&gate, Lane::Read).unwrap();
    assert!(admission.is_current().unwrap());

    // Pause the real preparation flow at its admission/publication boundary.
    // Close fences the same Gate and purges the same Host before publication.
    let closing = close.begin(|drained| host.purge_for_workspace_close(drained));
    assert_eq!(serde_json::to_value(closing).unwrap()["state"], "closing");
    assert_eq!(host.inspect_review(), None);
    assert_eq!(
        close.progress_native(|drained| host.purge_for_workspace_close(drained)),
        Ok(false)
    );

    let publication = host.retain_review(
        admission,
        target(),
        ReadKind::RecentWatch,
        Retention::SessionOnly,
    );
    let retained = host.state.lock().unwrap().pending.is_some();
    let completed = close.progress_native(|drained| host.purge_for_workspace_close(drained));
    assert_eq!(
        (publication.map(|_| ()), retained, completed),
        (Err(HostError::Unavailable), false, Ok(true))
    );
    assert_eq!(host.inspect_review(), None);
    assert!(gate.observe_open().is_err());
}

#[test]
fn published_pending_review_is_purged_before_native_close_completes() {
    for confirmed in [false, true] {
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let close = WorkspaceClose::new(SourceAccess::close_protocol_fixture(gate.clone()));
        let review = prepare(&host, &gate, ReadKind::RecentWatch);
        if confirmed {
            assert_eq!(
                host.confirm(&review.id, true),
                HostStatus::ReadyForNativeCredential
            );
        }
        assert_eq!(host.inspect_review(), Some(review.clone()));

        let closing = close.begin(|drained| host.purge_for_workspace_close(drained));
        assert_eq!(serde_json::to_value(closing).unwrap()["state"], "closing");
        assert_eq!(host.inspect_review(), None);
        assert!(host.state.lock().unwrap().pending.is_none());
        assert_eq!(
            close.progress_native(|drained| host.purge_for_workspace_close(drained)),
            Ok(true)
        );
        assert_eq!(host.confirm(&review.id, true), HostStatus::Unavailable);
        assert_eq!(host.begin_production(&review.id), HostStatus::Unavailable);
        assert!(gate.observe_open().is_err());
    }
}

#[test]
fn workspace_drain_invalidates_pending_review_before_consent() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::PolicyStatus);
    let drain = gate.request_drain().unwrap();

    assert_eq!(host.inspect_review(), None);
    assert_eq!(host.confirm(&review.id, true), HostStatus::Unavailable);
    assert!(host.inspect_review().is_none());
    assert!(drain.is_quiescent().unwrap());
}

#[test]
fn workspace_drain_invalidates_confirmed_review_before_dispatch() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::RecentWatch);
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    let drain = gate.request_drain().unwrap();
    let (started, received) = mpsc::channel();

    assert_eq!(host.inspect_review(), None);
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            dummy_lease(),
            Box::new(SyntheticClient {
                expected_kind: ReadKind::RecentWatch,
                result: Err(ClientError::Cancelled),
                started: Some(started),
                release: None,
            }),
        ),
        HostStatus::Unavailable
    );
    assert!(host.inspect_review().is_none());
    assert!(matches!(
        received.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
    assert!(drain.is_quiescent().unwrap());
}

#[test]
fn synthetic_dispatch_uses_closed_kind_and_redacts_dummy_lease() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::ProtectedCheck);
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    assert_eq!(
        host.begin_synthetic(
            "wrong-review",
            dummy_lease(),
            Box::new(SyntheticClient {
                expected_kind: ReadKind::ProtectedCheck,
                result: Err(ClientError::PermissionDenied),
                started: None,
                release: None,
            }),
        ),
        HostStatus::Unavailable
    );
    assert_eq!(host.inspect_review(), Some(review.clone()));
    let (started, received) = mpsc::channel();
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            dummy_lease(),
            Box::new(SyntheticClient {
                expected_kind: ReadKind::ProtectedCheck,
                result: Err(ClientError::PermissionDenied),
                started: Some(started),
                release: None,
            }),
        ),
        HostStatus::Running
    );
    let (kind, debug) = received.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(kind, ReadKind::ProtectedCheck);
    assert!(!debug.contains("dummy-bearer-secret"));
    assert_eq!(
        finish_status(&host),
        HostStatus::Error(ClientError::PermissionDenied)
    );
}

#[test]
fn completed_projection_is_session_only_and_bound_to_review_and_generation() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::ProtectedCheck);
    let expected = protected_observation("workspace-1");
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            dummy_lease(),
            Box::new(SyntheticClient {
                expected_kind: ReadKind::ProtectedCheck,
                result: Ok(expected.clone()),
                started: None,
                release: None,
            }),
        ),
        HostStatus::Running
    );
    assert_eq!(finish_status(&host), HostStatus::Completed);
    let workspace = WorkspaceRequest::Managed(Arc::new(gate.observe_open().unwrap()));
    assert_eq!(
        host.completed_observation(&workspace, &review.id),
        Some(expected)
    );
    assert_eq!(host.completed_observation(&workspace, "wrong-review"), None);
    assert_eq!(
        host.completed_observation(&WorkspaceRequest::Legacy, &review.id),
        None
    );
    assert_eq!(host.stop(), HostStatus::Cancelled);
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
}

#[test]
fn wrong_scope_or_kind_never_enters_completed_projection_slot() {
    for (kind, outcome) in [
        (
            ReadKind::ProtectedCheck,
            protected_observation("workspace-2"),
        ),
        (ReadKind::RecentLogs, protected_observation("workspace-1")),
    ] {
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let review = prepare(&host, &gate, kind);
        assert_eq!(
            host.confirm(&review.id, true),
            HostStatus::ReadyForNativeCredential
        );
        assert_eq!(
            host.begin_synthetic(
                &review.id,
                dummy_lease(),
                Box::new(SyntheticClient {
                    expected_kind: kind,
                    result: Ok(outcome),
                    started: None,
                    release: None,
                }),
            ),
            HostStatus::Running
        );
        assert_eq!(finish_status(&host), HostStatus::Error(ClientError::Source));
        let workspace = WorkspaceRequest::Managed(Arc::new(gate.observe_open().unwrap()));
        assert_eq!(host.completed_observation(&workspace, &review.id), None);
    }
}

#[test]
fn non_check_result_requires_matching_protected_version_and_scope() {
    for version in [PRODUCT_VERSION, "v0.1.3"] {
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let review = prepare(&host, &gate, ReadKind::RecentLogs);
        assert_eq!(
            host.confirm(&review.id, true),
            HostStatus::ReadyForNativeCredential
        );
        let mut check = protected_check("workspace-1");
        check.reported_product_version = version.to_owned();
        let outcome = ReadOutcome {
            protected_check: check,
            observation: Observation::RecentLogs(LogWindow {
                authenticity: Authenticity::Unqualified,
                installation: "installation-1".to_owned(),
                workspace: "workspace-1".to_owned(),
                sandbox: "sandbox-1".to_owned(),
                line_count: 0,
                buffer_total: 0,
                native_sandbox_id: None,
                source_times_millis: Vec::new(),
                producers: Vec::new(),
                received_time_millis: 12,
                losses: Vec::new(),
            }),
        };
        assert_eq!(
            host.begin_synthetic(
                &review.id,
                dummy_lease(),
                Box::new(SyntheticClient {
                    expected_kind: ReadKind::RecentLogs,
                    result: Ok(outcome.clone()),
                    started: None,
                    release: None,
                }),
            ),
            HostStatus::Running
        );
        let workspace = WorkspaceRequest::Managed(Arc::new(gate.observe_open().unwrap()));
        if version == PRODUCT_VERSION {
            assert_eq!(finish_status(&host), HostStatus::Completed);
            assert_eq!(
                host.completed_observation(&workspace, &review.id),
                Some(outcome)
            );
        } else {
            assert_eq!(finish_status(&host), HostStatus::Error(ClientError::Source));
            assert_eq!(host.completed_observation(&workspace, &review.id), None);
        }
    }
}

#[test]
fn exit_as_first_canceller_rejects_late_success_and_drains() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::ProtectedCheck);
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    let (started, received) = mpsc::channel();
    let (release, release_received) = mpsc::channel();
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            dummy_lease(),
            Box::new(SyntheticClient {
                expected_kind: ReadKind::ProtectedCheck,
                result: Ok(protected_observation("workspace-1")),
                started: Some(started),
                release: Some(release_received),
            }),
        ),
        HostStatus::Running
    );
    received.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(!host.cancel_and_check_exit());
    release.send(()).unwrap();
    assert_eq!(finish_status(&host), HostStatus::Cancelled);
    let workspace = WorkspaceRequest::Managed(Arc::new(gate.observe_open().unwrap()));
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    assert!(host.cancel_and_check_exit());
    assert_eq!(host.status(), HostStatus::Unavailable);
    assert!(gate.request_drain().unwrap().is_quiescent().unwrap());
}

#[test]
fn stop_or_workspace_drain_drops_completed_and_late_success() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::ProtectedCheck);
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    let (started, received) = mpsc::channel();
    let (release, release_received) = mpsc::channel();
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            dummy_lease(),
            Box::new(SyntheticClient {
                expected_kind: ReadKind::ProtectedCheck,
                result: Ok(protected_observation("workspace-1")),
                started: Some(started),
                release: Some(release_received),
            }),
        ),
        HostStatus::Running
    );
    received.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(host.stop(), HostStatus::Closing);
    release.send(()).unwrap();
    assert_eq!(finish_status(&host), HostStatus::Cancelled);
    let workspace = WorkspaceRequest::Managed(Arc::new(gate.observe_open().unwrap()));
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    assert_eq!(host.stop(), HostStatus::Cancelled);

    let review = prepare(&host, &gate, ReadKind::ProtectedCheck);
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            dummy_lease(),
            Box::new(SyntheticClient {
                expected_kind: ReadKind::ProtectedCheck,
                result: Ok(protected_observation("workspace-1")),
                started: None,
                release: None,
            }),
        ),
        HostStatus::Running
    );
    assert_eq!(finish_status(&host), HostStatus::Completed);
    let drain = gate.request_drain().unwrap();
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    assert!(host.purge_for_workspace_close(false));
    assert!(drain.is_quiescent().unwrap());
    assert!(host.purge_for_workspace_close(true));
    assert_eq!(host.status(), HostStatus::Unavailable);
}

#[test]
fn synthetic_cursor_gap_and_bounds_remain_distinct() {
    for (kind, error) in [
        (ReadKind::RecentWatch, ClientError::Gap),
        (ReadKind::RecentLogs, ClientError::Bounds),
    ] {
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let review = prepare(&host, &gate, kind);
        assert_eq!(
            host.confirm(&review.id, true),
            HostStatus::ReadyForNativeCredential
        );
        assert_eq!(
            host.begin_synthetic(
                &review.id,
                dummy_lease(),
                Box::new(SyntheticClient {
                    expected_kind: kind,
                    result: Err(error),
                    started: None,
                    release: None,
                }),
            ),
            HostStatus::Running
        );
        assert_eq!(finish_status(&host), HostStatus::Error(error));
    }
}

#[test]
fn synthetic_credential_error_stays_distinct_without_an_observation() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::PolicyStatus);
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    // This fake callback proves error propagation only. The real_client_
    // expiry tests above separately exercise the accepted TLS bridge.
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            dummy_lease(),
            Box::new(SyntheticClient {
                expected_kind: ReadKind::PolicyStatus,
                result: Err(ClientError::CredentialLeaseExpired),
                started: None,
                release: None,
            }),
        ),
        HostStatus::Running
    );
    assert_eq!(
        finish_status(&host),
        HostStatus::Error(ClientError::CredentialLeaseExpired)
    );
    let workspace = WorkspaceRequest::Managed(Arc::new(gate.observe_open().unwrap()));
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
}

#[test]
fn stop_and_close_cancel_without_abandoning_callback_custody() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::RecentWatch);
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    let (started, received) = mpsc::channel();
    let (release, release_received) = mpsc::channel();
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            dummy_lease(),
            Box::new(SyntheticClient {
                expected_kind: ReadKind::RecentWatch,
                result: Err(ClientError::Cancelled),
                started: Some(started),
                release: Some(release_received),
            }),
        ),
        HostStatus::Running
    );
    received.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(host.stop(), HostStatus::Closing);
    assert!(!host.cancel_and_check_exit());
    assert!(host.purge_for_workspace_close(false));
    assert!(!host.purge_for_workspace_close(true));
    assert!(gate.request_drain().is_ok());
    let _ = release.send(());
    assert_eq!(finish_status(&host), HostStatus::Cancelled);
    assert!(host.purge_for_workspace_close(true));
}

#[test]
fn workspace_close_coordinator_waits_for_active_callback_before_native_completion() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let close = WorkspaceClose::new(SourceAccess::close_protocol_fixture(gate.clone()));
    let review = prepare(&host, &gate, ReadKind::RecentWatch);
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    let (started, received) = mpsc::channel();
    let (release, release_received) = mpsc::channel();
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            dummy_lease(),
            Box::new(SyntheticClient {
                expected_kind: ReadKind::RecentWatch,
                result: Err(ClientError::Cancelled),
                started: Some(started),
                release: Some(release_received),
            }),
        ),
        HostStatus::Running
    );
    received.recv_timeout(Duration::from_secs(1)).unwrap();

    let closing = close.begin(|drained| host.purge_for_workspace_close(drained));
    assert_eq!(serde_json::to_value(closing).unwrap()["state"], "closing");
    assert!(host.cancellation_signalled());
    assert_eq!(
        close.progress_native(|drained| host.purge_for_workspace_close(drained)),
        Ok(false)
    );
    assert_eq!(host.status(), HostStatus::Closing);

    release.send(()).unwrap();
    assert_eq!(finish_status(&host), HostStatus::Cancelled);
    assert_eq!(
        close.progress_native(|drained| host.purge_for_workspace_close(drained)),
        Ok(true)
    );
    assert_eq!(host.status(), HostStatus::Unavailable);
    assert!(gate.observe_open().is_err());
}

#[test]
fn native_review_callback_holds_admission_and_binds_only_its_parent() {
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let review = prepare(&host, &gate, ReadKind::ProtectedCheck);
    let original_deadline = match host.state.lock().unwrap().pending.as_ref().unwrap() {
        Pending::Awaiting { selection, .. } => selection.deadline,
        Pending::Confirmed { .. } => unreachable!("new review must await native consent"),
    };
    assert_eq!(
        host.begin_callback(
            &review.id,
            None,
            CallbackStart::Confirmed,
            None,
            |_, _, _, _, _| Err(ClientError::Source),
        ),
        HostStatus::AwaitingConsent
    );
    assert_eq!(host.inspect_review(), Some(review.clone()));
    assert_eq!(
        host.begin_callback(
            "wrong-review",
            None,
            CallbackStart::NativeReview,
            Some("native-parent".into()),
            |_, _, _, _, _| Err(ClientError::Source),
        ),
        HostStatus::Unavailable
    );
    assert_eq!(host.inspect_review(), Some(review.clone()));

    let (entered, started) = mpsc::channel();
    let (release, held) = mpsc::channel();
    assert_eq!(
        host.begin_callback(
            &review.id,
            None,
            CallbackStart::NativeReview,
            Some("native-parent".into()),
            move |_, _, _, active, selection| {
                entered.send(selection.deadline).unwrap();
                let _ = held.recv();
                active.accept_credential(selection.deadline)?;
                Err(ClientError::Cancelled)
            },
        ),
        HostStatus::Running
    );
    assert_eq!(
        started.recv_timeout(Duration::from_secs(1)).unwrap(),
        original_deadline
    );
    assert_eq!(host.confirm(&review.id, true), HostStatus::Unavailable);
    assert_eq!(
        host.begin_callback(
            &review.id,
            None,
            CallbackStart::NativeReview,
            Some("native-parent".into()),
            |_, _, _, _, _| Err(ClientError::Source),
        ),
        HostStatus::Unavailable
    );
    host.parent_closed("other-parent");
    assert!(!host.cancellation_signalled());
    assert_eq!(host.status(), HostStatus::Running);
    host.parent_closed("native-parent");
    assert!(host.cancellation_signalled());
    assert_eq!(host.status(), HostStatus::Closing);
    let drain = gate.request_drain().unwrap();
    assert!(!drain.is_quiescent().unwrap());

    release.send(()).unwrap();
    assert_eq!(finish_status(&host), HostStatus::Cancelled);
    assert!(drain.is_quiescent().unwrap());
    assert_eq!(host.stop(), HostStatus::Cancelled);
}

#[path = "openshell_credential_tests.rs"]
mod credential_binding;

#[test]
fn native_fixture_input_rejects_and_clears_every_non_fixture_value_before_client() {
    use zeroize::Zeroizing;
    for value in [
        "",
        "credential-example",
        " test-token",
        "test-token ",
        "test-token\n",
        "TEST-TOKEN",
    ] {
        let mut held = Zeroizing::new(value.to_owned());
        assert_eq!(
            super::native_input::validate_fixture_input(&mut held),
            Err(ClientError::InvalidConfiguration)
        );
        assert!(
            held.is_empty(),
            "rejected owned input must be cleared before returning"
        );
    }
    let mut accepted = Zeroizing::new("test-token".to_owned());
    assert_eq!(
        super::native_input::validate_fixture_input(&mut accepted),
        Ok(())
    );
    assert_eq!(accepted.as_str(), "test-token");
}

// This enters after native confirmation/input. It only exercises the shared
// post-input tail used by that callback; no test constructs a native widget.
fn begin_fixture_post_input_callback<F>(
    host: &Host,
    review: &Review,
    value: String,
    read: F,
) -> HostStatus
where
    F: FnOnce(
            ClientConfig,
            BearerLease,
            ReadKind,
            WatchControl,
        ) -> Result<ReadOutcome, ClientError>
        + Send
        + 'static,
{
    host.begin_callback(
        &review.id,
        None,
        CallbackStart::NativeReview,
        Some("fixture-native-parent".into()),
        move |config, kind, control, active, selection| {
            super::native_input::finish_after_native_input(
                active,
                selection.claim(),
                zeroize::Zeroizing::new(value),
                None,
                |lease| read(config, lease, kind, control),
            )
        },
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_fixture_post_input_tail_seals_and_reads_only_after_exact_marker() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let (workspace, review) = real_review(&host, &gate, &peer, ReadKind::ProtectedCheck);
    let reader_started = Arc::new(AtomicUsize::new(0));
    let reader_callback = Arc::clone(&reader_started);
    assert_eq!(
        begin_fixture_post_input_callback(
            &host,
            &review,
            "test-token".to_owned(),
            move |config, lease, kind, control| {
                reader_callback.fetch_add(1, Ordering::SeqCst);
                super::native_input::read_once(config, lease, kind, control)
            },
        ),
        HostStatus::Running
    );
    assert_eq!(real_terminal_status(&host).await, HostStatus::Completed);
    assert_eq!(reader_started.load(Ordering::SeqCst), 1);
    assert_eq!(
        peer.calls().unwrap(),
        vec![SyntheticRpc::Health, SyntheticRpc::GetSandboxPolicyStatus]
    );
    assert!(host.completed_observation(&workspace, &review.id).is_some());
    assert_eq!(
        begin_fixture_post_input_callback(&host, &review, "test-token".to_owned(), |_, _, _, _| {
            Err(ClientError::Source)
        }),
        HostStatus::Unavailable
    );
    let drain = gate.request_drain().unwrap();
    assert!(drain.is_quiescent().unwrap());
    assert!(host.cancel_and_check_exit());
    peer.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_fixture_post_input_tail_rejects_non_markers_before_reader() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    for value in [
        "".to_owned(),
        "credential-example".to_owned(),
        " test-token".to_owned(),
        "test-token ".to_owned(),
        "test-token\n".to_owned(),
        "TEST-TOKEN".to_owned(),
        "tést-token".to_owned(),
        "x".repeat(16 * 1024 + 1),
    ] {
        let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
            .await
            .unwrap();
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let (workspace, review) = real_review(&host, &gate, &peer, ReadKind::ProtectedCheck);
        let reader_started = Arc::new(AtomicUsize::new(0));
        let reader_callback = Arc::clone(&reader_started);
        assert_eq!(
            begin_fixture_post_input_callback(
                &host,
                &review,
                value,
                move |config, lease, kind, control| {
                    reader_callback.fetch_add(1, Ordering::SeqCst);
                    super::native_input::read_once(config, lease, kind, control)
                },
            ),
            HostStatus::Running
        );
        assert_eq!(
            real_terminal_status(&host).await,
            HostStatus::Error(ClientError::InvalidConfiguration)
        );
        assert_eq!(reader_started.load(Ordering::SeqCst), 0);
        assert_eq!(host.completed_observation(&workspace, &review.id), None);
        assert!(peer.calls().unwrap().is_empty());
        assert_eq!(
            begin_fixture_post_input_callback(
                &host,
                &review,
                "test-token".to_owned(),
                |_, _, _, _| { Err(ClientError::Source) }
            ),
            HostStatus::Unavailable
        );
        let drain = gate.request_drain().unwrap();
        assert!(drain.is_quiescent().unwrap());
        assert!(host.cancel_and_check_exit());
        peer.shutdown().await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_fixture_post_input_tail_stop_before_tail_refuses_valid_marker() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let (workspace, review) = real_review(&host, &gate, &peer, ReadKind::ProtectedCheck);
    let reader_started = Arc::new(AtomicUsize::new(0));
    let reader_callback = Arc::clone(&reader_started);
    let (entered, started) = mpsc::channel();
    let (release, held) = mpsc::channel();
    assert_eq!(
        host.begin_callback(
            &review.id,
            None,
            CallbackStart::NativeReview,
            Some("fixture-native-parent".into()),
            move |config, kind, control, active, selection| {
                entered.send(()).unwrap();
                let value = held.recv().map_err(|_| ClientError::Source)?;
                super::native_input::finish_after_native_input(
                    active,
                    selection.claim(),
                    zeroize::Zeroizing::new(value),
                    None,
                    |lease| {
                        reader_callback.fetch_add(1, Ordering::SeqCst);
                        super::native_input::read_once(config, lease, kind, control)
                    },
                )
            },
        ),
        HostStatus::Running
    );
    started.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(host.stop(), HostStatus::Closing);
    let drain = gate.request_drain().unwrap();
    assert!(!drain.is_quiescent().unwrap());
    release.send("test-token".to_owned()).unwrap();
    assert_eq!(real_terminal_status(&host).await, HostStatus::Cancelled);
    assert_eq!(reader_started.load(Ordering::SeqCst), 0);
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    assert!(peer.calls().unwrap().is_empty());
    assert_eq!(
        begin_fixture_post_input_callback(&host, &review, "test-token".to_owned(), |_, _, _, _| {
            Err(ClientError::Source)
        },),
        HostStatus::Unavailable
    );
    assert!(drain.is_quiescent().unwrap());
    assert!(host.cancel_and_check_exit());
    peer.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_fixture_post_input_tail_expired_received_selection_never_reads() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
        .await
        .unwrap();
    let gate = Gate::new();
    let host = Host::new(gate.clone());
    let (workspace, review) = real_review(&host, &gate, &peer, ReadKind::ProtectedCheck);
    let reader_started = Arc::new(AtomicUsize::new(0));
    let reader_callback = Arc::clone(&reader_started);
    assert_eq!(
        host.begin_callback(
            &review.id,
            None,
            CallbackStart::NativeReview,
            Some("fixture-native-parent".into()),
            move |config, kind, control, active, mut selection| {
                selection.deadline = Instant::now() - Duration::from_secs(1);
                super::native_input::finish_after_native_input(
                    active,
                    selection.claim(),
                    zeroize::Zeroizing::new("test-token".to_owned()),
                    None,
                    |lease| {
                        reader_callback.fetch_add(1, Ordering::SeqCst);
                        super::native_input::read_once(config, lease, kind, control)
                    },
                )
            },
        ),
        HostStatus::Running
    );
    assert_eq!(
        real_terminal_status(&host).await,
        HostStatus::Error(ClientError::CredentialLeaseExpired)
    );
    assert_eq!(reader_started.load(Ordering::SeqCst), 0);
    assert_eq!(host.completed_observation(&workspace, &review.id), None);
    assert!(peer.calls().unwrap().is_empty());
    assert_eq!(
        begin_fixture_post_input_callback(&host, &review, "test-token".to_owned(), |_, _, _, _| {
            Err(ClientError::Source)
        }),
        HostStatus::Unavailable
    );
    let drain = gate.request_drain().unwrap();
    assert!(drain.is_quiescent().unwrap());
    assert!(host.cancel_and_check_exit());
    peer.shutdown().await.unwrap();
}
