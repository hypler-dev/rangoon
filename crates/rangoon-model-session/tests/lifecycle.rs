use rangoon_import::analyze;
use rangoon_model_session::{Diagnostic, Freshness, LocalSession, OperationKind};
use rangoon_store::Workspace;
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rangoon-model-session-{}-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        )))
    }

    fn workspace(&self) -> Workspace {
        Workspace::new(self.0.clone())
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn identifier(kind: &str, byte: char) -> String {
    format!("{kind}:{}", byte.to_string().repeat(64))
}

fn profile(model: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.local-profile-request.v1",
        "profileId": "fixture.local",
        "host": "127.0.0.1",
        "port": 11434,
        "model": model,
        "maxOutputTokens": 256,
    }))
    .unwrap()
}

fn session() -> LocalSession {
    let result = LocalSession::default();
    result.configure(&profile("fixture:v1")).unwrap();
    result
}

fn source_selector(id: &str) -> Value {
    json!({"kind": "source", "sourceId": id})
}

fn capability_selector(capability_id: &str, revision_id: &str) -> Value {
    json!({
        "kind": "capability",
        "capabilityId": capability_id,
        "revisionId": revision_id,
    })
}

fn selection(task: &str, inputs: Vec<Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.local-selection.v1",
        "task": task,
        "inputs": inputs,
    }))
    .unwrap()
}

fn send(prepared_id: &str, request_id: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.local-send.v1",
        "preparedId": prepared_id,
        "requestId": request_id,
    }))
    .unwrap()
}

fn cancel(run_id: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.local-cancel.v1",
        "runId": run_id,
    }))
    .unwrap()
}

fn save(dir: &TestDir, name: &str, bytes: &[u8]) -> rangoon_domain::AnalysisReport {
    let report = analyze(name, bytes).unwrap();
    dir.workspace().save_v1(&report).unwrap();
    report
}

fn prepare_source(
    session: &LocalSession,
    store: &Workspace,
    source_id: &str,
) -> rangoon_model_session::PreparedView {
    session
        .begin_prepare(&selection("classify_v1", vec![source_selector(source_id)]))
        .unwrap()
        .prepare(store)
        .unwrap()
}

macro_rules! assert_error {
    ($result:expr, $diagnostic:expr) => {
        assert!(matches!($result, Err(error) if error == $diagnostic));
    };
}

#[test]
fn selector_is_closed_bounded_and_rejects_duplicate_json_keys() {
    let session = session();
    let source = identifier("source", 'a');
    let valid = selection("classify_v1", vec![source_selector(&source)]);

    let preparation = session.begin_prepare(&valid).unwrap();
    assert_eq!(
        session.inspect().unwrap().active.unwrap().kind,
        OperationKind::Prepare
    );
    drop(preparation);

    for raw in [
        br#"{"schemaVersion":"rangoon.local-selection.v1","task":"classify_v1","inputs":[{"kind":"source","sourceId":"source:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}],"extra":true}"#.to_vec(),
        br#"{"schemaVersion":"rangoon.local-selection.v1","schema\u0056ersion":"rangoon.local-selection.v1","task":"classify_v1","inputs":[{"kind":"source","sourceId":"source:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]}"#.to_vec(),
        br#"{"schemaVersion":"rangoon.local-selection.v1","task":"classify_v1","inputs":[{"kind":"source","k\u0069nd":"source","sourceId":"source:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]}"#.to_vec(),
        selection("compare_v1", vec![source_selector(&source), source_selector(&source)]),
        selection("classify_v1", vec![]),
        selection("classify_v1", vec![source_selector(&source), source_selector(&source)]),
    ] {
        assert_error!(session.begin_prepare(&raw), Diagnostic::InvalidRequest);
    }

    let many = (0..17)
        .map(|index| source_selector(&format!("source:{index:064x}")))
        .collect();
    assert_error!(
        session.begin_prepare(&selection("classify_v1", many)),
        Diagnostic::InvalidRequest
    );

    let sixteen = (0..16)
        .map(|index| source_selector(&format!("source:{index:064x}")))
        .collect();
    let preparation = session
        .begin_prepare(&selection("classify_v1", sixteen))
        .unwrap();
    drop(preparation);

    let mut at_limit = valid.clone();
    at_limit.resize(8192, b' ');
    let preparation = session.begin_prepare(&at_limit).unwrap();
    drop(preparation);
    at_limit.push(b' ');
    assert_error!(session.begin_prepare(&at_limit), Diagnostic::InvalidRequest);
}

#[test]
fn send_and_cancel_raw_commands_obey_closed_schema_and_exact_limits() {
    let session = session();
    let prepared = identifier("prepared", 'a');
    let request = "b".repeat(64);
    let mut at_send_limit = send(&prepared, &request);
    at_send_limit.resize(512, b' ');
    assert_error!(
        session.begin_send(&at_send_limit),
        Diagnostic::StalePrepared
    );
    at_send_limit.push(b' ');
    assert_error!(
        session.begin_send(&at_send_limit),
        Diagnostic::InvalidRequest
    );

    for raw in [
        br#"{"schemaVersion":"rangoon.local-send.v1","preparedId":"prepared:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","requestId":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","request\u0049d":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}"#.to_vec(),
        br#"{"schemaVersion":"rangoon.local-send.v1","preparedId":"prepared:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","requestId":"BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"}"#.to_vec(),
    ] {
        assert_error!(session.begin_send(&raw), Diagnostic::InvalidRequest);
    }

    let active = session.begin_check().unwrap();
    let run_id = active.run_id().to_owned();
    let mut at_cancel_limit = cancel(&run_id);
    at_cancel_limit.resize(256, b' ');
    session.cancel(&at_cancel_limit).unwrap();
    assert_eq!(active.ensure_current(), Err(Diagnostic::Cancelled));
    drop(active);

    let active = session.begin_check().unwrap();
    let duplicate_cancel = format!(
        "{{\"schemaVersion\":\"rangoon.local-cancel.v1\",\"runId\":\"{}\",\"run\\u0049d\":\"{}\"}}",
        active.run_id(),
        active.run_id(),
    );
    assert_eq!(
        session.cancel(duplicate_cancel.as_bytes()),
        Err(Diagnostic::InvalidRequest)
    );
    active.ensure_current().unwrap();
    drop(active);

    let active = session.begin_check().unwrap();
    let mut over_cancel_limit = cancel(active.run_id());
    over_cancel_limit.resize(257, b' ');
    assert_eq!(
        session.cancel(&over_cancel_limit),
        Err(Diagnostic::InvalidRequest)
    );
    active.ensure_current().unwrap();
    drop(active);

    assert_eq!(
        session.cancel(&cancel(&identifier("run", 'c'))),
        Err(Diagnostic::RunNotFound)
    );
}

#[test]
fn preparation_preserves_exact_bom_crlf_and_unicode_saved_bytes() {
    let dir = TestDir::new();
    let bytes = b"\xef\xbb\xbf# \xce\x94elta\r\nKeep \xf0\x9f\x94\x92 exact.\r\n";
    let source = save(&dir, "AGENTS.md", bytes);
    let session = session();
    let view = prepare_source(&session, &dir.workspace(), &source.source.id);

    assert_eq!(view.inputs.len(), 1);
    assert_eq!(view.inputs[0].byte_length, bytes.len());
    let outer: Value = serde_json::from_str(&view.body_json).unwrap();
    let pack: Value =
        serde_json::from_str(outer["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(pack["blocks"][0]["text"], source.source.content);
    assert_eq!(
        pack["inputs"][0]["selectedRanges"],
        json!([{ "startByte": 0, "endByte": bytes.len() }])
    );
    assert_eq!(
        pack["inputs"][0]["protectedRanges"],
        json!([{ "startByte": 0, "endByte": bytes.len() }])
    );
    assert_eq!(view.authority, "none");
    assert_eq!(view.processing_location, "unknown");
    assert_eq!(view.retention, "unknown");
}

#[test]
fn empty_saved_input_is_rejected_and_cannot_stage_a_request() {
    let dir = TestDir::new();
    let source = save(&dir, "empty.md", b"");
    let session = session();
    let preparation = session
        .begin_prepare(&selection(
            "classify_v1",
            vec![source_selector(&source.source.id)],
        ))
        .unwrap();
    assert_error!(
        preparation.prepare(&dir.workspace()),
        Diagnostic::InputEmpty
    );
    assert_error!(
        session.begin_send(&send(&identifier("prepared", 'a'), &"b".repeat(64))),
        Diagnostic::StalePrepared
    );
}

#[test]
fn profile_replacement_malformed_preparation_and_replay_consume_old_handles() {
    let dir = TestDir::new();
    let source = save(&dir, "rules.md", b"Keep this selected text.\n");
    let session = session();
    let first = prepare_source(&session, &dir.workspace(), &source.source.id);

    session.configure(&profile("replacement:v2")).unwrap();
    assert_error!(
        session.begin_send(&send(&first.prepared_id, &first.request_id)),
        Diagnostic::StalePrepared
    );

    let second = prepare_source(&session, &dir.workspace(), &source.source.id);
    assert_error!(
        session.begin_prepare(
            br#"{"schemaVersion":"rangoon.local-selection.v1","task":"wrong","inputs":[]}"#
        ),
        Diagnostic::InvalidRequest
    );
    assert_error!(
        session.begin_send(&send(&second.prepared_id, &second.request_id)),
        Diagnostic::StalePrepared
    );

    let third = prepare_source(&session, &dir.workspace(), &source.source.id);
    let transmission = session
        .begin_send(&send(&third.prepared_id, &third.request_id))
        .unwrap();
    drop(transmission);
    assert_error!(
        session.begin_send(&send(&third.prepared_id, &third.request_id)),
        Diagnostic::StalePrepared
    );
}

#[test]
fn failed_prepare_while_busy_discards_old_handle_but_invalid_profile_does_not() {
    let dir = TestDir::new();
    let source = save(&dir, "rules.md", b"Keep this selected text.\n");
    let session = session();

    let preserved = prepare_source(&session, &dir.workspace(), &source.source.id);
    assert_error!(
        session.configure(&profile("not-a-valid-model")),
        Diagnostic::InvalidProfile
    );
    let transmission = session
        .begin_send(&send(&preserved.prepared_id, &preserved.request_id))
        .unwrap();
    drop(transmission);

    let discarded = prepare_source(&session, &dir.workspace(), &source.source.id);
    let active = session.begin_check().unwrap();
    assert_error!(
        session.begin_prepare(&selection(
            "classify_v1",
            vec![source_selector(&source.source.id)],
        )),
        Diagnostic::Busy
    );
    drop(active);
    assert_error!(
        session.begin_send(&send(&discarded.prepared_id, &discarded.request_id)),
        Diagnostic::StalePrepared
    );
}

#[test]
fn wrong_owner_revision_is_unavailable_and_never_resolves_by_revision_alone() {
    let dir = TestDir::new();
    let source = save(&dir, "rules.md", b"Keep this selected text.\n");
    let fragment = source.fragments.first().unwrap();
    let first = dir
        .workspace()
        .create_capability_v1(&source.source.id, &fragment.id, "First")
        .unwrap()
        .capability;
    let second = dir
        .workspace()
        .create_capability_v1(&source.source.id, &fragment.id, "Second")
        .unwrap()
        .capability;
    let session = session();
    let preparation = session
        .begin_prepare(&selection(
            "classify_v1",
            vec![capability_selector(&first.id, &second.latest_revision_id)],
        ))
        .unwrap();

    assert_error!(
        preparation.prepare(&dir.workspace()),
        Diagnostic::InputUnavailable
    );
}

#[test]
fn cancel_or_clear_before_prepare_prevents_staging() {
    let dir = TestDir::new();
    let source = save(&dir, "rules.md", b"Keep this selected text.\n");
    let selection = selection("classify_v1", vec![source_selector(&source.source.id)]);
    let session = session();

    let preparation = session.begin_prepare(&selection).unwrap();
    session
        .cancel(&cancel(preparation.operation().run_id()))
        .unwrap();
    assert_error!(preparation.prepare(&dir.workspace()), Diagnostic::Cancelled);
    assert!(session.inspect().unwrap().active.is_none());

    let preparation = session.begin_prepare(&selection).unwrap();
    session.clear().unwrap();
    assert_error!(preparation.prepare(&dir.workspace()), Diagnostic::Cancelled);
    let view = session.inspect().unwrap();
    assert!(view.active.is_none());
    assert!(view.profile.is_none());
}

#[test]
fn cancelled_transmission_freshness_returns_cancelled() {
    let dir = TestDir::new();
    let source = save(&dir, "rules.md", b"Keep this selected text.\n");
    let session = session();
    let view = prepare_source(&session, &dir.workspace(), &source.source.id);
    let transmission = session
        .begin_send(&send(&view.prepared_id, &view.request_id))
        .unwrap();
    session
        .cancel(&cancel(transmission.operation().run_id()))
        .unwrap();

    assert_error!(
        transmission.freshness(&dir.workspace()),
        Diagnostic::Cancelled
    );
}

#[test]
fn stale_run_cancellation_cannot_affect_newer_operation() {
    let session = session();
    let old = session.begin_check().unwrap();
    let stale_run_id = old.run_id().to_owned();
    drop(old);

    let current = session.begin_check().unwrap();
    assert_eq!(
        session.cancel(&cancel(&stale_run_id)),
        Err(Diagnostic::RunNotFound)
    );
    current.ensure_current().unwrap();
    drop(current);
}

#[test]
fn saved_multiline_input_at_native_byte_limit_rejects_without_truncating_workspace_content() {
    let dir = TestDir::new();
    let mut bytes = Vec::with_capacity(262_144);
    for _ in 0..256 {
        bytes.extend_from_slice(&[b'x'; 1023]);
        bytes.push(b'\n');
    }
    assert_eq!(bytes.len(), 262_144);
    let source = save(&dir, "large.md", &bytes);
    assert_eq!(source.source.content.as_bytes(), bytes);
    let session = session();
    let preparation = session
        .begin_prepare(&selection(
            "classify_v1",
            vec![source_selector(&source.source.id)],
        ))
        .unwrap();

    assert_error!(
        preparation.prepare(&dir.workspace()),
        Diagnostic::PackOverBudget
    );
    let reopened = dir.workspace().open(&source.source.id).unwrap();
    assert_eq!(reopened.source.content.as_bytes(), bytes);
}

#[test]
fn cancellation_clear_and_dropped_leases_only_release_their_own_slot() {
    let session = session();
    let active = session.begin_check().unwrap();
    let run_id = active.run_id().to_owned();
    assert_eq!(session.inspect().unwrap().active.unwrap().run_id, run_id);
    session.clear().unwrap();
    assert_eq!(active.ensure_current(), Err(Diagnostic::Cancelled));
    assert!(session.inspect().unwrap().profile.is_none());
    assert_error!(session.configure(&profile("fixture:v1")), Diagnostic::Busy);
    drop(active);

    session.configure(&profile("fixture:v1")).unwrap();
    let dropped = session.begin_check().unwrap();
    drop(dropped);
    assert!(session.inspect().unwrap().active.is_none());
    let next = session.begin_check().unwrap();
    assert_eq!(next.profile().model(), "fixture:v1");
    drop(next);
}

#[test]
fn capability_freshness_detects_changed_head_but_ignores_unrelated_insertions() {
    let dir = TestDir::new();
    let source = save(&dir, "rules.md", b"One selected rule.\n");
    let fragment = source.fragments.first().unwrap();
    let capability = dir
        .workspace()
        .create_capability_v1(&source.source.id, &fragment.id, "Rules")
        .unwrap()
        .capability;
    let other_capability = dir
        .workspace()
        .create_capability_v1(&source.source.id, &fragment.id, "Other rules")
        .unwrap()
        .capability;
    let session = session();
    assert_error!(
        session.begin_prepare(&selection(
            "compare_v1",
            vec![
                capability_selector(&capability.id, &capability.latest_revision_id),
                capability_selector(&capability.id, &capability.latest_revision_id),
            ],
        )),
        Diagnostic::InvalidRequest
    );

    let compare = session
        .begin_prepare(&selection(
            "compare_v1",
            vec![
                capability_selector(&capability.id, &capability.latest_revision_id),
                capability_selector(&other_capability.id, &other_capability.latest_revision_id),
            ],
        ))
        .unwrap()
        .prepare(&dir.workspace())
        .unwrap();
    assert_eq!(compare.inputs.len(), 2);

    let staged = session
        .begin_prepare(&selection(
            "classify_v1",
            vec![capability_selector(
                &capability.id,
                &capability.latest_revision_id,
            )],
        ))
        .unwrap()
        .prepare(&dir.workspace())
        .unwrap();
    let transmission = session
        .begin_send(&send(&staged.prepared_id, &staged.request_id))
        .unwrap();
    assert_eq!(
        transmission.freshness(&dir.workspace()).unwrap(),
        Freshness::Current
    );

    let _unrelated = save(&dir, "unrelated.md", b"Unrelated saved input.\n");
    assert_eq!(
        transmission.freshness(&dir.workspace()).unwrap(),
        Freshness::Current
    );

    let revised = dir
        .workspace()
        .revise_capability_v1(
            &capability.id,
            &capability.latest_revision_id,
            "Rules v2",
            "A changed head.\n",
        )
        .unwrap()
        .capability;
    assert_ne!(revised.latest_revision_id, capability.latest_revision_id);
    assert_eq!(
        transmission.freshness(&dir.workspace()).unwrap(),
        Freshness::Stale
    );
}

#[test]
fn missing_or_corrupt_saved_records_are_unavailable_before_dispatch() {
    let dir = TestDir::new();
    let source = save(&dir, "rules.md", b"Selected record.\n");
    let first_session = session();
    let view = prepare_source(&first_session, &dir.workspace(), &source.source.id);
    let transmission = first_session
        .begin_send(&send(&view.prepared_id, &view.request_id))
        .unwrap();
    fs::remove_file(dir.0.join("workspace.sqlite3")).unwrap();
    assert_eq!(
        transmission.freshness(&dir.workspace()).unwrap(),
        Freshness::Unavailable
    );

    let corrupt = TestDir::new();
    let source = save(&corrupt, "corrupt.md", b"Selected record.\n");
    let session = session();
    let view = prepare_source(&session, &corrupt.workspace(), &source.source.id);
    let transmission = session
        .begin_send(&send(&view.prepared_id, &view.request_id))
        .unwrap();
    fs::write(
        corrupt.0.join("workspace.sqlite3"),
        b"not a sqlite database",
    )
    .unwrap();
    assert_eq!(
        transmission.freshness(&corrupt.workspace()).unwrap(),
        Freshness::Unavailable
    );
}
