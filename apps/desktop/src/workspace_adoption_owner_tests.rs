//! Synthetic Owner transfer and uncertain-write tests. No OS vault.
#[cfg(feature = "encrypted-workspace")]
use super::*;

#[cfg(feature = "encrypted-workspace")]
mod fixture {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        collections::BTreeMap,
        rc::Rc,
    };
    #[derive(Clone, Default)]
    pub(super) struct TestVault {
        pub records: Rc<RefCell<BTreeMap<String, Vec<u8>>>>,
        pub writes: Rc<Cell<usize>>,
    }
    pub(super) struct Slot {
        identity: WorkspaceId,
        vault: TestVault,
    }
    impl Vault for TestVault {
        type Slot = Slot;
        fn open(&self, identity: &WorkspaceId) -> Result<Slot, workspace_keys::Error> {
            Ok(Slot {
                identity: identity.clone(),
                vault: self.clone(),
            })
        }
    }
    impl Store for Slot {
        fn identity(&self) -> &WorkspaceId {
            &self.identity
        }
        fn read(&self) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>, workspace_keys::Error> {
            Ok(self
                .vault
                .records
                .borrow()
                .get(&self.identity.as_hex())
                .cloned()
                .map(zeroize::Zeroizing::new))
        }
        fn write(&self, bytes: &[u8]) -> Result<(), workspace_keys::Error> {
            self.vault.writes.set(self.vault.writes.get() + 1);
            self.vault
                .records
                .borrow_mut()
                .insert(self.identity.as_hex(), bytes.to_vec());
            Ok(())
        }
    }
    pub(super) struct Root(pub PathBuf);
    impl Root {
        pub fn new() -> Self {
            let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "rangoon-owner-adoption-{}",
                WorkspaceId::generate().unwrap().as_hex()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    pub(super) fn source(root: &Root) -> WorkspaceBackup {
        let workspace = Workspace::new(root.0.join("source"));
        workspace
            .save(&rangoon_import::analyze("AGENTS.md", b"# Transfer\nExact original.\n").unwrap())
            .unwrap();
        WorkspaceBackup::decode(&workspace.export_workflow_backup().unwrap()).unwrap()
    }
    pub(super) fn empty(root: &Root, vault: &TestVault) -> (Owner, Preview) {
        let path = root.0.join("managed");
        fs::create_dir(&path).unwrap();
        let mut owner = Owner::acquire(path).unwrap();
        let preview = owner.prepare_create_empty().unwrap();
        owner.confirm(&preview.id, vault).unwrap();
        (owner, preview)
    }
}

#[cfg(feature = "encrypted-workspace")]
use fixture::*;

#[test]
#[cfg(feature = "encrypted-workspace")]
fn owner_strict_transfer_is_exact_and_reopens_with_existing_slot_only() {
    let root = Root::new();
    let vault = TestVault::default();
    let backup = source(&root);
    let (mut owner, preview) = empty(&root, &vault);
    let plan = owner.prepare_adoption_restore(&vault, &backup, 1).unwrap();
    owner.restore_adoption(&vault, &backup, &plan).unwrap();
    let expected = owner.export_workflow_backup(&vault).unwrap();
    assert_eq!(owner.list(&vault).unwrap().len(), 1);
    owner.lock();
    drop(owner);
    let mut owner = Owner::acquire(root.0.join("managed")).unwrap();
    let unlock = owner.prepare_unlock().unwrap();
    assert_eq!(unlock.workspace_id, preview.workspace_id);
    owner.confirm(&unlock.id, &vault).unwrap();
    assert!(owner.export_workflow_backup(&vault).unwrap() == expected);
    assert_eq!(vault.writes.get(), 1);
    assert!(owner.prepare_adoption_restore(&vault, &backup, 1).is_err());
}

#[test]
#[cfg(feature = "encrypted-workspace")]
fn owner_uncertain_transfer_locks_and_retains_real_restore_recovery() {
    let root = Root::new();
    let vault = TestVault::default();
    let backup = source(&root);
    let (mut owner, _) = empty(&root, &vault);
    let plan = owner.prepare_adoption_restore(&vault, &backup, 1).unwrap();
    owner.inject_fault(Fault::AfterRestore);
    assert!(matches!(
        owner.restore_adoption(&vault, &backup, &plan),
        Err(Error::WriteUncertain)
    ));
    assert!(!owner.is_unlocked());
    assert!(owner.requires_recovery());
    drop(owner);
    let mut owner = Owner::acquire(root.0.join("managed")).unwrap();
    assert!(matches!(
        owner.prepare_unlock(),
        Err(Error::RecoveryRequired)
    ));
    assert_eq!(vault.writes.get(), 1);
}

#[test]
#[cfg(feature = "encrypted-workspace")]
fn owner_transfer_intent_and_completion_failures_cannot_leave_an_unlocked_client() {
    for fault in [Fault::BeforeIntentWrite, Fault::BeforeCompletionWrite] {
        let root = Root::new();
        let vault = TestVault::default();
        let backup = source(&root);
        let (mut owner, _) = empty(&root, &vault);
        let plan = owner.prepare_adoption_restore(&vault, &backup, 1).unwrap();
        owner.inject_fault(fault);
        assert!(owner.restore_adoption(&vault, &backup, &plan).is_err());
        assert!(!owner.is_unlocked());
        assert!(owner.requires_recovery());
        assert_eq!(vault.writes.get(), 1);
    }
}

#[test]
#[cfg(feature = "encrypted-workspace")]
fn missing_key_after_transfer_is_not_replaced() {
    let root = Root::new();
    let vault = TestVault::default();
    let backup = source(&root);
    let (mut owner, _) = empty(&root, &vault);
    let plan = owner.prepare_adoption_restore(&vault, &backup, 1).unwrap();
    owner.restore_adoption(&vault, &backup, &plan).unwrap();
    owner.lock();
    drop(owner);
    vault.records.borrow_mut().clear();
    let mut owner = Owner::acquire(root.0.join("managed")).unwrap();
    let preview = owner.prepare_unlock().unwrap();
    assert!(owner.confirm(&preview.id, &vault).is_err());
    assert!(!owner.is_unlocked());
    assert_eq!(vault.writes.get(), 1);
}
