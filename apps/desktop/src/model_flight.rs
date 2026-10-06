//! One native operation across model routes, independent of workspace reads.
use rangoon_model_session::Diagnostic;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Default)]
pub struct ModelFlight(Arc<AtomicBool>);

/// Move into blocking callbacks or retained consent custody. Never release a
/// flight merely because its original IPC waiter has gone away.
pub struct ModelLease(Arc<AtomicBool>);
impl ModelFlight {
    pub fn begin(&self) -> Result<ModelLease, Diagnostic> {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| Diagnostic::Busy)?;
        Ok(ModelLease(Arc::clone(&self.0)))
    }
}
impl Drop for ModelLease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
