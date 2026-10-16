#[cfg(feature = "encrypted-workspace")]
use super::*;

#[cfg(feature = "encrypted-workspace")]
use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(feature = "encrypted-workspace")]
fn tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(root: &Path, current: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries = fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            if path.is_dir() {
                out.push((relative, Vec::new()));
                visit(root, &path, out);
            } else {
                out.push((relative, fs::read(path).unwrap()));
            }
        }
    }

    let mut observed = Vec::new();
    visit(root, root, &mut observed);
    observed
}

#[cfg(feature = "encrypted-workspace")]
fn locked(root: &TestRoot, vault: &FakeVault) -> String {
    let (mut owner, preview) = create_ready(root, vault, &selected_source());
    let id = preview.workspace_id;
    owner.lock();
    drop(owner);
    id
}

#[cfg(feature = "encrypted-workspace")]
fn inspect(root: &TestRoot) -> (Owner, Layout) {
    let owner = Owner::acquire(root.0.clone()).unwrap();
    let layout = owner.inspect_layout();
    (owner, layout)
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn layout_enum_is_finite_source_free_and_absent_observation_is_pure() {
    assert_eq!(
        serde_json::to_string(&Layout::Absent).unwrap(),
        "\"absent\""
    );
    assert_eq!(
        serde_json::to_string(&Layout::Locked).unwrap(),
        "\"locked\""
    );
    assert_eq!(serde_json::to_string(&Layout::Busy).unwrap(), "\"busy\"");
    assert_eq!(
        serde_json::to_string(&Layout::RecoveryRequired).unwrap(),
        "\"recovery_required\""
    );
    assert_eq!(
        serde_json::to_string(&Layout::Unavailable).unwrap(),
        "\"unavailable\""
    );

    let root = TestRoot::new("layout-absent");
    let vault = FakeVault::default();
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let before = tree(&root.0);
    let opens = vault.opens();
    assert_eq!(owner.inspect_layout(), Layout::Absent);
    assert_eq!(owner.inspect_layout(), Layout::Absent);
    assert_eq!(tree(&root.0), before);
    assert_eq!(vault.opens(), opens);

    let preview = owner.prepare_create_empty().unwrap();
    assert_eq!(preview.kind, Action::CreateEmpty);
    owner.lock();
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn layout_locked_is_repeatable_without_vault_or_file_mutation_and_real_unlock_still_prepares() {
    let root = TestRoot::new("layout-locked");
    let vault = FakeVault::default();
    let id = locked(&root, &vault);
    let (mut owner, layout) = inspect(&root);
    let before = tree(&root.0);
    let opens = vault.opens();
    assert_eq!(layout, Layout::Locked);
    assert_eq!(owner.inspect_layout(), Layout::Locked);
    assert_eq!(tree(&root.0), before);
    assert_eq!(vault.opens(), opens);

    let preview = owner.prepare_unlock().unwrap();
    assert_eq!(preview.workspace_id, id);
    owner.lock();
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn layout_reports_busy_before_filesystem_for_open_and_pending_owner_custody() {
    let root = TestRoot::new("layout-busy");
    let vault = FakeVault::default();
    let (mut owner, _) = create_ready(&root, &vault, &selected_source());
    let before = tree(&root.0);
    assert_eq!(owner.inspect_layout(), Layout::Busy);
    assert_eq!(tree(&root.0), before);
    owner.lock();

    let preview = owner.prepare_unlock().unwrap();
    let before = tree(&root.0);
    assert_eq!(owner.inspect_layout(), Layout::Busy);
    assert_eq!(tree(&root.0), before);
    owner.lock();
    assert_error(owner.confirm(&preview.id, &vault), Error::Stale);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn layout_reports_recovery_for_actual_owner_recovery_state_without_further_access() {
    let root = TestRoot::new("layout-owner-recovery");
    let vault = FakeVault::default();
    let (mut owner, _) = create_ready(&root, &vault, &selected_source());
    owner.inject_fault(Fault::AfterSave);
    assert_error(
        owner.save(&vault, &source(b"# uncertain layout observation\n")),
        Error::WriteUncertain,
    );
    assert!(owner.requires_recovery());
    let before = tree(&root.0);
    let opens = vault.opens();
    assert_eq!(owner.inspect_layout(), Layout::RecoveryRequired);
    assert_eq!(tree(&root.0), before);
    assert_eq!(vault.opens(), opens);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn layout_rejects_binding_checkpoint_parent_and_database_artifacts_without_vault_access() {
    for case in [
        "provisioning-binding",
        "torn-terminal",
        "orphan-directory",
        "extra-root-file",
        "sidecar",
        "zero-database",
        "oversized-database",
    ] {
        let root = TestRoot::new(case);
        let vault = FakeVault::default();
        let id = locked(&root, &vault);
        match case {
            "provisioning-binding" => write_binding(&root, 0, &id),
            "torn-terminal" => fs::write(
                root.0.join("encrypted-workspace.operation.terminal"),
                b"torn",
            )
            .unwrap(),
            "orphan-directory" => {
                fs::create_dir(root.encrypted_root().join("f".repeat(64))).unwrap()
            }
            "extra-root-file" => fs::write(root.0.join("unexpected"), b"x").unwrap(),
            "sidecar" => fs::write(
                database_path(&root, &id).with_extension("sqlite3-wal"),
                b"x",
            )
            .unwrap(),
            "zero-database" => fs::write(database_path(&root, &id), Vec::new()).unwrap(),
            "oversized-database" => {
                let file = fs::OpenOptions::new()
                    .write(true)
                    .open(database_path(&root, &id))
                    .unwrap();
                file.set_len(64 * 1024 * 1024 + 1).unwrap();
            }
            _ => unreachable!(),
        }
        let before = tree(&root.0);
        let opens = vault.opens();
        let (owner, layout) = inspect(&root);
        assert_eq!(layout, Layout::RecoveryRequired, "{case}");
        assert_eq!(owner.inspect_layout(), Layout::RecoveryRequired, "{case}");
        assert_eq!(tree(&root.0), before, "{case}");
        assert_eq!(vault.opens(), opens, "{case}");
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn layout_bounded_malformed_metadata_never_becomes_absent_or_locked() {
    for bytes in [
        Vec::new(),
        vec![0; BINDING_BYTES],
        vec![b'x'; BINDING_BYTES + 1],
    ] {
        let root = TestRoot::new("layout-mutant-binding");
        let vault = FakeVault::default();
        fs::write(root.binding(), &bytes).unwrap();
        let (owner, layout) = inspect(&root);
        let before = tree(&root.0);
        assert_eq!(layout, Layout::RecoveryRequired);
        assert_eq!(owner.inspect_layout(), Layout::RecoveryRequired);
        assert_eq!(tree(&root.0), before);
        assert_eq!(vault.opens(), 0);
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn layout_rejects_pending_and_identity_mismatched_checkpoints() {
    let root = TestRoot::new("layout-pending-only");
    let vault = FakeVault::default();
    let _id = locked(&root, &vault);
    fs::remove_file(root.0.join("encrypted-workspace.operation.terminal")).unwrap();
    let opens = vault.opens();
    let (owner, layout) = inspect(&root);
    assert_eq!(layout, Layout::RecoveryRequired);
    assert_eq!(owner.inspect_layout(), Layout::RecoveryRequired);
    assert_eq!(vault.opens(), opens);

    let first = TestRoot::new("layout-checkpoint-first");
    let second = TestRoot::new("layout-checkpoint-second");
    let first_vault = FakeVault::default();
    let second_vault = FakeVault::default();
    locked(&first, &first_vault);
    locked(&second, &second_vault);
    fs::copy(
        second.0.join("encrypted-workspace.operation.terminal"),
        first.0.join("encrypted-workspace.operation.terminal"),
    )
    .unwrap();
    let opens = first_vault.opens();
    let (_, layout) = inspect(&first);
    assert_eq!(layout, Layout::RecoveryRequired);
    assert_eq!(first_vault.opens(), opens);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn layout_rejects_nonregular_database_and_mid_observation_changes() {
    let root = TestRoot::new("layout-database-directory");
    let vault = FakeVault::default();
    let id = locked(&root, &vault);
    let database = database_path(&root, &id);
    fs::remove_file(&database).unwrap();
    fs::create_dir(&database).unwrap();
    let opens = vault.opens();
    let (_, layout) = inspect(&root);
    assert_eq!(layout, Layout::RecoveryRequired);
    assert_eq!(vault.opens(), opens);

    let root = TestRoot::new("layout-mid-resize");
    let vault = FakeVault::default();
    let id = locked(&root, &vault);
    let database = database_path(&root, &id);
    let original = fs::metadata(&database).unwrap().len();
    let owner = Owner::acquire(root.0.clone()).unwrap();
    let opens = vault.opens();
    assert_eq!(
        owner.inspect_layout_after_observation_for_test(|| {
            fs::OpenOptions::new()
                .write(true)
                .open(&database)
                .unwrap()
                .set_len(original + 1)
                .unwrap();
        }),
        Layout::RecoveryRequired
    );
    assert_eq!(vault.opens(), opens);

    let root = TestRoot::new("layout-mid-root-entry");
    let vault = FakeVault::default();
    locked(&root, &vault);
    let unknown = root.0.join("appeared-during-observation");
    let owner = Owner::acquire(root.0.clone()).unwrap();
    let opens = vault.opens();
    assert_eq!(
        owner.inspect_layout_after_observation_for_test(|| fs::write(&unknown, b"x").unwrap()),
        Layout::RecoveryRequired
    );
    assert_eq!(vault.opens(), opens);
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn layout_rejects_replaced_identity_directory_and_unavailable_control() {
    let root = TestRoot::new("layout-mid-directory-replace");
    let vault = FakeVault::default();
    let id = locked(&root, &vault);
    let parent = root.encrypted_root();
    let directory = parent.join(&id);
    let replacement = parent.join("replacement");
    let previous = parent.join("previous");
    let owner = Owner::acquire(root.0.clone()).unwrap();
    assert_eq!(
        owner.inspect_layout_after_observation_for_test(|| {
            fs::create_dir(&replacement).unwrap();
            fs::copy(
                directory.join("workspace.sqlite3"),
                replacement.join("workspace.sqlite3"),
            )
            .unwrap();
            fs::rename(&directory, &previous).unwrap();
            fs::rename(&replacement, &directory).unwrap();
        }),
        Layout::RecoveryRequired
    );

    let root = TestRoot::new("layout-unavailable-control");
    let vault = FakeVault::default();
    let owner = Owner::acquire(root.0.clone()).unwrap();
    fs::remove_file(root.0.join("encrypted-workspace.lock")).unwrap();
    assert_eq!(owner.inspect_layout(), Layout::Unavailable);
    assert_eq!(vault.opens(), 0);
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn layout_rejects_linked_database_and_never_follows_it() {
    use std::os::unix::fs::symlink;

    let root = TestRoot::new("layout-linked-database");
    let vault = FakeVault::default();
    let id = locked(&root, &vault);
    let database = database_path(&root, &id);
    let target = root.0.join("database-target");
    fs::rename(&database, &target).unwrap();
    symlink(&target, &database).unwrap();
    let opens = vault.opens();
    let (_, layout) = inspect(&root);
    assert_eq!(layout, Layout::RecoveryRequired);
    assert_eq!(vault.opens(), opens);
}
