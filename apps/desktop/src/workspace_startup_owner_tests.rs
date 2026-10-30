//! Existing-only Owner acquisition tests. No vault or workspace data is used.
use super::*;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct TestRoot(PathBuf);

impl TestRoot {
    fn new(label: &str) -> Self {
        #[cfg(unix)]
        let parent = PathBuf::from("/tmp").canonicalize().unwrap();
        #[cfg(not(unix))]
        let parent = std::env::temp_dir().canonicalize().unwrap();
        let root = parent.join(format!(
            "rangoon-owner-{label}-{}-{}-{}",
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

    fn control(&self) -> PathBuf {
        self.0.join("encrypted-workspace.lock")
    }

    fn managed(&self) -> PathBuf {
        self.0.join("managed")
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_error<T>(actual: Result<T, Error>, expected: Error) {
    match actual {
        Ok(_) => panic!("expected {expected:?}"),
        Err(error) => assert_eq!(error, expected),
    }
}

fn assert_regular_root_bytes(root: &Path, expected: &[(&str, &[u8])]) {
    let actual = fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            let metadata = fs::symlink_metadata(entry.path()).unwrap();
            assert!(metadata.is_file(), "{name} must stay a regular file");
            (name, fs::read(entry.path()).unwrap())
        })
        .collect::<BTreeMap<_, _>>();
    let expected = expected
        .iter()
        .map(|(name, bytes)| ((*name).to_owned(), bytes.to_vec()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual, expected);
}

fn assert_control_only(root: &Path) {
    assert_regular_root_bytes(root, &[("encrypted-workspace.lock", b"")]);
    for absent in [
        "encrypted-workspace.binding",
        "encrypted-workspaces",
        "workspace.sqlite3",
        "operation",
    ] {
        assert!(!root.join(absent).exists(), "{absent} must not be created");
    }
}

#[test]
fn acquire_existing_missing_root_or_control_never_creates_a_path() {
    let fixture = TestRoot::new("missing");
    let missing_root = fixture.managed();
    assert!(!missing_root.exists());
    assert_error(
        Owner::acquire_existing(missing_root.clone()),
        Error::Unavailable,
    );
    assert!(!missing_root.exists());
    assert_regular_root_bytes(&fixture.0, &[]);

    fs::create_dir(&missing_root).unwrap();
    assert_error(
        Owner::acquire_existing(missing_root.clone()),
        Error::Unavailable,
    );
    assert_regular_root_bytes(&missing_root, &[]);
}

#[test]
fn acquire_existing_locks_only_an_existing_zero_byte_regular_control_and_drop_releases_it() {
    let root = TestRoot::new("valid");
    fs::write(root.control(), b"").unwrap();
    assert_control_only(&root.0);

    let first = Owner::acquire_existing(root.0.clone()).unwrap();
    assert!(!first.is_unlocked());
    assert_control_only(&root.0);
    assert_error(Owner::acquire_existing(root.0.clone()), Error::Busy);
    assert_control_only(&root.0);

    drop(first);
    let second = Owner::acquire_existing(root.0.clone()).unwrap();
    assert!(!second.is_unlocked());
    drop(second);
    assert_control_only(&root.0);
}

#[test]
fn acquire_existing_rejects_nonzero_control_without_changing_its_bytes() {
    let root = TestRoot::new("nonzero");
    let bytes = b"preexisting nonempty control";
    fs::write(root.control(), bytes).unwrap();

    assert_error(
        Owner::acquire_existing(root.0.clone()),
        Error::InvalidBinding,
    );
    assert_regular_root_bytes(&root.0, &[("encrypted-workspace.lock", bytes)]);
}

#[test]
fn acquire_existing_detects_replaced_root_before_any_workspace_setup() {
    let fixture = TestRoot::new("replaced-root");
    let managed = fixture.managed();
    fs::create_dir(&managed).unwrap();
    fs::write(managed.join("encrypted-workspace.lock"), b"").unwrap();
    let mut owner = Owner::acquire_existing(managed.clone()).unwrap();

    let retained = fixture.0.join("retained-root");
    fs::rename(&managed, &retained).unwrap();
    fs::create_dir(&managed).unwrap();
    fs::write(managed.join("encrypted-workspace.lock"), b"").unwrap();

    assert_error(owner.prepare_create_empty(), Error::Unavailable);
    assert_control_only(&managed);
    assert_control_only(&retained);
    drop(owner);
}

#[test]
fn acquire_existing_detects_replaced_control_before_any_workspace_setup() {
    let root = TestRoot::new("replaced-control");
    fs::write(root.control(), b"").unwrap();
    let mut owner = Owner::acquire_existing(root.0.clone()).unwrap();

    let retained = root.0.join("retained-control");
    fs::rename(root.control(), &retained).unwrap();
    fs::write(root.control(), b"").unwrap();

    assert_error(owner.prepare_create_empty(), Error::InvalidBinding);
    assert_regular_root_bytes(
        &root.0,
        &[("encrypted-workspace.lock", b""), ("retained-control", b"")],
    );
    assert!(!root.0.join("encrypted-workspace.binding").exists());
    assert!(!root.0.join("encrypted-workspaces").exists());
    drop(owner);
}

#[cfg(unix)]
#[test]
fn acquire_existing_rejects_linked_directory_and_special_controls_without_repair() {
    use std::os::unix::{
        fs::{FileTypeExt, symlink},
        net::UnixListener,
    };

    let root = TestRoot::new("symlink");
    let target = root.0.join("target");
    fs::write(&target, b"").unwrap();
    symlink(&target, root.control()).unwrap();
    assert_error(
        Owner::acquire_existing(root.0.clone()),
        Error::InvalidBinding,
    );
    assert!(
        fs::symlink_metadata(root.control())
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(&target).unwrap(), b"");

    let root = TestRoot::new("hardlink");
    fs::write(root.control(), b"").unwrap();
    let alias = root.0.join("control-alias");
    fs::hard_link(root.control(), &alias).unwrap();
    assert_error(
        Owner::acquire_existing(root.0.clone()),
        Error::InvalidBinding,
    );
    assert_eq!(fs::read(root.control()).unwrap(), b"");
    assert_eq!(fs::read(alias).unwrap(), b"");

    let root = TestRoot::new("directory");
    fs::create_dir(root.control()).unwrap();
    assert_error(
        Owner::acquire_existing(root.0.clone()),
        Error::InvalidBinding,
    );
    assert!(fs::symlink_metadata(root.control()).unwrap().is_dir());

    let root = TestRoot::new("socket");
    let _listener = UnixListener::bind(root.control()).unwrap();
    assert_error(
        Owner::acquire_existing(root.0.clone()),
        Error::InvalidBinding,
    );
    assert!(
        fs::symlink_metadata(root.control())
            .unwrap()
            .file_type()
            .is_socket()
    );
}
