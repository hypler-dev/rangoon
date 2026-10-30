//! Fixed disposable Legacy fixtures for the native qualification executable.
//! These helpers never create a Managed database or access a credential store.
use crate::native_encrypted_case_plan::Seed;
use rangoon_store::{WorkflowRecoveryCounts, Workspace};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::Path,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Inventory {
    sources: u32,
    capabilities: u32,
    revisions: u32,
    reviews: u32,
    recipes: u32,
    applications: u32,
    derivations: u32,
    workflows: u32,
    workflow_revisions: u32,
}
impl From<&WorkflowRecoveryCounts> for Inventory {
    fn from(c: &WorkflowRecoveryCounts) -> Self {
        Self {
            sources: c.sources,
            capabilities: c.capabilities,
            revisions: c.revisions,
            reviews: c.reviews,
            recipes: c.recipes,
            applications: c.applications,
            derivations: c.derivations,
            workflows: c.workflows,
            workflow_revisions: c.workflow_revisions,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Original {
    schema: u32,
    inventory: Inventory,
    archive_sha256: String,
    identity: String,
    database_sha256: String,
    database_bytes: u64,
}

/// Observe an existing original through the actual bounded read-only capture.
/// Physical hashing supplements logical parity; neither authenticates an author.
pub(crate) fn observe(anchor: &Path) -> Result<Original, ()> {
    let store = Workspace::new(anchor.join("source-workspace"));
    let mut capture = store.capture_existing_snapshot().map_err(|_| ())?;
    let path = anchor.join("source-workspace/workspace.sqlite3");
    let before = fs::symlink_metadata(&path).map_err(|_| ())?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.nlink() != 1
        || before.mode() & 0o777 != 0o600
        || before.len() > 64 * 1024 * 1024
    {
        return Err(());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)
        .map_err(|_| ())?;
    let same = |m: &fs::Metadata| {
        m.dev() == before.dev()
            && m.ino() == before.ino()
            && m.uid() == before.uid()
            && m.mode() == before.mode()
            && m.nlink() == before.nlink()
            && m.len() == before.len()
            && m.mtime() == before.mtime()
            && m.mtime_nsec() == before.mtime_nsec()
            && m.ctime() == before.ctime()
            && m.ctime_nsec() == before.ctime_nsec()
    };
    if !same(&file.metadata().map_err(|_| ())?) {
        return Err(());
    }
    let mut hash = Sha256::new();
    let mut bytes = 0u64;
    let mut buffer = [0u8; 8192];
    loop {
        let count = file.read(&mut buffer).map_err(|_| ())?;
        if count == 0 {
            break;
        }
        bytes = bytes.checked_add(count as u64).ok_or(())?;
        if bytes > 64 * 1024 * 1024 {
            return Err(());
        }
        hash.update(&buffer[..count]);
    }
    if bytes != before.len()
        || !same(&file.metadata().map_err(|_| ())?)
        || !same(&fs::symlink_metadata(&path).map_err(|_| ())?)
    {
        return Err(());
    }
    store
        .revalidate_existing_capture(&mut capture)
        .map_err(|_| ())?;
    Ok(Original {
        schema: capture.schema_version().map_err(|_| ())?,
        inventory: capture.inventory().map_err(|_| ())?.into(),
        archive_sha256: capture.digest().map_err(|_| ())?.to_owned(),
        identity: capture.identity_fingerprint().map_err(|_| ())?,
        database_sha256: hash
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        database_bytes: bytes,
    })
}

/// Called only for a plan's initial phase after the runner validates its empty
/// fixed root. Repeated or partial seeds are refused without repair or deletion.
pub(crate) fn seed(anchor: &Path, kind: Seed) -> Result<Original, ()> {
    let metadata = fs::symlink_metadata(anchor).map_err(|_| ())?;
    if kind == Seed::None
        || !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.mode() & 0o777 != 0o700
        || fs::read_dir(anchor).map_err(|_| ())?.next().is_some()
    {
        return Err(());
    }
    let source = anchor.join("source-workspace");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&source)
        .map_err(|_| ())?;
    match kind {
        Seed::LegacyEmpty => {
            // Independent schema-0 SQLite page, also used by adoption tests.
            let mut bytes = vec![0; 4096];
            bytes[..16].copy_from_slice(b"SQLite format 3\0");
            bytes[16..18].copy_from_slice(&4096u16.to_be_bytes());
            bytes[18] = 1;
            bytes[19] = 1;
            bytes[21] = 64;
            bytes[22] = 32;
            bytes[23] = 32;
            for (offset, value) in [
                (24, 1u32),
                (28, 1),
                (44, 4),
                (56, 1),
                (92, 1),
                (96, 3040000),
            ] {
                bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
            }
            bytes[100] = 13;
            bytes[105..107].copy_from_slice(&4096u16.to_be_bytes());
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(source.join("workspace.sqlite3"))
                .map_err(|_| ())?;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|_| ())?;
        }
        Seed::LegacyRich => populate(&Workspace::new(source.clone()))?,
        Seed::None => return Err(()),
    }
    for directory in [&source, anchor] {
        File::open(directory)
            .and_then(|file| file.sync_all())
            .map_err(|_| ())?;
    }
    observe(anchor)
}

fn populate(store: &Workspace) -> Result<(), ()> {
    let report = rangoon_import::analyze(
        "AGENTS.md",
        b"\xef\xbb\xbf# Rules\r\nKeep originals.\r\n# Tests\r\nCheck edits.\r\n",
    )
    .map_err(|_| ())?;
    store.save(&report).map_err(|_| ())?;
    let capability = store
        .create_capability(&report.source.id, &report.fragments[0].id, "Transfer skill")
        .map_err(|_| ())?
        .capability;
    store
        .review_capability(&capability.id, &capability.latest_revision_id)
        .map_err(|_| ())?;
    let revised = store
        .revise_capability(
            &capability.id,
            &capability.latest_revision_id,
            "Retained history",
            "Reviewed historical content.\r\n",
        )
        .map_err(|_| ())?
        .capability;
    store
        .review_capability(&revised.id, &revised.latest_revision_id)
        .map_err(|_| ())?;
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/composition/decompose-v0.json"
    ))
    .map_err(|_| ())?;
    let request = rangoon_compose::application::Request {
        schema_version: rangoon_compose::application::REQUEST_SCHEMA.into(),
        draft: serde_json::from_value(value["draft"].clone()).map_err(|_| ())?,
        targets: vec![
            rangoon_compose::application::Target::New {},
            rangoon_compose::application::Target::New {},
        ],
    };
    let preview = store.preview_composition(&request).map_err(|_| ())?;
    store.apply_composition(&preview, true).map_err(|_| ())?;
    use rangoon_workflow::records::{SaveIntent, prepare_revision, workflow_id_from_nonce};
    let id = workflow_id_from_nonce(&[97; 32]);
    let first = prepare_revision(&id, None, SaveIntent::Draft, br#"{"schemaVersion":"rangoon.workflow-definition.v1","title":"Transfer workflow","nodes":[],"controlEdges":[],"dataEdges":[]}"#, br#"{"positions":[]}"#).map_err(|_| ())?;
    let plan = store.inspect_workflow_save(&first).map_err(|_| ())?;
    store
        .save_workflow(&first, &plan.expected_state_id)
        .map_err(|_| ())?;
    let next = prepare_revision(&id, Some(first.id()), SaveIntent::Draft, br#"{"schemaVersion":"rangoon.workflow-definition.v1","title":"Retained workflow history","nodes":[],"controlEdges":[],"dataEdges":[]}"#, br#"{"positions":[]}"#).map_err(|_| ())?;
    let plan = store.inspect_workflow_save(&next).map_err(|_| ())?;
    store
        .save_workflow(&next, &plan.expected_state_id)
        .map_err(|_| ())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::symlink,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Root(PathBuf, fs::Metadata);
    impl Root {
        fn new() -> Self {
            let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "rangoon-e1c5c-seed-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
            let metadata = fs::symlink_metadata(&root).unwrap();
            Self(root, metadata)
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            if fs::symlink_metadata(&self.0).is_ok_and(|m| {
                m.is_dir()
                    && !m.file_type().is_symlink()
                    && m.dev() == self.1.dev()
                    && m.ino() == self.1.ino()
            }) {
                fs::remove_dir_all(&self.0).unwrap();
            }
        }
    }
    #[test]
    fn empty_original_is_real_existing_schema_zero_and_observation_preserves_bytes() {
        let root = Root::new();
        let original = seed(&root.0, Seed::LegacyEmpty).unwrap();
        assert_eq!(original.schema, 0);
        assert_eq!(
            original.inventory,
            (&WorkflowRecoveryCounts::default()).into()
        );
        assert_eq!(original.database_bytes, 4096);
        assert_eq!(observe(&root.0).unwrap(), original);
        assert_eq!(
            fs::read_dir(root.0.join("source-workspace"))
                .unwrap()
                .count(),
            1
        );
    }
    #[test]
    fn rich_original_retains_all_record_families_and_both_revision_histories() {
        let root = Root::new();
        let original = seed(&root.0, Seed::LegacyRich).unwrap();
        assert_eq!(original.schema, 4);
        assert_eq!(
            original.inventory,
            Inventory {
                sources: 1,
                capabilities: 3,
                revisions: 4,
                reviews: 2,
                recipes: 1,
                applications: 1,
                derivations: 2,
                workflows: 1,
                workflow_revisions: 2
            }
        );
        let bytes = fs::read(root.0.join("source-workspace/workspace.sqlite3")).unwrap();
        assert_eq!(observe(&root.0).unwrap(), original);
        assert_eq!(
            fs::read(root.0.join("source-workspace/workspace.sqlite3")).unwrap(),
            bytes
        );
        assert_eq!(original.archive_sha256.len(), 64);
        assert_eq!(original.identity.len(), 64);
        assert_eq!(original.database_sha256.len(), 64);
    }
    #[test]
    fn repeated_or_partial_seed_refuses_without_overwriting_original() {
        let root = Root::new();
        assert!(seed(&root.0, Seed::None).is_err());
        let original = seed(&root.0, Seed::LegacyRich).unwrap();
        assert!(seed(&root.0, Seed::LegacyEmpty).is_err());
        assert_eq!(observe(&root.0).unwrap(), original);
        fs::write(root.0.join("unknown"), b"preserve me").unwrap();
        assert!(seed(&root.0, Seed::LegacyRich).is_err());
        assert_eq!(fs::read(root.0.join("unknown")).unwrap(), b"preserve me");
    }
    #[test]
    fn absent_corrupt_or_linked_original_is_never_created_or_repaired() {
        let root = Root::new();
        assert!(observe(&root.0).is_err());
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
        let other = Root::new();
        let original = seed(&other.0, Seed::LegacyEmpty).unwrap();
        symlink(
            other.0.join("source-workspace"),
            root.0.join("source-workspace"),
        )
        .unwrap();
        assert!(observe(&root.0).is_err());
        assert_eq!(observe(&other.0).unwrap(), original);
        let bad = Root::new();
        fs::create_dir(bad.0.join("source-workspace")).unwrap();
        fs::write(
            bad.0.join("source-workspace/workspace.sqlite3"),
            b"invalid fixture",
        )
        .unwrap();
        assert!(observe(&bad.0).is_err());
        assert_eq!(
            fs::read(bad.0.join("source-workspace/workspace.sqlite3")).unwrap(),
            b"invalid fixture"
        );
    }
    #[test]
    fn legitimate_legacy_edit_changes_observation_instead_of_reseeding() {
        let root = Root::new();
        let old = seed(&root.0, Seed::LegacyRich).unwrap();
        let store = Workspace::new(root.0.join("source-workspace"));
        let report =
            rangoon_import::analyze("SKILL.md", b"# Later original\nKeep this change.\n").unwrap();
        store.save_v1(&report).unwrap();
        let new = observe(&root.0).unwrap();
        assert_eq!(new.inventory.sources, old.inventory.sources + 1);
        assert_ne!(new.database_sha256, old.database_sha256);
        assert_ne!(new.archive_sha256, old.archive_sha256);
        assert!(seed(&root.0, Seed::LegacyRich).is_err());
        assert_eq!(observe(&root.0).unwrap(), new);
    }
}
