use super::*;
use rangoon_domain::{AnalysisReport, byte_digest, capability::capability_id};
use rangoon_import::analyze;
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const MAGIC: &[u8] = b"RANGOON-BACKUP-V1\n";
static NEXT: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rangoon-workspace-data-{}-{}-{}",
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

fn report(name: &str, bytes: &[u8]) -> AnalysisReport {
    analyze(name, bytes).unwrap()
}

fn save_source(dir: &TestDir, name: &str, bytes: &[u8]) -> AnalysisReport {
    let source = report(name, bytes);
    dir.store().save(&source).unwrap();
    source
}

fn first_fragment(source: &AnalysisReport) -> String {
    source.fragments.first().unwrap().id.clone()
}

fn archive(manifest: &[u8], content: &[u8]) -> Vec<u8> {
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&(manifest.len() as u32).to_be_bytes());
    bytes.extend_from_slice(manifest);
    bytes.extend_from_slice(content);
    bytes.extend_from_slice(byte_digest(&bytes).as_bytes());
    bytes
}

fn rewrite_manifest(bytes: &[u8], change: impl FnOnce(&mut Value)) -> Vec<u8> {
    assert!(bytes.starts_with(MAGIC));
    let length_offset = MAGIC.len();
    let old_length =
        u32::from_be_bytes(bytes[length_offset..length_offset + 4].try_into().unwrap()) as usize;
    let manifest_start = length_offset + 4;
    let manifest_end = manifest_start + old_length;
    let body_end = bytes.len() - 64;
    let mut manifest: Value = serde_json::from_slice(&bytes[manifest_start..manifest_end]).unwrap();
    change(&mut manifest);
    let manifest = serde_json::to_vec(&manifest).unwrap();
    archive(&manifest, &bytes[manifest_end..body_end])
}

fn manifest_string(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}

struct ArchiveRecord {
    source: AnalysisReport,
    capability_id: String,
    revision_id: String,
    title: String,
    content: String,
}

fn constructed_backup_bytes(prefix: &str, count: usize, body: &str) -> Vec<u8> {
    let mut records = (0..count)
        .map(|index| {
            let source = report(&format!("{prefix}-{index:03}.md"), body.as_bytes());
            let fragment = source.fragments.first().unwrap();
            assert_eq!(fragment.text, source.source.content);
            let fragment_id = fragment.id.clone();
            let content = fragment.text.clone();
            let title = format!("Rules {prefix} {index:03}");
            let capability_id = capability_id(&source.source.id, &fragment_id, &title);
            let revision_id =
                rangoon_domain::capability::revision_id(&capability_id, None, &title, &content);
            ArchiveRecord {
                source,
                capability_id,
                revision_id,
                title,
                content,
            }
        })
        .collect::<Vec<_>>();
    records.sort_by(|left, right| left.source.source.id.cmp(&right.source.source.id));

    let sources = records
        .iter()
        .map(|record| {
            let source = &record.source.source;
            serde_json::json!({
                "metadata": {
                    "sourceId": source.id,
                    "displayName": source.display_name,
                    "sha256": source.sha256,
                    "byteLength": source.byte_length,
                    "savedAtMs": 1,
                }
            })
        })
        .collect::<Vec<_>>();
    let mut capabilities = records
        .iter()
        .map(|record| {
            let fragment = record.source.fragments.first().unwrap();
            serde_json::json!({
                "id": record.capability_id,
                "sourceId": record.source.source.id,
                "fragmentId": fragment.id,
                "latestRevisionId": record.revision_id,
            })
        })
        .collect::<Vec<_>>();
    capabilities.sort_by_key(|capability| capability["id"].as_str().unwrap().to_owned());
    let mut revisions = records
        .iter()
        .map(|record| {
            serde_json::json!({
                "id": record.revision_id,
                "capabilityId": record.capability_id,
                "parentRevisionId": Value::Null,
                "title": record.title,
                "sha256": byte_digest(record.content.as_bytes()),
                "createdAtMs": 1,
                "byteLength": record.content.len(),
            })
        })
        .collect::<Vec<_>>();
    revisions.sort_by_key(|revision| revision["id"].as_str().unwrap().to_owned());
    let manifest = serde_json::json!({
        "version": 1,
        "sources": sources,
        "capabilities": capabilities,
        "revisions": revisions,
        "reviews": [],
    });
    let mut content = records
        .iter()
        .map(|record| record.source.source.content.as_bytes())
        .collect::<Vec<_>>();
    let mut revisions = records.iter().collect::<Vec<_>>();
    revisions.sort_by(|left, right| left.revision_id.cmp(&right.revision_id));
    content.extend(
        revisions
            .into_iter()
            .map(|record| record.content.as_bytes()),
    );
    let content = content.concat();
    archive(&serde_json::to_vec(&manifest).unwrap(), &content)
}

fn constructed_backup(prefix: &str, count: usize, body: &str) -> Backup {
    Backup::decode(&constructed_backup_bytes(prefix, count, body)).unwrap()
}

#[test]
fn backup_round_trip_preserves_exact_bytes_history_reviews_and_stale_receipts() {
    let source_dir = TestDir::new();
    let source = save_source(
        &source_dir,
        "AGENTS.md",
        b"\xef\xbb\xbf# Rules\r\nKeep exact source bytes.\r\n",
    );
    let fragment = first_fragment(&source);
    let created = source_dir
        .store()
        .create_capability(&source.source.id, &fragment, "Rules")
        .unwrap()
        .capability;
    source_dir
        .store()
        .review_capability(&created.id, &created.latest_revision_id)
        .unwrap();
    let edited = source_dir
        .store()
        .revise_capability(
            &created.id,
            &created.latest_revision_id,
            "Rules v2",
            "Local revision.\r\n",
        )
        .unwrap()
        .capability;
    source_dir
        .store()
        .review_capability(&edited.id, &edited.latest_revision_id)
        .unwrap();

    let backup_bytes = source_dir.store().export_backup().unwrap();
    assert_eq!(source_dir.store().export_backup().unwrap(), backup_bytes);
    let backup = Backup::decode(&backup_bytes).unwrap();

    let target = TestDir::new();
    let before = target.store().data().unwrap();
    let preview = target.store().prepare_restore(&backup).unwrap();
    assert_eq!(preview.expected_state_id, before.state_id);
    assert_eq!(preview.add_sources, 1);
    assert_eq!(preview.add_capabilities, 1);
    assert_eq!(preview.add_revisions, 2);
    assert_eq!(preview.add_reviews, 2);
    assert_eq!(
        target
            .store()
            .restore_backup(&backup, &before.state_id)
            .unwrap(),
        preview
    );
    assert_eq!(
        target.store().restore_backup(&backup, &before.state_id),
        Err(StoreError::WorkspaceChanged)
    );
    assert_eq!(target.store().open(&source.source.id).unwrap(), source);
    assert_eq!(
        target.store().open_capability(&edited.id, None).unwrap(),
        source_dir
            .store()
            .open_capability(&edited.id, None)
            .unwrap()
    );
    let fresh = target.store().prepare_restore(&backup).unwrap();
    assert_eq!(
        (
            fresh.add_sources,
            fresh.add_capabilities,
            fresh.add_revisions,
            fresh.add_reviews
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(fresh.kept_capabilities, 1);
}

#[test]
fn backup_golden_bytes_are_independently_constructed_for_empty_and_source_only_workspaces() {
    let empty_manifest =
        br#"{"version":1,"sources":[],"capabilities":[],"revisions":[],"reviews":[]}"#;
    let empty_golden = archive(empty_manifest, b"");
    let empty = TestDir::new();
    assert_eq!(empty.store().export_backup().unwrap(), empty_golden);
    assert_eq!(
        Backup::decode(&empty_golden).unwrap().byte_length(),
        empty_golden.len()
    );

    let populated = TestDir::new();
    let source = save_source(&populated, "AGENTS.md", b"# Rules\r\nExact bytes.\r\n");
    let metadata = populated.store().data().unwrap().sources.remove(0);
    let manifest = format!(
        "{{\"version\":1,\"sources\":[{{\"metadata\":{{\"sourceId\":{},\"displayName\":{},\"sha256\":{},\"byteLength\":{},\"savedAtMs\":{}}}}}],\"capabilities\":[],\"revisions\":[],\"reviews\":[]}}",
        manifest_string(&metadata.source_id),
        manifest_string(&metadata.display_name),
        manifest_string(&metadata.sha256),
        metadata.byte_length,
        metadata.saved_at_ms,
    );
    let golden = archive(manifest.as_bytes(), source.source.content.as_bytes());
    assert_eq!(populated.store().export_backup().unwrap(), golden);
    assert!(Backup::decode(&golden).is_ok());
}

#[test]
fn restore_keeps_existing_newer_capability_and_quota_failure_rolls_back() {
    let archived = TestDir::new();
    let source = save_source(&archived, "AGENTS.md", b"# Rules\nArchive content.\n");
    let fragment = first_fragment(&source);
    let capability = archived
        .store()
        .create_capability(&source.source.id, &fragment, "Rules")
        .unwrap()
        .capability;
    let backup = Backup::decode(&archived.store().export_backup().unwrap()).unwrap();

    let target = TestDir::new();
    let initial = target.store().data().unwrap().state_id;
    target.store().restore_backup(&backup, &initial).unwrap();
    let newer = target
        .store()
        .revise_capability(
            &capability.id,
            &capability.latest_revision_id,
            "Newer rules",
            "Current target edit.\n",
        )
        .unwrap()
        .capability;
    let preview = target.store().prepare_restore(&backup).unwrap();
    assert_eq!((preview.add_capabilities, preview.add_revisions), (0, 0));
    assert_eq!(preview.kept_capabilities, 1);
    target
        .store()
        .restore_backup(&backup, &preview.expected_state_id)
        .unwrap();
    assert_eq!(
        target
            .store()
            .open_capability(&capability.id, None)
            .unwrap(),
        newer
    );

    let full = TestDir::new();
    for index in 0..MAX_SNAPSHOTS {
        save_source(
            &full,
            &format!("source-{index}.md"),
            format!("# Source {index}\n").as_bytes(),
        );
    }
    let full_backup = Backup::decode(&full.store().export_backup().unwrap()).unwrap();
    let limited = TestDir::new();
    let retained = save_source(&limited, "retained.md", b"# Retained\n");
    assert_eq!(
        limited.store().prepare_restore(&full_backup),
        Err(StoreError::Full)
    );
    assert_eq!(limited.store().open(&retained.source.id).unwrap(), retained);
    assert_eq!(limited.store().data().unwrap().usage.sources, 1);
}

#[test]
fn deletion_blocks_dependencies_checks_stale_before_missing_and_preserves_source() {
    let dir = TestDir::new();
    let source = save_source(&dir, "AGENTS.md", b"# Rules\nDelete only owned history.\n");
    let fragment = first_fragment(&source);
    let capability = dir
        .store()
        .create_capability(&source.source.id, &fragment, "Rules")
        .unwrap()
        .capability;
    dir.store()
        .review_capability(&capability.id, &capability.latest_revision_id)
        .unwrap();
    let stale = dir.store().data().unwrap().state_id;
    let current = dir
        .store()
        .revise_capability(
            &capability.id,
            &capability.latest_revision_id,
            "Rules v2",
            "Second revision.\n",
        )
        .unwrap()
        .capability;
    let missing = capability_id(&source.source.id, &fragment, "Missing");
    assert_eq!(
        dir.store()
            .delete_record(RecordKind::Capability, &missing, &stale),
        Err(StoreError::WorkspaceChanged)
    );
    let source_preview = dir
        .store()
        .inspect_deletion(RecordKind::Source, &source.source.id)
        .unwrap();
    assert_eq!(source_preview.dependencies.len(), 1);
    assert_eq!(
        dir.store().delete_record(
            RecordKind::Source,
            &source.source.id,
            &source_preview.expected_state_id,
        ),
        Err(StoreError::SourceInUse)
    );
    let capability_preview = dir
        .store()
        .inspect_deletion(RecordKind::Capability, &current.id)
        .unwrap();
    assert_eq!(
        (capability_preview.revisions, capability_preview.reviews),
        (2, 1)
    );
    dir.store()
        .delete_record(
            RecordKind::Capability,
            &current.id,
            &capability_preview.expected_state_id,
        )
        .unwrap();
    assert_eq!(dir.store().open(&source.source.id).unwrap(), source);
    assert_eq!(
        dir.store().open_capability(&current.id, None),
        Err(StoreError::CapabilityNotFound)
    );
    let source_preview = dir
        .store()
        .inspect_deletion(RecordKind::Source, &source.source.id)
        .unwrap();
    dir.store()
        .delete_record(
            RecordKind::Source,
            &source.source.id,
            &source_preview.expected_state_id,
        )
        .unwrap();
    assert_eq!(dir.store().data().unwrap().usage.sources, 0);
}

#[test]
fn empty_recovery_file_previews_without_writes_then_retries_canonical_restore() {
    let archived = TestDir::new();
    save_source(&archived, "AGENTS.md", b"# Rules\nRecover this source.\n");
    let backup = Backup::decode(&archived.store().export_backup().unwrap()).unwrap();
    let target = TestDir::new();
    fs::create_dir_all(&target.0).unwrap();
    fs::write(target.store().path(), b"").unwrap();
    let before_preview = fs::read(target.store().path()).unwrap();
    let preview = target.store().prepare_restore(&backup).unwrap();
    assert_eq!(fs::read(target.store().path()).unwrap(), before_preview);

    let stale = format!("workspace:{}", "0".repeat(64));
    assert_eq!(
        target.store().restore_backup(&backup, &stale),
        Err(StoreError::WorkspaceChanged)
    );
    let retry = target.store().prepare_restore(&backup).unwrap();
    assert_eq!(retry.expected_state_id, preview.expected_state_id);
    target
        .store()
        .restore_backup(&backup, &retry.expected_state_id)
        .unwrap();
    let db = Connection::open(target.store().path()).unwrap();
    let page_size: u32 = db
        .pragma_query_value(None, "page_size", |row| row.get(0))
        .unwrap();
    assert_eq!(page_size, 4096);
}

#[test]
fn noncanonical_empty_sqlite_file_is_never_vacuumed_or_replaced() {
    let archived = TestDir::new();
    save_source(&archived, "AGENTS.md", b"# Rules\nNever replace data.\n");
    let backup = Backup::decode(&archived.store().export_backup().unwrap()).unwrap();
    let target = TestDir::new();
    fs::create_dir_all(&target.0).unwrap();
    let db = Connection::open(target.store().path()).unwrap();
    db.pragma_update(None, "page_size", 512).unwrap();
    db.execute_batch("VACUUM").unwrap();
    drop(db);
    let before = fs::read(target.store().path()).unwrap();
    assert_eq!(
        target.store().prepare_restore(&backup),
        Err(StoreError::UnsupportedSchema)
    );
    assert_eq!(
        target
            .store()
            .restore_backup(&backup, &format!("workspace:{}", "0".repeat(64)),),
        Err(StoreError::UnsupportedSchema)
    );
    assert_eq!(fs::read(target.store().path()).unwrap(), before);
}

#[test]
fn decoder_rejects_corruption_unknown_fields_duplicates_and_forged_records() {
    let dir = TestDir::new();
    let source = save_source(&dir, "AGENTS.md", b"# Rules\nPortable record.\n");
    let fragment = first_fragment(&source);
    let created = dir
        .store()
        .create_capability(&source.source.id, &fragment, "Rules")
        .unwrap()
        .capability;
    dir.store()
        .review_capability(&created.id, &created.latest_revision_id)
        .unwrap();
    let revised = dir
        .store()
        .revise_capability(
            &created.id,
            &created.latest_revision_id,
            "Rules v2",
            "Changed revision.\n",
        )
        .unwrap()
        .capability;
    dir.store()
        .review_capability(&revised.id, &revised.latest_revision_id)
        .unwrap();
    let bytes = dir.store().export_backup().unwrap();

    let mut bad_checksum = bytes.clone();
    bad_checksum[0] ^= 1;
    let truncated = bytes[..bytes.len() - 1].to_vec();
    let mut trailing = bytes.clone();
    trailing.push(b'x');
    let wrong_length = {
        let mut value = bytes.clone();
        let offset = MAGIC.len();
        value[offset..offset + 4].copy_from_slice(&(u32::MAX).to_be_bytes());
        value
    };
    let variants = vec![
        bad_checksum,
        truncated,
        trailing,
        wrong_length,
        rewrite_manifest(&bytes, |manifest| {
            manifest["surprise"] = Value::Bool(true);
        }),
        rewrite_manifest(&bytes, |manifest| {
            let duplicate = manifest["sources"][0].clone();
            manifest["sources"].as_array_mut().unwrap().push(duplicate);
        }),
        rewrite_manifest(&bytes, |manifest| {
            manifest["sources"][0]["metadata"]["sourceId"] =
                Value::String(format!("source:{}", "0".repeat(64)));
        }),
        rewrite_manifest(&bytes, |manifest| {
            manifest["revisions"][1]["parentRevisionId"] = manifest["revisions"][1]["id"].clone();
        }),
        rewrite_manifest(&bytes, |manifest| {
            manifest["revisions"][0]
                .as_object_mut()
                .unwrap()
                .remove("parentRevisionId");
        }),
        rewrite_manifest(&bytes, |manifest| {
            manifest["reviews"][0]["reviewer"] = Value::String("other_operator".into());
        }),
    ];
    for variant in variants {
        assert!(matches!(
            Backup::decode(&variant),
            Err(StoreError::BackupInvalid)
        ));
    }
}

#[test]
fn canonical_64_mib_content_archive_is_rejected_for_sqlite_page_overhead() {
    let body = format!("{}\n", "x".repeat(16_383)).repeat(16);
    assert_eq!(body.len(), 256 * 1024);
    let bytes = constructed_backup_bytes("allocation", 128, &body);
    assert!(bytes.len() > 64 * 1024 * 1024);
    assert!(bytes.len() <= MAX_BACKUP_BYTES);
    assert!(matches!(
        Backup::decode(&bytes),
        Err(StoreError::BackupInvalid)
    ));
}

#[test]
fn large_valid_backup_restores_into_fresh_canonical_workspace() {
    let body = format!("{}\n", "x".repeat(16_383)).repeat(16);
    assert_eq!(body.len(), 256 * 1024);
    let backup = constructed_backup("large", 120, &body);
    assert!(backup.byte_length() > 60 * 1024 * 1024);
    let fresh = TestDir::new();
    let state = fresh.store().data().unwrap().state_id;
    fresh.store().restore_backup(&backup, &state).unwrap();
    let usage = fresh.store().data().unwrap().usage;
    assert_eq!(
        (usage.sources, usage.capabilities, usage.revisions),
        (120, 120, 120)
    );
}

#[test]
fn additive_restore_that_exceeds_sqlite_allocation_preserves_existing_workspace() {
    let body = format!("{}\n", "x".repeat(16_383)).repeat(16);
    let incoming = constructed_backup("incoming", 120, &body);
    let existing = constructed_backup("existing", 8, &body);
    let target = TestDir::new();
    let empty_state = target.store().data().unwrap().state_id;
    target
        .store()
        .restore_backup(&existing, &empty_state)
        .unwrap();
    let before_data = target.store().data().unwrap();
    let before_export = target.store().export_backup().unwrap();
    let preview = target.store().prepare_restore(&incoming).unwrap();
    assert_eq!((preview.add_sources, preview.add_capabilities), (120, 120));
    assert!(matches!(
        target
            .store()
            .restore_backup(&incoming, &preview.expected_state_id),
        Err(StoreError::Unavailable) | Err(StoreError::Full)
    ));
    assert_eq!(
        target.store().data().unwrap().state_id,
        before_data.state_id
    );
    assert_eq!(target.store().export_backup().unwrap(), before_export);
}
