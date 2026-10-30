use super::*;

#[test]
#[cfg(all(unix, feature = "encrypted-workspace"))]
fn recovery_secret_debug_and_receipt_do_not_serialize_key_material() {
    let key = RecoveryKey(Zeroizing::new([87; 32]));
    assert_eq!(format!("{key:?}"), "RecoveryKey(<redacted>)");
    assert_eq!(FORMAT, "rangoon.recovery.sqlcipher.v1");
}

#[cfg(all(unix, feature = "encrypted-workspace"))]
mod unix {
    use super::*;
    #[test]
    fn selected_key_and_cipher_replacement_before_open_or_after_read_refuses() {
        for phase in [ReadPhase::BeforeOpen, ReadPhase::AfterRead] {
            for key in [false, true] {
                let root = TestRoot::new("recovery-selected-swap");
                let path = root.0.join("selected");
                if key {
                    create_key(&path).unwrap();
                } else {
                    write_cipher(&path, &[17; 4096]).unwrap();
                }
                let replacement = root.0.join("replacement");
                fs::copy(&path, &replacement).unwrap();
                fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
                let selected = path.clone();
                READ_PROBE.with(|slot| {
                    *slot.borrow_mut() = Some((
                        phase,
                        Box::new(move || {
                            fs::rename(replacement, selected).unwrap();
                        }),
                    ))
                });
                if key {
                    assert!(read_key(&path).is_err());
                } else {
                    assert!(read_cipher(&path).is_err());
                }
                assert!(READ_PROBE.with(|slot| slot.borrow().is_none()));
            }
        }
    }
    struct TestRoot(std::path::PathBuf);
    impl TestRoot {
        fn new(label: &str) -> Self {
            let mut nonce = [0; 16];
            getrandom::fill(&mut nonce).unwrap();
            let suffix: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
            let path = std::env::temp_dir().join(format!("{label}-{suffix}"));
            fs::create_dir(&path).unwrap();
            Self(path.canonicalize().unwrap())
        }
    }
    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
    };

    #[test]
    fn key_files_are_bounded_private_exact_and_never_overwritten() {
        let root = TestRoot::new("recovery-key");
        let path = root.0.join("recovery.rangoon-key");
        create_key(&path).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let original = Zeroizing::new(fs::read(&path).unwrap());
        assert!(read_key(&path).is_ok());
        assert!(create_key(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), *original);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read_key(&path).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let alias = root.0.join("alias");
        symlink(&path, &alias).unwrap();
        assert!(read_key(&alias).is_err());
        fs::remove_file(&alias).unwrap();
        fs::hard_link(&path, &alias).unwrap();
        assert!(read_key(&path).is_err());
        fs::remove_file(alias).unwrap();
        for bytes in [b"".as_slice(), b"RANGOON-RECOVERY-KEY-V2\n", &[b'0'; 129]] {
            fs::write(&path, bytes).unwrap();
            assert!(read_key(&path).is_err());
        }
    }

    #[test]
    fn private_cipher_staging_roundtrip_wrong_keys_trailing_pages_and_no_overwrite() {
        let root = TestRoot::new("recovery-codec");
        let store = rangoon_store::Workspace::new(root.0.join("source"));
        let report =
            rangoon_import::analyze("AGENTS.md", b"# Rules\nSynthetic recovery sentinel\n")
                .unwrap();
        store.save_v1(&report).unwrap();
        let archive = store.export_workflow_backup().unwrap();
        let backup = WorkspaceBackup::decode(&archive).unwrap();
        let key = RecoveryKey(Zeroizing::new([23; 32]));
        let bytes = encode(&backup, &key).unwrap();
        assert!(
            decode(&bytes, &key)
                .unwrap()
                .same_recovery_records(&backup)
                .unwrap()
        );
        assert!(decode(&bytes, &RecoveryKey(Zeroizing::new([24; 32]))).is_err());
        let path = root.0.join("backup.rangoon-encrypted");
        write_cipher(&path, &bytes).unwrap();
        assert!(write_cipher(&path, &bytes).is_err());
        assert_eq!(read_cipher(&path).unwrap(), bytes);
        for alteration in 0..3 {
            let mut bad = bytes.clone();
            match alteration {
                0 => bad[100] ^= 1,
                1 => bad.extend_from_slice(&[0; 4096]),
                _ => bad.push(0),
            }
            assert!(decode(&bad, &key).is_err());
        }
        assert!(decode(b"RANGOON-BACKUP-V3\n", &key).is_err());
    }
}
