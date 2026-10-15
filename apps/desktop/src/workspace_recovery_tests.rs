#[cfg(feature = "encrypted-workspace")]
use super::*;

#[cfg(feature = "encrypted-workspace")]
use std::fs;

#[cfg(feature = "encrypted-workspace")]
fn recovery_pending(root: &TestRoot) -> std::path::PathBuf {
    root.0.join("encrypted-workspace.operation.pending")
}

#[cfg(feature = "encrypted-workspace")]
fn recovery_terminal(root: &TestRoot) -> std::path::PathBuf {
    root.0.join("encrypted-workspace.operation.terminal")
}

#[cfg(feature = "encrypted-workspace")]
fn recovery_fixture_workspace(
    root: &TestRoot,
    vault: &FakeVault,
    workspace_id: &str,
) -> rangoon_store::Workspace {
    let identity = workspace_keys::WorkspaceId::parse(workspace_id).unwrap();
    let slot = vault.open(&identity).unwrap();
    let retained = workspace_keys::read_key(&slot).unwrap();
    workspace_keys::verified_workspace(&slot, &retained, root.encrypted_root().join(workspace_id))
        .unwrap()
}

#[cfg(feature = "encrypted-workspace")]
fn start_uncertain_save(
    root: &TestRoot,
    vault: &FakeVault,
) -> (String, rangoon_domain::AnalysisReport) {
    let original = selected_source();
    let (mut owner, preview) = create_ready(root, vault, &original);
    let changed = source(b"\xef\xbb\xbf# Recovery source\r\nExact persisted bytes.\r\n");
    owner.inject_fault(Fault::AfterSave);
    assert_error(owner.save(vault, &changed), Error::WriteUncertain);
    assert!(!owner.is_unlocked());
    drop(owner);
    (preview.workspace_id, changed)
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn clean_source_and_empty_restarts_accept_terminal_evidence() {
    let source_root = TestRoot::new("recovery-clean-source");
    let source_vault = FakeVault::default();
    let report = selected_source();
    let (mut source_owner, preview) = create_ready(&source_root, &source_vault, &report);
    source_owner.lock();
    drop(source_owner);
    let mut source_restart = Owner::acquire(source_root.0.clone()).unwrap();
    let unlock = source_restart.prepare_unlock().unwrap();
    source_restart.confirm(&unlock.id, &source_vault).unwrap();
    assert_eq!(
        source_restart
            .open(&source_vault, &report.source.id)
            .unwrap(),
        report
    );
    assert_eq!(unlock.workspace_id, preview.workspace_id);

    let empty_root = TestRoot::new("recovery-clean-empty");
    let empty_vault = FakeVault::default();
    let mut empty_owner = Owner::acquire(empty_root.0.clone()).unwrap();
    let empty = empty_owner.prepare_create_empty().unwrap();
    empty_owner.confirm(&empty.id, &empty_vault).unwrap();
    empty_owner.lock();
    drop(empty_owner);
    let mut empty_restart = Owner::acquire(empty_root.0.clone()).unwrap();
    let unlock = empty_restart.prepare_unlock().unwrap();
    empty_restart.confirm(&unlock.id, &empty_vault).unwrap();
    assert!(empty_restart.list(&empty_vault).unwrap().is_empty());
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn pending_save_requires_explicit_recovery_then_preserves_exact_source() {
    let root = TestRoot::new("recovery-pending-save");
    let vault = FakeVault::default();
    let (workspace_id, changed) = start_uncertain_save(&root, &vault);
    let database_before = fs::read(database_path(&root, &workspace_id)).unwrap();
    let key_writes = vault.writes(&workspace_id);
    let opens = vault.opens();

    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    assert_error(owner.prepare_unlock(), Error::RecoveryRequired);
    assert_eq!(vault.opens(), opens);

    let inspected = owner.inspect_recovery(&vault).unwrap();
    assert_eq!(inspected.workspace_id, workspace_id);
    assert_ne!(inspected.id, inspected.workspace_id);
    assert_ne!(inspected.id, inspected.attempt_id);
    assert_eq!(inspected.operation, RecoveryOperation::SaveSource);
    assert_eq!(inspected.outcome, "unknown");
    assert!(!inspected.metadata_damage);
    assert!(!inspected.inventory.state_id.is_empty());
    assert!(!owner.is_unlocked());
    assert_eq!(
        fs::read(database_path(&root, &workspace_id)).unwrap(),
        database_before
    );
    assert_eq!(vault.writes(&workspace_id), key_writes);

    let mut display_mutated = inspected.id.clone();
    let replacement = if inspected.id.starts_with('0') {
        "1"
    } else {
        "0"
    };
    display_mutated.replace_range(0..1, replacement);
    assert_error(owner.confirm_recovery("forged", &vault), Error::Stale);
    assert_error(
        owner.confirm_recovery(&display_mutated, &vault),
        Error::Stale,
    );
    owner.confirm_recovery(&inspected.id, &vault).unwrap();
    assert!(!owner.is_unlocked());
    assert_error(owner.confirm_recovery(&inspected.id, &vault), Error::Stale);

    let unlock = owner.prepare_unlock().unwrap();
    owner.confirm(&unlock.id, &vault).unwrap();
    assert_eq!(owner.open(&vault, &changed.source.id).unwrap(), changed);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn recovery_refuses_database_or_key_drift_without_metadata_or_key_writes() {
    let root = TestRoot::new("recovery-drift");
    let vault = FakeVault::default();
    let (workspace_id, _) = start_uncertain_save(&root, &vault);
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let inspected = owner.inspect_recovery(&vault).unwrap();
    let pending_before = fs::read(recovery_pending(&root)).unwrap();
    let terminal_before = fs::read(recovery_terminal(&root)).unwrap();
    let writes = vault.writes(&workspace_id);

    let changed = source(b"# External change\nAuthenticated state differs.\n");
    recovery_fixture_workspace(&root, &vault, &workspace_id)
        .save_v1(&changed)
        .unwrap();
    assert_error(owner.confirm_recovery(&inspected.id, &vault), Error::Stale);
    assert_eq!(fs::read(recovery_pending(&root)).unwrap(), pending_before);
    assert_eq!(fs::read(recovery_terminal(&root)).unwrap(), terminal_before);
    assert_eq!(vault.writes(&workspace_id), writes);

    let root = TestRoot::new("recovery-key-drift");
    let vault = FakeVault::default();
    let (workspace_id, _) = start_uncertain_save(&root, &vault);
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let inspected = owner.inspect_recovery(&vault).unwrap();
    let pending_before = fs::read(recovery_pending(&root)).unwrap();
    let terminal_before = fs::read(recovery_terminal(&root)).unwrap();
    let writes = vault.writes(&workspace_id);
    vault.substitute_identity(workspace_keys::WorkspaceId::parse(&"52".repeat(32)).unwrap());
    assert_error(
        owner.confirm_recovery(&inspected.id, &vault),
        Error::Key(workspace_keys::Error::Changed),
    );
    assert_eq!(fs::read(recovery_pending(&root)).unwrap(), pending_before);
    assert_eq!(fs::read(recovery_terminal(&root)).unwrap(), terminal_before);
    assert_eq!(vault.writes(&workspace_id), writes);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn damaged_slots_require_explicit_recovery_and_only_repair_matching_inventory() {
    let root = TestRoot::new("recovery-truncated-terminal");
    let vault = FakeVault::default();
    let (workspace_id, changed) = start_uncertain_save(&root, &vault);
    fs::write(recovery_terminal(&root), b"").unwrap();
    let opens = vault.opens();
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    assert_error(owner.prepare_unlock(), Error::RecoveryRequired);
    assert_eq!(vault.opens(), opens);
    let inspected = owner.inspect_recovery(&vault).unwrap();
    assert!(inspected.metadata_damage);
    owner.confirm_recovery(&inspected.id, &vault).unwrap();
    let unlock = owner.prepare_unlock().unwrap();
    owner.confirm(&unlock.id, &vault).unwrap();
    assert_eq!(owner.open(&vault, &changed.source.id).unwrap(), changed);
    assert_eq!(unlock.workspace_id, workspace_id);

    let root = TestRoot::new("recovery-corrupt-pending");
    let vault = FakeVault::default();
    let report = selected_source();
    let (mut owner, preview) = create_ready(&root, &vault, &report);
    owner.lock();
    drop(owner);
    let mut pending = fs::read(recovery_pending(&root)).unwrap();
    assert!(!pending.is_empty());
    pending[0] ^= 0x01;
    fs::write(recovery_pending(&root), pending).unwrap();
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    assert_error(owner.prepare_unlock(), Error::RecoveryRequired);
    let inspected = owner.inspect_recovery(&vault).unwrap();
    assert!(inspected.metadata_damage);
    owner.confirm_recovery(&inspected.id, &vault).unwrap();
    let unlock = owner.prepare_unlock().unwrap();
    owner.confirm(&unlock.id, &vault).unwrap();
    assert_eq!(owner.open(&vault, &report.source.id).unwrap(), report);
    assert_eq!(unlock.workspace_id, preview.workspace_id);

    let root = TestRoot::new("recovery-corrupt-pending-preinspect-drift");
    let vault = FakeVault::default();
    let report = selected_source();
    let (mut owner, preview) = create_ready(&root, &vault, &report);
    owner.lock();
    drop(owner);
    let mut pending = fs::read(recovery_pending(&root)).unwrap();
    pending[0] ^= 0x01;
    fs::write(recovery_pending(&root), pending).unwrap();
    recovery_fixture_workspace(&root, &vault, &preview.workspace_id)
        .save_v1(&source(b"# Changed before inspect\n"))
        .unwrap();
    let pending_before = fs::read(recovery_pending(&root)).unwrap();
    let terminal_before = fs::read(recovery_terminal(&root)).unwrap();
    let writes = vault.writes(&preview.workspace_id);
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    assert_error(owner.inspect_recovery(&vault), Error::RecoveryRequired);
    assert_eq!(fs::read(recovery_pending(&root)).unwrap(), pending_before);
    assert_eq!(fs::read(recovery_terminal(&root)).unwrap(), terminal_before);
    assert_eq!(vault.writes(&preview.workspace_id), writes);

    let root = TestRoot::new("recovery-corrupt-pending-drift");
    let vault = FakeVault::default();
    let report = selected_source();
    let (mut owner, preview) = create_ready(&root, &vault, &report);
    owner.lock();
    drop(owner);
    let mut pending = fs::read(recovery_pending(&root)).unwrap();
    pending[0] ^= 0x01;
    fs::write(recovery_pending(&root), pending).unwrap();
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let inspected = owner.inspect_recovery(&vault).unwrap();
    let pending_before = fs::read(recovery_pending(&root)).unwrap();
    let terminal_before = fs::read(recovery_terminal(&root)).unwrap();
    recovery_fixture_workspace(&root, &vault, &preview.workspace_id)
        .save_v1(&source(b"# Changed after terminal\n"))
        .unwrap();
    assert_error(owner.confirm_recovery(&inspected.id, &vault), Error::Stale);
    assert_eq!(fs::read(recovery_pending(&root)).unwrap(), pending_before);
    assert_eq!(fs::read(recovery_terminal(&root)).unwrap(), terminal_before);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn bounded_invalid_pending_records_never_open_vault_for_ordinary_unlock() {
    let prefix = b"rangoon.workspace-operation.v1\0";
    let phase = prefix.len();
    let operation = phase + 1;
    let sequence = operation + 1;
    let workspace = sequence + 8;

    for label in ["phase", "operation", "sequence", "workspace", "padding"] {
        let root = TestRoot::new(label);
        let vault = FakeVault::default();
        let report = selected_source();
        let (mut owner, _) = create_ready(&root, &vault, &report);
        owner.lock();
        drop(owner);
        let mut record = fs::read(recovery_pending(&root)).unwrap();
        match label {
            "phase" => record[phase] = 0xff,
            "operation" => record[operation] = 0xff,
            "sequence" => record[sequence..sequence + 8].fill(0),
            "workspace" => record[workspace] = b'g',
            "padding" => *record.last_mut().unwrap() = 1,
            _ => unreachable!(),
        }
        fs::write(recovery_pending(&root), record).unwrap();
        let mut owner = Owner::acquire(root.0.clone()).unwrap();
        let opens = vault.opens();
        assert_error(owner.prepare_unlock(), Error::RecoveryRequired);
        assert_eq!(vault.opens(), opens);
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn malformed_or_mismatched_operation_slots_block_unlock_before_vault_open() {
    let malformed_root = TestRoot::new("recovery-malformed");
    let malformed_vault = FakeVault::default();
    let report = selected_source();
    let (mut owner, _) = create_ready(&malformed_root, &malformed_vault, &report);
    owner.lock();
    drop(owner);
    fs::write(recovery_pending(&malformed_root), b"bad record").unwrap();
    let mut owner = Owner::acquire(malformed_root.0.clone()).unwrap();
    let opens = malformed_vault.opens();
    assert_error(owner.prepare_unlock(), Error::RecoveryRequired);
    assert_eq!(malformed_vault.opens(), opens);

    let oversized_root = TestRoot::new("recovery-oversized");
    let oversized_vault = FakeVault::default();
    let (mut owner, _) = create_ready(&oversized_root, &oversized_vault, &report);
    owner.lock();
    drop(owner);
    fs::write(recovery_terminal(&oversized_root), vec![0x41; 1024]).unwrap();
    let mut owner = Owner::acquire(oversized_root.0.clone()).unwrap();
    let opens = oversized_vault.opens();
    assert_error(owner.prepare_unlock(), Error::RecoveryRequired);
    assert_eq!(oversized_vault.opens(), opens);

    let first_root = TestRoot::new("recovery-mismatch-first");
    let second_root = TestRoot::new("recovery-mismatch-second");
    let vault = FakeVault::default();
    let (mut first, _) = create_ready(&first_root, &vault, &report);
    let (mut second, _) = create_ready(&second_root, &vault, &report);
    first.lock();
    second.lock();
    drop(first);
    drop(second);
    fs::copy(
        recovery_pending(&second_root),
        recovery_pending(&first_root),
    )
    .unwrap();
    let mut first = Owner::acquire(first_root.0.clone()).unwrap();
    let opens = vault.opens();
    assert_error(first.prepare_unlock(), Error::RecoveryRequired);
    assert_eq!(vault.opens(), opens);

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let root = TestRoot::new("recovery-linked");
        let vault = FakeVault::default();
        let (mut owner, _) = create_ready(&root, &vault, &report);
        owner.lock();
        drop(owner);
        let replacement = root.0.join("linked-operation");
        fs::write(&replacement, fs::read(recovery_pending(&root)).unwrap()).unwrap();
        fs::remove_file(recovery_pending(&root)).unwrap();
        symlink(&replacement, recovery_pending(&root)).unwrap();
        let mut owner = Owner::acquire(root.0.clone()).unwrap();
        let opens = vault.opens();
        assert_error(owner.prepare_unlock(), Error::RecoveryRequired);
        assert_eq!(vault.opens(), opens);
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn intent_and_completion_faults_keep_recovery_fences_for_source_and_empty_setup() {
    for creation in [false, true] {
        for fault in [
            Fault::BeforeIntentWrite,
            Fault::BeforeIntentSync,
            Fault::BeforeIntentReadback,
        ] {
            let root = TestRoot::new("recovery-intent-fault");
            let vault = FakeVault::default();
            let mut owner = Owner::acquire(root.0.clone()).unwrap();
            let preview = if creation {
                owner.prepare_create_empty().unwrap()
            } else {
                owner.prepare_create(&selected_source()).unwrap()
            };
            let opens = vault.opens();
            owner.inject_fault(fault);
            assert_error(owner.confirm(&preview.id, &vault), Error::WriteUncertain);
            assert_eq!(vault.opens(), opens);
            assert_eq!(vault.writes(&preview.workspace_id), 0);
        }

        for (fault, blocks_restart) in [
            (Fault::BeforeCompletionWrite, true),
            (Fault::BeforeCompletionSync, false),
            (Fault::BeforeCompletionReadback, false),
        ] {
            let root = TestRoot::new("recovery-completion-fault");
            let vault = FakeVault::default();
            let mut owner = Owner::acquire(root.0.clone()).unwrap();
            let preview = if creation {
                owner.prepare_create_empty().unwrap()
            } else {
                owner.prepare_create(&selected_source()).unwrap()
            };
            owner.inject_fault(fault);
            assert_error(owner.confirm(&preview.id, &vault), Error::WriteUncertain);
            assert!(vault.opens() > 0);
            assert_eq!(vault.writes(&preview.workspace_id), 1);
            drop(owner);
            let mut fresh = Owner::acquire(root.0.clone()).unwrap();
            if blocks_restart {
                assert_error(fresh.prepare_unlock(), Error::RecoveryRequired);
            } else {
                let unlock = fresh.prepare_unlock().unwrap();
                fresh.confirm(&unlock.id, &vault).unwrap();
                assert!(fresh.is_unlocked());
            }
        }
    }
}
