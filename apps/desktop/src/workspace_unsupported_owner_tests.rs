//! Unsupported native custody must refuse before touching any workspace state.
use super::*;

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rangoon-unsupported-owner-{}",
            WorkspaceId::generate().unwrap().as_hex()
        ));
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn refuse_both(root: &Path) {
    for acquire in [Owner::acquire, Owner::acquire_existing] {
        assert!(matches!(acquire(root.to_owned()), Err(Error::Unavailable)));
    }
}

#[test]
fn unsupported_owner_never_creates_a_missing_root_or_control() {
    let root = Root::new();
    assert!(!root.0.exists());
    refuse_both(&root.0);
    assert!(!root.0.exists());
    fs::create_dir(&root.0).unwrap();
    refuse_both(&root.0);
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
}

#[test]
fn unsupported_owner_preserves_existing_empty_and_malformed_controls() {
    for control in [b"".as_slice(), b"untrusted nonempty control"] {
        let root = Root::new();
        fs::create_dir(&root.0).unwrap();
        fs::write(root.0.join(CONTROL), control).unwrap();
        fs::write(root.0.join(BINDING), b"untrusted binding bytes").unwrap();
        refuse_both(&root.0);
        assert_eq!(fs::read(root.0.join(CONTROL)).unwrap(), control);
        assert_eq!(
            fs::read(root.0.join(BINDING)).unwrap(),
            b"untrusted binding bytes"
        );
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 2);
        assert!(!root.0.join(PARENT).exists());
    }
}

#[test]
fn unsupported_owner_refuses_non_directory_input_without_effects() {
    let root = Root::new();
    fs::create_dir(&root.0).unwrap();
    let file = root.0.join("sentinel");
    fs::write(&file, b"untouched").unwrap();
    refuse_both(&file);
    assert_eq!(fs::read(file).unwrap(), b"untouched");
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
}
