use super::*;
use std::cell::{Cell, RefCell};

#[cfg(feature = "encrypted-workspace")]
const HEADER: &[u8] = b"rangoon.workspace-key.v1\0";

#[cfg(feature = "encrypted-workspace")]
enum ReadAction {
    Value(Option<Vec<u8>>),
    Error(Error),
}

struct FakeStore {
    identity: WorkspaceId,
    #[cfg(feature = "encrypted-workspace")]
    reads: RefCell<Vec<ReadAction>>,
    writes: RefCell<Vec<Vec<u8>>>,
    #[cfg(feature = "encrypted-workspace")]
    write_error: Option<Error>,
    #[cfg(feature = "encrypted-workspace")]
    reflect_write: bool,
    read_calls: Cell<usize>,
}

impl FakeStore {
    #[cfg(feature = "encrypted-workspace")]
    fn new(identity: WorkspaceId, reads: Vec<ReadAction>) -> Self {
        Self {
            identity,
            reads: RefCell::new(reads),
            writes: RefCell::new(Vec::new()),
            write_error: None,
            reflect_write: false,
            read_calls: Cell::new(0),
        }
    }

    #[cfg(not(feature = "encrypted-workspace"))]
    fn new(identity: WorkspaceId) -> Self {
        Self {
            identity,
            writes: RefCell::new(Vec::new()),
            read_calls: Cell::new(0),
        }
    }

    #[cfg(feature = "encrypted-workspace")]
    fn absent_then_reflect(identity: WorkspaceId) -> Self {
        let mut store = Self::new(identity, vec![ReadAction::Value(None)]);
        store.reflect_write = true;
        store
    }

    #[cfg(feature = "encrypted-workspace")]
    fn with_write_error(mut self, error: Error) -> Self {
        self.write_error = Some(error);
        self
    }

    fn reads(&self) -> usize {
        self.read_calls.get()
    }

    fn writes(&self) -> Vec<Vec<u8>> {
        self.writes.borrow().clone()
    }
}

impl Store for FakeStore {
    fn identity(&self) -> &WorkspaceId {
        &self.identity
    }

    fn read(&self) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>, Error> {
        self.read_calls.set(self.read_calls.get() + 1);
        #[cfg(feature = "encrypted-workspace")]
        {
            let action = {
                let mut reads = self.reads.borrow_mut();
                (!reads.is_empty()).then(|| reads.remove(0))
            };
            match action {
                Some(ReadAction::Value(value)) => Ok(value.map(zeroize::Zeroizing::new)),
                Some(ReadAction::Error(error)) => Err(error),
                None if self.reflect_write => Ok(self
                    .writes
                    .borrow()
                    .last()
                    .cloned()
                    .map(zeroize::Zeroizing::new)),
                None => panic!("unexpected fake store read"),
            }
        }
        #[cfg(not(feature = "encrypted-workspace"))]
        panic!("default backend must reject before a store read")
    }

    fn write(&self, envelope: &[u8]) -> Result<(), Error> {
        self.writes.borrow_mut().push(envelope.to_vec());
        #[cfg(feature = "encrypted-workspace")]
        match self.write_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
        #[cfg(not(feature = "encrypted-workspace"))]
        Ok(())
    }
}

struct FakeEntropy {
    material: Vec<u8>,
    offset: Cell<usize>,
    calls: Cell<usize>,
    fail_on_call: Option<usize>,
}

impl FakeEntropy {
    fn new(material: Vec<u8>) -> Self {
        Self {
            material,
            offset: Cell::new(0),
            calls: Cell::new(0),
            fail_on_call: None,
        }
    }

    fn failing_on(mut self, call: usize) -> Self {
        self.fail_on_call = Some(call);
        self
    }

    fn calls(&self) -> usize {
        self.calls.get()
    }
}

impl Entropy for FakeEntropy {
    fn fill(&self, bytes: &mut [u8]) -> Result<(), Error> {
        let call = self.calls.get() + 1;
        self.calls.set(call);
        if self.fail_on_call == Some(call) {
            bytes.fill(0x5a);
            return Err(Error::Unavailable);
        }
        let start = self.offset.get();
        let end = start + bytes.len();
        if end > self.material.len() {
            return Err(Error::Unavailable);
        }
        bytes.copy_from_slice(&self.material[start..end]);
        self.offset.set(end);
        Ok(())
    }
}

fn assert_error<T>(result: Result<T, Error>, expected: Error) {
    match result {
        Ok(_) => panic!("expected {expected:?}"),
        Err(actual) => assert_eq!(actual, expected),
    }
}

fn id(byte: u8) -> WorkspaceId {
    WorkspaceId::parse(&format!("{byte:02x}").repeat(32)).unwrap()
}

#[cfg(feature = "encrypted-workspace")]
fn id_bytes(id: &WorkspaceId) -> Vec<u8> {
    let hex = id.as_hex();
    (0..32)
        .map(|index| u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap())
        .collect()
}

#[cfg(feature = "encrypted-workspace")]
fn envelope(id: &WorkspaceId, revision: u8, key: u8) -> Vec<u8> {
    let mut bytes = HEADER.to_vec();
    bytes.extend(id_bytes(id));
    bytes.extend([revision; 32]);
    bytes.extend([key; 32]);
    bytes
}

fn entropy() -> FakeEntropy {
    let mut bytes = vec![0x31; 32];
    bytes.extend([0x62; 32]);
    FakeEntropy::new(bytes)
}

#[cfg(feature = "encrypted-workspace")]
fn unique_directory(label: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().canonicalize().unwrap().join(format!(
        "rangoon-workspace-key-{label}-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ))
}

#[test]
fn workspace_id_parsing_is_strict_and_rejects_zero() {
    let valid = id(0xab);
    assert_eq!(valid.as_hex(), "ab".repeat(32));
    for value in [
        "0".repeat(64),
        "AB".repeat(32),
        "ab".repeat(31),
        "ab".repeat(33),
        "g0".repeat(32),
        "ab ".repeat(32),
    ] {
        assert_error(WorkspaceId::parse(&value), Error::InvalidRequest);
    }
}

#[test]
fn generated_workspace_ids_are_nonzero_and_distinct() {
    let first = WorkspaceId::generate().unwrap();
    let second = WorkspaceId::generate().unwrap();
    assert_ne!(first.as_hex(), "0".repeat(64));
    assert_ne!(second.as_hex(), "0".repeat(64));
    assert!(first != second);
}

#[test]
fn identity_entropy_failure_returns_closed_error_without_identity() {
    let entropy = FakeEntropy::new(vec![0x41; 32]).failing_on(1);
    assert_error(
        generate_identity_with_entropy(&entropy),
        Error::EntropyUnavailable,
    );
    assert_eq!(entropy.calls(), 1);
}

#[test]
fn namespace_names_are_exact_and_invalid_identifiers_never_open_a_slot() {
    let workspace = id(0x22);
    assert_eq!(
        slot_names("rangoon.desktop-1", &workspace).unwrap(),
        (
            "rangoon.desktop-1.encrypted-workspace-key.v1".to_owned(),
            format!("workspace-{}", workspace.as_hex()),
        )
    );
    let too_long = "a".repeat(129);
    for identifier in [
        "",
        "bad space",
        "bad/slash",
        "bad_underscore",
        too_long.as_str(),
    ] {
        assert_error(slot_names(identifier, &workspace), Error::InvalidRequest);
    }
}

#[cfg(not(feature = "encrypted-workspace"))]
#[test]
fn default_build_rejects_before_fake_store_or_entropy_side_effects() {
    let store = FakeStore::new(id(0x10));
    let entropy = entropy();
    assert_error(
        create_with_entropy(&store, &entropy),
        Error::BackendUnavailable,
    );
    assert_eq!(store.reads(), 0);
    assert!(store.writes().is_empty());
    assert_eq!(entropy.calls(), 0);
    assert_error(read_key(&store), Error::BackendUnavailable);
    assert_eq!(store.reads(), 0);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn create_writes_exact_literal_envelope_and_reopens_without_entropy() {
    let workspace = id(0x11);
    let store = FakeStore::absent_then_reflect(workspace.clone());
    let provision_entropy = entropy();
    let retained = create_with_entropy(&store, &provision_entropy).unwrap();
    let writes = store.writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].len(), 121);
    assert_eq!(writes[0], envelope(&workspace, 0x31, 0x62));
    assert_eq!(store.reads(), 2);
    assert_eq!(provision_entropy.calls(), 2);

    let reopened = FakeStore::new(workspace, vec![ReadAction::Value(Some(writes[0].clone()))]);
    let fresh_entropy = entropy();
    assert!(read_key(&reopened).is_ok());
    assert_eq!(reopened.reads(), 1);
    assert!(reopened.writes().is_empty());
    assert_eq!(fresh_entropy.calls(), 0);
    drop(retained);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn existing_valid_or_corrupt_slot_refuses_create_without_entropy_or_write() {
    let workspace = id(0x12);
    for record in [envelope(&workspace, 0x13, 0x14), b"corrupt".to_vec()] {
        let store = FakeStore::new(workspace.clone(), vec![ReadAction::Value(Some(record))]);
        let entropy = entropy();
        assert_error(create_with_entropy(&store, &entropy), Error::AlreadyExists);
        assert_eq!(store.reads(), 1);
        assert!(store.writes().is_empty());
        assert_eq!(entropy.calls(), 0);
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn partial_creation_entropy_failure_never_writes_a_record() {
    let store = FakeStore::new(id(0x14), vec![ReadAction::Value(None)]);
    let entropy = FakeEntropy::new(vec![0x47; 64]).failing_on(2);
    assert_error(
        create_with_entropy(&store, &entropy),
        Error::EntropyUnavailable,
    );
    assert_eq!(store.reads(), 1);
    assert!(store.writes().is_empty());
    assert_eq!(entropy.calls(), 2);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn unreadable_and_missing_slots_have_distinct_closed_errors() {
    let workspace = id(0x15);
    let unreadable = FakeStore::new(
        workspace.clone(),
        vec![ReadAction::Error(Error::InvalidRequest)],
    );
    let entropy = entropy();
    assert_error(
        create_with_entropy(&unreadable, &entropy),
        Error::Unavailable,
    );
    assert_eq!(entropy.calls(), 0);
    assert!(unreadable.writes().is_empty());

    let missing = FakeStore::new(workspace.clone(), vec![ReadAction::Value(None)]);
    assert_error(read_key(&missing), Error::Missing);
    let failed_read = FakeStore::new(workspace, vec![ReadAction::Error(Error::InvalidRequest)]);
    assert_error(read_key(&failed_read), Error::Unavailable);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn any_post_write_ambiguity_is_uncertain_without_retry_or_delete() {
    let workspace = id(0x16);
    let changed = envelope(&workspace, 0x77, 0x88);
    let cases = [
        (
            FakeStore::absent_then_reflect(workspace.clone())
                .with_write_error(Error::InvalidRequest),
            "committed-write-error",
        ),
        (
            FakeStore::new(
                workspace.clone(),
                vec![
                    ReadAction::Value(None),
                    ReadAction::Error(Error::InvalidRequest),
                ],
            ),
            "readback-error",
        ),
        (
            FakeStore::new(
                workspace.clone(),
                vec![ReadAction::Value(None), ReadAction::Value(None)],
            ),
            "missing-readback",
        ),
        (
            FakeStore::new(
                workspace.clone(),
                vec![ReadAction::Value(None), ReadAction::Value(Some(changed))],
            ),
            "different-readback",
        ),
    ];
    for (store, _name) in cases {
        let entropy = entropy();
        assert_error(create_with_entropy(&store, &entropy), Error::WriteUncertain);
        assert_eq!(store.writes().len(), 1);
        assert_eq!(store.reads(), 2);
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn strict_read_rejects_header_length_trailing_identity_revision_and_key_faults() {
    let workspace = id(0x20);
    let valid = envelope(&workspace, 0x21, 0x22);
    let mut bad_header = valid.clone();
    bad_header[0] ^= 1;
    let mut zero_revision = valid.clone();
    zero_revision[HEADER.len() + 32..HEADER.len() + 64].fill(0);
    let mut zero_key = valid.clone();
    zero_key[HEADER.len() + 64..].fill(0);
    let mut wrong_identity = valid.clone();
    wrong_identity[HEADER.len()] ^= 1;
    let mut trailing = valid.clone();
    trailing.push(0);
    for record in [
        bad_header,
        valid[..120].to_vec(),
        trailing,
        wrong_identity,
        zero_revision,
        zero_key,
    ] {
        let store = FakeStore::new(workspace.clone(), vec![ReadAction::Value(Some(record))]);
        assert_error(read_key(&store), Error::InvalidStoredKey);
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn retained_identity_mismatch_returns_changed_before_vault_or_filesystem_access() {
    let identity = id(0x30);
    let provision = FakeStore::absent_then_reflect(identity.clone());
    let retained = create_with_entropy(&provision, &entropy()).unwrap();
    let other_store = FakeStore::new(id(0x31), vec![]);
    let directory = unique_directory("identity-mismatch");
    assert_error(
        verified_workspace(&other_store, &retained, directory.clone()),
        Error::Changed,
    );
    assert_eq!(other_store.reads(), 0);
    assert!(other_store.writes().is_empty());
    assert!(!directory.exists());
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn verified_workspace_requires_complete_identical_reread_before_handle() {
    let identity = id(0x40);
    let provision = FakeStore::absent_then_reflect(identity.clone());
    let retained = create_with_entropy(&provision, &entropy()).unwrap();
    let record = provision.writes().pop().unwrap();
    let directory = unique_directory("verify");
    for value in [
        None,
        Some(b"corrupt".to_vec()),
        Some(envelope(&identity, 0x41, 0x42)),
    ] {
        let store = FakeStore::new(identity.clone(), vec![ReadAction::Value(value)]);
        assert_error(
            verified_workspace(&store, &retained, directory.clone()),
            Error::Changed,
        );
        assert!(!directory.exists());
    }
    let unreadable = FakeStore::new(
        identity.clone(),
        vec![ReadAction::Error(Error::InvalidRequest)],
    );
    assert_error(
        verified_workspace(&unreadable, &retained, directory.clone()),
        Error::Unavailable,
    );
    assert!(!directory.exists());

    let good = FakeStore::new(
        identity.clone(),
        vec![ReadAction::Value(Some(record.clone()))],
    );
    let workspace = verified_workspace(&good, &retained, directory.clone()).unwrap();
    let report = rangoon_import::analyze(
        "AGENTS.md",
        "\u{feff}# Exact report\r\nKeep café and CRLF bytes.\r\n".as_bytes(),
    )
    .unwrap();
    workspace.save_v1(&report).unwrap();
    drop(workspace);

    let disk = std::fs::read(directory.join("workspace.sqlite3")).unwrap();
    assert!(!disk.starts_with(b"SQLite format 3\0"));

    let mut same_revision_changed_key = record.clone();
    let last = same_revision_changed_key.len() - 1;
    same_revision_changed_key[last] ^= 1;
    let changed_store = FakeStore::new(
        identity.clone(),
        vec![ReadAction::Value(Some(same_revision_changed_key.clone()))],
    );
    assert_error(
        verified_workspace(&changed_store, &retained, directory.clone()),
        Error::Changed,
    );
    assert!(changed_store.writes().is_empty());

    let wrong_key_store = FakeStore::new(
        identity.clone(),
        vec![
            ReadAction::Value(Some(same_revision_changed_key.clone())),
            ReadAction::Value(Some(same_revision_changed_key)),
        ],
    );
    let wrong_retained = read_key(&wrong_key_store).unwrap();
    let wrong_workspace =
        verified_workspace(&wrong_key_store, &wrong_retained, directory.clone()).unwrap();
    assert_eq!(
        wrong_workspace.open(&report.source.id),
        Err(rangoon_store::StoreError::EncryptedStoreInvalid)
    );
    drop(wrong_workspace);
    assert_eq!(
        std::fs::read(directory.join("workspace.sqlite3")).unwrap(),
        disk
    );

    let reopened_store = FakeStore::new(identity, vec![ReadAction::Value(Some(record))]);
    let reopened = verified_workspace(&reopened_store, &retained, directory.clone()).unwrap();
    assert_eq!(reopened.open(&report.source.id).unwrap(), report);
    drop(reopened);
    std::fs::remove_dir_all(directory).unwrap();
}
