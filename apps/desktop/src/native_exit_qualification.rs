//! Explicit, no-webview native lifecycle qualification. Never a product entry.
use super::*;
use crate::{NativeCloud, NativeModel, SourceAccess, WorkspaceClose};
use rangoon_openshell_client::{
    OpenShellReadClient,
    synthetic_peer::{SyntheticPeer, SyntheticRpc, SyntheticScenario},
};
use serde_json::{Value, json};
use std::{
    io::Write,
    sync::{Condvar, OnceLock, mpsc},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};

const CASES: &[&str] = &[
    "idle-clean",
    "active-cancel",
    "duplicate-timeout-retry",
    "cleanup-failed",
    "anchor-unavailable",
    "anchor-install-failed",
];
static CASE: OnceLock<String> = OnceLock::new();
static EVENTS: Mutex<u64> = Mutex::new(0);
static WAIT: (Mutex<(usize, usize)>, Condvar) = (Mutex::new((0, 0)), Condvar::new());

fn emit(event: &'static str, fields: Value) {
    let mut seq = EVENTS.lock().expect("event lock");
    *seq += 1;
    let record =
        json!({"seq": *seq, "case": CASE.get().expect("case"), "event": event, "fields": fields});
    let line = serde_json::to_vec(&record).expect("fixed event");
    assert!(line.len() < 2048 && *seq <= 128);
    let mut out = std::io::stdout().lock();
    out.write_all(&line).expect("event output");
    out.write_all(b"\n").expect("event output");
    out.flush().expect("event output");
}

/// Observes real waiter invocations. It cannot choose readiness or reissue exit.
#[derive(Default)]
pub(crate) struct WaitProbe {
    checks: usize,
    drained: bool,
}
impl WaitProbe {
    pub(crate) fn checked(&mut self, drained: bool) {
        if self.checks == 0 {
            let mut state = WAIT.0.lock().expect("wait lock");
            state.0 += 1;
            assert_eq!(state.0, 1, "duplicate active waiter");
            emit("waiter_started", json!({"active": state.0}));
        }
        self.checks += 1;
        self.drained = drained;
    }
}
impl Drop for WaitProbe {
    fn drop(&mut self) {
        if self.checks == 0 {
            return;
        }
        let mut state = WAIT.0.lock().expect("wait lock");
        state.0 -= 1;
        state.1 += 1;
        emit(
            "waiter_finished",
            json!({"checks": self.checks, "drained": self.drained, "active": state.0}),
        );
        WAIT.1.notify_all();
    }
}

pub(crate) fn ready(app: &AppHandle, anchor: bool) {
    emit(
        "ready",
        json!({"anchor": anchor, "failedClosed": app.state::<SourceAccess>().failure_latched()}),
    );
}
pub(crate) fn exit_action(
    app: &AppHandle,
    code: Option<i32>,
    action: crate::workspace_close::failure::ExitAction,
) {
    use crate::workspace_close::failure::ExitAction::*;
    let action = match action {
        Pass => "pass",
        Prevent => "prevent",
        PreventAndStart => "start",
        PreventAndWait => "wait",
    };
    emit(
        "exit_request",
        json!({"code": code, "action": action,
        "admissionClosed": app.state::<Gate>().begin(Lane::Read).is_err(),
        "clean": app.state::<WorkspaceClose>().clean_native_completion()}),
    );
}
pub(crate) fn exited() {
    emit("exit", json!({}));
}

fn valid_context(config: &Value, assets_empty: bool) -> bool {
    let Some(app) = config.get("app") else {
        return false;
    };
    let Some(build) = config.get("build") else {
        return false;
    };
    let Some(security) = app.get("security") else {
        return false;
    };
    let Some(caps) = security.get("capabilities").and_then(Value::as_array) else {
        return false;
    };
    config["identifier"] == "ai.rangoon.native-exit-qualification"
        && config["bundle"]["active"] == false
        && app["withGlobalTauri"] == false
        && app["windows"].as_array().is_some_and(Vec::is_empty)
        && [
            "frontendDist",
            "devUrl",
            "beforeDevCommand",
            "beforeBuildCommand",
            "beforeBundleCommand",
        ]
        .iter()
        .all(|k| build.get(k).is_none_or(Value::is_null))
        && config
            .get("plugins")
            .is_none_or(|v| v.as_object().is_some_and(|o| o.is_empty()))
        && caps.len() == 1
        && caps[0]["identifier"] == "nx06-no-permissions"
        && caps[0]["local"] == false
        && caps[0].get("remote").is_none_or(Value::is_null)
        && ["windows", "webviews", "permissions"].iter().all(|k| {
            caps[0]
                .get(k)
                .is_none_or(|v| v.as_array().is_some_and(Vec::is_empty))
        })
        && assets_empty
}

// Pure negative predicates run before constructing a native application.
fn check_context(context: &tauri::Context<tauri::Wry>) {
    let mut config = serde_json::to_value(context.config()).expect("configuration");
    // Tauri serializes empty selector arrays by omission; normalize only those
    // empty defaults after reading the actual typed context.
    if let Some(cap) = config
        .pointer_mut("/app/security/capabilities/0")
        .and_then(Value::as_object_mut)
    {
        for key in ["windows", "webviews", "permissions"] {
            cap.entry(key).or_insert_with(|| json!([]));
        }
    }
    assert!(valid_context(
        &config,
        context.assets().iter().next().is_none()
    ));
    for (pointer, replacement) in [
        ("/identifier", json!("ai.rangoon.studio.dev")),
        ("/app/windows", json!([{"label":"main"}])),
        ("/app/withGlobalTauri", json!(true)),
        ("/app/security/capabilities", json!([])),
        ("/app/security/capabilities", json!(["source-analysis"])),
        ("/app/security/capabilities/0/local", json!(true)),
        (
            "/app/security/capabilities/0/permissions",
            json!(["core:default"]),
        ),
        ("/bundle/active", json!(true)),
    ] {
        let mut bad = config.clone();
        *bad.pointer_mut(pointer).expect("predicate field") = replacement;
        assert!(!valid_context(&bad, true));
    }
    assert!(!valid_context(&config, false));
    emit(
        "context_validated",
        json!({"emptyAssets": true, "effectiveGrants": 0, "negativeChecks": 9}),
    );
}

pub(crate) fn run() {
    std::panic::set_hook(Box::new(|_| {
        eprintln!("nx06: test panic");
    }));
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1 || !CASES.contains(&args[0].as_str()) {
        eprintln!("nx06: one exact case required");
        std::process::exit(64);
    }
    CASE.set(args[0].clone()).expect("one case");
    let context = tauri::generate_context!(
        "native-exit-qualification/tauri.conf.json",
        test = true,
        assets = tauri::utils::assets::EmbeddedAssets::new(
            tauri::utils::assets::phf::Map::new(),
            &[],
            tauri::utils::assets::phf::Map::new()
        )
    );
    check_context(&context);
    let (gate, flight) = crate::admission_state();
    let source = if args[0] == "anchor-unavailable" {
        crate::source_state(&gate, Err(rangoon_store::StoreError::Unavailable))
    } else {
        SourceAccess::close_protocol_fixture(gate.clone())
    };
    if args[0] == "anchor-install-failed" {
        source
            .set_failure_notifier(Arc::new(|| {}))
            .expect("occupied fixture notifier");
    }
    let cloud = NativeCloud::default();
    if args[0] == "cleanup-failed" {
        cloud.poison_for_native_exit_qualification();
    }
    let app = tauri::Builder::default()
        .manage(gate.clone())
        .manage(NativeModel::default())
        .manage(cloud)
        .manage(flight)
        .manage(crate::cloud_custody::Custody::with_gate(gate.clone()))
        .manage(Host::new(gate))
        .manage(WorkspaceClose::new(source.clone()))
        .manage(source)
        .build(context)
        .expect("qualification app");
    let mut anchor = None;
    let waiter = crate::workspace_close::failure::CallbackWaiter::default();
    app.run(move |app, event| {
        crate::coordinate_native_event(app, &event, &mut anchor, &waiter);
        if matches!(event, tauri::RunEvent::Ready) {
            let handle = app.clone();
            std::thread::spawn(move || {
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drive(handle))).is_err()
                {
                    // Never let an assertion failure look like a successful ordinary exit.
                    std::process::exit(70);
                }
            });
        }
    });
}

struct HeldRead {
    returned: mpsc::SyncSender<bool>,
    release: mpsc::Receiver<()>,
}
impl SyntheticReadClient for HeldRead {
    fn run(
        self: Box<Self>,
        config: ClientConfig,
        lease: BearerLease,
        kind: ReadKind,
        control: WatchControl,
    ) -> Result<ReadOutcome, ClientError> {
        assert_eq!(kind, ReadKind::RecentWatch);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("client runtime");
        let result = rt.block_on(async {
            let mut client = OpenShellReadClient::connect(config, lease).await?;
            let protected_check = client.check_protected_read().await?;
            let observation = Observation::RecentWatch(
                client
                    .watch_recent(&control, None, Duration::from_secs(30))
                    .await?,
            );
            Ok(ReadOutcome {
                protected_check,
                observation,
            })
        });
        drop(rt);
        let cancelled = result == Err(ClientError::Cancelled);
        emit("client_returned", json!({"cancelled": cancelled}));
        self.returned
            .send(cancelled)
            .expect("client result receiver");
        self.release
            .recv_timeout(Duration::from_secs(20))
            .expect("callback release");
        result
    }
}

fn roundtrip(app: &AppHandle, event: &'static str) {
    let (tx, rx) = mpsc::sync_channel(1);
    let checking = app.clone();
    app.run_on_main_thread(move || {
        assert!(checking.state::<Gate>().begin(Lane::Read).is_err());
        assert!(!checking.state::<Host>().cancel_and_check_exit());
        assert!(!checking.state::<WorkspaceClose>().clean_native_completion());
        emit(
            event,
            json!({"admissionClosed": true, "callbackHeld": true}),
        );
        tx.send(()).expect("roundtrip receiver");
    })
    .expect("native roundtrip");
    rx.recv_timeout(Duration::from_secs(5))
        .expect("native roundtrip returned");
}

fn drive(app: AppHandle) {
    let case = CASE.get().expect("case");
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("peer runtime");
    let host = app.state::<Host>().inner().clone();
    let gate = app.state::<Gate>().inner().clone();
    if case != "active-cancel" && case != "duplicate-timeout-retry" {
        // An inert local socket sentinel observes the no-start path without
        // opening a client or depending on private transport-crate counters.
        let sentinel =
            std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).expect("sentinel");
        sentinel
            .set_nonblocking(true)
            .expect("nonblocking sentinel");
        assert_eq!(
            host.begin_production("nx06-unavailable"),
            HostStatus::Unavailable
        );
        assert!(matches!(sentinel.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock));
        emit(
            "production_unavailable",
            json!({"connections": 0, "rpcs": 0, "probe": "inert_socket_sentinel"}),
        );
        drop(sentinel);
        emit("peer_stopped", json!({"connections": 0, "rpcs": 0}));
        emit("request_exit", json!({"code": 37}));
        app.exit(37);
        return;
    }
    let peer = rt
        .block_on(SyntheticPeer::start(SyntheticScenario::StalledWatch))
        .expect("synthetic peer");
    assert_eq!(
        host.begin_production("nx06-unavailable"),
        HostStatus::Unavailable
    );
    assert!(peer.calls().expect("calls").is_empty());
    emit("production_unavailable", json!({"rpcs": 0}));
    let workspace = WorkspaceRequest::Managed(Arc::new(gate.observe_open().expect("open witness")));
    let target = ProposedTarget::new(
        peer.origin().to_owned(),
        peer.selected_peer(),
        "installation-1".into(),
        "workspace-1".into(),
        "sandbox-1".into(),
        peer.root_der().to_vec(),
        peer.leaf_der().to_vec(),
    )
    .expect("peer target");
    let review = host
        .prepare(
            &workspace,
            target,
            ReadKind::RecentWatch,
            Retention::SessionOnly,
        )
        .expect("review");
    assert_eq!(
        host.confirm(&review.id, true),
        HostStatus::ReadyForNativeCredential
    );
    let (tx, rx) = mpsc::sync_channel(1);
    let (release, held) = mpsc::channel();
    let lease = BearerLease::new(
        "test-token".into(),
        Instant::now() + Duration::from_secs(30),
    )
    .expect("test lease");
    assert_eq!(
        host.begin_synthetic(
            &review.id,
            lease,
            Box::new(HeldRead {
                returned: tx,
                release: held
            })
        ),
        HostStatus::Running
    );
    rt.block_on(peer.wait_watch_started())
        .expect("watch signal");
    assert_eq!(
        peer.calls().expect("calls"),
        vec![
            SyntheticRpc::Health,
            SyntheticRpc::GetSandboxPolicyStatus,
            SyntheticRpc::WatchSandbox
        ]
    );
    emit("watch_started", json!({"rpcs": 3}));
    emit("request_exit", json!({"code": 37}));
    app.exit(37);
    assert!(
        rx.recv_timeout(Duration::from_secs(5))
            .expect("real cancellation")
    );
    roundtrip(&app, "held_roundtrip");
    if case == "duplicate-timeout-retry" {
        for _ in 0..3 {
            app.exit(37);
        }
        roundtrip(&app, "duplicates_roundtrip");
        let state = WAIT.0.lock().expect("wait lock");
        let (state, timeout) = WAIT
            .1
            .wait_timeout_while(state, Duration::from_secs(15), |s| s.1 == 0)
            .expect("waiter signal");
        assert!(!timeout.timed_out());
        assert_eq!(*state, (0, 1));
        drop(state);
        roundtrip(&app, "timeout_roundtrip");
    }
    assert!(host.completed_observation(&workspace, &review.id).is_none());
    // Shut down the synthetic server before release can permit native process exit.
    rt.block_on(peer.shutdown()).expect("peer shutdown");
    emit("peer_stopped", json!({"rpcs": 3}));
    emit("release_callback", json!({}));
    release.send(()).expect("release held callback");
    if case == "duplicate-timeout-retry" {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !host.cancel_and_check_exit() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        emit("callback_drained", json!({}));
        emit("retry_exit", json!({"code": 37}));
        app.exit(37);
    }
}
