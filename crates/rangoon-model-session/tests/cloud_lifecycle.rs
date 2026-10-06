use rangoon_import::analyze;
use rangoon_model_session::{
    Diagnostic, Freshness, OperationKind,
    cloud::{CloudSession, PreparedView},
};
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
            "rangoon-cloud-session-{}-{}-{}",
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

fn profile(model: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.cloud-profile-request.v1",
        "profileId": "fixture.cloud",
        "model": model,
        "maxOutputTokens": 256,
    }))
    .unwrap()
}

fn revision(byte: char) -> String {
    byte.to_string().repeat(64)
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
        "schemaVersion": "rangoon.cloud-selection.v1",
        "task": task,
        "inputs": inputs,
    }))
    .unwrap()
}

fn send(prepared_id: &str, request_id: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.cloud-send.v1",
        "preparedId": prepared_id,
        "requestId": request_id,
    }))
    .unwrap()
}

fn cancel(run_id: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.cloud-cancel.v1",
        "runId": run_id,
    }))
    .unwrap()
}

fn save(dir: &TestDir, name: &str, bytes: &[u8]) -> rangoon_domain::AnalysisReport {
    let report = analyze(name, bytes).unwrap();
    dir.workspace().save_v1(&report).unwrap();
    report
}

fn session() -> CloudSession {
    let session = CloudSession::default();
    session.configure(&profile("gpt-fixture-v1")).unwrap();
    session
}

fn prepare_source(
    session: &CloudSession,
    store: &Workspace,
    source_id: &str,
    revision: &str,
) -> PreparedView {
    session
        .begin_prepare(
            &selection("classify_v1", vec![source_selector(source_id)]),
            revision,
        )
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
fn retained_preparation_holds_slot_and_rechecks_exact_staged_identity() {
    let dir = TestDir::new();
    let source = save(&dir, "AGENTS.md", b"# Rules\nKeep exact evidence.\n");
    let cloud = session();
    let raw = selection("classify_v1", vec![source_selector(&source.source.id)]);
    let prepare = || {
        cloud
            .begin_prepare(&raw, &revision('a'))
            .unwrap()
            .prepare_retained(&dir.workspace())
            .unwrap()
    };
    let ready = prepare();
    ready.ensure_current().unwrap();
    assert!(cloud.inspect().unwrap().active.is_some());
    assert_error!(cloud.begin_check(), Diagnostic::Busy);
    ready.ensure_current().unwrap();
    let view = ready.into_view();
    assert!(cloud.inspect().unwrap().active.is_none());
    drop(
        cloud
            .begin_send(&send(&view.prepared_id, &view.request_id))
            .unwrap(),
    );

    let ready = prepare();
    // A competing preparation invalidates the retained request even when its
    // attempt fails busy without changing the generation or profile.
    let generation = cloud.inspect().unwrap().generation;
    assert_error!(cloud.begin_resolution(&raw), Diagnostic::Busy);
    assert_eq!(cloud.inspect().unwrap().generation, generation);
    assert_eq!(ready.ensure_current(), Err(Diagnostic::StalePrepared));
    drop(ready);
    assert!(cloud.inspect().unwrap().active.is_none());

    let ready = prepare();
    cloud.clear().unwrap();
    assert_eq!(ready.ensure_current(), Err(Diagnostic::Cancelled));
    assert_error!(
        cloud.configure(&profile("gpt-fixture-v1")),
        Diagnostic::Busy
    );
    drop(ready);
    cloud.configure(&profile("gpt-fixture-v1")).unwrap();
}

#[test]
fn prepared_view_binds_exact_saved_bytes_and_has_no_workspace_write() {
    let dir = TestDir::new();
    let bytes = b"\xef\xbb\xbf# \xce\x94elta\r\nKeep \xf0\x9f\x94\x92 exact.\r\n";
    let source = save(&dir, "AGENTS.md", bytes);
    let database = dir.0.join("workspace.sqlite3");
    let before = fs::read(&database).unwrap();
    let cloud = session();
    let credential_revision = revision('a');
    let view = prepare_source(
        &cloud,
        &dir.workspace(),
        &source.source.id,
        &credential_revision,
    );
    let after = fs::read(&database).unwrap();

    assert_eq!(before, after);
    assert_eq!(view.schema_version, "rangoon.cloud-prepared.v1");
    assert!(view.prepared_id.starts_with("cloud-prepared:"));
    assert_eq!(view.request_id.len(), 64);
    assert_eq!(view.origin, "https://api.openai.com");
    assert_eq!(view.model, "gpt-fixture-v1");
    assert_eq!(view.credential_revision, credential_revision);
    assert_eq!(view.body_bytes, view.body_json.len());
    assert_eq!(
        view.body_sha256,
        rangoon_domain::byte_digest(view.body_json.as_bytes())
    );
    assert_eq!(view.inputs.len(), 1);
    assert_eq!(view.inputs[0].byte_length, bytes.len());
    assert_eq!(view.processing_location, "unknown");
    assert_eq!(view.retention, "unknown");
    assert_eq!(view.authority, "none");

    let body: Value = serde_json::from_str(&view.body_json).unwrap();
    assert_eq!(body["model"], "gpt-fixture-v1");
    assert_eq!(body["input"], view.pack.body_json());
    assert_eq!(body["store"], false);
    assert_eq!(body["stream"], false);
    assert_eq!(body["background"], false);
    assert_eq!(body["tools"], json!([]));
    assert!(!view.body_json.contains(&view.credential_revision));

    let pack: Value = serde_json::from_str(view.pack.body_json()).unwrap();
    assert_eq!(pack["blocks"][0]["text"], source.source.content);
    assert_eq!(
        pack["inputs"][0]["protectedRanges"],
        json!([{ "startByte": 0, "endByte": bytes.len() }])
    );
}

#[test]
fn credential_revision_profile_replacement_and_claim_are_one_shot() {
    let dir = TestDir::new();
    let source = save(&dir, "rules.md", b"Keep the selected record.\n");
    let cloud = session();
    let first = prepare_source(&cloud, &dir.workspace(), &source.source.id, &revision('a'));
    let repeated = prepare_source(&cloud, &dir.workspace(), &source.source.id, &revision('a'));
    assert_eq!(first.request_id, repeated.request_id);
    assert_ne!(first.prepared_id, repeated.prepared_id);
    let changed_revision =
        prepare_source(&cloud, &dir.workspace(), &source.source.id, &revision('b'));
    assert_ne!(first.request_id, changed_revision.request_id);
    assert_error!(
        cloud.begin_send(&send(&first.prepared_id, &first.request_id)),
        Diagnostic::StalePrepared
    );

    cloud.configure(&profile("gpt-replacement-v2")).unwrap();
    assert_error!(
        cloud.begin_send(&send(
            &changed_revision.prepared_id,
            &changed_revision.request_id
        )),
        Diagnostic::StalePrepared
    );

    let invalidated = prepare_source(&cloud, &dir.workspace(), &source.source.id, &revision('b'));
    assert_error!(
        cloud.begin_prepare(
            br#"{"schemaVersion":"rangoon.cloud-selection.v1","task":"wrong","inputs":[]}"#,
            &revision('b')
        ),
        Diagnostic::InvalidRequest
    );
    assert_error!(
        cloud.begin_send(&send(&invalidated.prepared_id, &invalidated.request_id)),
        Diagnostic::StalePrepared
    );

    let invalid_revision =
        prepare_source(&cloud, &dir.workspace(), &source.source.id, &revision('b'));
    assert_error!(
        cloud.begin_prepare(
            &selection("classify_v1", vec![source_selector(&source.source.id)]),
            "not-a-revision"
        ),
        Diagnostic::InvalidRequest
    );
    assert_error!(
        cloud.begin_send(&send(
            &invalid_revision.prepared_id,
            &invalid_revision.request_id
        )),
        Diagnostic::StalePrepared
    );

    let missing_invalidates =
        prepare_source(&cloud, &dir.workspace(), &source.source.id, &revision('b'));
    let unavailable = cloud
        .begin_prepare(
            &selection(
                "classify_v1",
                vec![source_selector(&format!("source:{}", "c".repeat(64)))],
            ),
            &revision('b'),
        )
        .unwrap();
    assert_error!(
        unavailable.prepare(&dir.workspace()),
        Diagnostic::InputUnavailable
    );
    assert_error!(
        cloud.begin_send(&send(
            &missing_invalidates.prepared_id,
            &missing_invalidates.request_id
        )),
        Diagnostic::StalePrepared
    );

    let preserved = prepare_source(&cloud, &dir.workspace(), &source.source.id, &revision('b'));
    assert_error!(
        cloud.configure(&profile("_invalid")),
        Diagnostic::InvalidProfile
    );
    let preserved_transmission = cloud
        .begin_send(&send(&preserved.prepared_id, &preserved.request_id))
        .unwrap();
    drop(preserved_transmission);

    let mut staged = prepare_source(&cloud, &dir.workspace(), &source.source.id, &revision('b'));
    let prepared_id = staged.prepared_id.clone();
    let request_id = staged.request_id.clone();
    let exact_body = staged.body_json.clone();
    staged.body_json = "renderer-tampered".to_owned();
    staged.credential_revision = revision('f');
    let transmission = cloud.begin_send(&send(&prepared_id, &request_id)).unwrap();
    assert_eq!(transmission.request().request_id(), request_id);
    assert_eq!(transmission.request().body_json(), exact_body);
    assert!(!transmission.request().body_json().contains(&revision('b')));
    assert_eq!(transmission.credential_revision(), revision('b'));
    assert_eq!(transmission.input_count(), 1);
    assert_eq!(transmission.pack_id(), staged.pack.pack_id());
    assert_error!(
        cloud.begin_send(&send(&prepared_id, &request_id)),
        Diagnostic::Busy
    );
    drop(transmission);
    assert_error!(
        cloud.begin_send(&send(&prepared_id, &request_id)),
        Diagnostic::StalePrepared
    );
}

#[test]
fn cloud_commands_are_closed_bounded_and_cross_route_inputs_fail() {
    let cloud = session();
    let revision = revision('a');
    let source = format!("source:{}", "a".repeat(64));
    let valid = selection("classify_v1", vec![source_selector(&source)]);
    let preparation = cloud.begin_prepare(&valid, &revision).unwrap();
    assert_eq!(preparation.operation().profile().model(), "gpt-fixture-v1");
    drop(preparation);

    let local_selection = String::from_utf8(valid)
        .unwrap()
        .replace("rangoon.cloud-selection.v1", "rangoon.local-selection.v1");
    assert_error!(
        cloud.begin_prepare(local_selection.as_bytes(), &revision),
        Diagnostic::InvalidRequest
    );
    let many = (0..17)
        .map(|index| source_selector(&format!("source:{index:064x}")))
        .collect();
    assert_error!(
        cloud.begin_prepare(&selection("classify_v1", many), &revision),
        Diagnostic::InvalidRequest
    );
    let sixteen = (0..16)
        .map(|index| source_selector(&format!("source:{index:064x}")))
        .collect();
    let preparation = cloud
        .begin_prepare(&selection("classify_v1", sixteen), &revision)
        .unwrap();
    drop(preparation);
    let mut at_selection_limit = selection("classify_v1", vec![source_selector(&source)]);
    at_selection_limit.resize(8192, b' ');
    let preparation = cloud.begin_prepare(&at_selection_limit, &revision).unwrap();
    drop(preparation);
    at_selection_limit.push(b' ');
    assert_error!(
        cloud.begin_prepare(&at_selection_limit, &revision),
        Diagnostic::InvalidRequest
    );
    assert_error!(
        cloud.begin_prepare(
            br#"{"schemaVersion":"rangoon.cloud-selection.v1","schema\u0056ersion":"rangoon.cloud-selection.v1","task":"classify_v1","inputs":[{"kind":"source","sourceId":"source:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]}"#,
            &revision
        ),
        Diagnostic::InvalidRequest
    );
    assert_error!(
        cloud.begin_prepare(
            &selection("classify_v1", vec![source_selector(&source)]),
            "not-a-revision"
        ),
        Diagnostic::InvalidRequest
    );

    let prepared = format!("cloud-prepared:{}", "a".repeat(64));
    let request = "b".repeat(64);
    let mut at_send_limit = send(&prepared, &request);
    at_send_limit.resize(512, b' ');
    assert_error!(cloud.begin_send(&at_send_limit), Diagnostic::StalePrepared);
    at_send_limit.push(b' ');
    assert_error!(cloud.begin_send(&at_send_limit), Diagnostic::InvalidRequest);

    let run = format!("cloud-run:{}", "c".repeat(64));
    let mut at_cancel_limit = cancel(&run);
    at_cancel_limit.resize(256, b' ');
    assert_eq!(cloud.cancel(&at_cancel_limit), Err(Diagnostic::RunNotFound));
    at_cancel_limit.push(b' ');
    assert_error!(cloud.cancel(&at_cancel_limit), Diagnostic::InvalidRequest);
}

#[test]
fn clones_busy_cancel_clear_and_dropped_leases_preserve_slot_rules() {
    let dir = TestDir::new();
    let source = save(&dir, "rules.md", b"Selected record.\n");
    let cloud = session();
    let clone = cloud.clone();
    let selection = selection("classify_v1", vec![source_selector(&source.source.id)]);
    let preparation = cloud.begin_prepare(&selection, &revision('a')).unwrap();
    assert_error!(
        clone.begin_prepare(&selection, &revision('a')),
        Diagnostic::Busy
    );
    clone
        .cancel(&cancel(preparation.operation().run_id()))
        .unwrap();
    assert_error!(preparation.prepare(&dir.workspace()), Diagnostic::Cancelled);
    assert!(cloud.inspect().unwrap().active.is_none());

    let staged = prepare_source(&cloud, &dir.workspace(), &source.source.id, &revision('a'));
    let transmission = cloud
        .begin_send(&send(&staged.prepared_id, &staged.request_id))
        .unwrap();
    assert_eq!(
        cloud.inspect().unwrap().active.unwrap().kind,
        OperationKind::Send
    );
    cloud.clear().unwrap();
    assert_eq!(
        transmission.freshness(&dir.workspace()),
        Err(Diagnostic::Cancelled)
    );
    assert_error!(
        cloud.configure(&profile("gpt-fixture-v1")),
        Diagnostic::Busy
    );
    drop(transmission);
    cloud.configure(&profile("gpt-fixture-v1")).unwrap();
}

#[test]
fn saved_record_owner_head_missing_and_corrupt_states_are_closed() {
    let dir = TestDir::new();
    let source = save(&dir, "rules.md", b"One selected rule.\n");
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
    let cloud = session();
    let wrong_owner = cloud
        .begin_prepare(
            &selection(
                "classify_v1",
                vec![capability_selector(&first.id, &second.latest_revision_id)],
            ),
            &revision('a'),
        )
        .unwrap();
    assert_error!(
        wrong_owner.prepare(&dir.workspace()),
        Diagnostic::InputUnavailable
    );

    let staged = cloud
        .begin_prepare(
            &selection(
                "classify_v1",
                vec![capability_selector(&first.id, &first.latest_revision_id)],
            ),
            &revision('a'),
        )
        .unwrap()
        .prepare(&dir.workspace())
        .unwrap();
    let transmission = cloud
        .begin_send(&send(&staged.prepared_id, &staged.request_id))
        .unwrap();
    assert_eq!(
        transmission.freshness(&dir.workspace()).unwrap(),
        Freshness::Current
    );
    dir.workspace()
        .revise_capability_v1(
            &first.id,
            &first.latest_revision_id,
            "First revised",
            "Changed head.\n",
        )
        .unwrap();
    assert_eq!(
        transmission.freshness(&dir.workspace()).unwrap(),
        Freshness::Stale
    );
    drop(transmission);

    let missing = prepare_source(&cloud, &dir.workspace(), &source.source.id, &revision('a'));
    let transmission = cloud
        .begin_send(&send(&missing.prepared_id, &missing.request_id))
        .unwrap();
    fs::remove_file(dir.0.join("workspace.sqlite3")).unwrap();
    assert_eq!(
        transmission.freshness(&dir.workspace()).unwrap(),
        Freshness::Unavailable
    );

    let corrupt = TestDir::new();
    let source = save(&corrupt, "corrupt.md", b"Selected record.\n");
    let cloud = session();
    let staged = prepare_source(
        &cloud,
        &corrupt.workspace(),
        &source.source.id,
        &revision('a'),
    );
    let transmission = cloud
        .begin_send(&send(&staged.prepared_id, &staged.request_id))
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

#[test]
fn final_cloud_body_over_budget_fails_without_recreating_saved_source() {
    let dir = TestDir::new();
    let mut bytes = Vec::with_capacity(100_100);
    for _ in 0..100 {
        bytes.extend_from_slice(&[b'"'; 1_000]);
        bytes.push(b'\n');
    }
    let source = save(&dir, "escaped.md", &bytes);
    let database = dir.0.join("workspace.sqlite3");
    let before = fs::read(&database).unwrap();
    let cloud = session();
    let preparation = cloud
        .begin_prepare(
            &selection("classify_v1", vec![source_selector(&source.source.id)]),
            &revision('a'),
        )
        .unwrap();
    assert_error!(
        preparation.prepare(&dir.workspace()),
        Diagnostic::PackOverBudget
    );
    assert_eq!(fs::read(&database).unwrap(), before);
    assert_eq!(
        dir.workspace()
            .open(&source.source.id)
            .unwrap()
            .source
            .content
            .as_bytes(),
        bytes
    );
}

#[test]
fn foreign_send_cancel_namespaces_and_unknown_fields_cannot_claim_or_cancel() {
    let dir = TestDir::new();
    let source = save(&dir, "rules.md", b"Keep this exact selected source.\n");
    let cloud = session();
    let prepared = prepare_source(&cloud, &dir.workspace(), &source.source.id, &revision('a'));

    let mut bad_profile: Value = serde_json::from_slice(&profile("gpt-fixture-v1")).unwrap();
    bad_profile["origin"] = json!("https://different.example");
    assert_error!(
        cloud.configure(&serde_json::to_vec(&bad_profile).unwrap()),
        Diagnostic::InvalidProfile
    );

    let original_send: Value =
        serde_json::from_slice(&send(&prepared.prepared_id, &prepared.request_id)).unwrap();
    for (field, value) in [
        ("schemaVersion", json!("rangoon.local-send.v1")),
        (
            "preparedId",
            json!(prepared.prepared_id.replace("cloud-prepared:", "prepared:")),
        ),
        ("approved", json!(true)),
        ("bodyJson", json!("replacement content")),
    ] {
        let mut invalid = original_send.clone();
        invalid[field] = value;
        assert_error!(
            cloud.begin_send(&serde_json::to_vec(&invalid).unwrap()),
            Diagnostic::InvalidRequest
        );
    }
    // Invalid commands preserve the unclaimed native-owned request; only the
    // exact valid claim consumes it, and it still carries no consent authority.
    let transmission = cloud
        .begin_send(&send(&prepared.prepared_id, &prepared.request_id))
        .unwrap();
    let original_cancel: Value =
        serde_json::from_slice(&cancel(transmission.operation().run_id())).unwrap();
    for (field, value) in [
        ("schemaVersion", json!("rangoon.local-cancel.v1")),
        (
            "runId",
            json!(
                transmission
                    .operation()
                    .run_id()
                    .replace("cloud-run:", "run:")
            ),
        ),
        ("approved", json!(true)),
    ] {
        let mut invalid = original_cancel.clone();
        invalid[field] = value;
        assert_error!(
            cloud.cancel(&serde_json::to_vec(&invalid).unwrap()),
            Diagnostic::InvalidRequest
        );
        transmission.operation().ensure_current().unwrap();
    }
    cloud
        .cancel(&cancel(transmission.operation().run_id()))
        .unwrap();
    assert_eq!(
        transmission.operation().ensure_current(),
        Err(Diagnostic::Cancelled)
    );
}

#[test]
fn resolution_reserves_before_credential_read_and_cannot_revive_after_clear() {
    let dir = TestDir::new();
    let report = save(&dir, "AGENTS.md", b"# Rules\nKeep evidence.\n");
    let session = session();
    let raw = selection("classify_v1", vec![source_selector(&report.source.id)]);
    let prior = prepare_source(
        &session,
        &dir.workspace(),
        &report.source.id,
        &revision('a'),
    );
    let resolution = session.begin_resolution(&raw).unwrap();
    assert_eq!(
        session.inspect().unwrap().active.unwrap().kind,
        OperationKind::Prepare
    );
    assert!(matches!(session.begin_check(), Err(Diagnostic::Busy)));
    session.clear().unwrap();
    assert_eq!(
        resolution.operation().ensure_current(),
        Err(Diagnostic::Cancelled)
    );
    assert!(matches!(
        resolution.with_credential_revision(&revision('a')),
        Err(Diagnostic::Cancelled)
    ));
    session.configure(&profile("gpt-fixture-v1")).unwrap();
    assert!(matches!(
        session.begin_send(&send(&prior.prepared_id, &prior.request_id)),
        Err(Diagnostic::StalePrepared)
    ));
    let resolution = session.begin_resolution(&raw).unwrap();
    assert!(matches!(
        resolution.with_credential_revision("invalid"),
        Err(Diagnostic::InvalidRequest)
    ));
    assert!(session.inspect().unwrap().active.is_none());
    let check = session.begin_check().unwrap();
    assert_eq!(
        session.inspect().unwrap().active.unwrap().kind,
        OperationKind::Check
    );
    session.cancel(&cancel(check.run_id())).unwrap();
    assert_eq!(check.ensure_current(), Err(Diagnostic::Cancelled));
    assert!(matches!(session.begin_check(), Err(Diagnostic::Busy)));
    drop(check);
    assert!(session.begin_check().is_ok());
}
