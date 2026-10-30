//! Synthetic interrupted-empty provisioning tests. No OS vault or native UI.
use super::*;

use std::{cell::RefCell, collections::BTreeMap, fs, path::PathBuf, rc::Rc};

struct TestRoot(PathBuf);

impl TestRoot {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rangoon-empty-provisioning-{label}-{}",
            WorkspaceId::generate().unwrap().as_hex(),
        ));
        fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }

    fn binding(&self) -> PathBuf {
        self.0.join(BINDING)
    }

    fn encrypted_root(&self) -> PathBuf {
        self.0.join(PARENT)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn provisioning_binding(workspace_id: &str) -> Vec<u8> {
    Binding {
        ready: false,
        identity: WorkspaceId::parse(workspace_id).unwrap(),
    }
    .bytes()
}

fn database_path(root: &TestRoot, workspace_id: &str) -> PathBuf {
    root.encrypted_root().join(workspace_id).join(DATABASE)
}

fn assert_error<T>(actual: Result<T, Error>, expected: Error) {
    match actual {
        Ok(_) => panic!("expected {expected:?}"),
        Err(error) => assert_eq!(error, expected),
    }
}

fn selected_source() -> rangoon_domain::AnalysisReport {
    rangoon_import::analyze("AGENTS.md", b"# Test\nSynthetic source.\n").unwrap()
}

#[derive(Default)]
struct VaultState {
    records: BTreeMap<String, Vec<u8>>,
    opens: usize,
    reads: usize,
    writes: usize,
    uncertain_write_once: bool,
    after_read_effect: Option<ReadEffect>,
}

#[derive(Clone)]
struct ReadEffect {
    trigger: usize,
    path: PathBuf,
    bytes: Vec<u8>,
}

#[derive(Clone, Default)]
struct InterruptedVault(Rc<RefCell<VaultState>>);

struct InterruptedSlot {
    identity: WorkspaceId,
    vault: InterruptedVault,
}

impl InterruptedVault {
    fn arm_uncertain_write_once(&self) {
        self.0.borrow_mut().uncertain_write_once = true;
    }

    fn opens(&self) -> usize {
        self.0.borrow().opens
    }

    fn reads(&self) -> usize {
        self.0.borrow().reads
    }

    fn writes(&self) -> usize {
        self.0.borrow().writes
    }

    fn remove(&self, workspace_id: &str) {
        self.0.borrow_mut().records.remove(workspace_id);
    }

    fn corrupt(&self, workspace_id: &str) {
        let mut state = self.0.borrow_mut();
        let record = state.records.get_mut(workspace_id).unwrap();
        *record.last_mut().unwrap() ^= 1;
    }

    fn malformed(&self, workspace_id: &str) {
        self.0
            .borrow_mut()
            .records
            .insert(workspace_id.into(), vec![0]);
    }

    fn arm_after_reads(&self, reads: usize, path: PathBuf, bytes: &[u8]) {
        let mut state = self.0.borrow_mut();
        state.after_read_effect = Some(ReadEffect {
            trigger: state.reads + reads,
            path,
            bytes: bytes.to_vec(),
        });
    }

    fn snapshot(&self, workspace_id: &str) -> zeroize::Zeroizing<Vec<u8>> {
        zeroize::Zeroizing::new(self.0.borrow().records.get(workspace_id).unwrap().clone())
    }

    fn matches(&self, workspace_id: &str, expected: &zeroize::Zeroizing<Vec<u8>>) -> bool {
        self.0
            .borrow()
            .records
            .get(workspace_id)
            .is_some_and(|actual| actual.as_slice() == expected.as_slice())
    }
}

impl Vault for InterruptedVault {
    type Slot = InterruptedSlot;

    fn open(&self, identity: &WorkspaceId) -> Result<Self::Slot, workspace_keys::Error> {
        let mut state = self.0.borrow_mut();
        state.opens += 1;
        Ok(InterruptedSlot {
            identity: identity.clone(),
            vault: self.clone(),
        })
    }
}

impl Store for InterruptedSlot {
    fn identity(&self) -> &WorkspaceId {
        &self.identity
    }

    fn read(&self) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>, workspace_keys::Error> {
        let (record, effect) = {
            let mut state = self.vault.0.borrow_mut();
            state.reads += 1;
            let effect = if state
                .after_read_effect
                .as_ref()
                .is_some_and(|effect| effect.trigger == state.reads)
            {
                state.after_read_effect.take()
            } else {
                None
            };
            (state.records.get(&self.identity.as_hex()).cloned(), effect)
        };
        if let Some(effect) = effect {
            fs::write(effect.path, effect.bytes).map_err(|_| workspace_keys::Error::Unavailable)?;
        }
        Ok(record.map(zeroize::Zeroizing::new))
    }

    fn write(&self, envelope: &[u8]) -> Result<(), workspace_keys::Error> {
        let mut state = self.vault.0.borrow_mut();
        state.writes += 1;
        state
            .records
            .insert(self.identity.as_hex(), envelope.to_vec());
        if std::mem::take(&mut state.uncertain_write_once) {
            return Err(workspace_keys::Error::WriteUncertain);
        }
        Ok(())
    }
}

struct Interrupted {
    root: TestRoot,
    vault: InterruptedVault,
    workspace_id: String,
}

/// Persists the real key envelope, then reports an uncertain write. This leaves
/// Owner's genuine Pending/CreateEmpty operation and unready binding intact.
fn interrupted_empty(label: &str) -> Interrupted {
    let root = TestRoot::new(label);
    let vault = InterruptedVault::default();
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let create = owner.prepare_create_empty().unwrap();
    vault.arm_uncertain_write_once();
    assert_error(owner.confirm(&create.id, &vault), Error::WriteUncertain);
    assert_eq!(
        fs::read(root.binding()).unwrap(),
        provisioning_binding(&create.workspace_id)
    );
    assert!(!database_path(&root, &create.workspace_id).exists());
    assert_eq!(vault.writes(), 1);
    drop(owner);
    Interrupted {
        root,
        vault,
        workspace_id: create.workspace_id,
    }
}

#[test]
fn empty_provisioning_finishes_exact_interrupted_attempt_then_requires_unlock() {
    let interrupted = interrupted_empty("finish");
    let mut owner = Owner::acquire(interrupted.root.0.clone()).unwrap();
    let preview = owner
        .inspect_empty_provisioning(&interrupted.vault)
        .unwrap();

    assert_eq!(preview.workspace_id, interrupted.workspace_id);
    assert_ne!(preview.attempt_id, preview.workspace_id);
    assert_eq!(preview.status, "uninitialized");
    assert_eq!(interrupted.vault.writes(), 1);
    assert!(interrupted.vault.reads() > 0);

    owner
        .finish_empty_provisioning(&preview.id, &interrupted.vault)
        .unwrap();
    assert!(!owner.is_unlocked());
    assert_eq!(interrupted.vault.writes(), 1);

    let unlock = owner.prepare_unlock().unwrap();
    owner.confirm(&unlock.id, &interrupted.vault).unwrap();
    assert!(owner.list(&interrupted.vault).unwrap().is_empty());
}

#[test]
fn empty_provisioning_tokens_are_consumed_before_new_vault_access() {
    let interrupted = interrupted_empty("tokens");
    let mut owner = Owner::acquire(interrupted.root.0.clone()).unwrap();
    let preview = owner
        .inspect_empty_provisioning(&interrupted.vault)
        .unwrap();
    let opens = interrupted.vault.opens();
    let reads = interrupted.vault.reads();

    assert_error(
        owner.finish_empty_provisioning("wrong", &interrupted.vault),
        Error::Stale,
    );
    assert_eq!(interrupted.vault.opens(), opens);
    assert_eq!(interrupted.vault.reads(), reads);

    owner
        .finish_empty_provisioning(&preview.id, &interrupted.vault)
        .unwrap();
    let opens = interrupted.vault.opens();
    let reads = interrupted.vault.reads();
    assert_error(
        owner.finish_empty_provisioning(&preview.id, &interrupted.vault),
        Error::Stale,
    );
    assert_eq!(interrupted.vault.opens(), opens);
    assert_eq!(interrupted.vault.reads(), reads);
}

#[test]
fn replacement_inspection_drops_old_plan_even_when_new_inspection_refuses() {
    let interrupted = interrupted_empty("replacement");
    let mut owner = Owner::acquire(interrupted.root.0.clone()).unwrap();
    let first = owner
        .inspect_empty_provisioning(&interrupted.vault)
        .unwrap();
    interrupted.vault.remove(&interrupted.workspace_id);
    assert!(
        owner
            .inspect_empty_provisioning(&interrupted.vault)
            .is_err()
    );
    let opens = interrupted.vault.opens();
    let reads = interrupted.vault.reads();

    assert_error(
        owner.finish_empty_provisioning(&first.id, &interrupted.vault),
        Error::Stale,
    );
    assert_eq!(interrupted.vault.opens(), opens);
    assert_eq!(interrupted.vault.reads(), reads);
    assert!(!database_path(&interrupted.root, &interrupted.workspace_id).exists());
}

#[test]
fn empty_provisioning_refuses_missing_or_malformed_existing_key() {
    for case in ["missing", "malformed"] {
        let interrupted = interrupted_empty(case);
        match case {
            "missing" => interrupted.vault.remove(&interrupted.workspace_id),
            "malformed" => interrupted.vault.malformed(&interrupted.workspace_id),
            _ => unreachable!(),
        }
        let mut owner = Owner::acquire(interrupted.root.0.clone()).unwrap();
        let writes = interrupted.vault.writes();
        assert!(
            owner
                .inspect_empty_provisioning(&interrupted.vault)
                .is_err()
        );
        assert_eq!(interrupted.vault.writes(), writes);
        assert!(!database_path(&interrupted.root, &interrupted.workspace_id).exists());
    }
}

#[test]
fn changed_key_after_inspection_refuses_completion_without_initialization() {
    let interrupted = interrupted_empty("changed-after-inspection");
    let mut owner = Owner::acquire(interrupted.root.0.clone()).unwrap();
    let preview = owner
        .inspect_empty_provisioning(&interrupted.vault)
        .unwrap();
    interrupted.vault.corrupt(&interrupted.workspace_id);
    let writes = interrupted.vault.writes();

    // Without a database, a fresh inspection cannot authenticate a different
    // well-formed envelope. Completion revalidates its retained inspection.
    assert!(
        owner
            .finish_empty_provisioning(&preview.id, &interrupted.vault)
            .is_err()
    );
    assert_eq!(interrupted.vault.writes(), writes);
    assert!(!database_path(&interrupted.root, &interrupted.workspace_id).exists());

    let opens = interrupted.vault.opens();
    let reads = interrupted.vault.reads();
    assert_error(
        owner.finish_empty_provisioning(&preview.id, &interrupted.vault),
        Error::Stale,
    );
    assert_eq!(interrupted.vault.opens(), opens);
    assert_eq!(interrupted.vault.reads(), reads);
}

#[test]
fn empty_provisioning_refuses_nonempty_or_non_create_empty_evidence_before_vault() {
    for case in ["extra", "zero", "source", "ready"] {
        let interrupted = if case == "source" {
            let root = TestRoot::new("source-operation");
            let vault = InterruptedVault::default();
            let mut owner = Owner::acquire(root.0.clone()).unwrap();
            let create = owner.prepare_create(&selected_source()).unwrap();
            vault.arm_uncertain_write_once();
            assert_error(owner.confirm(&create.id, &vault), Error::WriteUncertain);
            drop(owner);
            Interrupted {
                root,
                vault,
                workspace_id: create.workspace_id,
            }
        } else if case == "ready" {
            let root = TestRoot::new("ready-operation");
            let vault = InterruptedVault::default();
            let mut owner = Owner::acquire(root.0.clone()).unwrap();
            let create = owner.prepare_create_empty().unwrap();
            owner.confirm(&create.id, &vault).unwrap();
            owner.lock();
            drop(owner);
            Interrupted {
                root,
                vault,
                workspace_id: create.workspace_id,
            }
        } else {
            interrupted_empty(case)
        };
        if case == "extra" {
            fs::write(
                interrupted
                    .root
                    .encrypted_root()
                    .join(&interrupted.workspace_id)
                    .join("unexpected"),
                b"x",
            )
            .unwrap();
        }
        if case == "zero" {
            fs::write(
                database_path(&interrupted.root, &interrupted.workspace_id),
                [],
            )
            .unwrap();
        }
        let mut owner = Owner::acquire(interrupted.root.0.clone()).unwrap();
        let opens = interrupted.vault.opens();
        assert!(
            owner
                .inspect_empty_provisioning(&interrupted.vault)
                .is_err()
        );
        assert_eq!(interrupted.vault.opens(), opens);
        assert_eq!(interrupted.vault.writes(), 1);
    }
}

#[test]
fn post_initialization_faults_recover_with_existing_metadata_path_without_replay() {
    for fault in [
        Fault::BeforeReconcileReadyWrite,
        Fault::BeforeReconcileReadySync,
        Fault::BeforeReconcileReadyReadback,
        Fault::BeforeCompletionWrite,
        Fault::BeforeCompletionSync,
        Fault::BeforeCompletionReadback,
    ] {
        let interrupted = interrupted_empty("finish-fault");
        let mut owner = Owner::acquire(interrupted.root.0.clone()).unwrap();
        let preview = owner
            .inspect_empty_provisioning(&interrupted.vault)
            .unwrap();
        owner.inject_fault(fault);
        assert_error(
            owner.finish_empty_provisioning(&preview.id, &interrupted.vault),
            Error::WriteUncertain,
        );
        assert!(!owner.is_unlocked());
        assert!(database_path(&interrupted.root, &interrupted.workspace_id).is_file());
        assert_eq!(interrupted.vault.writes(), 1);
        drop(owner);

        let mut fresh = Owner::acquire(interrupted.root.0.clone()).unwrap();
        assert!(
            fresh
                .inspect_empty_provisioning(&interrupted.vault)
                .is_err()
        );
        let recovery = fresh.inspect_recovery(&interrupted.vault).unwrap();
        fresh
            .confirm_recovery(&recovery.id, &interrupted.vault)
            .unwrap();
        assert_eq!(interrupted.vault.writes(), 1);
    }
}

#[test]
fn empty_provisioning_refuses_damaged_or_foreign_metadata_before_vault_access() {
    let prefix = b"rangoon.workspace-operation.v1\0";
    let phase = prefix.len();
    let sequence = phase + 2;
    let workspace = sequence + 8;
    let attempt = workspace + 64;

    for case in [
        "truncated-pending",
        "unknown-operation",
        "observed-pending",
        "foreign-workspace",
        "attempt-is-workspace",
        "sequence-two",
        "terminal-present",
        "terminal-damaged",
        "ready-binding",
        "corrupt-binding",
    ] {
        let interrupted = interrupted_empty(case);
        let pending = interrupted
            .root
            .0
            .join("encrypted-workspace.operation.pending");
        let terminal = interrupted
            .root
            .0
            .join("encrypted-workspace.operation.terminal");
        let binding = interrupted.root.binding();
        match case {
            "truncated-pending" => fs::write(&pending, b"truncated").unwrap(),
            "unknown-operation" => {
                let mut bytes = fs::read(&pending).unwrap();
                bytes[phase + 1] = u8::MAX;
                fs::write(&pending, bytes).unwrap();
            }
            "observed-pending" => {
                let mut bytes = fs::read(&pending).unwrap();
                bytes[phase] = 1;
                fs::write(&pending, bytes).unwrap();
            }
            "foreign-workspace" => {
                let mut bytes = fs::read(&pending).unwrap();
                bytes[workspace] = if bytes[workspace] == b'a' { b'b' } else { b'a' };
                fs::write(&pending, bytes).unwrap();
            }
            "attempt-is-workspace" => {
                let mut bytes = fs::read(&pending).unwrap();
                let identity = bytes[workspace..workspace + 64].to_vec();
                bytes[attempt..attempt + 64].copy_from_slice(&identity);
                fs::write(&pending, bytes).unwrap();
            }
            "sequence-two" => {
                let mut bytes = fs::read(&pending).unwrap();
                bytes[sequence + 7] = 2;
                fs::write(&pending, bytes).unwrap();
            }
            "terminal-present" => fs::write(&terminal, fs::read(&pending).unwrap()).unwrap(),
            "terminal-damaged" => fs::write(&terminal, b"damaged").unwrap(),
            "ready-binding" => {
                let mut bytes = fs::read(&binding).unwrap();
                bytes[29] = 1;
                fs::write(&binding, bytes).unwrap();
            }
            "corrupt-binding" => {
                let mut bytes = fs::read(&binding).unwrap();
                bytes[0] ^= 1;
                fs::write(&binding, bytes).unwrap();
            }
            _ => unreachable!(),
        }
        let expected_pending = fs::read(&pending).unwrap();
        let expected_binding = fs::read(&binding).unwrap();
        let expected_terminal = terminal.exists().then(|| fs::read(&terminal).unwrap());
        let mut owner = Owner::acquire(interrupted.root.0.clone()).unwrap();
        let opens = interrupted.vault.opens();
        let reads = interrupted.vault.reads();

        assert!(
            owner
                .inspect_empty_provisioning(&interrupted.vault)
                .is_err()
        );
        assert_eq!(interrupted.vault.opens(), opens);
        assert_eq!(interrupted.vault.reads(), reads);
        assert_eq!(interrupted.vault.writes(), 1);
        assert_eq!(fs::read(&pending).unwrap(), expected_pending);
        assert_eq!(fs::read(&binding).unwrap(), expected_binding);
        assert_eq!(terminal.exists(), expected_terminal.is_some());
        if let Some(expected) = expected_terminal {
            assert_eq!(fs::read(terminal).unwrap(), expected);
        }
    }
}

#[test]
fn empty_provisioning_lock_and_reacquisition_invalidate_retained_finish_tokens() {
    let interrupted = interrupted_empty("lock-stale");
    let binding = fs::read(interrupted.root.binding()).unwrap();
    let pending = fs::read(
        interrupted
            .root
            .0
            .join("encrypted-workspace.operation.pending"),
    )
    .unwrap();
    let envelope = interrupted.vault.snapshot(&interrupted.workspace_id);
    let mut owner = Owner::acquire(interrupted.root.0.clone()).unwrap();
    let preview = owner
        .inspect_empty_provisioning(&interrupted.vault)
        .unwrap();
    owner.lock();
    let opens = interrupted.vault.opens();
    let reads = interrupted.vault.reads();
    assert_error(
        owner.finish_empty_provisioning(&preview.id, &interrupted.vault),
        Error::Stale,
    );
    assert_eq!(interrupted.vault.opens(), opens);
    assert_eq!(interrupted.vault.reads(), reads);
    drop(owner);

    let mut fresh = Owner::acquire(interrupted.root.0.clone()).unwrap();
    let opens = interrupted.vault.opens();
    let reads = interrupted.vault.reads();
    assert_error(
        fresh.finish_empty_provisioning(&preview.id, &interrupted.vault),
        Error::Stale,
    );
    assert_eq!(interrupted.vault.opens(), opens);
    assert_eq!(interrupted.vault.reads(), reads);
    assert_eq!(fs::read(interrupted.root.binding()).unwrap(), binding);
    assert_eq!(
        fs::read(
            interrupted
                .root
                .0
                .join("encrypted-workspace.operation.pending"),
        )
        .unwrap(),
        pending
    );
    assert!(
        interrupted
            .vault
            .matches(&interrupted.workspace_id, &envelope)
    );
    assert!(fresh.inspect_empty_provisioning(&interrupted.vault).is_ok());
    assert_eq!(interrupted.vault.writes(), 1);
}

#[test]
fn empty_provisioning_finish_refuses_replaced_or_nonempty_destination_before_vault() {
    for case in [
        "empty-database",
        "partial-database",
        "sidecar",
        "extra",
        "replaced-directory",
        "replaced-control",
        "replaced-binding",
    ] {
        let interrupted = interrupted_empty(case);
        let pending = interrupted
            .root
            .0
            .join("encrypted-workspace.operation.pending");
        let pending_before = fs::read(&pending).unwrap();
        let envelope = interrupted.vault.snapshot(&interrupted.workspace_id);
        let mut owner = Owner::acquire(interrupted.root.0.clone()).unwrap();
        let preview = owner
            .inspect_empty_provisioning(&interrupted.vault)
            .unwrap();
        let destination = interrupted
            .root
            .encrypted_root()
            .join(&interrupted.workspace_id);
        let mut preserved = None;
        match case {
            "empty-database" => {
                let path = database_path(&interrupted.root, &interrupted.workspace_id);
                fs::write(&path, []).unwrap();
                preserved = Some(path);
            }
            "partial-database" => {
                let path = database_path(&interrupted.root, &interrupted.workspace_id);
                fs::write(&path, b"partial").unwrap();
                preserved = Some(path);
            }
            "sidecar" => {
                let path = destination.join("workspace.sqlite3-wal");
                fs::write(&path, b"sidecar").unwrap();
                preserved = Some(path);
            }
            "extra" => {
                let path = destination.join("unexpected");
                fs::write(&path, b"extra").unwrap();
                preserved = Some(path);
            }
            "replaced-directory" => {
                fs::rename(&destination, interrupted.root.0.join("retained-directory")).unwrap();
                fs::create_dir(&destination).unwrap();
            }
            "replaced-control" => {
                let control = interrupted.root.0.join(CONTROL);
                fs::rename(&control, interrupted.root.0.join("retained-control")).unwrap();
                fs::write(control, []).unwrap();
            }
            "replaced-binding" => {
                let binding = interrupted.root.binding();
                fs::rename(&binding, interrupted.root.0.join("retained-binding")).unwrap();
                fs::write(binding, b"replacement").unwrap();
            }
            _ => unreachable!(),
        }
        let preserved_bytes = preserved.as_ref().map(|path| fs::read(path).unwrap());
        let opens = interrupted.vault.opens();
        let reads = interrupted.vault.reads();

        assert!(
            owner
                .finish_empty_provisioning(&preview.id, &interrupted.vault)
                .is_err()
        );
        assert_eq!(interrupted.vault.opens(), opens);
        assert_eq!(interrupted.vault.reads(), reads);
        assert_eq!(interrupted.vault.writes(), 1);
        assert_eq!(fs::read(&pending).unwrap(), pending_before);
        assert!(
            interrupted
                .vault
                .matches(&interrupted.workspace_id, &envelope)
        );
        if let (Some(path), Some(expected)) = (preserved, preserved_bytes) {
            assert_eq!(fs::read(path).unwrap(), expected);
        }
    }
}

#[test]
fn post_initialization_sidecars_or_extras_refuse_before_ready_or_terminal() {
    for name in [
        "unexpected",
        "workspace.sqlite3-journal",
        "workspace.sqlite3-wal",
        "workspace.sqlite3-shm",
    ] {
        let interrupted = interrupted_empty(name);
        let pending = interrupted
            .root
            .0
            .join("encrypted-workspace.operation.pending");
        let pending_before = fs::read(&pending).unwrap();
        let envelope = interrupted.vault.snapshot(&interrupted.workspace_id);
        let mut owner = Owner::acquire(interrupted.root.0.clone()).unwrap();
        let preview = owner
            .inspect_empty_provisioning(&interrupted.vault)
            .unwrap();
        let injected = interrupted
            .root
            .encrypted_root()
            .join(&interrupted.workspace_id)
            .join(name);

        // The first finish read revalidates before initialization. This hook
        // runs on the second one, after the actual encrypted database exists.
        interrupted
            .vault
            .arm_after_reads(2, injected.clone(), b"injected");
        assert_error(
            owner.finish_empty_provisioning(&preview.id, &interrupted.vault),
            Error::WriteUncertain,
        );

        assert_eq!(
            fs::read(interrupted.root.binding()).unwrap(),
            provisioning_binding(&interrupted.workspace_id)
        );
        assert_eq!(fs::read(&pending).unwrap(), pending_before);
        assert!(
            !interrupted
                .root
                .0
                .join("encrypted-workspace.operation.terminal")
                .exists()
        );
        assert!(database_path(&interrupted.root, &interrupted.workspace_id).is_file());
        assert_eq!(fs::read(injected).unwrap(), b"injected");
        assert_eq!(interrupted.vault.writes(), 1);
        assert!(
            interrupted
                .vault
                .matches(&interrupted.workspace_id, &envelope)
        );
    }
}
#[test]
fn changed_valid_pending_after_inspection_refuses_before_vault_and_preserves_bytes() {
    let interrupted = interrupted_empty("pending-after-inspect");
    let mut owner = Owner::acquire(interrupted.root.0.clone()).unwrap();
    let preview = owner
        .inspect_empty_provisioning(&interrupted.vault)
        .unwrap();
    let envelope = interrupted.vault.snapshot(&interrupted.workspace_id);
    let pending_path = interrupted
        .root
        .0
        .join("encrypted-workspace.operation.pending");
    let mut changed = fs::read(&pending_path).unwrap();
    let prefix = b"rangoon.workspace-operation.v1\0";
    assert!(changed.starts_with(prefix));
    let attempt_start = prefix.len() + 74;
    let workspace_start = prefix.len() + 10;
    // Change only one lowercase hexadecimal attempt byte. The alternate is
    // distinct from both old attempt and workspace, retaining a valid record.
    changed[attempt_start] = b"abc"
        .iter()
        .copied()
        .find(|byte| *byte != changed[attempt_start] && *byte != changed[workspace_start])
        .unwrap();
    fs::write(&pending_path, &changed).unwrap();
    assert!(owner.read_operation().unwrap().is_some());
    let opens = interrupted.vault.opens();
    let reads = interrupted.vault.reads();
    assert_error(
        owner.finish_empty_provisioning(&preview.id, &interrupted.vault),
        Error::Stale,
    );
    assert_eq!(interrupted.vault.opens(), opens);
    assert_eq!(interrupted.vault.reads(), reads);
    assert_eq!(interrupted.vault.writes(), 1);
    assert!(
        interrupted
            .vault
            .matches(&interrupted.workspace_id, &envelope)
    );
    assert!(fs::read(&pending_path).unwrap() == changed);
    assert!(!database_path(&interrupted.root, &interrupted.workspace_id).exists());
    assert!(
        !interrupted
            .root
            .0
            .join("encrypted-workspace.operation.terminal")
            .exists()
    );
    assert_error(
        owner.finish_empty_provisioning(&preview.id, &interrupted.vault),
        Error::Stale,
    );
    assert_eq!(interrupted.vault.opens(), opens);
    assert_eq!(interrupted.vault.reads(), reads);
}
