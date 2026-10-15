#[cfg(feature = "encrypted-sqlite")]
use rangoon_compose::application::{self, Request, Target};
#[cfg(feature = "encrypted-sqlite")]
use rangoon_compose::{Draft, InputRange, InputReference, Operation, OutputRecipe, Piece};
#[cfg(feature = "encrypted-sqlite")]
use rangoon_store::WorkspaceKey;
use rangoon_store::{StoreError, Workspace};
#[cfg(feature = "encrypted-sqlite")]
use rangoon_workflow::records::{SaveIntent, prepare_revision, workflow_id_from_nonce};
#[cfg(feature = "encrypted-sqlite")]
use std::sync::Barrier;
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
            "rangoon-empty-encrypted-{label}-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        )))
    }

    #[cfg(feature = "encrypted-sqlite")]
    fn database(&self) -> PathBuf {
        self.0.join("workspace.sqlite3")
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(feature = "encrypted-sqlite")]
fn key(seed: u8) -> WorkspaceKey {
    WorkspaceKey::from_bytes([seed; 32])
}

#[test]
fn unkeyed_empty_initialization_refuses_before_directory_access() {
    let directory = TestDir::new("unkeyed");
    let workspace = Workspace::new(directory.0.clone());

    assert_eq!(
        workspace.initialize_empty_encrypted(),
        Err(StoreError::EncryptionUnavailable)
    );
    assert!(!directory.0.exists());
}

#[cfg(feature = "encrypted-sqlite")]
#[test]
fn empty_initialization_has_zero_canonical_inventory_and_ciphertext() {
    let directory = TestDir::new("inventory");
    let workspace = Workspace::encrypted(directory.0.clone(), key(0x41)).unwrap();

    workspace.initialize_empty_encrypted().unwrap();
    assert!(workspace.list().unwrap().is_empty());
    let data = workspace.data().unwrap();
    assert!(data.sources.is_empty());
    assert!(data.capabilities.is_empty());
    let inventory = workspace.workflow_data().unwrap();
    assert!(inventory.sources.is_empty());
    assert!(inventory.capabilities.is_empty());
    assert!(inventory.workflows.is_empty());
    assert!(inventory.database_bytes > 0);

    let reopened = Workspace::encrypted(directory.0.clone(), key(0x41)).unwrap();
    assert!(reopened.list().unwrap().is_empty());
    let bytes = fs::read(directory.database()).unwrap();
    assert!(!bytes.starts_with(b"SQLite format 3\0"));
    for marker in [b"snapshots".as_slice(), b"workspace.sqlite3".as_slice()] {
        assert!(!bytes.windows(marker.len()).any(|window| window == marker));
    }
}

#[cfg(feature = "encrypted-sqlite")]
#[test]
fn empty_initialization_refuses_existing_base_and_sidecars_without_changes() {
    for suffix in ["", "-journal", "-wal", "-shm"] {
        for original in [Vec::new(), format!("do not adopt {suffix}").into_bytes()] {
            let directory = TestDir::new("existing");
            fs::create_dir(&directory.0).unwrap();
            let path = directory.0.join(format!("workspace.sqlite3{suffix}"));
            fs::write(&path, &original).unwrap();
            let workspace = Workspace::encrypted(directory.0.clone(), key(0x42)).unwrap();

            assert_eq!(
                workspace.initialize_empty_encrypted(),
                Err(StoreError::Unavailable)
            );
            assert_eq!(fs::read(&path).unwrap(), original);
            assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
        }
    }
}

#[cfg(feature = "encrypted-sqlite")]
#[test]
fn empty_initialization_refuses_existing_plain_sqlite_without_change() {
    let directory = TestDir::new("plain-sqlite");
    let source = rangoon_import::analyze("AGENTS.md", b"# Existing plaintext\n").unwrap();
    Workspace::new(directory.0.clone()).save(&source).unwrap();
    let original = fs::read(directory.database()).unwrap();
    assert!(original.starts_with(b"SQLite format 3\0"));

    let encrypted = Workspace::encrypted(directory.0.clone(), key(0x48)).unwrap();
    assert_eq!(
        encrypted.initialize_empty_encrypted(),
        Err(StoreError::Unavailable)
    );
    assert_eq!(fs::read(directory.database()).unwrap(), original);
}

#[cfg(feature = "encrypted-sqlite")]
#[test]
fn empty_initialization_refuses_nonregular_base_and_sidecars_without_changes() {
    for suffix in ["", "-journal", "-wal", "-shm"] {
        let directory = TestDir::new("nonregular");
        fs::create_dir(&directory.0).unwrap();
        let path = directory.0.join(format!("workspace.sqlite3{suffix}"));
        fs::create_dir(&path).unwrap();
        let workspace = Workspace::encrypted(directory.0.clone(), key(0x49)).unwrap();

        assert_eq!(
            workspace.initialize_empty_encrypted(),
            Err(StoreError::Unavailable)
        );
        assert!(fs::symlink_metadata(&path).unwrap().is_dir());
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }
}

#[cfg(all(feature = "encrypted-sqlite", unix))]
#[test]
fn empty_initialization_refuses_linked_base_and_sidecars_without_changes() {
    use std::os::unix::fs::symlink;

    for suffix in ["", "-journal", "-wal", "-shm"] {
        let directory = TestDir::new("linked");
        fs::create_dir(&directory.0).unwrap();
        let target = directory.0.join("existing-target");
        let original = format!("linked {suffix}").into_bytes();
        fs::write(&target, &original).unwrap();
        let path = directory.0.join(format!("workspace.sqlite3{suffix}"));
        symlink(&target, &path).unwrap();
        let workspace = Workspace::encrypted(directory.0.clone(), key(0x4a)).unwrap();

        assert_eq!(
            workspace.initialize_empty_encrypted(),
            Err(StoreError::Unavailable)
        );
        assert!(
            fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(&target).unwrap(), original);
    }
}

#[cfg(feature = "encrypted-sqlite")]
#[test]
fn repeated_empty_initialization_refuses_existing_ciphertext_without_change() {
    let directory = TestDir::new("repeat");
    let workspace = Workspace::encrypted(directory.0.clone(), key(0x43)).unwrap();
    workspace.initialize_empty_encrypted().unwrap();
    let bytes = fs::read(directory.database()).unwrap();

    assert_eq!(
        workspace.initialize_empty_encrypted(),
        Err(StoreError::Unavailable)
    );
    assert_eq!(fs::read(directory.database()).unwrap(), bytes);
}

#[cfg(feature = "encrypted-sqlite")]
#[test]
fn concurrent_empty_initializers_allow_exactly_one_creator() {
    let directory = TestDir::new("concurrent");
    let barrier = std::sync::Arc::new(Barrier::new(2));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let path = directory.0.clone();
        let gate = barrier.clone();
        workers.push(std::thread::spawn(move || {
            let workspace = Workspace::encrypted(path, key(0x44)).unwrap();
            gate.wait();
            workspace.initialize_empty_encrypted()
        }));
    }
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(StoreError::Unavailable)))
            .count(),
        1
    );
    assert!(
        Workspace::encrypted(directory.0.clone(), key(0x44))
            .unwrap()
            .list()
            .unwrap()
            .is_empty()
    );
}

#[cfg(feature = "encrypted-sqlite")]
#[test]
fn wrong_key_refuses_empty_inventory_without_changing_ciphertext() {
    let directory = TestDir::new("wrong-key");
    let workspace = Workspace::encrypted(directory.0.clone(), key(0x45)).unwrap();
    workspace.initialize_empty_encrypted().unwrap();
    let original = fs::read(directory.database()).unwrap();
    let wrong = Workspace::encrypted(directory.0.clone(), key(0x46)).unwrap();

    assert!(matches!(
        wrong.workflow_data(),
        Err(StoreError::EncryptedStoreInvalid)
    ));
    assert_eq!(fs::read(directory.database()).unwrap(), original);
}

#[cfg(feature = "encrypted-sqlite")]
#[test]
fn first_source_save_after_empty_initialization_uses_existing_save_path() {
    let directory = TestDir::new("first-save");
    let workspace = Workspace::encrypted(directory.0.clone(), key(0x47)).unwrap();
    workspace.initialize_empty_encrypted().unwrap();
    let source = rangoon_import::analyze(
        "AGENTS.md",
        b"# Rules\nFirst source follows an actual empty encrypted inventory.\n",
    )
    .unwrap();

    let receipt = workspace.save_v1(&source).unwrap();
    assert!(!receipt.already_saved);
    assert_eq!(workspace.open(&source.source.id).unwrap(), source);
    assert_eq!(workspace.data().unwrap().sources.len(), 1);

    let capability = workspace
        .create_capability_v1(
            &source.source.id,
            &source.fragments[0].id,
            "Empty workspace follow-on capability",
        )
        .unwrap()
        .capability;
    workspace
        .review_capability_v1(&capability.id, &capability.latest_revision_id)
        .unwrap();
    let request = Request {
        schema_version: application::REQUEST_SCHEMA.into(),
        draft: Draft {
            schema_version: rangoon_compose::DRAFT_SCHEMA.into(),
            operation: Operation::Decompose,
            inputs: vec![InputReference::Source {
                source_id: source.source.id.clone(),
                sha256: source.source.sha256.clone(),
            }],
            outputs: vec![OutputRecipe {
                title: "Empty workspace exact composition".into(),
                pieces: vec![Piece::Copy {
                    range: InputRange {
                        input_index: 0,
                        start_byte: 0,
                        end_byte: source.source.content.len() as u64,
                    },
                }],
            }],
            exclusions: vec![],
            duplications: vec![],
            conflicts: vec![],
        },
        targets: vec![Target::New {}],
    };
    let composition = workspace.preview_composition(&request).unwrap();
    let committed = workspace.apply_composition(&composition, true).unwrap();
    assert_eq!(committed.capabilities.len(), 1);
    let revision = &committed.capabilities[0].revision;
    assert_eq!(revision.content, source.source.content);
    match &revision.provenance {
        rangoon_domain::capability_v1::RevisionProvenance::Composition {
            application_id,
            composition_id,
            output_index,
        } => {
            assert_eq!(application_id, &committed.application_id);
            assert_eq!(composition_id, &committed.composition_id);
            assert_eq!(*output_index, 0);
        }
        _ => panic!("expected composition provenance"),
    }

    let definition = serde_json::json!({
        "schemaVersion": "rangoon.workflow-definition.v1",
        "title": "Empty workspace follow-on workflow",
        "nodes": [],
        "controlEdges": [],
        "dataEdges": []
    });
    let workflow = prepare_revision(
        &workflow_id_from_nonce(&[0x47; 32]),
        None,
        SaveIntent::Draft,
        &serde_json::to_vec(&definition).unwrap(),
        br#"{"positions":[]}"#,
    )
    .unwrap();
    let plan = workspace.inspect_workflow_save(&workflow).unwrap();
    workspace
        .save_workflow(&workflow, &plan.expected_state_id)
        .unwrap();
    assert_eq!(workspace.workflow_data().unwrap().workflows.len(), 1);
}
