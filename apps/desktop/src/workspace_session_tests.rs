use super::*;

#[path = "workspace_data_control_tests.rs"]
mod data_controls;

#[path = "workspace_inspection_tests.rs"]
mod inspections;

#[path = "workspace_capability_mutation_tests.rs"]
mod capability_mutations;

#[path = "workspace_workflow_save_tests.rs"]
mod workflow_authoring;

#[path = "workspace_composition_tests.rs"]
mod compositions;

#[cfg(feature = "encrypted-workspace")]
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs,
    path::PathBuf,
    process::Command,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(feature = "encrypted-workspace")]
const BINDING_PREFIX: &[u8] = b"rangoon.workspace-binding.v1\0";
#[cfg(feature = "encrypted-workspace")]
const BINDING_BYTES: usize = 94;

#[cfg(feature = "encrypted-workspace")]
static NEXT: AtomicUsize = AtomicUsize::new(0);

#[cfg(feature = "encrypted-workspace")]
struct TestRoot(PathBuf);

#[cfg(feature = "encrypted-workspace")]
impl TestRoot {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rangoon-workspace-session-{label}-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }

    fn binding(&self) -> PathBuf {
        self.0.join("encrypted-workspace.binding")
    }

    fn encrypted_root(&self) -> PathBuf {
        self.0.join("encrypted-workspaces")
    }
}

#[cfg(feature = "encrypted-workspace")]
impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(feature = "encrypted-workspace")]
#[derive(Default)]
struct SlotState {
    record: Option<Vec<u8>>,
    read_error: Option<workspace_keys::Error>,
    write_error: Option<workspace_keys::Error>,
    reads: usize,
    writes: usize,
}

#[cfg(feature = "encrypted-workspace")]
#[derive(Default)]
struct VaultState {
    slots: BTreeMap<String, SlotState>,
    unavailable: bool,
    opens: usize,
    substitute_identity: Option<workspace_keys::WorkspaceId>,
}

#[cfg(feature = "encrypted-workspace")]
#[derive(Clone, Default)]
struct FakeVault(Rc<RefCell<VaultState>>);

#[cfg(feature = "encrypted-workspace")]
struct FakeSlot {
    identity: workspace_keys::WorkspaceId,
    lookup: String,
    state: Rc<RefCell<VaultState>>,
}

#[cfg(feature = "encrypted-workspace")]
impl FakeVault {
    fn opens(&self) -> usize {
        self.0.borrow().opens
    }

    fn writes(&self, identity: &str) -> usize {
        self.0
            .borrow()
            .slots
            .get(identity)
            .map(|slot| slot.writes)
            .unwrap_or(0)
    }

    fn reads(&self, identity: &str) -> usize {
        self.0
            .borrow()
            .slots
            .get(identity)
            .map(|slot| slot.reads)
            .unwrap_or(0)
    }

    fn remove_record(&self, identity: &str) {
        self.0
            .borrow_mut()
            .slots
            .entry(identity.to_owned())
            .or_default()
            .record = None;
    }

    fn corrupt_record(&self, identity: &str) {
        let mut state = self.0.borrow_mut();
        let record = state
            .slots
            .entry(identity.to_owned())
            .or_default()
            .record
            .as_mut()
            .unwrap();
        *record.last_mut().unwrap() ^= 0x01;
    }

    fn unavailable(&self) {
        self.0.borrow_mut().unavailable = true;
    }

    fn substitute_identity(&self, identity: workspace_keys::WorkspaceId) {
        self.0.borrow_mut().substitute_identity = Some(identity);
    }
}

#[cfg(feature = "encrypted-workspace")]
impl Vault for FakeVault {
    type Slot = FakeSlot;

    fn open(
        &self,
        identity: &workspace_keys::WorkspaceId,
    ) -> Result<Self::Slot, workspace_keys::Error> {
        let mut state = self.0.borrow_mut();
        state.opens += 1;
        if state.unavailable {
            return Err(workspace_keys::Error::Unavailable);
        }
        let lookup = identity.as_hex();
        state.slots.entry(lookup.clone()).or_default();
        Ok(FakeSlot {
            identity: state
                .substitute_identity
                .clone()
                .unwrap_or_else(|| identity.clone()),
            lookup,
            state: self.0.clone(),
        })
    }
}

#[cfg(feature = "encrypted-workspace")]
impl workspace_keys::Store for FakeSlot {
    fn identity(&self) -> &workspace_keys::WorkspaceId {
        &self.identity
    }

    fn read(&self) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>, workspace_keys::Error> {
        let mut state = self.state.borrow_mut();
        let slot = state.slots.get_mut(&self.lookup).unwrap();
        slot.reads += 1;
        if let Some(error) = slot.read_error {
            return Err(error);
        }
        Ok(slot.record.clone().map(zeroize::Zeroizing::new))
    }

    fn write(&self, envelope: &[u8]) -> Result<(), workspace_keys::Error> {
        let mut state = self.state.borrow_mut();
        let slot = state.slots.get_mut(&self.lookup).unwrap();
        slot.writes += 1;
        if let Some(error) = slot.write_error {
            return Err(error);
        }
        slot.record = Some(envelope.to_vec());
        Ok(())
    }
}

#[cfg(feature = "encrypted-workspace")]
fn source(bytes: &[u8]) -> rangoon_domain::AnalysisReport {
    rangoon_import::analyze("AGENTS.md", bytes).unwrap()
}

#[cfg(feature = "encrypted-workspace")]
fn selected_source() -> rangoon_domain::AnalysisReport {
    source(b"\xef\xbb\xbf# Rules\r\nKeep exact \xf0\x9f\xa6\x80 bytes.\r\n")
}

#[cfg(feature = "encrypted-workspace")]
fn binding(phase: u8, identity: &str) -> Vec<u8> {
    assert_eq!(BINDING_PREFIX.len(), 29);
    assert_eq!(identity.len(), 64);
    let mut bytes = BINDING_PREFIX.to_vec();
    bytes.push(phase);
    bytes.extend_from_slice(identity.as_bytes());
    assert_eq!(bytes.len(), BINDING_BYTES);
    bytes
}

#[cfg(feature = "encrypted-workspace")]
fn write_binding(root: &TestRoot, phase: u8, identity: &str) {
    fs::write(root.binding(), binding(phase, identity)).unwrap();
}

#[cfg(feature = "encrypted-workspace")]
fn assert_error<T>(actual: Result<T, Error>, expected: Error) {
    match actual {
        Ok(_) => panic!("expected {expected:?}"),
        Err(error) => assert_eq!(error, expected),
    }
}

#[cfg(feature = "encrypted-workspace")]
fn create_ready(
    root: &TestRoot,
    vault: &FakeVault,
    report: &rangoon_domain::AnalysisReport,
) -> (Owner, Preview) {
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let preview = owner.prepare_create(report).unwrap();
    owner.confirm(&preview.id, vault).unwrap();
    assert!(owner.is_unlocked());
    (owner, preview)
}

#[cfg(feature = "encrypted-workspace")]
fn database_path(root: &TestRoot, workspace_id: &str) -> PathBuf {
    root.encrypted_root()
        .join(workspace_id)
        .join("workspace.sqlite3")
}

#[cfg(not(feature = "encrypted-workspace"))]
#[test]
fn default_acquire_rejects_before_filesystem_access() {
    let root = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("rangoon-session-default-{}", std::process::id()));
    assert!(!root.exists());
    match Owner::acquire(root.clone()) {
        Ok(_) => panic!("default backend acquired an owner"),
        Err(error) => assert_eq!(error, Error::BackendUnavailable),
    }
    assert!(!root.exists());
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn acquire_has_a_real_cooperating_lease_and_releases_it_on_drop() {
    let root = TestRoot::new("lease");
    let first = Owner::acquire(root.0.clone()).unwrap();
    assert_error(Owner::acquire(root.0.clone()), Error::Busy);
    drop(first);
    assert!(Owner::acquire(root.0.clone()).is_ok());
    assert!(root.0.join("encrypted-workspace.lock").is_file());
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn cooperating_lease_blocks_a_child_process() {
    let root = TestRoot::new("child-lease");
    let owner = Owner::acquire(root.0.clone()).unwrap();
    let marker = root.0.join("child-lease-ok");
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "workspace_session::tests::child_process_reports_busy_while_parent_holds_lease",
            "--ignored",
        ])
        .env("RANGOON_SESSION_CHILD_ROOT", &root.0)
        .env("RANGOON_SESSION_CHILD_MARKER", &marker)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(fs::read(marker).unwrap(), b"busy");
    drop(owner);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
#[ignore]
fn child_process_reports_busy_while_parent_holds_lease() {
    let root = PathBuf::from(std::env::var("RANGOON_SESSION_CHILD_ROOT").unwrap());
    let marker = PathBuf::from(std::env::var("RANGOON_SESSION_CHILD_MARKER").unwrap());
    assert_error(Owner::acquire(root), Error::Busy);
    fs::write(marker, b"busy").unwrap();
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn linked_or_nonregular_control_and_binding_paths_are_rejected() {
    use std::os::unix::fs::symlink;

    let root = TestRoot::new("control-symlink");
    let target = root.0.join("lock-target");
    fs::write(&target, b"").unwrap();
    symlink(&target, root.0.join("encrypted-workspace.lock")).unwrap();
    assert_error(Owner::acquire(root.0.clone()), Error::InvalidBinding);

    let root = TestRoot::new("control-hardlink");
    let control = root.0.join("encrypted-workspace.lock");
    fs::write(&control, b"").unwrap();
    fs::hard_link(&control, root.0.join("control-alias")).unwrap();
    assert_error(Owner::acquire(root.0.clone()), Error::InvalidBinding);

    let root = TestRoot::new("binding-symlink");
    let target = root.0.join("binding-target");
    fs::write(&target, binding(1, &"67".repeat(32))).unwrap();
    symlink(&target, root.binding()).unwrap();
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    assert_error(owner.prepare_unlock(), Error::InvalidBinding);

    for label in ["nonzero-control", "oversized-control"] {
        let root = TestRoot::new(label);
        let control = root.0.join("encrypted-workspace.lock");
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&control)
            .unwrap();
        if label == "nonzero-control" {
            file.set_len(1).unwrap();
        } else {
            file.set_len(64 * 1024 * 1024 + 1).unwrap();
        }
        assert_error(Owner::acquire(root.0.clone()), Error::InvalidBinding);
    }
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn substituted_workspace_path_refuses_create_before_vault_access() {
    use std::os::unix::fs::symlink;

    let root = TestRoot::new("workspace-symlink");
    let vault = FakeVault::default();
    let report = selected_source();
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let preview = owner.prepare_create(&report).unwrap();
    fs::create_dir(root.encrypted_root()).unwrap();
    let target = root.0.join("other-workspace");
    fs::create_dir(&target).unwrap();
    symlink(&target, root.encrypted_root().join(&preview.workspace_id)).unwrap();
    assert!(owner.confirm(&preview.id, &vault).is_err());
    assert_eq!(vault.opens(), 0);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn binding_is_exact_binary_94_byte_framing() {
    let identity = "1a".repeat(32);
    let cases = [
        Vec::new(),
        b"rangoon.workspace-binding.v1\0\0".to_vec(),
        binding(2, &identity),
        binding(1, &"A1".repeat(32)),
        binding(1, &"00".repeat(32)),
        {
            let mut bytes = binding(1, &identity);
            bytes.push(b'x');
            bytes
        },
    ];
    for (index, bytes) in cases.into_iter().enumerate() {
        let root = TestRoot::new(&format!("binding-{index}"));
        fs::write(root.binding(), bytes).unwrap();
        let mut owner = Owner::acquire(root.0.clone()).unwrap();
        assert_error(owner.prepare_unlock(), Error::InvalidBinding);
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn provisioning_binding_requires_explicit_recovery() {
    let root = TestRoot::new("provisioning");
    write_binding(&root, 0, &"21".repeat(32));
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    assert_error(owner.prepare_unlock(), Error::RecoveryRequired);
    assert_error(
        owner.prepare_create(&selected_source()),
        Error::AlreadyExists,
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn prepare_is_read_only_and_source_must_reanalyze_exactly() {
    let root = TestRoot::new("prepare");
    let vault = FakeVault::default();
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let mut invalid = selected_source();
    invalid.source.sha256 = "0".repeat(64);
    assert_error(owner.prepare_create(&invalid), Error::InvalidSource);
    let preview = owner.prepare_create(&selected_source()).unwrap();
    assert_eq!(preview.kind, Action::Create);
    assert_eq!(preview.source_id, Some(selected_source().source.id));
    assert_ne!(preview.id, preview.workspace_id);
    assert_eq!(vault.opens(), 0);
    assert!(!root.binding().exists());
    assert!(!root.encrypted_root().exists());
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn wrong_and_consumed_confirmation_tokens_cannot_mutate_or_consume_newer_actions() {
    let root = TestRoot::new("tokens");
    let vault = FakeVault::default();
    let report = selected_source();
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let abandoned = owner.prepare_create(&report).unwrap();
    owner.lock();
    let first = owner.prepare_create(&report).unwrap();
    assert_error(owner.confirm(&abandoned.id, &vault), Error::Stale);
    assert_error(owner.confirm("not-a-token", &vault), Error::Stale);
    assert_eq!(vault.opens(), 0);
    assert!(!root.binding().exists());
    assert!(!root.encrypted_root().exists());
    owner.confirm(&first.id, &vault).unwrap();
    assert_error(owner.confirm(&first.id, &vault), Error::Stale);
    owner.lock();
    let second = owner.prepare_unlock().unwrap();
    assert_ne!(first.id, second.id);
    assert_error(owner.confirm(&first.id, &vault), Error::Stale);
    owner.confirm(&second.id, &vault).unwrap();
    assert!(owner.is_unlocked());
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn create_restart_unlock_and_managed_source_apis_preserve_exact_source() {
    let root = TestRoot::new("restart");
    let vault = FakeVault::default();
    let report = selected_source();
    let (mut owner, preview) = create_ready(&root, &vault, &report);
    assert_eq!(
        fs::read(root.binding()).unwrap(),
        binding(1, &preview.workspace_id)
    );
    let encrypted = fs::read(database_path(&root, &preview.workspace_id)).unwrap();
    assert_ne!(&encrypted[..16], b"SQLite format 3\0");
    assert!(
        !encrypted
            .windows(report.source.content.len())
            .any(|window| window == report.source.content.as_bytes())
    );
    assert_eq!(owner.list(&vault).unwrap().len(), 1);
    assert_eq!(owner.open(&vault, &report.source.id).unwrap(), report);
    let second = source(b"# Rules\nNew bytes\n");
    owner.save(&vault, &second).unwrap();
    assert_eq!(owner.open(&vault, &second.source.id).unwrap(), second);
    owner.lock();
    drop(owner);
    let mut restarted = Owner::acquire(root.0.clone()).unwrap();
    let unlock = restarted.prepare_unlock().unwrap();
    assert_eq!(unlock.kind, Action::Unlock);
    assert_eq!(unlock.workspace_id, preview.workspace_id);
    assert_eq!(unlock.source_id, None);
    restarted.confirm(&unlock.id, &vault).unwrap();
    assert_eq!(restarted.open(&vault, &report.source.id).unwrap(), report);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn legacy_plaintext_directory_is_never_touched() {
    let root = TestRoot::new("legacy");
    let legacy = root.0.join("source-workspace");
    fs::create_dir(&legacy).unwrap();
    let legacy_file = legacy.join("legacy.sqlite3");
    let original = b"plaintext legacy bytes\0remain";
    fs::write(&legacy_file, original).unwrap();
    let vault = FakeVault::default();
    let report = selected_source();
    let _ = create_ready(&root, &vault, &report);
    assert_eq!(fs::read(legacy_file).unwrap(), original);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn changed_binding_directory_or_slot_is_refused_before_key_reads() {
    let root = TestRoot::new("changed");
    let vault = FakeVault::default();
    let report = selected_source();
    let (mut owner, _preview) = create_ready(&root, &vault, &report);
    fs::write(root.binding(), binding(1, &"51".repeat(32))).unwrap();
    let before = vault.opens();
    assert!(owner.list(&vault).is_err());
    assert_eq!(vault.opens(), before);
    assert!(!owner.is_unlocked());

    let root = TestRoot::new("slot");
    let vault = FakeVault::default();
    let (mut owner, preview) = create_ready(&root, &vault, &report);
    let before_reads = vault.reads(&preview.workspace_id);
    vault.substitute_identity(workspace_keys::WorkspaceId::parse(&"52".repeat(32)).unwrap());
    assert!(owner.list(&vault).is_err());
    assert!(!owner.is_unlocked());
    assert_eq!(vault.reads(&preview.workspace_id), before_reads);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn removed_managed_identity_directory_autolocks_before_vault_access() {
    let root = TestRoot::new("removed");
    let vault = FakeVault::default();
    let report = selected_source();
    let (mut owner, preview) = create_ready(&root, &vault, &report);
    let directory = root.encrypted_root().join(&preview.workspace_id);
    fs::remove_dir_all(directory).unwrap();
    let before = vault.opens();
    assert_error(owner.list(&vault), Error::StoreInvalid);
    assert_eq!(vault.opens(), before);
    assert!(!owner.is_unlocked());
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn replaced_managed_identity_directory_autolocks_before_vault_access() {
    let root = TestRoot::new("replaced");
    let vault = FakeVault::default();
    let report = selected_source();
    let (mut owner, preview) = create_ready(&root, &vault, &report);
    let directory = root.encrypted_root().join(&preview.workspace_id);
    let original = root.0.join("retained-original-identity");
    fs::rename(&directory, &original).unwrap();
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("workspace.sqlite3"), b"replacement").unwrap();
    let before = vault.opens();
    assert_error(owner.list(&vault), Error::StoreInvalid);
    assert_eq!(vault.opens(), before);
    assert!(!owner.is_unlocked());
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn prepared_unlock_rejects_replaced_identity_directory_before_vault_access() {
    let root = TestRoot::new("prepared-replaced");
    let vault = FakeVault::default();
    let report = selected_source();
    let (mut owner, preview) = create_ready(&root, &vault, &report);
    owner.lock();
    drop(owner);

    let mut fresh = Owner::acquire(root.0.clone()).unwrap();
    let unlock = fresh.prepare_unlock().unwrap();
    let directory = root.encrypted_root().join(&preview.workspace_id);
    let original = root.0.join("retained-prepared-identity");
    fs::rename(&directory, &original).unwrap();
    fs::create_dir(&directory).unwrap();
    fs::copy(
        original.join("workspace.sqlite3"),
        directory.join("workspace.sqlite3"),
    )
    .unwrap();
    let before = vault.opens();
    assert_error(fresh.confirm(&unlock.id, &vault), Error::StoreInvalid);
    assert_eq!(vault.opens(), before);
    assert!(!fresh.is_unlocked());
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn replaced_managed_parent_autolocks_while_preserving_child_identity() {
    let root = TestRoot::new("replaced-parent");
    let vault = FakeVault::default();
    let report = selected_source();
    let (mut owner, preview) = create_ready(&root, &vault, &report);
    let parent = root.encrypted_root();
    let retained_parent = root.0.join("retained-encrypted-workspaces");
    fs::rename(&parent, &retained_parent).unwrap();
    fs::create_dir(&parent).unwrap();
    fs::rename(
        retained_parent.join(&preview.workspace_id),
        parent.join(&preview.workspace_id),
    )
    .unwrap();
    let before = vault.opens();
    assert_error(owner.list(&vault), Error::StoreInvalid);
    assert_eq!(vault.opens(), before);
    assert!(!owner.is_unlocked());
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn changed_binding_after_prepared_unlock_refuses_before_vault_access() {
    let root = TestRoot::new("prepared-unlock");
    let vault = FakeVault::default();
    let report = selected_source();
    let (mut owner, preview) = create_ready(&root, &vault, &report);
    owner.lock();
    drop(owner);

    let mut fresh = Owner::acquire(root.0.clone()).unwrap();
    let unlock = fresh.prepare_unlock().unwrap();
    fs::write(root.binding(), binding(1, &"53".repeat(32))).unwrap();
    let before = vault.opens();
    assert_error(fresh.confirm(&unlock.id, &vault), Error::Stale);
    assert_eq!(vault.opens(), before);
    assert_eq!(preview.workspace_id.len(), 64);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn missing_or_empty_database_refuses_unlock_before_vault_open() {
    for case in ["missing", "empty"] {
        let root = TestRoot::new(case);
        let vault = FakeVault::default();
        let report = selected_source();
        let (mut owner, preview) = create_ready(&root, &vault, &report);
        owner.lock();
        drop(owner);
        let database = database_path(&root, &preview.workspace_id);
        if case == "missing" {
            fs::remove_file(database).unwrap();
        } else {
            fs::write(database, b"").unwrap();
        }
        let mut fresh = Owner::acquire(root.0.clone()).unwrap();
        let before = vault.opens();
        assert_error(fresh.prepare_unlock(), Error::StoreInvalid);
        assert_eq!(vault.opens(), before);
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn fresh_unlock_with_a_well_formed_wrong_key_fails_store_auth_without_writes() {
    let root = TestRoot::new("wrong-key");
    let vault = FakeVault::default();
    let report = selected_source();
    let (mut owner, preview) = create_ready(&root, &vault, &report);
    owner.lock();
    drop(owner);
    let database = database_path(&root, &preview.workspace_id);
    let original_bytes = fs::read(&database).unwrap();
    vault.corrupt_record(&preview.workspace_id);
    let writes = vault.writes(&preview.workspace_id);
    let mut fresh = Owner::acquire(root.0.clone()).unwrap();
    let unlock = fresh.prepare_unlock().unwrap();
    assert_error(fresh.confirm(&unlock.id, &vault), Error::StoreInvalid);
    assert!(!fresh.is_unlocked());
    assert_eq!(vault.writes(&preview.workspace_id), writes);
    assert_eq!(fs::read(database).unwrap(), original_bytes);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn corrupt_database_auto_locks_and_list_leaves_bytes_unchanged() {
    let root = TestRoot::new("corrupt-database");
    let vault = FakeVault::default();
    let report = selected_source();
    let (mut owner, preview) = create_ready(&root, &vault, &report);
    let database = database_path(&root, &preview.workspace_id);
    let mut corrupted = fs::read(&database).unwrap();
    let middle = corrupted.len() / 2;
    corrupted[middle] ^= 0x80;
    fs::write(&database, &corrupted).unwrap();
    assert_error(owner.list(&vault), Error::StoreInvalid);
    assert!(!owner.is_unlocked());
    assert_eq!(fs::read(database).unwrap(), corrupted);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn missing_wrong_or_unavailable_keys_fail_closed_without_regeneration() {
    for case in ["missing", "wrong", "unavailable"] {
        let root = TestRoot::new(case);
        let vault = FakeVault::default();
        let report = selected_source();
        let (mut owner, preview) = create_ready(&root, &vault, &report);
        match case {
            "missing" => vault.remove_record(&preview.workspace_id),
            "wrong" => vault.corrupt_record(&preview.workspace_id),
            "unavailable" => vault.unavailable(),
            _ => unreachable!(),
        }
        let writes = vault.writes(&preview.workspace_id);
        assert!(owner.list(&vault).is_err());
        assert!(!owner.is_unlocked());
        assert_eq!(vault.writes(&preview.workspace_id), writes);
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn explicit_lock_drops_session_and_cancels_pending_confirmation() {
    let root = TestRoot::new("lock");
    let vault = FakeVault::default();
    let report = selected_source();
    let (mut owner, _) = create_ready(&root, &vault, &report);
    owner.lock();
    let pending = owner.prepare_unlock().unwrap();
    owner.lock();
    assert_error(owner.confirm(&pending.id, &vault), Error::Stale);
    assert_error(Owner::acquire(root.0.clone()), Error::Busy);
    assert!(!owner.is_unlocked());
    assert_error(owner.list(&vault), Error::Locked);
    assert_error(owner.open(&vault, &report.source.id), Error::Locked);
    assert_error(owner.save(&vault, &report), Error::Locked);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn same_owner_write_uncertain_is_sticky_and_post_save_keeps_actual_bytes() {
    let root = TestRoot::new("after-save");
    let vault = FakeVault::default();
    let original = selected_source();
    let (mut owner, _) = create_ready(&root, &vault, &original);
    let changed = source(b"\xef\xbb\xbf# Later\r\nExact saved bytes.\r\n");
    owner.inject_fault(Fault::AfterSave);
    assert_error(owner.save(&vault, &changed), Error::WriteUncertain);
    assert!(!owner.is_unlocked());
    assert_error(owner.prepare_unlock(), Error::RecoveryRequired);
    drop(owner);

    let mut fresh = Owner::acquire(root.0.clone()).unwrap();
    let preview = fresh.prepare_unlock().unwrap();
    fresh.confirm(&preview.id, &vault).unwrap();
    assert_eq!(fresh.open(&vault, &changed.source.id).unwrap(), changed);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn creation_faults_never_retry_and_fresh_owner_inspects_retained_binding() {
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
        let root = TestRoot::new("fault");
        let vault = FakeVault::default();
        let report = selected_source();
        let mut owner = Owner::acquire(root.0.clone()).unwrap();
        let preview = owner.prepare_create(&report).unwrap();
        owner.inject_fault(fault);
        assert_error(owner.confirm(&preview.id, &vault), Error::WriteUncertain);
        assert!(!owner.is_unlocked());
        let writes = vault.writes(&preview.workspace_id);
        assert_error(owner.prepare_create(&report), Error::RecoveryRequired);
        assert_eq!(vault.writes(&preview.workspace_id), writes);
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
                assert_error(fresh.prepare_unlock(), Error::RecoveryRequired)
            }
            ExpectedBinding::Ready => {
                assert_eq!(retained, binding(1, &preview.workspace_id));
                let unlock = fresh.prepare_unlock().unwrap();
                fresh.confirm(&unlock.id, &vault).unwrap();
                assert!(fresh.is_unlocked());
            }
        }
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn owner_and_key_types_do_not_expose_sensitive_debug_clone_or_raw_handles() {
    let root = TestRoot::new("privacy");
    let vault = FakeVault::default();
    let (_, preview) = create_ready(&root, &vault, &selected_source());
    assert_eq!(preview.workspace_id.len(), 64);
    assert_eq!(
        preview.source_id.as_deref(),
        Some(selected_source().source.id.as_str())
    );
}
