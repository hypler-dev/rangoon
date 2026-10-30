//! Experimental, unmounted recovery envelopes for disposable tests only.
//!
//! This codec does not validate logical backups, access files or keys, restore
//! records, authenticate a sender, grant authority, or establish archive freshness.
//! Native/production adoption remains blocked by separate dependency/custody gates.
#![forbid(unsafe_code)]

use std::{
    fmt,
    io::{self, Read, Write},
};
use zeroize::Zeroizing;

pub const MAX_BACKUP_BYTES: usize = 68 * 1024 * 1024;
pub const MAX_CIPHERTEXT_BYTES: usize = 69 * 1024 * 1024;
pub const MAX_HEADER_BYTES: usize = 4096;
const MAX_STANZAS: usize = 8;
const MAX_KEY_TEXT: usize = 128;
const OUTER_VERSION: &[u8] = b"age-encryption.org/v1\n";
const INNER_VERSION: &[u8] = b"RANGOON-RECOVERY-V1\n";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    TooLarge,
    Unsupported,
    InvalidOrUnreadable,
    Unavailable,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TooLarge => "Recovery input exceeds a supported limit.",
            Self::Unsupported => "Recovery format is unsupported.",
            Self::InvalidOrUnreadable => "Recovery data or identity is invalid or unreadable.",
            Self::Unavailable => "Recovery operation is unavailable.",
        })
    }
}
impl std::error::Error for Error {}

/// Explicit caller-supplied X25519 identity, with no file/key discovery.
///
/// ```compile_fail
/// fn serializable<T: serde::Serialize>() {}
/// serializable::<rangoon_recovery::RecoveryIdentity>();
/// ```
pub struct RecoveryIdentity(age::x25519::Identity);

impl RecoveryIdentity {
    pub fn parse(secret: Zeroizing<String>) -> Result<Self, Error> {
        if secret.len() > MAX_KEY_TEXT {
            return Err(Error::InvalidOrUnreadable);
        }
        secret
            .parse()
            .map(Self)
            .map_err(|_| Error::InvalidOrUnreadable)
    }
    pub fn recipient(&self) -> RecoveryRecipient {
        RecoveryRecipient(self.0.to_public())
    }
}
impl fmt::Debug for RecoveryIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryIdentity(<redacted>)")
    }
}

pub struct RecoveryRecipient(age::x25519::Recipient);
impl RecoveryRecipient {
    pub fn parse(public: &str) -> Result<Self, Error> {
        if public.len() > MAX_KEY_TEXT {
            return Err(Error::InvalidOrUnreadable);
        }
        public
            .parse()
            .map(Self)
            .map_err(|_| Error::InvalidOrUnreadable)
    }
}
impl fmt::Debug for RecoveryRecipient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryRecipient")
    }
}

/// Fully decrypted envelope body; logical backup validity remains unassessed.
/// Caller copies and OS memory are outside this buffer's cleanup guarantee.
///
/// ```compile_fail
/// fn serializable<T: serde::Serialize>() {}
/// serializable::<rangoon_recovery::RecoveredBackup>();
/// ```
pub struct RecoveredBackup(Zeroizing<Vec<u8>>);
impl RecoveredBackup {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0[INNER_VERSION.len()..]
    }
}
impl fmt::Debug for RecoveredBackup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveredBackup(<redacted>)")
    }
}

struct BoundedOutput(Vec<u8>);
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_CIPHERTEXT_BYTES.saturating_sub(self.0.len()) {
            return Err(io::Error::other("Ciphertext limit."));
        }
        self.0
            .try_reserve(bytes.len())
            .map_err(|_| io::Error::other("Allocation unavailable."))?;
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Wrap opaque backup bytes. This does not establish logical backup validity.
pub fn encrypt(backup: &[u8], recipient: &RecoveryRecipient) -> Result<Vec<u8>, Error> {
    if backup.len() > MAX_BACKUP_BYTES {
        return Err(Error::TooLarge);
    }
    let encryptor =
        age::Encryptor::with_recipients(std::iter::once(&recipient.0 as &dyn age::Recipient))
            .map_err(|_| Error::Unavailable)?;
    let mut writer = encryptor
        .wrap_output(BoundedOutput(Vec::new()))
        .map_err(|_| Error::Unavailable)?;
    writer
        .write_all(INNER_VERSION)
        .map_err(|_| Error::Unavailable)?;
    writer.write_all(backup).map_err(|_| Error::Unavailable)?;
    let bytes = writer.finish().map_err(|_| Error::Unavailable)?.0;
    guard_header(&bytes).map_err(|_| Error::Unavailable)?;
    Ok(bytes)
}

// Resource preflight only. Cryptographic header validation belongs to age.
fn guard_header(bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() > MAX_CIPHERTEXT_BYTES {
        return Err(Error::TooLarge);
    }
    if !bytes.starts_with(OUTER_VERSION) {
        return Err(Error::Unsupported);
    }
    let bounded = &bytes[..bytes.len().min(MAX_HEADER_BYTES)];
    let start = bounded
        .windows(5)
        .position(|part| part == b"\n--- ")
        .ok_or(Error::InvalidOrUnreadable)?
        + 1;
    let end = start
        + bounded[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .ok_or(Error::InvalidOrUnreadable)?
        + 1;
    if bounded[..end]
        .split(|byte| *byte == b'\n')
        .filter(|line| line.starts_with(b"-> "))
        .count()
        > MAX_STANZAS
    {
        return Err(Error::InvalidOrUnreadable);
    }
    Ok(())
}

/// Never exposes chunks before the complete authenticated EOF and inner version.
pub fn decrypt(ciphertext: &[u8], identity: &RecoveryIdentity) -> Result<RecoveredBackup, Error> {
    guard_header(ciphertext)?;
    let decryptor =
        age::Decryptor::new_buffered(ciphertext).map_err(|_| Error::InvalidOrUnreadable)?;
    if decryptor.is_scrypt() {
        return Err(Error::Unsupported);
    }
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity.0 as &dyn age::Identity))
        .map_err(|_| Error::InvalidOrUnreadable)?;
    let limit = MAX_BACKUP_BYTES + INNER_VERSION.len();
    let mut body = Zeroizing::new(Vec::new());
    let mut chunk = Zeroizing::new([0u8; 8192]);
    loop {
        let count = reader
            .read(&mut chunk[..])
            .map_err(|_| Error::InvalidOrUnreadable)?;
        if count == 0 {
            break;
        }
        if count > limit.saturating_sub(body.len()) {
            return Err(Error::TooLarge);
        }
        body.try_reserve(count).map_err(|_| Error::Unavailable)?;
        body.extend_from_slice(&chunk[..count]);
    }
    if !body.starts_with(INNER_VERSION) {
        return Err(Error::Unsupported);
    }
    Ok(RecoveredBackup(body))
}
