use super::*;
use std::{
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::Instant,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct TestDir(PathBuf);
impl TestDir {
    fn new() -> Self {
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
                "rangoon-store-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
    }
    fn store(&self) -> Workspace {
        Workspace::new(self.0.clone())
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn report(bytes: &[u8]) -> AnalysisReport {
    analyze("AGENTS.md", bytes).unwrap()
}
fn sample() -> AnalysisReport {
    report(b"\xef\xbb\xbf# Rules\r\nKeep original bytes.\r\n")
}

#[test]
fn listing_missing_workspace_creates_nothing() {
    let dir = TestDir::new();
    assert!(dir.store().list().unwrap().is_empty());
    assert_eq!(
        dir.store().open(&sample().source.id),
        Err(StoreError::NotFound)
    );
    assert!(!dir.0.exists());
}
#[test]
fn explicit_save_survives_reopen_with_exact_bytes_and_deduplicates() {
    let dir = TestDir::new();
    let original = sample();
    let first = dir.store().save(&original).unwrap();
    assert!(!first.already_saved);
    let reopened = Workspace::new(dir.0.clone());
    assert_eq!(reopened.open(&original.source.id).unwrap(), original);
    let repeated = reopened.save(&original).unwrap();
    assert!(repeated.already_saved);
    assert_eq!(repeated.snapshot, first.snapshot);
    let changed = report(b"# Rules\nNew bytes\n");
    reopened.save(&changed).unwrap();
    assert_eq!(reopened.list().unwrap().len(), 2);
    assert_eq!(reopened.open(&original.source.id).unwrap(), original);
    let metadata = serde_json::to_string(&reopened.list().unwrap()).unwrap();
    assert!(!metadata.contains("content"));
    assert!(!metadata.contains(&dir.0.to_string_lossy().to_string()));
    assert_eq!(original.authority, rangoon_domain::Authority::None);
    assert!(
        original
            .fragments
            .iter()
            .all(|f| f.review_state == rangoon_domain::ReviewState::Unreviewed)
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&dir.0).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(dir.store().path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
#[test]
fn identities_and_tampered_sources_are_rejected() {
    let dir = TestDir::new();
    let mut original = sample();
    assert_eq!(dir.store().open("../../file"), Err(StoreError::InvalidId));
    original.source.sha256 = "0".repeat(64);
    assert_eq!(
        dir.store().save(&original).unwrap_err(),
        StoreError::Corrupt
    );
    assert!(!dir.0.exists());
    let original = sample();
    dir.store().save(&original).unwrap();
    let db = Connection::open(dir.store().path()).unwrap();
    db.execute(
        "UPDATE snapshots SET content = ?1",
        [b"# Changed".as_slice()],
    )
    .unwrap();
    assert_eq!(
        dir.store().open(&original.source.id),
        Err(StoreError::Corrupt)
    );
    assert_eq!(
        dir.store().save(&original).unwrap_err(),
        StoreError::Corrupt
    );
}
#[test]
fn foreign_future_and_modified_schemas_are_not_replaced() {
    for sql in [
        "CREATE TABLE foreign_data (value TEXT)",
        "PRAGMA user_version = 2",
        "CREATE TRIGGER surprise AFTER INSERT ON snapshots BEGIN DELETE FROM snapshots; END",
    ] {
        let dir = TestDir::new();
        dir.store().save(&sample()).unwrap();
        let db = Connection::open(dir.store().path()).unwrap();
        db.execute_batch(sql).unwrap();
        drop(db);
        let before = fs::read(dir.store().path()).unwrap();
        assert_eq!(
            dir.store().save(&sample()).unwrap_err(),
            StoreError::UnsupportedSchema
        );
        assert_eq!(fs::read(dir.store().path()).unwrap(), before);
    }
}
#[test]
fn cap_preserves_existing_snapshots_and_duplicates() {
    let dir = TestDir::new();
    for n in 0..MAX_SNAPSHOTS {
        dir.store()
            .save(&report(format!("# Source {n}\n").as_bytes()))
            .unwrap();
    }
    assert_eq!(dir.store().save(&sample()).unwrap_err(), StoreError::Full);
    assert!(
        dir.store()
            .save(&report(b"# Source 0\n"))
            .unwrap()
            .already_saved
    );
    assert_eq!(dir.store().list().unwrap().len(), MAX_SNAPSHOTS);
}
#[test]
fn rollback_and_full_database_preserve_committed_source() {
    let dir = TestDir::new();
    let original = sample();
    dir.store().save(&original).unwrap();
    let mut db = Connection::open(dir.store().path()).unwrap();
    let pages: u32 = db
        .pragma_query_value(None, "page_count", |r| r.get(0))
        .unwrap();
    db.pragma_update(None, "max_page_count", pages).unwrap();
    let mut large = String::new();
    for _ in 0..100 {
        large.push_str(&"x".repeat(2000));
        large.push('\n');
    }
    let next = report(large.as_bytes());
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let meta = SnapshotMetadata {
        source_id: next.source.id,
        display_name: next.source.display_name,
        sha256: next.source.sha256,
        byte_length: next.source.byte_length as u32,
        saved_at_ms: 1,
    };
    let result = insert(&tx, &meta, next.source.content.as_bytes());
    assert!(result.is_err(), "page limit must exercise SQLITE_FULL");
    drop(tx);
    drop(db);
    assert_eq!(dir.store().list().unwrap().len(), 1);
    assert_eq!(dir.store().open(&original.source.id).unwrap(), original);
}
#[test]
fn failed_schema_transaction_is_recoverable_on_save() {
    let dir = TestDir::new();
    fs::create_dir_all(&dir.0).unwrap();
    let mut db = Connection::open(dir.store().path()).unwrap();
    let tx = db.transaction().unwrap();
    initialize_or_verify(&tx).unwrap();
    drop(tx);
    drop(db);
    assert_eq!(
        dir.store().list().unwrap_err(),
        StoreError::UnsupportedSchema
    );
    dir.store().save(&sample()).unwrap();
    assert_eq!(dir.store().list().unwrap().len(), 1);
}
#[test]
fn wrong_file_type_and_corrupt_database_fail_without_reset() {
    let dir = TestDir::new();
    fs::create_dir_all(dir.store().path()).unwrap();
    assert_eq!(
        dir.store().save(&sample()).unwrap_err(),
        StoreError::Unavailable
    );
    fs::remove_dir(dir.store().path()).unwrap();
    fs::write(dir.store().path(), b"not sqlite").unwrap();
    assert!(dir.store().list().is_err());
    assert!(dir.store().save(&sample()).is_err());
    assert_eq!(fs::read(dir.store().path()).unwrap(), b"not sqlite");
}
#[cfg(unix)]
#[test]
fn symlinks_and_permissions_do_not_redirect_storage() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = TestDir::new();
    let target = TestDir::new();
    fs::create_dir_all(&target.0).unwrap();
    symlink(&target.0, &dir.0).unwrap();
    assert_eq!(
        dir.store().save(&sample()).unwrap_err(),
        StoreError::Unavailable
    );
    fs::remove_file(&dir.0).unwrap();
    fs::create_dir(&dir.0).unwrap();
    fs::write(target.0.join("target"), b"untouched").unwrap();
    symlink(target.0.join("target"), dir.store().path()).unwrap();
    assert_eq!(
        dir.store().save(&sample()).unwrap_err(),
        StoreError::Unavailable
    );
    assert_eq!(fs::read(target.0.join("target")).unwrap(), b"untouched");
    fs::remove_file(dir.store().path()).unwrap();
    dir.store().save(&sample()).unwrap();
    fs::set_permissions(dir.store().path(), fs::Permissions::from_mode(0o400)).unwrap();
    // CI runs as a normal user. A root test process may bypass file mode restrictions.
    if fs::OpenOptions::new()
        .write(true)
        .open(dir.store().path())
        .is_err()
    {
        assert_eq!(
            dir.store().save(&report(b"next")).unwrap_err(),
            StoreError::Unavailable
        );
    }
    fs::set_permissions(dir.store().path(), fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(dir.store().list().unwrap().len(), 1);
}

#[test]
fn interrupted_writer_recovers_committed_data() {
    let dir = TestDir::new();
    dir.store().save(&sample()).unwrap();
    let marker = dir.0.join("ready");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "tests::crash_writer_child"])
        .env("RANGOON_TEST_CRASH_DB", dir.store().path())
        .env("RANGOON_TEST_CRASH_READY", &marker)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !marker.exists() && Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            panic!("crash writer exited before preparing transaction");
        }
        thread::sleep(Duration::from_millis(20));
    }
    let ready = marker.exists();
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(ready, "writer did not reach uncommitted transaction");
    assert_eq!(dir.store().list().unwrap().len(), 1);
    assert_eq!(dir.store().open(&sample().source.id).unwrap(), sample());
}
#[test]
#[ignore = "subprocess helper killed by interrupted_writer_recovers_committed_data"]
fn crash_writer_child() {
    let path = std::env::var_os("RANGOON_TEST_CRASH_DB").unwrap();
    let marker = std::env::var_os("RANGOON_TEST_CRASH_READY").unwrap();
    let mut db = Connection::open(PathBuf::from(path)).unwrap();
    db.pragma_update(None, "cache_size", 1).unwrap();
    db.pragma_update(None, "cache_spill", true).unwrap();
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    tx.execute("UPDATE snapshots SET content = zeroblob(200000)", [])
        .unwrap();
    fs::write(marker, b"transaction uncommitted").unwrap();
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
