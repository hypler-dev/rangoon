use super::*;
use std::cell::{Cell, RefCell};

#[derive(Default)]
struct Fake {
    value: RefCell<Option<Vec<u8>>>,
    reads: Cell<usize>,
    writes: Cell<usize>,
    deletes: Cell<usize>,
    fail_read: Cell<bool>,
    uncertain_write: Cell<bool>,
    keep_deleted: Cell<bool>,
}
impl Store for Fake {
    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        self.reads.set(self.reads.get() + 1);
        if self.fail_read.get() {
            return Err(Error::Unavailable);
        }
        Ok(self.value.borrow().clone().map(Zeroizing::new))
    }
    fn write(&self, value: &[u8]) -> Result<(), Error> {
        self.writes.set(self.writes.get() + 1);
        *self.value.borrow_mut() = Some(value.to_vec());
        if self.uncertain_write.get() {
            Err(Error::Unavailable)
        } else {
            Ok(())
        }
    }
    fn delete(&self) -> Result<(), Error> {
        self.deletes.set(self.deletes.get() + 1);
        if !self.keep_deleted.get() {
            *self.value.borrow_mut() = None;
        }
        Ok(())
    }
}
#[test]
fn secret_boundaries_envelope_and_random_revision() {
    for invalid in [
        vec![],
        vec![b'x'; MAX_SECRET + 1],
        b"with space".to_vec(),
        b"key\r\n".to_vec(),
        "é".as_bytes().to_vec(),
        vec![0],
        vec![127],
    ] {
        assert!(matches!(Secret::new(&invalid), Err(Error::InvalidRequest)));
    }
    for valid in [b"x".to_vec(), vec![b'x'; MAX_SECRET], b"!\"\\~".to_vec()] {
        let first = Secret::new(&valid).unwrap();
        let next = Secret::new(&valid).unwrap();
        assert_ne!(first.revision(), next.revision());
        let decoded = Secret::decode(first.0.clone()).unwrap();
        assert_eq!(first.revision(), decoded.revision());
    }
    for bytes in [
        vec![],
        b"plain-secret".to_vec(),
        [MARKER, &[0; 32]].concat(),
        [MARKER, &[0; 32], b"bad\n"].concat(),
    ] {
        assert!(matches!(
            Secret::decode(Zeroizing::new(bytes)),
            Err(Error::InvalidStoredCredential)
        ));
    }
}
#[test]
fn lifecycle_verifies_storage_and_never_retries_uncertain_write() {
    let fake = Fake::default();
    assert!(inspect(&fake).unwrap().is_none());
    let secret = Secret::new(b"synthetic-fixture-not-a-provider-key").unwrap();
    assert_eq!(save(&fake, &secret, None).unwrap(), secret.revision());
    assert_eq!(
        inspect(&fake).unwrap().unwrap().revision(),
        secret.revision()
    );
    remove(&fake, &secret).unwrap();
    assert!(inspect(&fake).unwrap().is_none());
    assert_eq!(fake.writes.get(), 1);
    assert_eq!(fake.deletes.get(), 1);
    fake.uncertain_write.set(true);
    assert_eq!(save(&fake, &secret, None), Err(Error::WriteUncertain));
    assert_eq!(fake.writes.get(), 2);
    assert!(inspect(&fake).unwrap().is_some()); // Write happened despite its error.
}
#[test]
fn missing_changed_and_unavailable_never_delete() {
    let fake = Fake::default();
    let expected = Secret::new(b"synthetic-one").unwrap();
    assert_eq!(remove(&fake, &expected), Err(Error::Changed));
    let changed = Secret::new(b"synthetic-two").unwrap();
    save(&fake, &changed, None).unwrap();
    assert_eq!(remove(&fake, &expected), Err(Error::Changed));
    fake.fail_read.set(true);
    assert_eq!(remove(&fake, &expected), Err(Error::Unavailable));
    assert!(matches!(inspect(&fake), Err(Error::Unavailable)));
    assert_eq!(fake.deletes.get(), 0);
}
#[test]
fn unverifiable_mutations_are_uncertain() {
    let fake = Fake::default();
    let secret = Secret::new(b"synthetic").unwrap();
    fake.uncertain_write.set(true);
    assert_eq!(save(&fake, &secret, None), Err(Error::WriteUncertain));
    fake.fail_read.set(false);
    fake.keep_deleted.set(true);
    assert_eq!(remove(&fake, &secret), Err(Error::WriteUncertain));
    assert_eq!(fake.deletes.get(), 1);
}
#[test]
fn corrupt_stored_secret_and_invalid_namespace_fail_closed() {
    let fake = Fake::default();
    *fake.value.borrow_mut() = Some(b"not-the-envelope".to_vec());
    assert!(matches!(
        inspect(&fake),
        Err(Error::InvalidStoredCredential)
    ));
    for id in ["", "../outside", "a/b", "a\0b"] {
        assert!(matches!(OsStore::open(id), Err(Error::InvalidRequest)));
    }
}

/// Opt-in only. Creates a unique synthetic slot, never reads a normal app slot.
#[test]
#[ignore = "requires explicit OS credential-store QA permission"]
fn isolated_os_credential_lifecycle() {
    assert_eq!(
        std::env::var("RANGOON_CREDENTIAL_QA").as_deref(),
        Ok("synthetic-only")
    );
    let secret = Secret::new(b"rangoon-disposable-synthetic-credential").unwrap();
    let identifier = format!("ai.rangoon.credential.qa.{}", secret.revision());
    let store = OsStore::open(&identifier).unwrap();
    assert!(inspect(&store).unwrap().is_none());
    struct Cleanup<'a>(&'a OsStore);
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = self.0.delete();
        }
    }
    let _cleanup = Cleanup(&store);
    assert_eq!(save(&store, &secret, None).unwrap(), secret.revision());
    let reopened = OsStore::open(&identifier).unwrap();
    assert_eq!(
        inspect(&reopened).unwrap().unwrap().revision(),
        secret.revision()
    );
    let replacement = Secret::new(b"rangoon-disposable-replacement").unwrap();
    save(&reopened, &replacement, Some(&secret)).unwrap();
    assert_eq!(remove(&reopened, &secret), Err(Error::Changed));
    remove(&reopened, &replacement).unwrap();
    assert!(inspect(&reopened).unwrap().is_none());
}

#[test]
fn save_rejects_intervening_create_replace_or_delete() {
    let fake = Fake::default();
    let first = Secret::new(b"first-synthetic").unwrap();
    let next = Secret::new(b"next-synthetic").unwrap();
    save(&fake, &first, None).unwrap();
    assert_eq!(save(&fake, &next, None), Err(Error::Changed));
    save(&fake, &next, Some(&first)).unwrap();
    assert_eq!(save(&fake, &first, Some(&first)), Err(Error::Changed));
    remove(&fake, &next).unwrap();
    assert_eq!(save(&fake, &first, Some(&next)), Err(Error::Changed));
    assert_eq!(fake.writes.get(), 2);
}
