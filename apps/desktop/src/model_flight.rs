//! One native model flight, sharing drain custody with workspace operations.
use rangoon_desktop::workspace_lifecycle::{Error, Gate, Lane, Lease};
use rangoon_model_session::Diagnostic;

#[derive(Clone, Default)]
pub struct ModelFlight(Gate);

/// Move into blocking callbacks or retained consent custody. Never release a
/// flight merely because its original IPC waiter has gone away.
pub struct ModelLease {
    _lease: Lease,
}
impl ModelFlight {
    pub fn with_gate(gate: Gate) -> Self {
        Self(gate)
    }
    pub fn begin(&self) -> Result<ModelLease, Diagnostic> {
        let lease = self.0.begin(Lane::Model).map_err(|error| match error {
            Error::Busy => Diagnostic::Busy,
            Error::Closed | Error::Stale | Error::Unavailable | Error::Exhausted => {
                Diagnostic::SessionUnavailable
            }
        })?;
        Ok(ModelLease { _lease: lease })
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
