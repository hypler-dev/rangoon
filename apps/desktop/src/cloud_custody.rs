//! Single-operation native custody, independent of model transport and SQLite.
use crate::cloud_store::{Error, Secret};
use rangoon_desktop::workspace_lifecycle::{
    Gate, GenerationWitness, Lane, Lease as AdmissionLease,
};
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
    Using,
}
struct Pending {
    id: String,
    witness: GenerationWitness,
    phase: Phase,
    cancelled: bool,
    sender: Option<oneshot::Sender<Option<Secret>>>,
}
#[derive(Clone, Default)]
pub struct Custody(Arc<Mutex<Option<Pending>>>, Gate);
pub struct Lease {
    owner: Custody,
    pub id: String,
    admission: AdmissionLease,
}
impl Lease {
    pub fn workspace_is_current(&self) -> bool {
        self.admission.is_current().unwrap_or(false)
    }
    pub fn is_current(&self) -> bool {
        self.workspace_is_current()
            && self.owner.0.lock().ok().is_some_and(|slot| {
                slot.as_ref()
                    .is_some_and(|p| p.id == self.id && !p.cancelled)
            })
    }
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
    /// Deletion-only cleanup. Callback leases live outside Pending and remain
    /// counted even when a caller or the renderer disappears.
    pub(super) fn purge_for_workspace_close(
        &self,
        drained: bool,
    ) -> crate::workspace_close::Cleanup {
        let (mut slot, healthy) = match self.0.lock() {
            Ok(slot) => (slot, true),
            Err(error) => (error.into_inner(), false),
        };
        let mut keep_parent = false;
        if let Some(pending) = slot.as_mut() {
            keep_parent = !drained && pending.phase != Phase::Editing;
            if pending.phase != Phase::Storage {
                pending.cancelled = true;
                if let Some(sender) = pending.sender.take() {
                    let _ = sender.send(None);
                }
            }
            if !keep_parent {
                *slot = None;
            }
        }
        crate::workspace_close::Cleanup {
            healthy,
            keep_parent,
        }
    }
    pub fn with_gate(gate: Gate) -> Self {
        Self(Arc::new(Mutex::new(None)), gate)
    }
    #[cfg(test)]
    pub fn begin(&self, phase: Phase) -> Result<(Lease, oneshot::Receiver<Option<Secret>>), Error> {
        let admission = self.1.begin(Lane::Read).map_err(|error| match error {
            rangoon_desktop::workspace_lifecycle::Error::Busy => Error::Busy,
            _ => Error::Unavailable,
        })?;
        self.begin_admitted(phase, admission)
    }
    pub(crate) fn begin_for(
        &self,
        request: &crate::workspace_requests::WorkspaceRequest,
        phase: Phase,
    ) -> Result<(Lease, oneshot::Receiver<Option<Secret>>), Error> {
        let admission = request
            .admit(&self.1, Lane::Read)
            .map_err(|error| match error {
                crate::AdmissionError::Busy => Error::Busy,
                crate::AdmissionError::Unavailable => Error::Unavailable,
            })?;
        self.begin_admitted(phase, admission)
    }
    fn begin_admitted(
        &self,
        phase: Phase,
        admission: AdmissionLease,
    ) -> Result<(Lease, oneshot::Receiver<Option<Secret>>), Error> {
        let mut slot = self.0.lock().map_err(|_| Error::Unavailable)?;
        if !admission.is_current().unwrap_or(false) {
            return Err(Error::Unavailable);
        }
        if slot.is_some() {
            return Err(Error::Busy);
        }
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|_| Error::Unavailable)?;
        let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let (sender, receiver) = oneshot::channel();
        *slot = Some(Pending {
            id: id.clone(),
            witness: admission.witness(),
            phase,
            cancelled: false,
            sender: Some(sender),
        });
        Ok((
            Lease {
                owner: self.clone(),
                id,
                admission,
            },
            receiver,
        ))
    }
    pub fn edit_id(&self) -> Option<String> {
        let slot = self.0.lock().ok()?;
        slot.as_ref()
            .filter(|p| {
                p.phase == Phase::Editing && !p.cancelled && p.witness.is_current().unwrap_or(false)
            })
            .map(|p| p.id.clone())
    }
    pub fn submit(&self, id: &str, secret: Secret) -> Result<(), Error> {
        let mut slot = self.0.lock().map_err(|_| Error::Unavailable)?;
        let pending = slot
            .as_mut()
            .filter(|p| {
                p.id == id
                    && p.phase == Phase::Editing
                    && !p.cancelled
                    && p.witness.is_current().unwrap_or(false)
            })
            .ok_or(Error::InvalidRequest)?;
        let sender = pending.sender.take().ok_or(Error::InvalidRequest)?;
        pending.phase = Phase::Submitted;
        sender.send(Some(secret)).map_err(|_| Error::Unavailable)
    }
    pub fn cancel_edit(&self, id: &str) -> bool {
        let Ok(mut slot) = self.0.lock() else {
            return false;
        };
        let Some(p) = slot.as_mut().filter(|p| {
            p.id == id && !matches!(p.phase, Phase::Storage | Phase::Using) && !p.cancelled
        }) else {
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
        let Some(p) = slot
            .as_mut()
            .filter(|p| p.id == id && !p.cancelled && p.witness.is_current().unwrap_or(false))
        else {
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

#[cfg(test)]
mod close_cleanup_tests {
    use super::*;
    use rangoon_desktop::workspace_lifecycle::Lane;
    #[test]
    fn close_keeps_storage_lease_and_poison_never_becomes_healthy() {
        for poison in [false, true] {
            let gate = Gate::new();
            let custody = Custody::with_gate(gate.clone());
            let (lease, receiver) = custody.begin(Phase::Storage).unwrap();
            drop(receiver);
            if poison {
                let _ = std::panic::catch_unwind(|| {
                    let _slot = custody.0.lock().unwrap();
                    panic!("synthetic credential custody poison");
                });
            }
            let drain = gate.request_drain().unwrap();
            let status = custody.purge_for_workspace_close(false);
            assert_eq!(status.healthy, !poison);
            assert!(status.keep_parent);
            assert!(!drain.is_quiescent().unwrap());
            drop(lease);
            assert!(drain.is_quiescent().unwrap());
            let final_status = custody.purge_for_workspace_close(true);
            assert_eq!(final_status.healthy, !poison);
            assert!(!final_status.keep_parent);
            let slot = custody
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            assert!(slot.is_none());
            drop(slot);
            assert!(gate.begin(Lane::Read).is_err());
            if poison {
                assert!(custody.0.lock().is_err());
            }
        }
    }
    #[test]
    fn close_cancels_unsubmitted_editor_without_releasing_callback_custody() {
        let gate = Gate::new();
        let custody = Custody::with_gate(gate.clone());
        let (lease, mut receiver) = custody.begin(Phase::Editing).unwrap();
        let drain = gate.request_drain().unwrap();
        let status = custody.purge_for_workspace_close(false);
        assert!(status.healthy);
        assert!(!status.keep_parent);
        assert!(matches!(receiver.try_recv(), Ok(None)));
        assert!(custody.edit_id().is_none());
        assert!(!lease.is_current());
        assert!(!drain.is_quiescent().unwrap());
        drop(lease);
        assert!(drain.is_quiescent().unwrap());
    }
}
