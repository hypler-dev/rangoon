//! One native model flight, sharing drain custody with workspace operations.
#[cfg(test)]
use rangoon_desktop::workspace_lifecycle::Error;
use rangoon_desktop::workspace_lifecycle::{Gate, GenerationWitness, Lane, Lease};
use rangoon_model_session::Diagnostic;

#[derive(Clone, Default)]
pub struct ModelFlight(Gate);

/// Move into blocking callbacks or retained consent custody. Never release a
/// flight merely because its original IPC waiter has gone away.
pub struct ModelLease {
    _lease: Lease,
}
impl ModelLease {
    pub fn is_current(&self) -> bool {
        self._lease.is_current().unwrap_or(false)
    }
    pub fn witness(&self) -> GenerationWitness {
        self._lease.witness()
    }
}
impl ModelFlight {
    pub fn with_gate(gate: Gate) -> Self {
        Self(gate)
    }
    #[cfg(test)]
    pub fn begin(&self) -> Result<ModelLease, Diagnostic> {
        self.admit(Lane::Model)
    }
    #[cfg(test)]
    pub fn read(&self) -> Result<ModelLease, Diagnostic> {
        self.admit(Lane::Read)
    }
    #[cfg(test)]
    fn admit(&self, lane: Lane) -> Result<ModelLease, Diagnostic> {
        let lease = self.0.begin(lane).map_err(|error| match error {
            Error::Busy => Diagnostic::Busy,
            Error::Closed | Error::Stale | Error::Unavailable | Error::Exhausted => {
                Diagnostic::SessionUnavailable
            }
        })?;
        Ok(ModelLease { _lease: lease })
    }
    pub(crate) fn begin_for(
        &self,
        request: &crate::workspace_requests::WorkspaceRequest,
    ) -> Result<ModelLease, Diagnostic> {
        self.admit_for(request, Lane::Model)
    }
    pub(crate) fn read_for(
        &self,
        request: &crate::workspace_requests::WorkspaceRequest,
    ) -> Result<ModelLease, Diagnostic> {
        self.admit_for(request, Lane::Read)
    }
    fn admit_for(
        &self,
        request: &crate::workspace_requests::WorkspaceRequest,
        lane: Lane,
    ) -> Result<ModelLease, Diagnostic> {
        request
            .admit(&self.0, lane)
            .map(|lease| ModelLease { _lease: lease })
            .map_err(|error| match error {
                crate::AdmissionError::Busy => Diagnostic::Busy,
                crate::AdmissionError::Unavailable => Diagnostic::SessionUnavailable,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn callback_custody_outlives_waiter_and_releases_only_after_last_owner() {
        let gate = ModelFlight::default();
        let owner = Arc::new(gate.begin().unwrap());
        let callback = Arc::clone(&owner);
        drop(owner);
        assert!(matches!(gate.clone().begin(), Err(Diagnostic::Busy)));
        drop(callback);
        let next = gate.begin().unwrap();
        assert!(matches!(gate.begin(), Err(Diagnostic::Busy)));
        drop(next);
        assert!(gate.begin().is_ok());
    }

    #[test]
    fn shared_drain_waits_for_callback_and_compound_source_inspection() {
        let gate = Gate::new();
        let model = ModelFlight::with_gate(gate.clone());
        let waiter = Arc::new(model.begin().unwrap());
        let callback = Arc::clone(&waiter);
        let source = gate.begin(Lane::Exclusive).unwrap();
        let read = gate.begin(Lane::Read).unwrap();
        drop(waiter);
        let drain = gate.request_drain().unwrap();
        assert!(matches!(model.begin(), Err(Diagnostic::SessionUnavailable)));
        assert!(!drain.is_quiescent().unwrap());
        drop(source);
        drop(read);
        assert!(!drain.is_quiescent().unwrap());
        drop(callback);
        assert!(drain.is_quiescent().unwrap());
        gate.resume(&drain).unwrap();
        assert!(model.begin().is_ok());
    }
}
