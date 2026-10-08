//! Explicit native database-key custody, independent of provider credentials.
//! These primitives do not mount an unlock UI, authenticate a database, or own
//! the provisioning/session leases required before production adoption.
use keyring_core::{Entry, api::CredentialStoreApi};
use rangoon_store::{Workspace, WorkspaceKey};
use serde::Serialize;
use std::path::PathBuf;
use zeroize::Zeroizing;

const MARKER: &[u8] = b"rangoon.workspace-key.v1\0";
const ENVELOPE_BYTES: usize = MARKER.len() + 96;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    InvalidRequest,
    BackendUnavailable,
    EntropyUnavailable,
    Unavailable,
    Missing,
    AlreadyExists,
    InvalidStoredKey,
    Changed,
    WriteUncertain,
}

/// Nonsecret native workspace identity, never a path or a key fingerprint.
#[derive(Clone, PartialEq, Eq)]
pub struct WorkspaceId([u8; 32]);
impl WorkspaceId {
    pub fn generate() -> Result<Self, Error> {
        generate_identity_with_entropy(&SystemEntropy)
    }
    /// Parse only a trusted host record; do not accept a renderer slot selector.
    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::InvalidRequest);
        }
        let mut bytes = [0; 32];
        for (i, pair) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            bytes[i] = (digit(pair[0]) << 4) | digit(pair[1]);
        }
        if !nonzero(&bytes) {
            return Err(Error::InvalidRequest);
        }
        Ok(Self(bytes))
    }
    pub fn as_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
}
fn digit(byte: u8) -> u8 {
    if byte.is_ascii_digit() {
        byte - b'0'
    } else {
        byte - b'a' + 10
    }
}
fn nonzero(bytes: &[u8]) -> bool {
    bytes.iter().any(|b| *b != 0)
}
trait Entropy {
    fn fill(&self, bytes: &mut [u8]) -> Result<(), Error>;
}
struct SystemEntropy;
impl Entropy for SystemEntropy {
    fn fill(&self, bytes: &mut [u8]) -> Result<(), Error> {
        getrandom::fill(bytes).map_err(|_| Error::EntropyUnavailable)
    }
}
fn random_nonzero(entropy: &impl Entropy) -> Result<Zeroizing<[u8; 32]>, Error> {
    let mut bytes = Zeroizing::new([0; 32]);
    entropy
        .fill(&mut *bytes)
        .map_err(|_| Error::EntropyUnavailable)?;
    if !nonzero(bytes.as_slice()) {
        return Err(Error::EntropyUnavailable);
    }
    Ok(bytes)
}
fn generate_identity_with_entropy(entropy: &impl Entropy) -> Result<WorkspaceId, Error> {
    Ok(WorkspaceId(*random_nonzero(entropy)?))
}

/// Trusted native vault boundary. Implementations must keep slot identity fixed.
/// This interface has no atomic create-only operation or cross-process lease.
pub trait Store {
    fn identity(&self) -> &WorkspaceId;
    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, Error>;
    fn write(&self, envelope: &[u8]) -> Result<(), Error>;
}

/// Opaque native evidence of the exact record read or created in one OS slot.
/// It grants no session or execution authority and does not revoke store handles.
///
/// ```compile_fail
/// use rangoon_desktop::workspace_keys::RetainedKey;
/// fn requires_debug<T: std::fmt::Debug>() {}
/// requires_debug::<RetainedKey>();
/// ```
/// ```compile_fail
/// use rangoon_desktop::workspace_keys::RetainedKey;
/// fn requires_serialization<T: serde::Serialize>() {}
/// requires_serialization::<RetainedKey>();
/// ```
/// ```compile_fail
/// use rangoon_desktop::workspace_keys::RetainedKey;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<RetainedKey>();
/// ```
pub struct RetainedKey {
    identity: WorkspaceId,
    envelope: Zeroizing<Vec<u8>>,
}
impl RetainedKey {
    fn decode(identity: &WorkspaceId, envelope: Zeroizing<Vec<u8>>) -> Result<Self, Error> {
        if envelope.len() != ENVELOPE_BYTES || !envelope.starts_with(MARKER) {
            return Err(Error::InvalidStoredKey);
        }
        let start = MARKER.len();
        if envelope[start..start + 32] != identity.0
            || !nonzero(&envelope[start..start + 32])
            || !nonzero(&envelope[start + 32..start + 64])
            || !nonzero(&envelope[start + 64..])
        {
            return Err(Error::InvalidStoredKey);
        }
        Ok(Self {
            identity: identity.clone(),
            envelope,
        })
    }
    fn key(&self) -> WorkspaceKey {
        let mut bytes = Zeroizing::new([0; 32]);
        bytes.copy_from_slice(&self.envelope[MARKER.len() + 64..]);
        WorkspaceKey::from_bytes(*bytes)
    }
}
pub(crate) fn ensure_backend() -> Result<(), Error> {
    if !cfg!(feature = "encrypted-workspace") {
        return Err(Error::BackendUnavailable);
    }
    // The constructor verifies the pinned cipher/profile entirely in memory.
    // This constant is only a disposable backend probe, never a stored key.
    Workspace::encrypted(PathBuf::new(), WorkspaceKey::from_bytes([0xa7; 32]))
        .map(|_| ())
        .map_err(|_| Error::BackendUnavailable)
}

/// Provision only for a new native-generated identity under an exclusive host
/// lease and fresh consent. A failed/uncertain attempt must not be auto-retried.
/// Any observed existing value is preserved, even if its encoding is malformed.
pub fn create_key(store: &impl Store) -> Result<RetainedKey, Error> {
    create_with_entropy(store, &SystemEntropy)
}
fn create_with_entropy(store: &impl Store, entropy: &impl Entropy) -> Result<RetainedKey, Error> {
    ensure_backend()?;
    if store.read().map_err(|_| Error::Unavailable)?.is_some() {
        return Err(Error::AlreadyExists);
    }
    let revision = random_nonzero(entropy)?;
    let key = random_nonzero(entropy)?;
    let mut envelope = Zeroizing::new(Vec::with_capacity(ENVELOPE_BYTES));
    envelope.extend_from_slice(MARKER);
    envelope.extend_from_slice(&store.identity().0);
    envelope.extend_from_slice(revision.as_slice());
    envelope.extend_from_slice(key.as_slice());
    let retained = RetainedKey::decode(store.identity(), envelope)?;
    // Once a write is attempted, failure cannot establish whether it committed.
    let written = store.write(&retained.envelope).is_ok();
    let observed = store.read();
    match observed {
        Ok(Some(actual)) if written && *actual == *retained.envelope => Ok(retained),
        _ => Err(Error::WriteUncertain),
    }
}

/// Explicit read only. Missing, corrupt and unavailable slots are never repaired.
pub fn read_key(store: &impl Store) -> Result<RetainedKey, Error> {
    ensure_backend()?;
    let bytes = store
        .read()
        .map_err(|_| Error::Unavailable)?
        .ok_or(Error::Missing)?;
    RetainedKey::decode(store.identity(), bytes)
}

/// Revalidate the entire retained record before returning a keyed store handle.
/// This does not authenticate database pages until an actual store operation.
/// The host owns directory validation, session leases and handle-drop on lock.
pub fn verified_workspace(
    store: &impl Store,
    retained: &RetainedKey,
    host_directory: PathBuf,
) -> Result<Workspace, Error> {
    ensure_backend()?;
    if store.identity() != &retained.identity {
        return Err(Error::Changed);
    }
    let actual = store
        .read()
        .map_err(|_| Error::Unavailable)?
        .ok_or(Error::Changed)?;
    let actual = RetainedKey::decode(store.identity(), actual).map_err(|_| Error::Changed)?;
    if *actual.envelope != *retained.envelope {
        return Err(Error::Changed);
    }
    Workspace::encrypted(host_directory, actual.key()).map_err(|_| Error::BackendUnavailable)
}

/// A separate explicit OS database-key namespace; no cloud credential fallback.
/// Opening a slot is not user consent and does not authenticate a workspace.
pub struct OsSlot {
    identity: WorkspaceId,
    entry: Entry,
}
fn slot_names(identifier: &str, identity: &WorkspaceId) -> Result<(String, String), Error> {
    if identifier.is_empty()
        || identifier.len() > 128
        || !identifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
    {
        return Err(Error::InvalidRequest);
    }
    Ok((
        format!("{identifier}.encrypted-workspace-key.v1"),
        format!("workspace-{}", identity.as_hex()),
    ))
}
impl OsSlot {
    pub fn open(application_identifier: &str, identity: WorkspaceId) -> Result<Self, Error> {
        let (service, account) = slot_names(application_identifier, &identity)?;
        ensure_backend()?;
        let entry = native_entry(&service, &account).map_err(|_| Error::Unavailable)?;
        Ok(Self { identity, entry })
    }
}
#[cfg(target_os = "macos")]
fn native_entry(service: &str, account: &str) -> keyring_core::Result<Entry> {
    apple_native_keyring_store::keychain::Store::new()?.build(service, account, None)
}
#[cfg(target_os = "windows")]
fn native_entry(service: &str, account: &str) -> keyring_core::Result<Entry> {
    let modifiers = std::collections::HashMap::from([("persistence", "Local")]);
    windows_native_keyring_store::Store::new()?.build(service, account, Some(&modifiers))
}
#[cfg(target_os = "linux")]
fn native_entry(service: &str, account: &str) -> keyring_core::Result<Entry> {
    zbus_secret_service_keyring_store::Store::new()?.build(service, account, None)
}
impl Store for OsSlot {
    fn identity(&self) -> &WorkspaceId {
        &self.identity
    }
    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        match self.entry.get_secret() {
            Ok(bytes) => Ok(Some(Zeroizing::new(bytes))),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(_) => Err(Error::Unavailable),
        }
    }
    fn write(&self, envelope: &[u8]) -> Result<(), Error> {
        self.entry
            .set_secret(envelope)
            .map_err(|_| Error::Unavailable)
    }
}

#[cfg(test)]
#[path = "workspace_keys_tests.rs"]
mod tests;
