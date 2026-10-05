use super::*;
use rangoon_domain::{
    AnalysisReport, Authority, SourceFragment, byte_digest,
    capability::{MAX_CONTENT_BYTES, capability_id, revision_id},
};
use rangoon_import::analyze;
use rusqlite::{Connection, TransactionBehavior, params};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rangoon-capability-store-{}-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
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

fn saved_source(dir: &TestDir) -> (AnalysisReport, SourceFragment) {
    let source = report(b"\xef\xbb\xbf# Rules\r\nKeep this exact fragment.\r\n");
    dir.store().save(&source).unwrap();
    let fragment = source
        .fragments
        .iter()
        .find(|fragment| fragment.text.contains("Keep this exact fragment."))
        .unwrap()
        .clone();
    (source, fragment)
}

fn create(
    dir: &TestDir,
    source: &AnalysisReport,
    fragment: &SourceFragment,
    title: &str,
) -> CapabilityReceipt {
    dir.store()
        .create_capability(&source.source.id, &fragment.id, title)
        .unwrap()
}

#[test]
fn capability_lifecycle_keeps_exact_provenance_and_reviewed_history() {
    let dir = TestDir::new();
    let (source, fragment) = saved_source(&dir);
    assert!(source.source.content.starts_with('\u{feff}'));
    assert!(source.source.content.contains("\r\n"));

    let created = create(&dir, &source, &fragment, "Rules");
    assert!(!created.already_applied);
    let first = created.capability.revision.id.clone();
    assert_eq!(created.capability.source_id, source.source.id);
    assert_eq!(created.capability.fragment_id, fragment.id);
    assert_eq!(created.capability.span, fragment.span);
    assert_eq!(created.capability.original_text, fragment.text);
    assert_eq!(created.capability.revision.content, fragment.text);
    assert_eq!(created.capability.authority, Authority::None);

    let reviewed = dir
        .store()
        .review_capability(&created.capability.id, &first)
        .unwrap();
    assert!(!reviewed.already_applied);
    assert_eq!(reviewed.capability.revision.id, first);
    assert!(reviewed.capability.revision.review.is_some());
    let reviewed_again = dir
        .store()
        .review_capability(&created.capability.id, &first)
        .unwrap();
    assert!(reviewed_again.already_applied);
    assert_eq!(reviewed_again.capability, reviewed.capability);
    let unchanged = dir
        .store()
        .revise_capability(&created.capability.id, &first, "Rules", &fragment.text)
        .unwrap();
    assert!(unchanged.already_applied);
    assert_eq!(unchanged.capability, reviewed.capability);

    let edited = dir
        .store()
        .revise_capability(
            &created.capability.id,
            &first,
            "Rules v2",
            "Edited locally.\r\n",
        )
        .unwrap();
    assert!(!edited.already_applied);
    assert_ne!(edited.capability.revision.id, first);
    assert!(edited.capability.revision.review.is_none());
    assert_eq!(edited.capability.history.len(), 2);
    assert!(edited.capability.history[0].review.is_some());
    assert!(edited.capability.history[1].review.is_none());

    let reopened = Workspace::new(dir.0.clone());
    let old = reopened
        .open_capability(&created.capability.id, Some(&first))
        .unwrap();
    assert_eq!(old.latest_revision_id, edited.capability.latest_revision_id);
    assert_eq!(old.revision.id, first);
    assert!(old.revision.review.is_some());
    assert_eq!(old.original_text, fragment.text);
    assert_eq!(reopened.open(&source.source.id).unwrap(), source);
    assert!(matches!(
        reopened.review_capability(&created.capability.id, &first),
        Err(StoreError::CapabilityConflict)
    ));
    let recreated = create(&dir, &source, &fragment, "Rules");
    assert!(recreated.already_applied);
    assert_eq!(recreated.capability, edited.capability);
}

#[test]
fn capability_retries_are_deterministic_and_stale_writes_conflict() {
    let dir = TestDir::new();
    let (source, fragment) = saved_source(&dir);
    let created = create(&dir, &source, &fragment, "Rules");
    let first = created.capability.latest_revision_id.clone();

    let changed = dir
        .store()
        .revise_capability(&created.capability.id, &first, "Rules", "First edit\n")
        .unwrap();
    assert!(!changed.already_applied);

    let retry = dir
        .store()
        .revise_capability(&created.capability.id, &first, "Rules", "First edit\n")
        .unwrap();
    assert!(retry.already_applied);
    assert_eq!(retry.capability, changed.capability);

    assert!(matches!(
        dir.store().revise_capability(
            &created.capability.id,
            &first,
            "Rules",
            "Different stale edit\n"
        ),
        Err(StoreError::CapabilityConflict)
    ));
}

#[test]
fn capability_requires_saved_source_and_rejects_bad_ids_and_input() {
    let dir = TestDir::new();
    let source = report(b"# Rules\nSource must be saved.\n");
    let fragment = source.fragments[0].clone();
    assert!(matches!(
        dir.store()
            .create_capability(&source.source.id, &fragment.id, "Rules"),
        Err(StoreError::NotFound)
    ));

    dir.store().save(&source).unwrap();
    assert!(matches!(
        dir.store()
            .create_capability("source:invalid", &fragment.id, "Rules"),
        Err(StoreError::CapabilityInvalid)
    ));
    assert!(matches!(
        dir.store()
            .create_capability(&source.source.id, "fragment:invalid", "Rules"),
        Err(StoreError::CapabilityInvalid)
    ));
    assert!(matches!(
        dir.store()
            .create_capability(&source.source.id, &fragment.id, " title"),
        Err(StoreError::CapabilityInvalid)
    ));
    assert!(matches!(
        dir.store().open_capability("capability:invalid", None),
        Err(StoreError::CapabilityInvalid)
    ));

    let created = create(&dir, &source, &fragment, "Rules");
    let revision = created.capability.latest_revision_id.clone();
    assert!(matches!(
        dir.store()
            .revise_capability(&created.capability.id, &revision, "Rules", " \r\n"),
        Err(StoreError::CapabilityInvalid)
    ));
    let absent = capability_id(&source.source.id, &fragment.id, "Absent");
    assert!(matches!(
        dir.store().open_capability(&absent, None),
        Err(StoreError::CapabilityNotFound)
    ));
}

#[test]
fn source_only_schema_stays_v1_until_first_capability_creation() {
    let dir = TestDir::new();
    let (source, fragment) = saved_source(&dir);
    let version = |path: PathBuf| -> i64 {
        Connection::open(path)
            .unwrap()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap()
    };
    assert_eq!(version(dir.store().path()), 1);
    assert!(dir.store().list_capabilities().unwrap().is_empty());
    assert_eq!(version(dir.store().path()), 1);
    assert_eq!(dir.store().open(&source.source.id).unwrap(), source);

    create(&dir, &source, &fragment, "Rules");
    assert_eq!(version(dir.store().path()), 2);
}

#[test]
fn corrupt_lineage_and_review_records_fail_closed() {
    let lineage = TestDir::new();
    let (source, fragment) = saved_source(&lineage);
    let created = create(&lineage, &source, &fragment, "Rules");
    let db = Connection::open(lineage.store().path()).unwrap();
    db.execute(
        "UPDATE revisions SET parent_revision_id=?1 WHERE revision_id=?1",
        [&created.capability.latest_revision_id],
    )
    .unwrap();
    drop(db);
    assert!(matches!(
        lineage
            .store()
            .open_capability(&created.capability.id, None),
        Err(StoreError::Corrupt)
    ));

    let review = TestDir::new();
    let (source, fragment) = saved_source(&review);
    let created = create(&review, &source, &fragment, "Rules");
    review
        .store()
        .review_capability(
            &created.capability.id,
            &created.capability.latest_revision_id,
        )
        .unwrap();
    let db = Connection::open(review.store().path()).unwrap();
    db.execute(
        "UPDATE reviews SET reviewer='someone_else' WHERE revision_id=?1",
        [&created.capability.latest_revision_id],
    )
    .unwrap();
    drop(db);
    assert!(matches!(
        review.store().open_capability(&created.capability.id, None),
        Err(StoreError::Corrupt)
    ));
}

#[test]
fn concurrent_same_head_writers_have_one_winner() {
    let dir = TestDir::new();
    let (source, fragment) = saved_source(&dir);
    let created = create(&dir, &source, &fragment, "Rules");
    let capability = created.capability.id;
    let expected = created.capability.latest_revision_id;
    let start = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for (title, content) in [("Writer A", "A\n"), ("Writer B", "B\n")] {
        let store = dir.store();
        let capability = capability.clone();
        let expected = expected.clone();
        let start = Arc::clone(&start);
        workers.push(thread::spawn(move || {
            start.wait();
            store.revise_capability(&capability, &expected, title, content)
        }));
    }
    start.wait();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(StoreError::CapabilityConflict)))
            .count(),
        1
    );
}

#[test]
fn capability_and_per_capability_revision_quotas_and_content_limit_hold() {
    let dir = TestDir::new();
    let (source, fragment) = saved_source(&dir);
    let first = create(&dir, &source, &fragment, "Skill 0");
    let capability = first.capability.id;
    let mut head = first.capability.latest_revision_id;
    let largest = "x".repeat(MAX_CONTENT_BYTES);
    let saved = dir
        .store()
        .revise_capability(&capability, &head, "Skill 0", &largest)
        .unwrap();
    head = saved.capability.latest_revision_id;
    assert!(matches!(
        dir.store().revise_capability(
            &capability,
            &head,
            "Skill 0",
            &"x".repeat(MAX_CONTENT_BYTES + 1)
        ),
        Err(StoreError::CapabilityInvalid)
    ));
    for revision in 2..MAX_REVISIONS {
        let saved = dir
            .store()
            .revise_capability(
                &capability,
                &head,
                "Skill 0",
                &format!("Revision {revision}\n"),
            )
            .unwrap();
        head = saved.capability.latest_revision_id;
    }
    assert!(matches!(
        dir.store()
            .revise_capability(&capability, &head, "Skill 0", "Too many revisions\n"),
        Err(StoreError::CapabilityFull)
    ));
    for capability_number in 1..MAX_CAPABILITIES {
        create(
            &dir,
            &source,
            &fragment,
            &format!("Skill {capability_number}"),
        );
    }
    assert!(matches!(
        dir.store()
            .create_capability(&source.source.id, &fragment.id, "Overflow skill"),
        Err(StoreError::CapabilityFull)
    ));
    assert_eq!(
        dir.store().list_capabilities().unwrap().len(),
        MAX_CAPABILITIES
    );
}

#[test]
fn total_revision_quota_rejects_new_creation_without_losing_existing_records() {
    let dir = TestDir::new();
    let (source, fragment) = saved_source(&dir);
    for capability_number in 0..(MAX_TOTAL_REVISIONS / MAX_REVISIONS) {
        let title = format!("Skill {capability_number}");
        let created = create(&dir, &source, &fragment, &title);
        let mut head = created.capability.latest_revision_id;
        for revision_number in 1..MAX_REVISIONS {
            let saved = dir
                .store()
                .revise_capability(
                    &created.capability.id,
                    &head,
                    &title,
                    &format!("Revision {revision_number}\n"),
                )
                .unwrap();
            head = saved.capability.latest_revision_id;
        }
    }
    assert!(matches!(
        dir.store()
            .create_capability(&source.source.id, &fragment.id, "One too many revisions"),
        Err(StoreError::CapabilityFull)
    ));
    assert_eq!(
        dir.store().list_capabilities().unwrap().len(),
        MAX_TOTAL_REVISIONS / MAX_REVISIONS
    );
}

#[test]
fn sqlite_full_during_revision_write_keeps_the_reviewed_head() {
    let dir = TestDir::new();
    let (source, fragment) = saved_source(&dir);
    let created = create(&dir, &source, &fragment, "Rules");
    let reviewed = dir
        .store()
        .review_capability(
            &created.capability.id,
            &created.capability.latest_revision_id,
        )
        .unwrap()
        .capability;
    let (_, title, content, sha256) = spilled_successor(&reviewed.id, &reviewed.latest_revision_id);

    let mut db = Connection::open(dir.store().path()).unwrap();
    let pages: u32 = db
        .pragma_query_value(None, "page_count", |row| row.get(0))
        .unwrap();
    db.pragma_update(None, "max_page_count", pages).unwrap();
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let revision = revision_id(
        &reviewed.id,
        Some(&reviewed.latest_revision_id),
        &title,
        &content,
    );
    assert!(
        tx.execute(
            "INSERT INTO revisions VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                revision,
                reviewed.id,
                reviewed.latest_revision_id,
                title,
                content.as_bytes(),
                sha256,
                1_i64,
            ],
        )
        .is_err()
    );
    drop(tx);
    drop(db);

    assert_eq!(
        dir.store().open_capability(&reviewed.id, None).unwrap(),
        reviewed
    );
    assert_eq!(dir.store().open(&source.source.id).unwrap(), source);
}

#[test]
fn interrupted_schema_v2_write_recovers_reviewed_head_and_source() {
    let dir = TestDir::new();
    let (source, fragment) = saved_source(&dir);
    let created = create(&dir, &source, &fragment, "Rules");
    let reviewed = dir
        .store()
        .review_capability(
            &created.capability.id,
            &created.capability.latest_revision_id,
        )
        .unwrap()
        .capability;
    let marker = dir.0.join("revision-ready");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "capability_tests::crash_revision_writer_child",
        ])
        .env("RANGOON_TEST_REVISION_DB", dir.store().path())
        .env("RANGOON_TEST_REVISION_READY", &marker)
        .env("RANGOON_TEST_REVISION_CAPABILITY", &reviewed.id)
        .env("RANGOON_TEST_REVISION_HEAD", &reviewed.latest_revision_id)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !marker.exists() && Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            panic!("revision child exited before preparing transaction");
        }
        thread::sleep(Duration::from_millis(20));
    }
    let ready = marker.exists();
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(
        ready,
        "revision child did not reach uncommitted transaction"
    );

    let reopened = dir.store().open_capability(&reviewed.id, None).unwrap();
    assert_eq!(reopened, reviewed);
    assert_eq!(reopened.history.len(), 1);
    assert!(reopened.revision.review.is_some());
    assert_eq!(dir.store().open(&source.source.id).unwrap(), source);
}

#[test]
fn failed_and_interrupted_migrations_leave_schema_v1_source_recoverable() {
    let dir = TestDir::new();
    let (source, _fragment) = saved_source(&dir);
    let mut db = Connection::open(dir.store().path()).unwrap();
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    for (_, sql) in capabilities::SCHEMAS {
        tx.execute_batch(sql).unwrap();
    }
    tx.pragma_update(None, "user_version", 2).unwrap();
    assert!(
        tx.execute("INSERT INTO missing_migration_target VALUES (1)", [])
            .is_err()
    );
    drop(tx);
    drop(db);
    let version = |path: PathBuf| -> i64 {
        Connection::open(path)
            .unwrap()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap()
    };
    assert_eq!(version(dir.store().path()), 1);
    assert_eq!(dir.store().open(&source.source.id).unwrap(), source);

    let interrupted = TestDir::new();
    let (source, fragment) = saved_source(&interrupted);
    let marker = interrupted.0.join("migration-ready");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "capability_tests::crash_migration_child",
        ])
        .env("RANGOON_TEST_MIGRATION_DB", interrupted.store().path())
        .env("RANGOON_TEST_MIGRATION_READY", &marker)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !marker.exists() && Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            panic!("migration child exited before preparing transaction");
        }
        thread::sleep(Duration::from_millis(20));
    }
    let ready = marker.exists();
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(
        ready,
        "migration child did not reach uncommitted transaction"
    );
    assert_eq!(interrupted.store().open(&source.source.id).unwrap(), source);
    assert!(interrupted.store().list_capabilities().unwrap().is_empty());
    create(&interrupted, &source, &fragment, "Rules");
}

#[test]
#[ignore = "subprocess helper killed by failed_and_interrupted_migrations_leave_schema_v1_source_recoverable"]
fn crash_migration_child() {
    let path = std::env::var_os("RANGOON_TEST_MIGRATION_DB").unwrap();
    let marker = std::env::var_os("RANGOON_TEST_MIGRATION_READY").unwrap();
    let mut db = Connection::open(PathBuf::from(path)).unwrap();
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    for (_, sql) in capabilities::SCHEMAS {
        tx.execute_batch(sql).unwrap();
    }
    tx.pragma_update(None, "user_version", 2).unwrap();
    fs::write(marker, b"migration transaction uncommitted").unwrap();
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

fn spilled_successor(capability: &str, parent: &str) -> (String, String, String, String) {
    let title = "Rules after interruption".to_owned();
    let content = format!("Pending revision\n{}", "x".repeat(200 * 1024));
    let revision = revision_id(capability, Some(parent), &title, &content);
    let sha256 = byte_digest(content.as_bytes());
    (revision, title, content, sha256)
}

#[test]
#[ignore = "subprocess helper killed by interrupted_schema_v2_write_recovers_reviewed_head_and_source"]
fn crash_revision_writer_child() {
    // This exercises SQLite transaction recovery, not API fault injection.
    let path = std::env::var_os("RANGOON_TEST_REVISION_DB").unwrap();
    let marker = std::env::var_os("RANGOON_TEST_REVISION_READY").unwrap();
    let capability = std::env::var("RANGOON_TEST_REVISION_CAPABILITY").unwrap();
    let parent = std::env::var("RANGOON_TEST_REVISION_HEAD").unwrap();
    let (revision, title, content, sha256) = spilled_successor(&capability, &parent);
    let mut db = Connection::open(PathBuf::from(path)).unwrap();
    db.pragma_update(None, "cache_size", 1).unwrap();
    db.pragma_update(None, "cache_spill", true).unwrap();
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    tx.execute(
        "INSERT INTO revisions VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            revision,
            capability,
            parent,
            title,
            content.as_bytes(),
            sha256,
            1_i64,
        ],
    )
    .unwrap();
    tx.execute(
        "INSERT INTO reviews VALUES (?1,'local_operator',1)",
        [&revision],
    )
    .unwrap();
    assert_eq!(
        tx.execute(
            "UPDATE capabilities SET latest_revision_id=?1 WHERE capability_id=?2 AND latest_revision_id=?3",
            params![revision, capability, parent],
        )
        .unwrap(),
        1
    );
    fs::write(marker, b"revision transaction uncommitted").unwrap();
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
