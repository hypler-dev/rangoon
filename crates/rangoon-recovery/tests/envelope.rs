use rangoon_recovery::{
    Error, MAX_BACKUP_BYTES, MAX_CIPHERTEXT_BYTES, MAX_HEADER_BYTES, RecoveryIdentity,
    RecoveryRecipient, decrypt, encrypt,
};
use serde::Deserialize;
use std::io::Write;
use zeroize::Zeroizing;

#[derive(Deserialize)]
struct Fixture {
    identity: String,
    recipient: String,
    #[serde(rename = "backupHex")]
    backup_hex: String,
    #[serde(rename = "ageCiphertextHex")]
    ciphertext_hex: String,
}
fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/reference-age-v1.json")).unwrap()
}
fn identity() -> RecoveryIdentity {
    RecoveryIdentity::parse(Zeroizing::new(fixture().identity)).unwrap()
}
fn recipient() -> RecoveryRecipient {
    RecoveryRecipient::parse(&fixture().recipient).unwrap()
}
fn hex(value: &str) -> Vec<u8> {
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs
        .iter()
        .map(|part| u8::from_str_radix(std::str::from_utf8(part).unwrap(), 16).unwrap())
        .collect()
}
// Independent library call lets tests create authenticated unsupported/oversized bodies.
fn raw_age_body(body: &[u8]) -> Vec<u8> {
    let public: age::x25519::Recipient = fixture().recipient.parse().unwrap();
    let e =
        age::Encryptor::with_recipients(std::iter::once(&public as &dyn age::Recipient)).unwrap();
    let mut writer = e.wrap_output(Vec::new()).unwrap();
    writer.write_all(body).unwrap();
    writer.finish().unwrap()
}

#[test]
fn independent_go_reference_decrypts_exact_binary_backup() {
    let f = fixture();
    let recovered = decrypt(&hex(&f.ciphertext_hex), &identity()).unwrap();
    assert_eq!(recovered.as_bytes(), hex(&f.backup_hex));
}

#[test]
fn round_trip_preserves_empty_binary_bom_crlf_and_unicode() {
    for body in [
        b"".as_slice(),
        b"\0\xff\x01",
        b"\xef\xbb\xbf# Instructions\r\nKeep exact bytes.\r\n",
        "# Review\nစမ်းသပ် ✓".as_bytes(),
    ] {
        let ciphertext = encrypt(body, &recipient()).unwrap();
        assert!(ciphertext.starts_with(b"age-encryption.org/v1\n"));
        assert_eq!(decrypt(&ciphertext, &identity()).unwrap().as_bytes(), body);
    }
}

#[test]
fn supplied_public_recipient_matches_identity_derived_recipient() {
    let body = b"Exact supplied public recipient";
    let encrypted = encrypt(body, &identity().recipient()).unwrap();
    assert_eq!(decrypt(&encrypted, &identity()).unwrap().as_bytes(), body);
}

#[test]
fn fresh_randomness_changes_repeated_ciphertext() {
    let first = encrypt(b"Repeated public canary", &recipient()).unwrap();
    let second = encrypt(b"Repeated public canary", &recipient()).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        decrypt(&first, &identity()).unwrap().as_bytes(),
        decrypt(&second, &identity()).unwrap().as_bytes()
    );
}

#[test]
fn chunk_boundaries_require_final_authentication() {
    for size in [65516, 65517, 65518, 65535, 65536, 65537, 131072] {
        let body = vec![0x93; size];
        let ciphertext = encrypt(&body, &recipient()).unwrap();
        assert_eq!(decrypt(&ciphertext, &identity()).unwrap().as_bytes(), body);
        assert!(decrypt(&ciphertext[..ciphertext.len() - 1], &identity()).is_err());
        let mut corrupt = ciphertext;
        *corrupt.last_mut().unwrap() ^= 1;
        assert!(decrypt(&corrupt, &identity()).is_err());
    }
}

#[test]
fn maximum_backup_round_trips_and_one_extra_byte_is_refused() {
    let mut body = vec![0x5a; MAX_BACKUP_BYTES];
    let ciphertext = encrypt(&body, &recipient()).unwrap();
    assert!(ciphertext.len() < MAX_CIPHERTEXT_BYTES);
    assert_eq!(decrypt(&ciphertext, &identity()).unwrap().as_bytes(), body);
    body.push(1);
    assert_eq!(encrypt(&body, &recipient()).unwrap_err(), Error::TooLarge);
}

#[test]
fn ciphertext_limit_refuses_before_version_or_decryption() {
    assert_eq!(
        decrypt(&vec![0; MAX_CIPHERTEXT_BYTES + 1], &identity()).unwrap_err(),
        Error::TooLarge
    );
}

#[test]
fn fully_authenticated_oversized_plaintext_is_not_returned() {
    let mut body = b"RANGOON-RECOVERY-V1\n".to_vec();
    body.resize(body.len() + MAX_BACKUP_BYTES + 1, 0xa1);
    let ciphertext = raw_age_body(&body);
    assert!(ciphertext.len() < MAX_CIPHERTEXT_BYTES);
    assert_eq!(
        decrypt(&ciphertext, &identity()).unwrap_err(),
        Error::TooLarge
    );
}

#[test]
fn wrong_identity_never_returns_any_plaintext() {
    use age::secrecy::ExposeSecret;
    let wrong = age::x25519::Identity::generate().to_string();
    let wrong = RecoveryIdentity::parse(Zeroizing::new(wrong.expose_secret().to_owned())).unwrap();
    let ciphertext = encrypt(b"Do not release partial data", &recipient()).unwrap();
    assert_eq!(
        decrypt(&ciphertext, &wrong).unwrap_err(),
        Error::InvalidOrUnreadable
    );
}

#[test]
fn every_single_byte_tamper_of_small_reference_is_refused() {
    let bytes = hex(&fixture().ciphertext_hex);
    for index in 0..bytes.len() {
        let mut altered = bytes.clone();
        altered[index] ^= 1;
        assert!(decrypt(&altered, &identity()).is_err(), "Tamper at {index}");
    }
}

#[test]
fn every_reference_truncation_trailing_byte_and_concatenation_is_refused() {
    let bytes = hex(&fixture().ciphertext_hex);
    for size in 0..bytes.len() {
        assert!(
            decrypt(&bytes[..size], &identity()).is_err(),
            "Truncation at {size}"
        );
    }
    for tail in [b"x".as_slice(), b"\n", bytes.as_slice()] {
        let mut appended = bytes.clone();
        appended.extend_from_slice(tail);
        assert!(decrypt(&appended, &identity()).is_err());
    }
}

#[test]
fn unsupported_authenticated_inner_versions_are_not_backup_bytes() {
    for body in [
        b"RANGOON-RECOVERY-V2\nSecret".as_slice(),
        b"RANGOON-RECOVERY-V1\r\nSecret",
        b"Unframed secret",
        b"",
    ] {
        assert_eq!(
            decrypt(&raw_age_body(body), &identity()).unwrap_err(),
            Error::Unsupported
        );
    }
}

#[test]
fn outer_versions_armor_and_invalid_key_types_are_refused() {
    for body in [
        b"age-encryption.org/v2\n".as_slice(),
        b"-----BEGIN AGE ENCRYPTED FILE-----\n",
        b"not an archive",
    ] {
        assert_eq!(decrypt(body, &identity()).unwrap_err(), Error::Unsupported);
    }
    for key in [
        "",
        "age-plugin-../../canary",
        "AGE-PLUGIN-FAKE-1",
        "ssh-ed25519 AAAA",
        &"a".repeat(129),
    ] {
        assert_eq!(
            RecoveryIdentity::parse(Zeroizing::new(key.to_owned())).unwrap_err(),
            Error::InvalidOrUnreadable
        );
        assert_eq!(
            RecoveryRecipient::parse(key).unwrap_err(),
            Error::InvalidOrUnreadable
        );
    }
}

#[test]
fn passphrase_envelopes_are_unsupported_without_attempting_password_recovery() {
    // Deliberately cheap fixture generation; this is never an accepted profile.
    let mut passphrase = age::scrypt::Recipient::new(age::secrecy::SecretString::from(
        "PUBLIC TEST PASSPHRASE".to_owned(),
    ));
    passphrase.set_work_factor(1);
    let e = age::Encryptor::with_recipients(std::iter::once(&passphrase as &dyn age::Recipient))
        .unwrap();
    let mut writer = e.wrap_output(Vec::new()).unwrap();
    writer.write_all(b"RANGOON-RECOVERY-V1\nCanary").unwrap();
    let ciphertext = writer.finish().unwrap();
    assert_eq!(
        decrypt(&ciphertext, &identity()).unwrap_err(),
        Error::Unsupported
    );
}

#[test]
fn resource_preflight_refuses_large_headers_and_many_stanzas() {
    let mut header = b"age-encryption.org/v1\n".to_vec();
    header.extend_from_slice(&vec![b'x'; MAX_HEADER_BYTES]);
    header.extend_from_slice(b"\n--- ");
    header.extend_from_slice(&[b'A'; 43]);
    header.push(b'\n');
    assert!(decrypt(&header, &identity()).is_err());
    let mut many = b"age-encryption.org/v1\n".to_vec();
    for _ in 0..9 {
        many.extend_from_slice(b"-> X25519 AAAA\nAAAA\n");
    }
    many.extend_from_slice(b"--- ");
    many.extend_from_slice(&[b'A'; 43]);
    many.push(b'\n');
    assert_eq!(
        decrypt(&many, &identity()).unwrap_err(),
        Error::InvalidOrUnreadable
    );
}

#[test]
fn malformed_corpus_never_panics_or_returns_content() {
    let mut seed = 0x72616e676f6f6eu64;
    for size in 0..512 {
        let mut bytes = Vec::with_capacity(size);
        for _ in 0..size {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            bytes.push(seed as u8);
        }
        if size % 2 == 0 {
            bytes.splice(0..0, b"age-encryption.org/v1\n".iter().copied());
        }
        assert!(decrypt(&bytes, &identity()).is_err());
    }
}

#[test]
fn errors_and_secret_owners_have_fixed_redacted_diagnostics() {
    let f = fixture();
    let identity = identity();
    let recovered = decrypt(&hex(&f.ciphertext_hex), &identity).unwrap();
    assert_eq!(format!("{identity:?}"), "RecoveryIdentity(<redacted>)");
    assert_eq!(format!("{recovered:?}"), "RecoveredBackup(<redacted>)");
    for error in [
        Error::TooLarge,
        Error::Unsupported,
        Error::InvalidOrUnreadable,
        Error::Unavailable,
    ] {
        assert!(!format!("{error:?}: {error}").contains(&f.identity));
        assert!(std::error::Error::source(&error).is_none());
    }
}
