#[cfg(feature = "encrypted-workspace")]
use super::*;

#[cfg(feature = "encrypted-workspace")]
use std::fs;

#[cfg(feature = "encrypted-workspace")]
fn create_empty(root: &TestRoot, vault: &FakeVault) -> (Owner, Preview) {
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let preview = owner.prepare_create_empty().unwrap();
    owner.confirm(&preview.id, vault).unwrap();
    (owner, preview)
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn empty_preparation_has_no_vault_or_database_side_effects() {
    let root = TestRoot::new("empty-prepare");
    let vault = FakeVault::default();
    let mut owner = Owner::acquire(root.0.clone()).unwrap();

    let preview = owner.prepare_create_empty().unwrap();
    assert_eq!(preview.kind, Action::CreateEmpty);
    assert!(preview.source_id.is_none());
    assert_eq!(vault.opens(), 0);
    assert!(!root.binding().exists());
    assert!(!root.encrypted_root().exists());
    assert_error(owner.prepare_create_empty(), Error::Busy);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn mutated_empty_display_copy_cannot_change_retained_action() {
    let root = TestRoot::new("empty-display-copy");
    let vault = FakeVault::default();
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let mut preview = owner.prepare_create_empty().unwrap();
    let token = preview.id.clone();
    let original_workspace_id = preview.workspace_id.clone();
    preview.kind = Action::Create;
    preview.workspace_id = "0".repeat(64);
    preview.source_id = Some("source:forged".into());

    owner.confirm(&token, &vault).unwrap();
    assert!(owner.is_unlocked());
    assert!(owner.list(&vault).unwrap().is_empty());
    assert_eq!(
        fs::read(root.binding()).unwrap(),
        binding(1, &original_workspace_id)
    );
    assert_eq!(vault.writes(&original_workspace_id), 1);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn malformed_empty_tokens_never_open_vault_or_consume_valid_token() {
    let root = TestRoot::new("empty-token");
    let vault = FakeVault::default();
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let preview = owner.prepare_create_empty().unwrap();
    let unknowns = [
        String::new(),
        format!("{}x", preview.id),
        preview.id[..preview.id.len() - 1].into(),
        "not-a-confirmation-token".into(),
    ];

    for token in unknowns {
        assert_error(owner.confirm(&token, &vault), Error::Stale);
        assert_eq!(vault.opens(), 0);
        assert!(!root.binding().exists());
    }
    owner.confirm(&preview.id, &vault).unwrap();
    assert!(owner.is_unlocked());
    assert!(owner.list(&vault).unwrap().is_empty());
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn matching_empty_token_consumes_once() {
    let root = TestRoot::new("empty-consume");
    let vault = FakeVault::default();
    let (mut owner, preview) = create_empty(&root, &vault);
    let opens = vault.opens();

    assert_error(owner.confirm(&preview.id, &vault), Error::Stale);
    assert_eq!(vault.opens(), opens);
    assert!(owner.list(&vault).unwrap().is_empty());
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn empty_workspace_locks_drops_and_reacquires_as_empty() {
    let root = TestRoot::new("empty-reacquire");
    let vault = FakeVault::default();
    let (mut owner, preview) = create_empty(&root, &vault);
    assert!(owner.list(&vault).unwrap().is_empty());
    owner.lock();
    assert!(!owner.is_unlocked());
    drop(owner);

    let mut reopened = Owner::acquire(root.0.clone()).unwrap();
    let unlock = reopened.prepare_unlock().unwrap();
    assert_eq!(unlock.kind, Action::Unlock);
    assert_eq!(unlock.workspace_id, preview.workspace_id);
    reopened.confirm(&unlock.id, &vault).unwrap();
    assert!(reopened.list(&vault).unwrap().is_empty());
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn wrong_key_cannot_unlock_empty_workspace_or_change_ciphertext() {
    let root = TestRoot::new("empty-wrong-key");
    let vault = FakeVault::default();
    let (mut owner, preview) = create_empty(&root, &vault);
    let database = database_path(&root, &preview.workspace_id);
    let original = fs::read(&database).unwrap();
    owner.lock();
    drop(owner);
    vault.remove_record(&preview.workspace_id);
    let identity = workspace_keys::WorkspaceId::parse(&preview.workspace_id).unwrap();
    let slot = vault.open(&identity).unwrap();
    let replacement = workspace_keys::create_key(&slot).unwrap();
    drop(replacement);

    let mut fresh = Owner::acquire(root.0.clone()).unwrap();
    let unlock = fresh.prepare_unlock().unwrap();
    let writes = vault.writes(&preview.workspace_id);
    assert_error(fresh.confirm(&unlock.id, &vault), Error::StoreInvalid);
    assert!(!fresh.is_unlocked());
    assert_eq!(vault.writes(&preview.workspace_id), writes);
    assert_eq!(fs::read(database).unwrap(), original);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn empty_creation_faults_preserve_binding_evidence_and_expose_ready_reopen_limit() {
    enum ExpectedBinding {
        Empty,
        Provisioning,
        Ready,
    }

    for (fault, expected) in [
        (Fault::BeforeProvisionWrite, ExpectedBinding::Empty),
        (Fault::BeforeReadyWrite, ExpectedBinding::Provisioning),
        (Fault::BeforeReadySync, ExpectedBinding::Ready),
        (Fault::BeforeReadyReadback, ExpectedBinding::Ready),
    ] {
        let root = TestRoot::new("empty-fault");
        let vault = FakeVault::default();
        let mut owner = Owner::acquire(root.0.clone()).unwrap();
        let preview = owner.prepare_create_empty().unwrap();
        owner.inject_fault(fault);
        assert_error(owner.confirm(&preview.id, &vault), Error::WriteUncertain);
        assert!(!owner.is_unlocked());
        assert_error(owner.prepare_create_empty(), Error::RecoveryRequired);
        drop(owner);

        let mut fresh = Owner::acquire(root.0.clone()).unwrap();
        let retained = fs::read(root.binding()).unwrap();
        match expected {
            ExpectedBinding::Empty => {
                assert!(retained.is_empty());
                assert_error(fresh.prepare_unlock(), Error::InvalidBinding);
            }
            ExpectedBinding::Provisioning => {
                assert_eq!(retained, binding(0, &preview.workspace_id));
                assert_error(fresh.prepare_unlock(), Error::RecoveryRequired);
            }
            ExpectedBinding::Ready => {
                assert_eq!(retained, binding(1, &preview.workspace_id));
                // This in-process fixture records current source behavior only.
                // It is not a durable recovery or crash-proof acceptance claim.
                let unlock = fresh.prepare_unlock().unwrap();
                fresh.confirm(&unlock.id, &vault).unwrap();
                assert!(fresh.list(&vault).unwrap().is_empty());
            }
        }
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn empty_creation_never_adopts_or_changes_synthetic_legacy_source_workspace() {
    let root = TestRoot::new("empty-preserves-legacy-source-workspace");
    let source = selected_source();
    let legacy_directory = root.0.join("source-workspace");
    rangoon_store::Workspace::new(legacy_directory.clone())
        .save_v1(&source)
        .unwrap();
    let legacy_database = legacy_directory.join("workspace.sqlite3");
    let original = fs::read(&legacy_database).unwrap();
    assert!(original.starts_with(b"SQLite format 3\0"));

    let vault = FakeVault::default();
    let (mut owner, _) = create_empty(&root, &vault);
    assert!(owner.list(&vault).unwrap().is_empty());
    owner.lock();
    drop(owner);
    let mut reopened = Owner::acquire(root.0.clone()).unwrap();
    let unlock = reopened.prepare_unlock().unwrap();
    reopened.confirm(&unlock.id, &vault).unwrap();
    assert!(reopened.list(&vault).unwrap().is_empty());
    assert_eq!(fs::read(legacy_database).unwrap(), original);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn selected_source_creation_retains_its_existing_source_behavior() {
    let root = TestRoot::new("empty-keeps-source-create");
    let vault = FakeVault::default();
    let source = selected_source();
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let preview = owner.prepare_create(&source).unwrap();

    assert_eq!(preview.kind, Action::Create);
    assert_eq!(
        preview.source_id.as_deref(),
        Some(source.source.id.as_str())
    );
    owner.confirm(&preview.id, &vault).unwrap();
    assert_eq!(owner.open(&vault, &source.source.id).unwrap(), source);
}
