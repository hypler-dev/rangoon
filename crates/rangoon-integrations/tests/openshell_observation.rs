use rangoon_integrations::openshell_observation::{
    MAX_OPEN_SHELL_ITEMS, MAX_OPEN_SHELL_OUTPUT_BYTES, MAX_OPEN_SHELL_POLICY_REVISIONS,
    MAX_OPEN_SHELL_TIME_MS, OPEN_SHELL_VERSION, OpenShellItem, OpenShellObservationError,
    OpenShellProducer, OpenShellScope, OpenShellSourceKind, OpenShellStreamPosition,
    OpenShellWindow, PolicyLoadStatus, PolicyRevision, ReportedEvent, ReportedResult,
    WatchCondition, WatchStatus, map_openshell_window,
};
use serde_json::Value;

const FIXTURE: &str = include_str!("../../../fixtures/openshell-observation/v0.1.4.json");

fn scope(installation: &str) -> OpenShellScope {
    OpenShellScope {
        installation_id: installation.into(),
        gateway_display_name: "same untrusted gateway name".into(),
        workspace_id: "workspace:one".into(),
        sandbox_id: "sandbox:one".into(),
        source_kind: OpenShellSourceKind::SandboxStream,
        replica_id: Some("replica:one".into()),
        cursor_space: Some("cursor-space:one".into()),
        cursor: Some("cursor:one".into()),
    }
}

fn revision(id: &str, status: PolicyLoadStatus, active: Option<&str>) -> PolicyRevision {
    PolicyRevision {
        revision_id: id.into(),
        status,
        active_version: active.map(str::to_owned),
        provider_time_ms: Some(10),
        receipt_time_ms: Some(20),
    }
}

fn report(
    event_id: Option<&str>,
    native_id: Option<&str>,
    producer: OpenShellProducer,
    result: ReportedResult,
    revision_id: Option<&str>,
) -> OpenShellItem {
    OpenShellItem::Report(ReportedEvent {
        event_id: event_id.map(str::to_owned),
        native_id: native_id.map(str::to_owned),
        producer,
        result,
        policy_revision: revision_id.map(str::to_owned),
        provider_time_ms: Some(30),
        receipt_time_ms: Some(40),
        recent_buffer_truncated: false,
    })
}

fn window(items: Vec<OpenShellItem>, policy_revisions: Vec<PolicyRevision>) -> OpenShellWindow {
    OpenShellWindow {
        version: OPEN_SHELL_VERSION.into(),
        scope: scope("installation:one"),
        prior_stream: None,
        policy_revisions,
        items,
    }
}

fn policy_window(policy_revisions: Vec<PolicyRevision>) -> OpenShellWindow {
    let mut typed = window(vec![], policy_revisions);
    typed.scope.source_kind = OpenShellSourceKind::SandboxPolicyStatus;
    typed.scope.replica_id = None;
    typed.scope.cursor_space = None;
    typed.scope.cursor = None;
    typed
}

fn codes(mapped: &rangoon_integrations::openshell_observation::MappedOpenShellWindow) -> Vec<&str> {
    mapped
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn fixture_text<'a>(value: &'a Value, name: &str) -> &'a str {
    value[name].as_str().expect("fixture string")
}

fn fixture_optional_text(value: &Value, name: &str) -> Option<String> {
    value.get(name).and_then(Value::as_str).map(str::to_owned)
}

fn fixture_source_kind(value: &str) -> OpenShellSourceKind {
    match value {
        "policy" => OpenShellSourceKind::SandboxPolicyStatus,
        "log" => OpenShellSourceKind::SandboxLog,
        "stream" => OpenShellSourceKind::SandboxStream,
        _ => panic!("fixture source kind"),
    }
}

fn fixture_producer(value: &str) -> OpenShellProducer {
    match value {
        "sandbox_log" => OpenShellProducer::SandboxLog,
        "platform_status" => OpenShellProducer::PlatformStatus,
        "stream_status" => OpenShellProducer::StreamStatus,
        _ => panic!("fixture producer"),
    }
}

fn fixture_result(value: &str) -> ReportedResult {
    match value {
        "allowed" => ReportedResult::Allowed,
        "denied" => ReportedResult::Denied,
        "uncertain" => ReportedResult::Uncertain,
        _ => panic!("fixture result"),
    }
}

fn fixture_policy_status(value: &str) -> PolicyLoadStatus {
    match value {
        "pending" => PolicyLoadStatus::Pending,
        "loaded" => PolicyLoadStatus::Loaded,
        "failed" => PolicyLoadStatus::Failed,
        "superseded" => PolicyLoadStatus::Superseded,
        _ => panic!("fixture policy status"),
    }
}

fn fixture_window(input: &Value) -> OpenShellWindow {
    let source_kind = fixture_source_kind(fixture_text(input, "sourceKind"));
    let stream = source_kind == OpenShellSourceKind::SandboxStream;
    let scope = OpenShellScope {
        installation_id: fixture_optional_text(input, "installationId")
            .unwrap_or_else(|| "installation:one".into()),
        gateway_display_name: "fixture gateway display name".into(),
        workspace_id: fixture_optional_text(input, "workspaceId")
            .unwrap_or_else(|| "workspace:one".into()),
        sandbox_id: fixture_optional_text(input, "sandboxId")
            .unwrap_or_else(|| "sandbox:one".into()),
        source_kind,
        replica_id: stream
            .then(|| fixture_optional_text(input, "replicaId"))
            .flatten(),
        cursor_space: stream
            .then(|| fixture_optional_text(input, "cursorSpace"))
            .flatten(),
        cursor: stream
            .then(|| fixture_optional_text(input, "cursor"))
            .flatten(),
    };
    let prior_stream = input.get("prior").map(|prior| OpenShellStreamPosition {
        replica_id: fixture_optional_text(prior, "replicaId"),
        cursor_space: fixture_optional_text(prior, "cursorSpace"),
    });
    let policy_revisions = input
        .get("policyRevisions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|revision| PolicyRevision {
            revision_id: fixture_text(revision, "id").into(),
            status: fixture_policy_status(fixture_text(revision, "status")),
            active_version: fixture_optional_text(revision, "active"),
            provider_time_ms: Some(10),
            receipt_time_ms: Some(20),
        })
        .collect();
    let items = input
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| match fixture_text(item, "kind") {
            "report" => OpenShellItem::Report(ReportedEvent {
                event_id: fixture_optional_text(item, "eventId"),
                native_id: fixture_optional_text(item, "nativeId"),
                producer: fixture_producer(fixture_text(item, "producer")),
                result: fixture_result(fixture_text(item, "result")),
                policy_revision: fixture_optional_text(item, "revision"),
                provider_time_ms: Some(30),
                receipt_time_ms: Some(40),
                recent_buffer_truncated: item["truncated"].as_bool().unwrap_or(false),
            }),
            "watch" => OpenShellItem::Watch(WatchStatus {
                event_id: fixture_optional_text(item, "eventId"),
                condition: match fixture_text(item, "condition") {
                    "warning" => WatchCondition::Warning,
                    "out_of_range" => WatchCondition::OutOfRange,
                    "invalid_argument" => WatchCondition::InvalidArgument,
                    _ => panic!("fixture watch condition"),
                },
                provider_time_ms: Some(30),
                receipt_time_ms: Some(40),
            }),
            _ => panic!("fixture item kind"),
        })
        .collect();
    OpenShellWindow {
        version: OPEN_SHELL_VERSION.into(),
        scope,
        prior_stream,
        policy_revisions,
        items,
    }
}

#[test]
fn fixture_is_small_synthetic_typed_test_data() {
    assert!(FIXTURE.len() <= 262_144);
    let fixture: Value = serde_json::from_str(FIXTURE).expect("fixture JSON only parses in test");
    assert_eq!(fixture["fixtureKind"], "synthetic_typed_surrogate");
    assert_eq!(fixture["openshellVersion"], OPEN_SHELL_VERSION);
    assert_eq!(
        fixture["releaseResearchCommit"],
        "5a7728c304b5528e954e9ae900cf21f84e1a12b1"
    );
    assert_eq!(fixture["capturedNativeMessage"], false);
    assert_eq!(fixture["limits"]["fixtureBytes"], 262_144);
    assert_eq!(fixture["limits"]["items"], MAX_OPEN_SHELL_ITEMS);
    assert_eq!(
        fixture["limits"]["policyRevisions"],
        MAX_OPEN_SHELL_POLICY_REVISIONS
    );
    assert_eq!(
        fixture["limits"]["outputBytes"],
        MAX_OPEN_SHELL_OUTPUT_BYTES
    );
    let cases = fixture["cases"].as_array().expect("cases array");
    assert_eq!(
        cases
            .iter()
            .map(|case| fixture_text(case, "id"))
            .collect::<Vec<_>>(),
        vec![
            "denied_and_allowed_reported_policy",
            "pending_loaded_failed_superseded_and_active_version_mismatch",
            "changed_revisions",
            "repeated_and_conflicting_event_ids",
            "same_display_name_and_native_id_across_installations",
            "replica_and_cursor_space_change",
            "watch_warning_out_of_range_and_invalid_argument",
            "recent_buffer_truncation",
            "missing_workspace",
            "missing_sandbox",
            "missing_event_id",
            "mixed_producer",
            "uncertain_reported_result",
            "malformed_input",
            "oversized_items_input",
        ]
    );
    assert!(
        cases
            .iter()
            .all(|case| case["provenance"] == "synthetic_typed_surrogate")
    );
}

#[test]
fn replays_each_synthetic_fixture_case_through_typed_mapper() {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("fixture JSON only parses in test");
    for case in fixture["cases"].as_array().expect("fixture cases") {
        let expected = &case["expected"];
        let result = map_openshell_window(&fixture_window(&case["input"]));
        if let Some(error) = expected.get("error").and_then(Value::as_str) {
            assert_eq!(result.expect_err(fixture_text(case, "id")).code(), error);
            continue;
        }
        let mapped = result.unwrap_or_else(|_| panic!("{}", fixture_text(case, "id")));
        let expected_losses: Vec<_> = expected["losses"]
            .as_array()
            .expect("expected losses")
            .iter()
            .map(|value| value.as_str().expect("loss string"))
            .collect();
        let expected_diagnostics: Vec<_> = expected["diagnostics"]
            .as_array()
            .expect("expected diagnostics")
            .iter()
            .map(|value| value.as_str().expect("diagnostic string"))
            .collect();
        assert_eq!(
            mapped.losses,
            expected_losses,
            "{}",
            fixture_text(case, "id")
        );
        assert_eq!(
            codes(&mapped),
            expected_diagnostics,
            "{}",
            fixture_text(case, "id")
        );
        assert_eq!(mapped.authority, "none");
        assert_eq!(mapped.lnsat_authorization, "unavailable");
        assert_eq!(mapped.execution, "unavailable");
        assert_eq!(mapped.verified_effect, "unavailable");
        let expected_scope = &expected["scope"];
        assert_eq!(
            mapped.scope.installation_id,
            fixture_text(expected_scope, "installationId")
        );
        assert_eq!(
            mapped.scope.workspace_id,
            fixture_text(expected_scope, "workspaceId")
        );
        assert_eq!(
            mapped.scope.sandbox_id,
            fixture_text(expected_scope, "sandboxId")
        );
        assert_eq!(
            mapped.scope.source_kind,
            fixture_source_kind(fixture_text(expected_scope, "sourceKind"))
        );
    }
}

#[test]
fn maps_reported_allow_deny_and_never_grants_authority() {
    let mapped = map_openshell_window(&window(
        vec![
            report(
                Some("event:allow"),
                Some("native:one"),
                OpenShellProducer::SandboxLog,
                ReportedResult::Allowed,
                Some("revision:loaded"),
            ),
            report(
                Some("event:deny"),
                Some("native:two"),
                OpenShellProducer::SandboxLog,
                ReportedResult::Denied,
                Some("revision:loaded"),
            ),
        ],
        vec![],
    ))
    .expect("typed reported results map");

    assert_eq!(
        mapped.items[0].reported_result,
        Some(ReportedResult::Allowed)
    );
    assert_eq!(
        mapped.items[1].reported_result,
        Some(ReportedResult::Denied)
    );
    assert_eq!(mapped.authority, "none");
    assert_eq!(mapped.lnsat_authorization, "unavailable");
    assert_eq!(mapped.execution, "unavailable");
    assert_eq!(mapped.verified_effect, "unavailable");
}

#[test]
fn preserves_policy_load_states_and_active_version_mismatch() {
    let mapped = map_openshell_window(&policy_window(vec![
        revision(
            "revision:pending",
            PolicyLoadStatus::Pending,
            Some("revision:loaded"),
        ),
        revision(
            "revision:loaded",
            PolicyLoadStatus::Loaded,
            Some("revision:loaded"),
        ),
        revision(
            "revision:failed",
            PolicyLoadStatus::Failed,
            Some("revision:loaded"),
        ),
        revision(
            "revision:superseded",
            PolicyLoadStatus::Superseded,
            Some("revision:loaded"),
        ),
    ]))
    .expect("typed revisions map");

    assert_eq!(mapped.policy_revisions[0].status, PolicyLoadStatus::Pending);
    assert_eq!(mapped.policy_revisions[0].effective_policy, "unassessed");
    assert_eq!(
        mapped.policy_revisions[1].effective_policy,
        "reported_loaded"
    );
    assert_eq!(mapped.policy_revisions[2].status, PolicyLoadStatus::Failed);
    assert_eq!(
        mapped.policy_revisions[3].status,
        PolicyLoadStatus::Superseded
    );
    assert!(codes(&mapped).contains(&"active_version_mismatch"));
    assert!(codes(&mapped).contains(&"policy_revision_changed"));
}

#[test]
fn detects_repeated_conflicting_and_mixed_producer_events() {
    let mapped = map_openshell_window(&window(
        vec![
            report(
                Some("event:repeat"),
                Some("native:one"),
                OpenShellProducer::SandboxLog,
                ReportedResult::Allowed,
                None,
            ),
            report(
                Some("event:repeat"),
                Some("native:one"),
                OpenShellProducer::SandboxLog,
                ReportedResult::Allowed,
                None,
            ),
            report(
                Some("event:repeat"),
                Some("native:two"),
                OpenShellProducer::PlatformStatus,
                ReportedResult::Denied,
                None,
            ),
        ],
        vec![],
    ))
    .expect("repeated typed IDs remain visible");

    assert!(codes(&mapped).contains(&"repeated_event_id"));
    assert!(codes(&mapped).contains(&"conflicting_event_id"));
    assert!(codes(&mapped).contains(&"mixed_producer"));
}

#[test]
fn source_qualified_identity_keeps_installations_replicas_and_cursor_spaces_separate() {
    let first = map_openshell_window(&window(
        vec![report(
            Some("event:one"),
            Some("native:same"),
            OpenShellProducer::SandboxLog,
            ReportedResult::Uncertain,
            None,
        )],
        vec![],
    ))
    .expect("first installation maps");
    let mut second_window = window(
        vec![report(
            Some("event:one"),
            Some("native:same"),
            OpenShellProducer::SandboxLog,
            ReportedResult::Uncertain,
            None,
        )],
        vec![],
    );
    second_window.scope = OpenShellScope {
        installation_id: "installation:two".into(),
        gateway_display_name: "same untrusted gateway name".into(),
        workspace_id: "workspace:one".into(),
        sandbox_id: "sandbox:one".into(),
        source_kind: OpenShellSourceKind::SandboxStream,
        replica_id: Some("replica:two".into()),
        cursor_space: Some("cursor-space:two".into()),
        cursor: Some("cursor:one".into()),
    };
    second_window.prior_stream = Some(OpenShellStreamPosition {
        replica_id: Some("replica:one".into()),
        cursor_space: Some("cursor-space:one".into()),
    });
    let second = map_openshell_window(&second_window).expect("second installation maps");

    assert_ne!(first.scope.installation_id, second.scope.installation_id);
    assert_ne!(first.scope.replica_id, second.scope.replica_id);
    assert_ne!(first.scope.cursor_space, second.scope.cursor_space);
    assert_eq!(first.items[0].native_id, second.items[0].native_id);
    assert_eq!(
        first.items[0].reported_result,
        Some(ReportedResult::Uncertain)
    );
    assert!(second.losses.contains(&"stream_continuity_replica_changed"));
    assert!(
        second
            .losses
            .contains(&"stream_continuity_cursor_space_changed")
    );
}

#[test]
fn classifies_watch_warning_gap_and_invalid_input_without_inventing_history() {
    let mapped = map_openshell_window(&window(
        vec![
            OpenShellItem::Watch(WatchStatus {
                event_id: Some("event:warning".into()),
                condition: WatchCondition::Warning,
                provider_time_ms: Some(1),
                receipt_time_ms: Some(2),
            }),
            OpenShellItem::Watch(WatchStatus {
                event_id: Some("event:gap".into()),
                condition: WatchCondition::OutOfRange,
                provider_time_ms: Some(3),
                receipt_time_ms: Some(4),
            }),
            OpenShellItem::Watch(WatchStatus {
                event_id: Some("event:invalid".into()),
                condition: WatchCondition::InvalidArgument,
                provider_time_ms: Some(5),
                receipt_time_ms: Some(6),
            }),
        ],
        vec![],
    ))
    .expect("watch statuses map");

    assert_eq!(mapped.items[0].stream_state, "watch_warning");
    assert_eq!(mapped.items[1].stream_state, "gap");
    assert_eq!(mapped.items[2].stream_state, "invalid_input");
    assert!(mapped.losses.contains(&"watch_gap_warning"));
    assert!(mapped.losses.contains(&"watch_gap_out_of_range"));
}

#[test]
fn records_buffer_truncation_and_unknown_revision_as_loss() {
    let mut event = match report(
        Some("event:truncated"),
        None,
        OpenShellProducer::StreamStatus,
        ReportedResult::Uncertain,
        Some("revision:missing"),
    ) {
        OpenShellItem::Report(event) => event,
        OpenShellItem::Watch(_) => unreachable!(),
    };
    event.recent_buffer_truncated = true;
    let mapped = map_openshell_window(&window(vec![OpenShellItem::Report(event)], vec![]))
        .expect("reported incompleteness maps");

    assert!(mapped.losses.contains(&"recent_buffer_truncated"));
    assert!(
        mapped
            .losses
            .contains(&"reported_policy_revision_not_in_window")
    );
}

#[test]
fn rejects_missing_or_malformed_ids_scope_times_versions_and_window_limits() {
    let mut missing_scope = window(vec![], vec![]);
    missing_scope.scope.workspace_id.clear();
    assert_eq!(
        map_openshell_window(&missing_scope),
        Err(OpenShellObservationError::InvalidInput)
    );

    let mut missing_event = window(
        vec![report(
            Some(""),
            None,
            OpenShellProducer::SandboxLog,
            ReportedResult::Allowed,
            None,
        )],
        vec![],
    );
    assert_eq!(
        map_openshell_window(&missing_event),
        Err(OpenShellObservationError::InvalidInput)
    );
    match &mut missing_event.items[0] {
        OpenShellItem::Report(event) => event.event_id = Some("event:ok".into()),
        OpenShellItem::Watch(_) => unreachable!(),
    }
    missing_event.scope.cursor_space = None;
    assert_eq!(
        map_openshell_window(&missing_event),
        Err(OpenShellObservationError::InvalidInput)
    );

    let mut too_late = window(
        vec![report(
            Some("event:late"),
            None,
            OpenShellProducer::SandboxLog,
            ReportedResult::Allowed,
            None,
        )],
        vec![],
    );
    match &mut too_late.items[0] {
        OpenShellItem::Report(event) => event.provider_time_ms = Some(MAX_OPEN_SHELL_TIME_MS + 1),
        OpenShellItem::Watch(_) => unreachable!(),
    }
    assert_eq!(
        map_openshell_window(&too_late),
        Err(OpenShellObservationError::InvalidInput)
    );

    let mut unsupported = window(vec![], vec![]);
    unsupported.version = "0.1.5".into();
    assert_eq!(
        map_openshell_window(&unsupported),
        Err(OpenShellObservationError::UnsupportedVersion)
    );

    let too_many_items = window(
        (0..=MAX_OPEN_SHELL_ITEMS)
            .map(|index| {
                report(
                    Some(&format!("event:{index}")),
                    None,
                    OpenShellProducer::SandboxLog,
                    ReportedResult::Allowed,
                    None,
                )
            })
            .collect(),
        vec![],
    );
    assert_eq!(
        map_openshell_window(&too_many_items),
        Err(OpenShellObservationError::InputLimit)
    );

    let too_many_revisions = policy_window(
        (0..=MAX_OPEN_SHELL_POLICY_REVISIONS)
            .map(|index| revision(&format!("revision:{index}"), PolicyLoadStatus::Loaded, None))
            .collect(),
    );
    assert_eq!(
        map_openshell_window(&too_many_revisions),
        Err(OpenShellObservationError::InputLimit)
    );
}

#[test]
fn rejects_mixed_source_shapes_before_mapping() {
    let mut policy_with_report = policy_window(vec![]);
    policy_with_report.items.push(report(
        Some("event:report"),
        None,
        OpenShellProducer::SandboxLog,
        ReportedResult::Allowed,
        None,
    ));
    assert_eq!(
        map_openshell_window(&policy_with_report),
        Err(OpenShellObservationError::InvalidInput)
    );

    let mut log_with_cursor = window(
        vec![report(
            Some("event:log"),
            None,
            OpenShellProducer::SandboxLog,
            ReportedResult::Allowed,
            None,
        )],
        vec![],
    );
    log_with_cursor.scope.source_kind = OpenShellSourceKind::SandboxLog;
    assert_eq!(
        map_openshell_window(&log_with_cursor),
        Err(OpenShellObservationError::InvalidInput)
    );

    let mut log_with_platform = log_with_cursor.clone();
    log_with_platform.scope.replica_id = None;
    log_with_platform.scope.cursor_space = None;
    log_with_platform.scope.cursor = None;
    match &mut log_with_platform.items[0] {
        OpenShellItem::Report(report) => report.producer = OpenShellProducer::PlatformStatus,
        OpenShellItem::Watch(_) => unreachable!(),
    }
    assert_eq!(
        map_openshell_window(&log_with_platform),
        Err(OpenShellObservationError::InvalidInput)
    );

    let stream_with_policy = window(
        vec![],
        vec![revision("revision:one", PolicyLoadStatus::Loaded, None)],
    );
    assert_eq!(
        map_openshell_window(&stream_with_policy),
        Err(OpenShellObservationError::InvalidInput)
    );

    let policy_status_in_stream = window(
        vec![report(
            Some("event:policy-status"),
            None,
            OpenShellProducer::PolicyStatus,
            ReportedResult::Uncertain,
            None,
        )],
        vec![],
    );
    assert_eq!(
        map_openshell_window(&policy_status_in_stream),
        Err(OpenShellObservationError::InvalidInput)
    );
}

#[test]
fn mapped_output_stays_bounded_and_does_not_copy_display_name() {
    let mapped = map_openshell_window(&window(
        vec![report(
            Some("event:one"),
            None,
            OpenShellProducer::SandboxLog,
            ReportedResult::Allowed,
            None,
        )],
        vec![],
    ))
    .expect("typed window maps");
    let bytes = mapped.minimized_json_bytes().expect("bounded output");
    assert!(bytes.len() <= MAX_OPEN_SHELL_OUTPUT_BYTES);
    assert!(
        !String::from_utf8(bytes)
            .expect("JSON UTF-8")
            .contains("same untrusted gateway name")
    );
}

#[test]
fn missing_native_event_id_is_loss_not_fabricated_identity_and_cursor_is_opaque() {
    let mut typed = window(
        vec![report(
            None,
            None,
            OpenShellProducer::SandboxLog,
            ReportedResult::Uncertain,
            None,
        )],
        vec![],
    );
    typed.scope.cursor = Some("opaque/base64+cursor=".into());
    let mapped = map_openshell_window(&typed).expect("opaque cursor and missing ID map");

    assert_eq!(mapped.items[0].event_id, None);
    assert!(mapped.losses.contains(&"native_event_id_missing"));
    assert_eq!(
        mapped.scope.cursor.as_deref(),
        Some("opaque/base64+cursor=")
    );

    typed.scope.cursor = Some("x".repeat(513));
    assert_eq!(
        map_openshell_window(&typed),
        Err(OpenShellObservationError::InvalidInput)
    );
    typed.scope.cursor = Some("cursor\ncontrol".into());
    assert_eq!(
        map_openshell_window(&typed),
        Err(OpenShellObservationError::InvalidInput)
    );
}
