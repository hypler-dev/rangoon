//! App-owned OS credential slot. This module has no renderer or network API.
use keyring_core::{Entry, api::CredentialStoreApi};
use serde::Serialize;
use zeroize::Zeroizing;

const MARKER: &[u8] = b"rangoon.openai.credential.v1\0";
pub const MAX_SECRET: usize = 2048;
const ACCOUNT: &str = "openai-default-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    InvalidRequest,
    Busy,
    Unavailable,
    InvalidStoredCredential,
    Changed,
    WriteUncertain,
    ConfirmationUnavailable,
}

/// Intentionally neither Debug nor Serialize. The revision is independent of
/// the key, so exposing it does not expose even a partial key fingerprint.
pub struct Secret(Zeroizing<Vec<u8>>);
impl Secret {
    pub fn new(value: &[u8]) -> Result<Self, Error> {
        if value.is_empty() || value.len() > MAX_SECRET || !value.iter().all(u8::is_ascii_graphic) {
            return Err(Error::InvalidRequest);
        }
        let mut revision = [0; 32];
        getrandom::fill(&mut revision).map_err(|_| Error::Unavailable)?;
        let mut bytes = Zeroizing::new(Vec::with_capacity(MARKER.len() + 32 + value.len()));
        bytes.extend_from_slice(MARKER);
        bytes.extend_from_slice(&revision);
        bytes.extend_from_slice(value);
        Ok(Self(bytes))
    }
    fn decode(bytes: Zeroizing<Vec<u8>>) -> Result<Self, Error> {
        let offset = MARKER.len() + 32;
        if !bytes.starts_with(MARKER)
            || bytes.len() <= offset
            || bytes.len() > offset + MAX_SECRET
            || !bytes[offset..].iter().all(u8::is_ascii_graphic)
        {
            return Err(Error::InvalidStoredCredential);
        }
        Ok(Self(bytes))
    }
    pub fn revision(&self) -> String {
        self.0[MARKER.len()..MARKER.len() + 32]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

pub trait Store {
    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, Error>;
    fn write(&self, value: &[u8]) -> Result<(), Error>;
    fn delete(&self) -> Result<(), Error>;
}

pub fn inspect(store: &impl Store) -> Result<Option<Secret>, Error> {
    store.read()?.map(Secret::decode).transpose()
}
/// Re-read and compare the complete envelope, not just its public revision.
/// The caller must hold native credential-use custody and fresh OS consent.
/// No key getter or renderer serialization is exposed by this conversion.
pub fn transport_credential(
    store: &impl Store,
    expected: &Secret,
) -> Result<rangoon_model_cloud::Credential, Error> {
    let actual = inspect(store)?.ok_or(Error::Changed)?;
    if *actual.0 != *expected.0 {
        return Err(Error::Changed);
    }
    rangoon_model_cloud::Credential::new(
        &actual.revision(),
        Zeroizing::new(actual.0[MARKER.len() + 32..].to_vec()),
    )
    .map_err(|_| Error::InvalidStoredCredential)
}

pub fn save(
    store: &impl Store,
    secret: &Secret,
    expected: Option<&Secret>,
) -> Result<String, Error> {
    let actual = store.read()?;
    let matches = match (actual.as_ref(), expected) {
        (None, None) => true,
        (Some(actual), Some(expected)) => **actual == *expected.0,
        _ => false,
    };
    if !matches {
        return Err(Error::Changed);
    }
    store.write(&secret.0).map_err(|_| Error::WriteUncertain)?;
    match store.read() {
        Ok(Some(actual)) if *actual == *secret.0 => Ok(secret.revision()),
        _ => Err(Error::WriteUncertain),
    }
}
pub fn remove(store: &impl Store, expected: &Secret) -> Result<(), Error> {
    match store.read()? {
        Some(actual) if *actual == *expected.0 => {}
        _ => return Err(Error::Changed),
    }
    store.delete().map_err(|_| Error::WriteUncertain)?;
    match store.read() {
        Ok(None) => Ok(()),
        _ => Err(Error::WriteUncertain),
    }
}

pub struct OsStore(Entry);
impl OsStore {
    pub fn open(identifier: &str) -> Result<Self, Error> {
        // The production caller supplies the native application identifier.
        // Reject alternate namespace syntax even for internal callers/tests.
        if identifier.is_empty()
            || identifier.len() > 160
            || !identifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        {
            return Err(Error::InvalidRequest);
        }
        let service = format!("{identifier}.cloud-credential.v1");
        native_entry(&service)
            .map(Self)
            .map_err(|_| Error::Unavailable)
    }
}
#[cfg(target_os = "macos")]
fn native_entry(service: &str) -> keyring_core::Result<Entry> {
    apple_native_keyring_store::keychain::Store::new()?.build(service, ACCOUNT, None)
}
#[cfg(target_os = "windows")]
fn native_entry(service: &str) -> keyring_core::Result<Entry> {
    let modifiers = std::collections::HashMap::from([("persistence", "Local")]);
    windows_native_keyring_store::Store::new()?.build(service, ACCOUNT, Some(&modifiers))
}
#[cfg(target_os = "linux")]
fn native_entry(service: &str) -> keyring_core::Result<Entry> {
    zbus_secret_service_keyring_store::Store::new()?.build(service, ACCOUNT, None)
}
impl Store for OsStore {
    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        match self.0.get_secret() {
            Ok(bytes) => Ok(Some(Zeroizing::new(bytes))),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(_) => Err(Error::Unavailable),
        }
    }
    fn write(&self, value: &[u8]) -> Result<(), Error> {
        self.0.set_secret(value).map_err(|_| Error::Unavailable)
    }
    fn delete(&self) -> Result<(), Error> {
        self.0.delete_credential().map_err(|_| Error::Unavailable)
    }
}

#[cfg(test)]
#[path = "cloud_store_tests.rs"]
mod tests;
