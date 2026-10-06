//! Single-operation native custody, independent of model transport and SQLite.
use crate::cloud_store::{Error, Secret};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;
use zeroize::Zeroizing;

pub const EDIT_WINDOW: &str = "cloud-credential-entry";
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Reading,
    Editing,
    Submitted,
    Prompting,
    Storage,
}
struct Pending {
    id: String,
    phase: Phase,
    cancelled: bool,
    sender: Option<oneshot::Sender<Option<Secret>>>,
}
#[derive(Clone, Default)]
pub struct Custody(Arc<Mutex<Option<Pending>>>);
pub struct Lease {
    owner: Custody,
    pub id: String,
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.owner.0.lock()
            && slot.as_ref().is_some_and(|p| p.id == self.id)
        {
            *slot = None;
        }
    }
}
impl Custody {
    pub fn begin(&self, phase: Phase) -> Result<(Lease, oneshot::Receiver<Option<Secret>>), Error> {
        let mut slot = self.0.lock().map_err(|_| Error::Unavailable)?;
        if slot.is_some() {
            return Err(Error::Busy);
        }
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|_| Error::Unavailable)?;
        let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let (sender, receiver) = oneshot::channel();
        *slot = Some(Pending {
            id: id.clone(),
            phase,
            cancelled: false,
            sender: Some(sender),
        });
        Ok((
            Lease {
                owner: self.clone(),
                id,
            },
            receiver,
        ))
    }
    pub fn edit_id(&self) -> Option<String> {
        let slot = self.0.lock().ok()?;
        slot.as_ref()
            .filter(|p| p.phase == Phase::Editing && !p.cancelled)
            .map(|p| p.id.clone())
    }
    pub fn submit(&self, id: &str, secret: Secret) -> Result<(), Error> {
        let mut slot = self.0.lock().map_err(|_| Error::Unavailable)?;
        let pending = slot
            .as_mut()
            .filter(|p| p.id == id && p.phase == Phase::Editing && !p.cancelled)
            .ok_or(Error::InvalidRequest)?;
        let sender = pending.sender.take().ok_or(Error::InvalidRequest)?;
        pending.phase = Phase::Submitted;
        sender.send(Some(secret)).map_err(|_| Error::Unavailable)
    }
    pub fn cancel_edit(&self, id: &str) -> bool {
        let Ok(mut slot) = self.0.lock() else {
            return false;
        };
        let Some(p) = slot
            .as_mut()
            .filter(|p| p.id == id && p.phase != Phase::Storage && !p.cancelled)
        else {
            return false;
        };
        p.cancelled = true;
        if let Some(sender) = p.sender.take() {
            let _ = sender.send(None);
        }
        true
    }
    /// Cancelling storage cannot undo an in-flight mutation. True means the
    /// parent window must remain alive until the operation returns.
    pub fn cancel(&self, id: Option<&str>) -> bool {
        let Ok(mut slot) = self.0.lock() else {
            return true;
        };
        let Some(p) = slot.as_mut().filter(|p| id.is_none_or(|id| p.id == id)) else {
            return false;
        };
        if p.phase != Phase::Storage {
            p.cancelled = true;
            if let Some(sender) = p.sender.take() {
                let _ = sender.send(None);
            }
        }
        p.phase != Phase::Editing
    }
    /// Transition and cancellation check occur atomically before any mutation.
    pub fn advance(&self, id: &str, phase: Phase) -> bool {
        let Ok(mut slot) = self.0.lock() else {
            return false;
        };
        let Some(p) = slot.as_mut().filter(|p| p.id == id && !p.cancelled) else {
            return false;
        };
        p.phase = phase;
        true
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Decision {
    schema_version: String,
    pub edit_id: String,
}
impl Decision {
    pub fn parse(raw: &[u8]) -> Result<Self, Error> {
        if raw.len() > 256 {
            return Err(Error::InvalidRequest);
        }
        let value: Self = serde_json::from_slice(raw).map_err(|_| Error::InvalidRequest)?;
        if value.schema_version != "rangoon.cloud-credential-decision.v1"
            || !valid_id(&value.edit_id)
        {
            return Err(Error::InvalidRequest);
        }
        Ok(value)
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Submission {
    schema_version: String,
    edit_id: String,
    #[serde(deserialize_with = "secret_string")]
    secret: Zeroizing<String>,
}
fn secret_string<'de, D: serde::Deserializer<'de>>(de: D) -> Result<Zeroizing<String>, D::Error> {
    String::deserialize(de).map(Zeroizing::new)
}
pub fn parse_submission(raw: &[u8]) -> Result<(String, Secret), Error> {
    // JSON escaping can expand each of 2,048 ASCII bytes to six bytes.
    if raw.len() > 12_544 {
        return Err(Error::InvalidRequest);
    }
    let input: Submission = serde_json::from_slice(raw).map_err(|_| Error::InvalidRequest)?;
    if input.schema_version != "rangoon.cloud-credential-submit.v1" || !valid_id(&input.edit_id) {
        return Err(Error::InvalidRequest);
    }
    Ok((input.edit_id, Secret::new(input.secret.as_bytes())?))
}
fn valid_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
#[path = "cloud_custody_tests.rs"]
mod tests;
