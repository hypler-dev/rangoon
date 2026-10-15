use rangoon_import::analyze;
use rangoon_model_session::{
    Diagnostic, Freshness, InputReader, LocalSession, PreparedView as LocalPreparedView,
    cloud::{CloudSession, PreparedView as CloudPreparedView},
};
use rangoon_store::Workspace;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

macro_rules! assert_error {
    ($result:expr, $diagnostic:expr) => {
        assert!(matches!($result, Err(error) if error == $diagnostic));
    };
}

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rangoon-model-input-reader-{}-{}-{}",
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

#[derive(Default)]
struct Reader {
    sources: BTreeMap<String, rangoon_domain::AnalysisReport>,
    capabilities: BTreeMap<(String, String), rangoon_domain::capability_v1::CapabilityDetail>,
    source_error: bool,
    capability_error: bool,
}

impl Reader {
    fn source(mut self, report: rangoon_domain::AnalysisReport) -> Self {
        self.sources.insert(report.source.id.clone(), report);
        self
    }

    fn capability(mut self, detail: rangoon_domain::capability_v1::CapabilityDetail) -> Self {
        self.capabilities
            .insert((detail.id.clone(), detail.revision.id.clone()), detail);
        self
    }
}

impl InputReader for Reader {
    fn read_source(
        &mut self,
        source_id: &str,
    ) -> Result<rangoon_domain::AnalysisReport, Diagnostic> {
        if self.source_error {
            return Err(Diagnostic::SessionUnavailable);
        }
        self.sources
            .get(source_id)
            .cloned()
            .ok_or(Diagnostic::SessionUnavailable)
    }

    fn read_capability(
        &mut self,
        capability_id: &str,
        revision_id: &str,
    ) -> Result<rangoon_domain::capability_v1::CapabilityDetail, Diagnostic> {
        if self.capability_error {
            return Err(Diagnostic::SessionUnavailable);
        }
        self.capabilities
            .get(&(capability_id.to_owned(), revision_id.to_owned()))
            .cloned()
            .ok_or(Diagnostic::SessionUnavailable)
    }
}

fn local_profile() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.local-profile-request.v1",
        "profileId": "fixture.local",
        "host": "127.0.0.1",
        "port": 11434,
        "model": "fixture:v1",
        "maxOutputTokens": 256,
    }))
    .unwrap()
}

fn cloud_profile() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.cloud-profile-request.v1",
        "profileId": "fixture.cloud",
        "model": "fixture-v1",
        "maxOutputTokens": 256,
    }))
    .unwrap()
}

fn local_session() -> LocalSession {
    let session = LocalSession::default();
    session.configure(&local_profile()).unwrap();
    session
}

fn cloud_session() -> CloudSession {
    let session = CloudSession::default();
    session.configure(&cloud_profile()).unwrap();
    session
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

fn selection(schema: &str, task: &str, inputs: Vec<Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": schema,
        "task": task,
        "inputs": inputs,
    }))
    .unwrap()
}

fn local_selection(task: &str, inputs: Vec<Value>) -> Vec<u8> {
    selection("rangoon.local-selection.v1", task, inputs)
}

fn cloud_selection(task: &str, inputs: Vec<Value>) -> Vec<u8> {
    selection("rangoon.cloud-selection.v1", task, inputs)
}

fn local_send(prepared_id: &str, request_id: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.local-send.v1",
        "preparedId": prepared_id,
        "requestId": request_id,
    }))
    .unwrap()
}

fn cloud_send(prepared_id: &str, request_id: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.cloud-send.v1",
        "preparedId": prepared_id,
        "requestId": request_id,
    }))
    .unwrap()
}

fn save(dir: &TestDir, name: &str, bytes: &[u8]) -> rangoon_domain::AnalysisReport {
    let report = analyze(name, bytes).unwrap();
    dir.workspace().save_v1(&report).unwrap();
    report
}

fn capability(
    workspace: &Workspace,
    report: &rangoon_domain::AnalysisReport,
) -> rangoon_domain::capability_v1::CapabilityDetail {
    let fragment = report.fragments.first().unwrap();
    workspace
        .create_capability_v1(&report.source.id, &fragment.id, "Fixture rules")
        .unwrap()
        .capability
}

fn assert_local_equivalent(workspace: &LocalPreparedView, reader: &LocalPreparedView) {
    assert_ne!(workspace.prepared_id, reader.prepared_id);
    assert_eq!(workspace.request_id, reader.request_id);
    assert_eq!(workspace.body_json, reader.body_json);
    assert_eq!(workspace.body_bytes, reader.body_bytes);
    assert_eq!(
        serde_json::to_value(&workspace.inputs).unwrap(),
        serde_json::to_value(&reader.inputs).unwrap(),
    );
    assert_eq!(
        serde_json::to_value(&workspace.pack).unwrap(),
        serde_json::to_value(&reader.pack).unwrap(),
    );
}

fn assert_cloud_equivalent(workspace: &CloudPreparedView, reader: &CloudPreparedView) {
    assert_ne!(workspace.prepared_id, reader.prepared_id);
    assert_eq!(workspace.request_id, reader.request_id);
    assert_eq!(workspace.body_json, reader.body_json);
    assert_eq!(workspace.body_bytes, reader.body_bytes);
    assert_eq!(workspace.body_sha256, reader.body_sha256);
    assert_eq!(
        serde_json::to_value(&workspace.inputs).unwrap(),
        serde_json::to_value(&reader.inputs).unwrap(),
    );
    assert_eq!(
        serde_json::to_value(&workspace.pack).unwrap(),
        serde_json::to_value(&reader.pack).unwrap(),
    );
}

#[test]
fn local_input_reader_matches_workspace_source_pack_and_body() {
    let dir = TestDir::new();
    let report = save(&dir, "rules.md", b"# Rules\nKeep exact source bytes.\n");
    let raw = local_selection("classify_v1", vec![source_selector(&report.source.id)]);

    let workspace_session = local_session();
    let expected = workspace_session
        .begin_prepare(&raw)
        .unwrap()
        .prepare(&dir.workspace())
        .unwrap();

    let reader_session = local_session();
    let mut reader = Reader::default().source(report);
    let mut retained = reader_session
        .begin_prepare(&raw)
        .unwrap()
        .prepare_with_inputs(&mut reader)
        .unwrap();
    assert_local_equivalent(&expected, retained.view());
    retained.publish().unwrap();
    let published = retained.into_view();
    assert_local_equivalent(&expected, &published);
}

#[test]
fn cloud_input_reader_matches_workspace_historical_and_current_capability_pack_and_body() {
    let dir = TestDir::new();
    let workspace = dir.workspace();
    let report = save(&dir, "rules.md", b"# Rules\nKeep capability history.\n");
    let first = capability(&workspace, &report);
    let current = workspace
        .revise_capability_v1(
            &first.id,
            &first.latest_revision_id,
            "Fixture rules v2",
            "Current revision text.\n",
        )
        .unwrap()
        .capability;
    let historical = workspace
        .open_capability_v1(&first.id, Some(&first.latest_revision_id))
        .unwrap();
    let raw = cloud_selection(
        "compare_v1",
        vec![
            capability_selector(&historical.id, &historical.revision.id),
            capability_selector(&current.id, &current.revision.id),
        ],
    );

    let expected_session = cloud_session();
    let expected = expected_session
        .begin_prepare(&raw, &revision('a'))
        .unwrap()
        .prepare(&workspace)
        .unwrap();

    let reader_session = cloud_session();
    let mut reader = Reader::default().capability(historical).capability(current);
    let mut retained = reader_session
        .begin_prepare(&raw, &revision('a'))
        .unwrap()
        .prepare_retained_with_inputs(&mut reader)
        .unwrap();
    assert_cloud_equivalent(&expected, retained.view());
    retained.publish().unwrap();
    let published = retained.into_view();
    assert_cloud_equivalent(&expected, &published);
}

#[test]
fn swapped_reader_source_capability_and_revision_identity_are_refused() {
    let dir = TestDir::new();
    let workspace = dir.workspace();
    let source = save(&dir, "rules.md", b"# Rules\nIdentity must match.\n");
    let other_source = save(&dir, "other.md", b"# Other\nDifferent saved record.\n");
    let capability = capability(&workspace, &source);
    let revised = workspace
        .revise_capability_v1(
            &capability.id,
            &capability.latest_revision_id,
            "Fixture rules v2",
            "Revised content.\n",
        )
        .unwrap()
        .capability;

    let mut swapped_source = Reader::default();
    swapped_source
        .sources
        .insert(source.source.id.clone(), other_source);
    assert_error!(
        local_session()
            .begin_prepare(&local_selection(
                "classify_v1",
                vec![source_selector(&source.source.id)]
            ))
            .unwrap()
            .prepare_with_inputs(&mut swapped_source),
        Diagnostic::InputUnavailable
    );

    let mut swapped_capability = Reader::default();
    let mut wrong_capability = revised.clone();
    wrong_capability.id =
        "capability:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned();
    swapped_capability.capabilities.insert(
        (capability.id.clone(), capability.revision.id.clone()),
        wrong_capability,
    );
    assert_error!(
        cloud_session()
            .begin_prepare(
                &cloud_selection(
                    "classify_v1",
                    vec![capability_selector(&capability.id, &capability.revision.id)],
                ),
                &revision('a'),
            )
            .unwrap()
            .prepare_retained_with_inputs(&mut swapped_capability),
        Diagnostic::InputUnavailable
    );

    let mut reader = Reader::default();
    reader.capabilities.insert(
        (capability.id.clone(), capability.revision.id.clone()),
        revised,
    );
    assert_error!(
        local_session()
            .begin_prepare(&local_selection(
                "classify_v1",
                vec![capability_selector(&capability.id, &capability.revision.id)],
            ))
            .unwrap()
            .prepare_with_inputs(&mut reader),
        Diagnostic::InputUnavailable
    );
}

#[test]
fn reader_error_missing_empty_and_oversized_inputs_keep_existing_diagnostics() {
    let dir = TestDir::new();
    let report = save(&dir, "rules.md", b"# Rules\nBounded inputs.\n");
    let raw = local_selection("classify_v1", vec![source_selector(&report.source.id)]);

    for reader in [
        Reader {
            source_error: true,
            ..Reader::default()
        },
        Reader::default(),
    ] {
        let mut reader = reader;
        assert_error!(
            local_session()
                .begin_prepare(&raw)
                .unwrap()
                .prepare_with_inputs(&mut reader),
            Diagnostic::InputUnavailable
        );
    }

    let mut empty = report.clone();
    empty.source.content.clear();
    let mut reader = Reader::default().source(empty);
    assert_error!(
        local_session()
            .begin_prepare(&raw)
            .unwrap()
            .prepare_with_inputs(&mut reader),
        Diagnostic::InputEmpty
    );

    let mut oversized = report;
    oversized.source.content = "x".repeat(262_145);
    let mut reader = Reader::default().source(oversized);
    assert_error!(
        local_session()
            .begin_prepare(&raw)
            .unwrap()
            .prepare_with_inputs(&mut reader),
        Diagnostic::PackOverBudget
    );
}

#[test]
fn local_freshness_marks_same_source_identity_changed_bytes_and_hash_stale() {
    let dir = TestDir::new();
    let report = save(&dir, "rules.md", b"# Rules\nOriginal source.\n");
    let raw = local_selection("classify_v1", vec![source_selector(&report.source.id)]);
    let session = local_session();
    let mut reader = Reader::default().source(report.clone());
    let mut retained = session
        .begin_prepare(&raw)
        .unwrap()
        .prepare_with_inputs(&mut reader)
        .unwrap();
    let view = retained.view().clone();
    retained.publish().unwrap();
    drop(retained);
    let transmission = session
        .begin_send(&local_send(&view.prepared_id, &view.request_id))
        .unwrap();
    assert_eq!(
        transmission.freshness_with_inputs(&mut reader).unwrap(),
        Freshness::Current
    );

    let mut changed = report;
    changed.source.content = "# Rules\nChanged source bytes.\n".to_owned();
    changed.source.sha256 = "f".repeat(64);
    reader.sources.insert(changed.source.id.clone(), changed);
    assert_eq!(
        transmission.freshness_with_inputs(&mut reader).unwrap(),
        Freshness::Stale
    );
}

#[test]
fn cloud_freshness_marks_same_capability_revision_bytes_and_head_stale() {
    let dir = TestDir::new();
    let workspace = dir.workspace();
    let report = save(&dir, "rules.md", b"# Rules\nOriginal capability.\n");
    let detail = capability(&workspace, &report);
    let raw = cloud_selection(
        "classify_v1",
        vec![capability_selector(&detail.id, &detail.revision.id)],
    );
    let session = cloud_session();
    let mut reader = Reader::default().capability(detail.clone());
    let mut retained = session
        .begin_prepare(&raw, &revision('a'))
        .unwrap()
        .prepare_retained_with_inputs(&mut reader)
        .unwrap();
    let view = retained.view().clone();
    retained.publish().unwrap();
    drop(retained);
    let transmission = session
        .begin_send(&cloud_send(&view.prepared_id, &view.request_id))
        .unwrap();
    assert_eq!(
        transmission.freshness_with_inputs(&mut reader).unwrap(),
        Freshness::Current
    );

    let mut changed = detail;
    changed.revision.content = "Changed capability bytes.\n".to_owned();
    changed.revision.sha256 = "e".repeat(64);
    changed.latest_revision_id = revision('d');
    reader
        .capabilities
        .insert((changed.id.clone(), changed.revision.id.clone()), changed);
    assert_eq!(
        transmission.freshness_with_inputs(&mut reader).unwrap(),
        Freshness::Stale
    );
}

#[test]
fn local_unobserved_retained_preparation_clears_only_its_stage() {
    let dir = TestDir::new();
    let report = save(&dir, "rules.md", b"# Rules\nDrop cleanup.\n");
    let raw = local_selection("classify_v1", vec![source_selector(&report.source.id)]);
    let session = local_session();
    let mut reader = Reader::default().source(report);
    let first = session
        .begin_prepare(&raw)
        .unwrap()
        .prepare_with_inputs(&mut reader)
        .unwrap();
    let first_view = first.view().clone();
    drop(first);
    assert_error!(
        session.begin_send(&local_send(&first_view.prepared_id, &first_view.request_id)),
        Diagnostic::StalePrepared
    );

    let mut newer = session
        .begin_prepare(&raw)
        .unwrap()
        .prepare_with_inputs(&mut reader)
        .unwrap();
    let newer_view = newer.view().clone();
    newer.publish().unwrap();
    drop(newer);
    assert!(
        session
            .begin_send(&local_send(&newer_view.prepared_id, &newer_view.request_id))
            .is_ok()
    );
}

#[test]
fn cloud_unobserved_retained_preparation_clears_only_its_envelope() {
    let dir = TestDir::new();
    let report = save(&dir, "rules.md", b"# Rules\nCloud drop cleanup.\n");
    let raw = cloud_selection("classify_v1", vec![source_selector(&report.source.id)]);
    let session = cloud_session();
    let mut reader = Reader::default().source(report);
    let first = session
        .begin_prepare(&raw, &revision('a'))
        .unwrap()
        .prepare_retained_with_inputs(&mut reader)
        .unwrap();
    let first_view = first.view().clone();
    drop(first);
    assert_error!(
        session.begin_send(&cloud_send(&first_view.prepared_id, &first_view.request_id)),
        Diagnostic::StalePrepared
    );

    let mut newer = session
        .begin_prepare(&raw, &revision('a'))
        .unwrap()
        .prepare_retained_with_inputs(&mut reader)
        .unwrap();
    let newer_view = newer.view().clone();
    newer.publish().unwrap();
    drop(newer);
    assert!(
        session
            .begin_send(&cloud_send(&newer_view.prepared_id, &newer_view.request_id))
            .is_ok()
    );
}
