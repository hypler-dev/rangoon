#[cfg(feature = "encrypted-sqlite")]
use rangoon_compile::Profile;
#[cfg(feature = "encrypted-sqlite")]
use rangoon_compose::application::{self, Request, Target};
#[cfg(feature = "encrypted-sqlite")]
use rangoon_import::analyze;
#[cfg(feature = "encrypted-sqlite")]
use rangoon_store::{Backup, WorkflowRecordKind, WorkspaceBackup};
use rangoon_store::{StoreError, Workspace, WorkspaceKey};
#[cfg(feature = "encrypted-sqlite")]
use rangoon_workflow::records::{SaveIntent, prepare_revision, workflow_id_from_nonce};
#[cfg(feature = "encrypted-sqlite")]
use rusqlite::Connection;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rangoon-store-encryption-{label}-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        )))
    }

    #[cfg(feature = "encrypted-sqlite")]
    fn path(&self) -> PathBuf {
        self.0.join("workspace.sqlite3")
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn key(seed: u8) -> WorkspaceKey {
    WorkspaceKey::from_bytes([seed; 32])
}

#[cfg(feature = "encrypted-sqlite")]
fn exact_source() -> rangoon_domain::AnalysisReport {
    analyze(
        "AGENTS.md",
        b"\xef\xbb\xbf# Rules\r\nKeep exact \xf0\x9f\xa6\x80 bytes.\r\n# Tests\r\nPreserve source identities.\r\n",
    )
    .unwrap()
}

#[cfg(feature = "encrypted-sqlite")]
fn composition_source() -> rangoon_domain::AnalysisReport {
    analyze(
        "AGENTS.md",
        b"\xef\xbb\xbf# Rules\r\nKeep originals.\r\n# Tests\r\nCheck edits.\r\n",
    )
    .unwrap()
}

#[cfg(feature = "encrypted-sqlite")]
fn composition_request() -> Request {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/composition/decompose-v0.json"
    ))
    .unwrap();
    Request {
        schema_version: application::REQUEST_SCHEMA.into(),
        draft: serde_json::from_value(value["draft"].clone()).unwrap(),
        targets: vec![Target::New {}, Target::New {}],
    }
}

#[cfg(feature = "encrypted-sqlite")]
fn workflow() -> rangoon_workflow::records::WorkflowRevision {
    let definition = serde_json::json!({
        "schemaVersion": "rangoon.workflow-definition.v1",
        "title": "Encrypted workflow",
        "nodes": [],
        "controlEdges": [],
        "dataEdges": []
    });
    prepare_revision(
        &workflow_id_from_nonce(&[0x31; 32]),
        None,
        SaveIntent::Draft,
        &serde_json::to_vec(&definition).unwrap(),
        br#"{"positions":[]}"#,
    )
    .unwrap()
}

#[cfg(not(feature = "encrypted-sqlite"))]
#[test]
fn unsupported_constructor_returns_public_error_before_disk_access() {
    let dir = TestDir::new("unsupported");
    let error = match Workspace::encrypted(dir.0.clone(), key(0x11)) {
        Err(error) => error,
        Ok(_) => panic!("default build accepted encrypted storage"),
    };
    assert_eq!(error, StoreError::EncryptionUnavailable);
    assert_eq!(error.public().0, "encryption_unavailable");
    assert!(!dir.0.exists());
}

#[cfg(feature = "encrypted-sqlite")]
#[test]
fn encrypted_save_reopen_clone_keeps_exact_source_and_hides_markers() {
    let dir = TestDir::new("reopen");
    let source = exact_source();
    let workspace = Workspace::encrypted(dir.0.clone(), key(0x21)).unwrap();

    workspace.save(&source).unwrap();
    let cloned = workspace.clone();
    assert_eq!(cloned.open(&source.source.id).unwrap(), source);

    let reopened = Workspace::encrypted(dir.0.clone(), key(0x21)).unwrap();
    assert_eq!(reopened.open(&source.source.id).unwrap(), source);
    let bytes = fs::read(dir.path()).unwrap();
    assert!(!bytes.starts_with(b"SQLite format 3\0"));
    for marker in [
        b"Keep exact".as_slice(),
        b"Preserve source identities".as_slice(),
        source.source.id.as_bytes(),
    ] {
        assert!(!bytes.windows(marker.len()).any(|window| window == marker));
    }

    let raw = Connection::open(dir.path()).unwrap();
    assert!(
        raw.query_row("SELECT count(*) FROM sqlite_schema", [], |row| row
            .get::<_, i64>(0))
            .is_err()
    );

    let before = fs::read(dir.path()).unwrap();
    assert!(Workspace::new(dir.0.clone()).list().is_err());
    assert_eq!(fs::read(dir.path()).unwrap(), before);
}

#[cfg(feature = "encrypted-sqlite")]
#[test]
fn keyed_rejections_are_closed_and_never_create_missing_state() {
    let missing = TestDir::new("missing");
    let missing_workspace = Workspace::encrypted(missing.0.clone(), key(0x32)).unwrap();
    assert!(missing_workspace.list().unwrap().is_empty());
    assert_eq!(
        missing_workspace
            .open("source:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        Err(StoreError::NotFound)
    );
    assert!(!missing.0.exists());

    let encrypted = TestDir::new("rejections");
    let source = exact_source();
    Workspace::encrypted(encrypted.0.clone(), key(0x33))
        .unwrap()
        .save(&source)
        .unwrap();
    let ciphertext = fs::read(encrypted.path()).unwrap();
    let wrong = Workspace::encrypted(encrypted.0.clone(), key(0x34)).unwrap();
    let error = wrong.list().unwrap_err();
    assert_eq!(error, StoreError::EncryptedStoreInvalid);
    assert_eq!(error.public().0, "encrypted_store_invalid");
    assert_eq!(fs::read(encrypted.path()).unwrap(), ciphertext);

    let plain = TestDir::new("plain");
    Workspace::new(plain.0.clone()).save(&source).unwrap();
    let plaintext = fs::read(plain.path()).unwrap();
    let error = Workspace::encrypted(plain.0.clone(), key(0x35))
        .unwrap()
        .list()
        .unwrap_err();
    assert_eq!(error, StoreError::EncryptedStoreInvalid);
    assert_eq!(error.public().0, "encrypted_store_invalid");
    assert_eq!(fs::read(plain.path()).unwrap(), plaintext);
}

#[cfg(feature = "encrypted-sqlite")]
#[test]
fn allocated_page_tamper_fails_closed_without_returning_content() {
    let dir = TestDir::new("tamper");
    let workspace = Workspace::encrypted(dir.0.clone(), key(0x41)).unwrap();
    let selected = analyze(
        "selected.md",
        b"# Selected\nReturn no content after tamper.\n",
    )
    .unwrap();
    let line = format!("{}\n", "x".repeat(120));
    let filler = analyze(
        "unrelated.md",
        format!("# Unrelated\n{}", line.repeat(300)).as_bytes(),
    )
    .unwrap();
    workspace.save(&selected).unwrap();
    workspace.save(&filler).unwrap();
    drop(workspace);

    let original = fs::read(dir.path()).unwrap();
    assert!(original.len() >= 3 * 4096);
    let mut page_tamper = original.clone();
    let offset = page_tamper.len() - 2048;
    page_tamper[offset] ^= 0x80;
    let mut truncated = original.clone();
    truncated.pop();
    let mut appended = original;
    appended.push(0x5a);

    for (label, bytes) in [
        ("allocated-page", page_tamper),
        ("truncated", truncated),
        ("appended", appended),
    ] {
        let modified = TestDir::new(label);
        fs::create_dir(&modified.0).unwrap();
        fs::write(modified.path(), bytes).unwrap();
        let reopened = Workspace::encrypted(modified.0.clone(), key(0x41)).unwrap();
        let error = reopened.open(&selected.source.id).unwrap_err();
        assert_eq!(error, StoreError::EncryptedStoreInvalid);
        assert_eq!(error.public().0, "encrypted_store_invalid");
    }

    let empty = TestDir::new("truncated-empty");
    fs::create_dir(&empty.0).unwrap();
    fs::write(empty.path(), []).unwrap();
    let empty_store = Workspace::encrypted(empty.0.clone(), key(0x41)).unwrap();
    let error = empty_store.save(&selected).unwrap_err();
    assert_eq!(error, StoreError::EncryptedStoreInvalid);
    assert_eq!(error.public().0, "encrypted_store_invalid");
    assert!(fs::read(empty.path()).unwrap().is_empty());
}

#[cfg(feature = "encrypted-sqlite")]
#[test]
fn encrypted_public_store_paths_cover_legacy_backup_workflow_composition_and_compilation() {
    let source = exact_source();
    let legacy = TestDir::new("legacy-backup");
    let legacy_store = Workspace::new(legacy.0.clone());
    legacy_store.save(&source).unwrap();
    let legacy_bytes = legacy_store.export_backup().unwrap();
    assert!(
        legacy_bytes
            .windows(b"Keep exact".len())
            .any(|window| window == b"Keep exact")
    );
    let legacy_backup = Backup::decode(&legacy_bytes).unwrap();

    let restored = TestDir::new("legacy-restore");
    let restored_store = Workspace::encrypted(restored.0.clone(), key(0x51)).unwrap();
    let restore_plan = restored_store.prepare_restore(&legacy_backup).unwrap();
    restored_store
        .restore_backup(&legacy_backup, &restore_plan.expected_state_id)
        .unwrap();
    assert_eq!(restored_store.open(&source.source.id).unwrap(), source);

    let dir = TestDir::new("public-paths");
    let store = Workspace::encrypted(dir.0.clone(), key(0x52)).unwrap();
    store.save_v1(&source).unwrap();
    assert!(store.list_capabilities().unwrap().is_empty());
    assert_eq!(store.data().unwrap().sources.len(), 1);
    assert_eq!(store.composition_data().unwrap().records.sources, 1);
    let capability = store
        .create_capability_v1(&source.source.id, &source.fragments[0].id, "Rules")
        .unwrap()
        .capability;
    store
        .review_capability_v1(&capability.id, &capability.latest_revision_id)
        .unwrap();
    assert_eq!(
        store
            .compile_capability(
                &capability.id,
                &capability.latest_revision_id,
                Profile::AgentsMdV1
            )
            .unwrap()
            .compilation
            .artifact
            .content,
        source.fragments[0].text
    );

    let composition_source = composition_source();
    store.save_v1(&composition_source).unwrap();
    let preview = store.preview_composition(&composition_request()).unwrap();
    let composed = store.apply_composition(&preview, true).unwrap();
    assert_eq!(composed.capabilities.len(), 2);

    let candidate = workflow();
    let plan = store.inspect_workflow_save(&candidate).unwrap();
    store
        .save_workflow(&candidate, &plan.expected_state_id)
        .unwrap();
    assert_eq!(store.workflow_data().unwrap().records.workflows, 1);
    assert_eq!(
        store
            .open_workflow(candidate.workflow_id(), None)
            .unwrap()
            .revision,
        candidate
    );
    let workflow_backup =
        WorkspaceBackup::decode(&store.export_workflow_backup().unwrap()).unwrap();

    let destination = TestDir::new("workflow-restore");
    let destination_store = Workspace::encrypted(destination.0.clone(), key(0x53)).unwrap();
    let retained = destination_store
        .prepare_workflow_restore(&workflow_backup)
        .unwrap();
    let data = destination_store
        .restore_workflow_backup(&workflow_backup, &retained)
        .unwrap();
    assert_eq!(data.records.workflows, 1);
    assert_eq!(
        destination_store
            .open_workflow(candidate.workflow_id(), None)
            .unwrap()
            .revision,
        candidate
    );
    drop(destination_store);

    let reopened_destination = Workspace::encrypted(destination.0.clone(), key(0x53)).unwrap();
    assert_eq!(
        reopened_destination
            .open_workflow(candidate.workflow_id(), None)
            .unwrap()
            .revision,
        candidate
    );
    let deletion = reopened_destination
        .inspect_workflow_deletion(WorkflowRecordKind::Workflow, candidate.workflow_id())
        .unwrap();
    reopened_destination
        .delete_workflow_record(&deletion)
        .unwrap();
    assert!(reopened_destination.list_workflows().unwrap().is_empty());
}
