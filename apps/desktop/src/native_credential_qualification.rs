//! Explicit no-webview native C1 qualification. Dummy credentials and a
//! generated loopback TLS peer only. Never an ordinary application entry.
#![deny(warnings)]
#[cfg(not(target_os = "macos"))]
pub(crate) fn run() {
    eprintln!("native_credential_unavailable");
    std::process::exit(64);
}

#[cfg(target_os = "macos")]
pub(crate) use macos::run;

#[cfg(target_os = "macos")]
mod macos {
    use super::super::*;
    use rangoon_openshell_client::synthetic_peer::{
        SyntheticPeer, SyntheticRpc, SyntheticScenario,
    };
    use serde_json::{Value, json};
    use std::{
        io::{Read, Write},
        sync::{OnceLock, mpsc},
        time::{Duration, Instant},
    };
    use tauri::{
        Manager, Window,
        utils::config::{CapabilityEntry, Config, Csp},
    };

    const CASES: [&str; 6] = [
        "accept-read",
        "cancel-review",
        "stop-queued",
        "stop-open",
        "expire-open",
        "parent-close",
    ];
    static CASE: OnceLock<String> = OnceLock::new();
    static NONCE: OnceLock<String> = OnceLock::new();
    static EVENTS: Mutex<u64> = Mutex::new(0);
    static FINISHED: AtomicBool = AtomicBool::new(false);

    fn context_label(case: &str, nonce: &str) -> String {
        format!("c1-{case}-{nonce}")
    }

    fn case_context() -> String {
        context_label(CASE.get().expect("case"), NONCE.get().expect("nonce"))
    }

    #[test]
    fn native_case_context_keeps_full_case_and_nonce() {
        let nonce = "0123456789abcdef0123456789abcdef";
        for case in CASES {
            let context = context_label(case, nonce);
            assert_eq!(context, format!("c1-{case}-{nonce}"));
            assert!(context.len() <= 128);
            assert_ne!(context, context_label(case, &"f".repeat(32)));
        }
    }

    // Stop and expiry are independent qualification cases. Refuse a Stop
    // control near the original selection deadline instead of allowing an
    // expired attempt to masquerade as a Stop-only native result.
    fn timely_stop(deadline: Instant, now: Instant) -> bool {
        deadline.saturating_duration_since(now) >= Duration::from_secs(10)
    }

    fn stop_returned_before_expiry(deadline: Instant, now: Instant) -> bool {
        now < deadline
    }

    #[test]
    fn native_stop_timing_accepts_exact_margin_without_renewing_deadline() {
        let now = Instant::now();
        assert!(timely_stop(now + Duration::from_secs(10), now));
        assert!(timely_stop(now + Duration::from_secs(120), now));
    }

    #[test]
    fn native_stop_timing_rejects_insufficient_and_expired_margin() {
        let now = Instant::now();
        assert!(!timely_stop(now + Duration::from_millis(9_999), now));
        assert!(!timely_stop(now, now));
        assert!(!timely_stop(now - Duration::from_secs(1), now));
    }

    #[test]
    fn native_stop_return_rejects_exact_expiry_and_later() {
        let deadline = Instant::now();
        assert!(stop_returned_before_expiry(
            deadline,
            deadline - Duration::from_nanos(1)
        ));
        assert!(!stop_returned_before_expiry(deadline, deadline));
        assert!(!stop_returned_before_expiry(
            deadline,
            deadline + Duration::from_nanos(1)
        ));
    }

    fn emit(event: &'static str, fields: Value) {
        let mut sequence = EVENTS.lock().expect("fixed event lock");
        *sequence += 1;
        let line = serde_json::to_vec(&json!({
            "schema":"rangoon.native-credential-qualification.v2",
            "case":CASE.get().expect("case"),"nonce":NONCE.get().expect("nonce"),
            "sequence":*sequence,"event":event,"fields":fields,
        }))
        .expect("fixed event encoding");
        assert!(
            *sequence <= 128 && line.len() <= 8192,
            "fixed evidence bound"
        );
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(&line).expect("event write");
        stdout.write_all(b"\n").expect("event line");
        stdout.flush().expect("event flush");
    }

    fn context_valid(config: &Config, assets_empty: bool) -> bool {
        config.identifier == "ai.rangoon.native-credential-qualification"
            && !config.bundle.active
            && !config.app.with_global_tauri
            && config.app.windows.is_empty()
            && matches!(
                config.app.security.csp.as_ref(),
                Some(Csp::Policy(policy)) if policy == "default-src 'none'"
            )
            && matches!(
                config.app.security.capabilities.as_slice(),
                [CapabilityEntry::Inlined(capability)]
                    if capability.identifier == "c1-no-permissions"
                        && !capability.local
                        && capability.remote.is_none()
                        && capability.windows.is_empty()
                        && capability.webviews.is_empty()
                        && capability.permissions.is_empty()
            )
            && config.build.dev_url.is_none()
            && config.build.frontend_dist.is_none()
            && assets_empty
    }

    fn qualification_context() -> tauri::Context<tauri::Wry> {
        tauri::generate_context!(
            "native-credential-qualification/tauri.conf.json",
            test = true,
            assets = tauri::utils::assets::EmbeddedAssets::new(
                tauri::utils::assets::phf::Map::new(),
                &[],
                tauri::utils::assets::phf::Map::new()
            )
        )
    }

    #[test]
    fn generated_native_context_is_inert_despite_omitted_empty_capability_lists() {
        let context = qualification_context();
        assert!(context_valid(
            context.config(),
            context.assets.iter().next().is_none()
        ));
        // Pinned Tauri omits these empty vectors when serializing. Safety must
        // inspect the actual typed configuration, not serialization presence.
        let serialized = serde_json::to_value(context.config()).unwrap();
        let capability = &serialized["app"]["security"]["capabilities"][0];
        assert!(capability.get("windows").is_none());
        assert!(capability.get("webviews").is_none());
        assert_eq!(capability["permissions"], json!([]));
    }

    #[test]
    fn generated_native_context_rejects_each_capability_and_surface_expansion() {
        fn capability(config: &mut Config) -> &mut tauri::utils::acl::capability::Capability {
            let CapabilityEntry::Inlined(capability) = &mut config.app.security.capabilities[0]
            else {
                panic!("fixture must have inline capability");
            };
            capability
        }
        let context = qualification_context();
        let baseline = context.config();
        assert!(context_valid(baseline, true));
        let mutations: [fn(&mut Config); 18] = [
            |c| c.identifier = "ai.rangoon.other".into(),
            |c| c.bundle.active = true,
            |c| c.app.with_global_tauri = true,
            |c| c.app.windows.push(Default::default()),
            |c| c.app.security.csp = None,
            |c| c.app.security.csp = Some(Csp::Policy("default-src *".into())),
            |c| c.app.security.capabilities.clear(),
            |c| c.app.security.capabilities[0] = CapabilityEntry::Reference("other".into()),
            |c| {
                c.app
                    .security
                    .capabilities
                    .push(c.app.security.capabilities[0].clone())
            },
            |c| capability(c).identifier = "other".into(),
            |c| capability(c).local = true,
            |c| {
                capability(c).remote = Some(
                    serde_json::from_value(json!({"urls":["https://example.invalid"]})).unwrap(),
                )
            },
            |c| capability(c).windows.push("*".into()),
            |c| capability(c).webviews.push("*".into()),
            |c| {
                capability(c)
                    .permissions
                    .push(serde_json::from_value(json!("core:default")).unwrap())
            },
            |c| c.build.dev_url = Some("https://example.invalid".parse().unwrap()),
            |c| c.build.frontend_dist = Some(serde_json::from_value(json!("../preview")).unwrap()),
            |c| c.app.security.csp = Some(Csp::DirectiveMap(Default::default())),
        ];
        for (index, mutate) in mutations.into_iter().enumerate() {
            let mut changed = baseline.clone();
            mutate(&mut changed);
            assert!(!context_valid(&changed, true), "unsafe mutation {index}");
        }
        assert!(
            !context_valid(baseline, false),
            "embedded assets must stay empty"
        );
    }

    fn stop_with_drain(
        host: &Host,
        gate: &Gate,
    ) -> (HostStatus, rangoon_desktop::workspace_lifecycle::Drain) {
        let status = host.stop();
        let drain = gate.request_drain().expect("memory stop drain");
        (status, drain)
    }

    #[test]
    fn native_stop_proof_retains_real_host_callback_through_drain() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let peer = runtime
            .block_on(SyntheticPeer::start(SyntheticScenario::Healthy))
            .unwrap();
        let gate = Gate::new();
        let host = Host::new(gate.clone());
        let workspace = WorkspaceRequest::Managed(Arc::new(gate.observe_open().unwrap()));
        let target = ProposedTarget::new(
            peer.origin().to_owned(),
            peer.selected_peer(),
            "c1-synthetic".into(),
            "workspace-1".into(),
            "sandbox-1".into(),
            peer.root_der().to_vec(),
            peer.leaf_der().to_vec(),
        )
        .unwrap();
        let review = host
            .prepare(
                &workspace,
                target,
                ReadKind::RecentLogs,
                Retention::SessionOnly,
            )
            .unwrap();
        let (entered, began) = mpsc::channel();
        let (release, held) = mpsc::channel();
        assert_eq!(
            host.begin_callback(
                &review.id,
                None,
                CallbackStart::NativeReview,
                Some("test-parent".into()),
                move |_, _, _, _, _| {
                    entered.send(()).unwrap();
                    let _ = held.recv();
                    Err(ClientError::Cancelled)
                },
            ),
            HostStatus::Running
        );
        began.recv_timeout(Duration::from_secs(2)).unwrap();
        // A read callback does not occupy the independent Exclusive lane.
        // Drain quiescence, not unrelated lane admission, proves custody.
        drop(gate.begin(Lane::Exclusive).unwrap());
        let (status, drain) = stop_with_drain(&host, &gate);
        assert_eq!(status, HostStatus::Closing);
        assert!(!drain.is_quiescent().unwrap());
        assert!(gate.begin(Lane::Read).is_err());
        assert!(gate.begin(Lane::Exclusive).is_err());
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while host.status() == HostStatus::Closing && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(host.status(), HostStatus::Cancelled);
        assert!(drain.is_quiescent().unwrap());
        assert!(peer.calls().unwrap().is_empty());
        assert!(host.cancel_and_check_exit());
        runtime.block_on(peer.shutdown()).unwrap();
    }

    pub(crate) fn run() {
        let args: Vec<_> = std::env::args().skip(1).take(5).collect();
        if args.len() != 4
            || args[0] != "--case"
            || !CASES.contains(&args[1].as_str())
            || args[2] != "--nonce"
            || args[3].len() != 32
            || !args[3]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            eprintln!("native_credential_invalid_case");
            std::process::exit(64);
        }
        CASE.set(args[1].clone()).expect("one case");
        NONCE.set(args[3].clone()).expect("one nonce");
        let plan: Value = serde_json::from_str(include_str!(
            "../native-credential-qualification/cases.v1.json"
        ))
        .expect("compiled plan");
        assert_eq!(
            plan,
            json!({"schema":"rangoon.native-credential-cases.v1", "cases":CASES.map(|id| json!({"id":id}))})
        );
        let context = qualification_context();
        assert!(
            context_valid(context.config(), context.assets.iter().next().is_none()),
            "native_credential_unsafe_context"
        );
        let gate = Gate::new();
        let app = tauri::Builder::default()
            .manage(Host::new(gate.clone()))
            .on_window_event(|window, event| {
                if matches!(
                    event,
                    tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
                ) {
                    window.state::<Host>().parent_closed(window.label());
                }
            })
            .setup(move |app| {
                let title = format!("Rangoon C1-M {}", case_context());
                let parent = tauri::WindowBuilder::new(app, "c1-native-parent")
                    .title(title)
                    .inner_size(820.0, 680.0)
                    .build()?;
                let host = app.state::<Host>().inner().clone();
                std::thread::Builder::new()
                    .name("rangoon-c1-qualification".into())
                    .spawn(move || {
                        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            qualify(parent, host, gate)
                        }))
                        .is_err()
                        {
                            eprintln!("native_credential_case_failed");
                            std::process::exit(70);
                        }
                    })?;
                Ok(())
            })
            .build(context)
            .expect("isolated native context");
        app.run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event
                && (!FINISHED.load(Ordering::Acquire)
                    || !app.state::<Host>().cancel_and_check_exit())
            {
                api.prevent_exit();
            }
        });
    }

    struct MainHold {
        release: mpsc::Sender<()>,
    }
    fn hold_main(parent: &Window) -> MainHold {
        let (release, held) = mpsc::channel();
        let (entered, began) = mpsc::channel();
        parent
            .run_on_main_thread(move || {
                emit("main_held", json!({}));
                let _ = entered.send(());
                let _ = held.recv(); // only owned test main loop, not production UI
                emit("main_released", json!({}));
            })
            .expect("queue owned hold");
        began
            .recv_timeout(Duration::from_secs(5))
            .expect("owned hold entered");
        MainHold { release }
    }

    fn commands() -> mpsc::Receiver<String> {
        let (tx, rx) = mpsc::sync_channel(8);
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            let mut input = stdin.lock();
            loop {
                let mut line = Vec::new();
                let mut terminated = false;
                for byte in input.by_ref().bytes().take(65) {
                    let byte = byte.expect("fixed control input");
                    if byte == b'\n' {
                        terminated = true;
                        break;
                    }
                    line.push(byte);
                }
                if line.is_empty() && !terminated {
                    return;
                }
                assert!(terminated && line.len() <= 64, "fixed control bound");
                let line = String::from_utf8(line).expect("fixed control encoding");
                assert!(
                    [
                        "stop",
                        "hold-main",
                        "release-main",
                        "close-parent",
                        "finish"
                    ]
                    .contains(&line.as_str()),
                    "fixed control command"
                );
                tx.try_send(line).expect("fixed control capacity");
            }
        });
        rx
    }

    fn qualify(parent: Window, host: Host, gate: Gate) {
        let case = CASE.get().expect("case").as_str();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("synthetic runtime");
        let peer = runtime
            .block_on(SyntheticPeer::start(SyntheticScenario::Healthy))
            .expect("generated local peer");
        let workspace =
            WorkspaceRequest::Managed(Arc::new(gate.observe_open().expect("memory generation")));
        let target = ProposedTarget::new(
            peer.origin().to_owned(),
            peer.selected_peer(),
            case_context(),
            "workspace-1".into(),
            "sandbox-1".into(),
            peer.root_der().to_vec(),
            peer.leaf_der().to_vec(),
        )
        .expect("synthetic target");
        let commands = commands();
        emit(
            "ready",
            json!({"parentLabel":"c1-native-parent", "pid":std::process::id(), "context":case_context()}),
        );
        let mut hold = (case == "stop-queued").then(|| hold_main(&parent));
        let review = host
            .prepare(
                &workspace,
                target,
                ReadKind::RecentLogs,
                Retention::SessionOnly,
            )
            .expect("memory review");
        let review_deadline = {
            let state = host.state.lock().expect("owned Host state");
            let pending = state.pending.as_ref().expect("owned pending review");
            assert_eq!(pending.review().id, review.id);
            pending.selection().deadline
        };
        let observer = Arc::new(|phase: native_input::NativePhase| {
            emit("phase", json!({"phase":phase.code()}))
        });
        assert_eq!(
            native_input::begin_observed(&host, &review.id, parent.clone(), observer),
            HostStatus::Running
        );
        let started = Instant::now();
        let mut stop_drain = None;
        loop {
            if let Ok(command) = commands.try_recv() {
                emit("command_received", json!({"command":command}));
                match command.as_str() {
                    "hold-main" if case == "stop-open" && hold.is_none() => {
                        hold = Some(hold_main(&parent))
                    }
                    "release-main" => hold
                        .take()
                        .expect("owned held main")
                        .release
                        .send(())
                        .expect("release owned main"),
                    "stop" if ["stop-queued", "stop-open"].contains(&case) && hold.is_some() => {
                        assert!(stop_drain.is_none(), "one stop drain per case");
                        assert!(
                            timely_stop(review_deadline, Instant::now()),
                            "native_credential_late_stop"
                        );
                        let (status, drain) = stop_with_drain(&host, &gate);
                        assert!(
                            stop_returned_before_expiry(review_deadline, Instant::now()),
                            "native_credential_stop_missed_deadline"
                        );
                        let retained = !drain.is_quiescent().expect("memory stop drain state");
                        stop_drain = Some(drain);
                        assert_eq!(status, HostStatus::Closing);
                        assert!(retained, "native callback custody retained");
                        // The fixed test-event field denotes retained custody.
                        // Stop itself does not close unrelated workspace lanes.
                        emit("stopping", json!({"status":"closing", "admitted":retained}));
                    }
                    "close-parent" if case == "parent-close" => {
                        parent.close().expect("close owned parent")
                    }
                    _ => panic!("native_credential_wrong_control"),
                }
            }
            let status = host.status();
            if !matches!(status, HostStatus::Running | HostStatus::Closing) {
                assert!(
                    hold.is_none(),
                    "native completion cannot precede held main release"
                );
                let observation = host.completed_observation(&workspace, &review.id).is_some();
                let calls = peer.calls().expect("synthetic call ledger");
                let counts = [
                    SyntheticRpc::Health,
                    SyntheticRpc::GetSandboxPolicyStatus,
                    SyntheticRpc::GetSandboxLogs,
                ]
                .map(|rpc| calls.iter().filter(|call| **call == rpc).count());
                let (label, code) = match status {
                    HostStatus::Completed => ("completed", None),
                    HostStatus::Cancelled => ("cancelled", Some("cancelled")),
                    HostStatus::Error(ClientError::CredentialLeaseExpired) => {
                        ("error", Some("credential_lease_expired"))
                    }
                    _ => panic!("native_credential_unexpected_outcome"),
                };
                match case {
                    "accept-read" => {
                        assert_eq!(status, HostStatus::Completed);
                        assert!(observation);
                        assert_eq!(counts, [1, 1, 1]);
                        assert_eq!(calls.len(), 3);
                    }
                    "expire-open" => {
                        assert_eq!(
                            status,
                            HostStatus::Error(ClientError::CredentialLeaseExpired)
                        );
                        assert!(started.elapsed() >= Duration::from_secs(119));
                        assert!(!observation);
                        assert!(calls.is_empty());
                    }
                    _ => {
                        assert_eq!(status, HostStatus::Cancelled);
                        assert!(!observation);
                        assert!(calls.is_empty());
                    }
                }
                emit(
                    "result",
                    json!({"status":label,"code":code,"observation":observation,
                    "rpcCounts":{"health":counts[0],"policyStatus":counts[1],"logs":counts[2]},
                    "authority":"none","execution":"unavailable","outcome":"unknown"}),
                );
                runtime
                    .block_on(peer.shutdown())
                    .expect("synthetic shutdown");
                drop(runtime);
                assert!(host.cancel_and_check_exit());
                let drain = stop_drain
                    .take()
                    .unwrap_or_else(|| gate.request_drain().expect("memory drain"));
                assert!(drain.is_quiescent().expect("memory drain state"));
                emit("exit", json!({"code":0,"clean":true}));
                FINISHED.store(true, Ordering::Release);
                parent.app_handle().exit(0);
                return;
            }
            assert!(
                started.elapsed() <= Duration::from_secs(185),
                "native_credential_case_timeout"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
