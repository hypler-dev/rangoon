//! Private trusted-host transitions. No native command is mounted. The caller
//! must independently qualify native cleanup, renderer purge and OS consent.
use super::*;
use rangoon_desktop::workspace_lifecycle::ClosedLease;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    CreateEmpty,
    Unlock,
    Recovery,
    #[cfg(feature = "encrypted-workspace")]
    Startup,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ControlError {
    Unavailable,
    Stale,
    Busy,
    Invalid,
    Owner(ControlOwnerError),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    New,
    Prepared,
    Confirmed,
    Inspected,
    Reconciled,
    Failed,
}

/// Move into the actual worker, then into any retained native recovery plan.
/// A renderer/IPC waiter must never be its sole owner during a callback.
/// Dropping a failed, cancelled or unfinished control locks Owner before its
/// ClosedLease can disappear. It does not fake native/renderer acknowledgment.
pub(crate) struct Control {
    ticket: CloseTicket,
    lease: Option<ClosedLease>,
    kind: Kind,
    phase: Phase,
    finished: bool,
}
impl CloseTicket {
    pub(crate) fn start_control(&self, kind: Kind) -> Result<Control, ControlError> {
        self.start_control_inner(kind, false)
    }
    #[cfg(test)]
    pub(crate) fn fail_control_after_swap_for_test(
        &self,
        kind: Kind,
    ) -> Result<Control, ControlError> {
        self.start_control_inner(kind, true)
    }
    fn start_control_inner(
        &self,
        kind: Kind,
        _fail_after_swap: bool,
    ) -> Result<Control, ControlError> {
        let (mut slot, healthy) = self.current_slot().map_err(|_| ControlError::Stale)?;
        if !healthy {
            return Err(ControlError::Unavailable);
        }
        if self.cycle.drain.is_quiescent() != Ok(true) {
            return Err(ControlError::Busy);
        }
        if *self
            .cycle
            .released
            .lock()
            .map_err(|_| ControlError::Unavailable)?
            != Some(true)
        {
            return Err(ControlError::Unavailable);
        }
        let drain = self
            .access
            .0
            .gate
            .renew_drain(&self.cycle.drain)
            .map_err(|_| ControlError::Unavailable)?;
        let cycle = Arc::new(CloseCycle {
            drain,
            healthy: true,
            released: Mutex::new(None),
            cleanup_failed: AtomicBool::new(false),
        });
        // The old memo becomes unreachable before any Backend/key lookup.
        *slot = Some(Arc::clone(&cycle));
        #[cfg(test)]
        if _fail_after_swap {
            cycle.cleanup_failed.store(true, Ordering::SeqCst);
            return Err(ControlError::Unavailable);
        }
        let lease = match self.access.0.gate.begin_closed(&cycle.drain) {
            Ok(lease) => lease,
            Err(_) => {
                cycle.cleanup_failed.store(true, Ordering::SeqCst);
                return Err(ControlError::Unavailable);
            }
        };
        Ok(Control {
            ticket: CloseTicket {
                access: self.access.clone(),
                cycle,
            },
            lease: Some(lease),
            kind,
            phase: Phase::New,
            finished: false,
        })
    }
    /// Invalidate a callback without releasing any actual callback custody.
    pub(crate) fn renew_control(&self) -> Result<CloseTicket, ControlError> {
        let (mut slot, healthy) = self.current_slot().map_err(|_| ControlError::Stale)?;
        let drain = self
            .access
            .0
            .gate
            .renew_drain(&self.cycle.drain)
            .map_err(|_| ControlError::Unavailable)?;
        let cycle = Arc::new(CloseCycle {
            drain,
            healthy,
            released: Mutex::new(None),
            cleanup_failed: AtomicBool::new(false),
        });
        *slot = Some(Arc::clone(&cycle));
        Ok(CloseTicket {
            access: self.access.clone(),
            cycle,
        })
    }
}
impl Control {
    #[cfg(feature = "encrypted-workspace")]
    pub(crate) fn check_bound_identity(&mut self, expected: &str) -> Result<(), ControlError> {
        if self.kind != Kind::Startup || self.phase != Phase::New {
            return Err(ControlError::Invalid);
        }
        let result = self.run(|backend| backend.control_check_identity(expected));
        if result.is_err() {
            self.mark_cleanup_failed();
            self.fail();
        }
        result
    }
    /// Startup work keeps the real closed-operation lease. Request identities
    /// cannot replace this check or publish an ordinary content client.
    #[cfg(feature = "encrypted-workspace")]
    pub(crate) fn startup_checkpoint(&self) -> Result<(), ControlError> {
        if self.kind != Kind::Startup || self.phase != Phase::New {
            return Err(ControlError::Invalid);
        }
        if !self.current() {
            return Err(ControlError::Stale);
        }
        Ok(())
    }
    /// Bind once under actual closed-operation custody. This proof is created
    /// only by the native fixed-root inspector, never by IPC or a UI action.
    #[cfg(feature = "encrypted-workspace")]
    pub(crate) fn bind_locked_owner(
        &mut self,
        proof: crate::workspace_startup::BindingProof,
    ) -> Result<(), ControlError> {
        if self.phase != Phase::New || !self.current() || proof.check_before().is_err() {
            self.mark_cleanup_failed();
            self.fail();
            return Err(ControlError::Invalid);
        }
        let root = proof.root();
        let result = self.run(|backend| backend.control_bind_owner(&root, proof.existing()));
        if result.is_err() || proof.check_after().is_err() {
            self.mark_cleanup_failed();
            self.fail();
            return Err(ControlError::Unavailable);
        }
        Ok(())
    }
    fn current(&self) -> bool {
        self.lease
            .as_ref()
            .is_some_and(|l| l.is_current() == Ok(true))
            && self.ticket.current_slot().is_ok_and(|(_, healthy)| healthy)
    }
    fn fail(&mut self) {
        self.phase = Phase::Failed;
        self.close_owner();
    }
    fn close_owner(&self) {
        // A live control is the only possible Backend/key operation. Recover
        // poison only to discard custody; never turn it into operational access.
        let (mut backend, healthy) = match self.ticket.access.0.backend.try_lock() {
            Ok(backend) => (backend, true),
            Err(TryLockError::Poisoned(error)) => (error.into_inner(), false),
            Err(TryLockError::WouldBlock) => {
                self.mark_cleanup_failed();
                return;
            }
        };
        let healthy_for_reopen =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| backend.close_session()))
                .unwrap_or(false);
        drop(backend);
        let purged = self.ticket.access.purge_retained();
        if !healthy || !healthy_for_reopen || !purged {
            self.mark_cleanup_failed();
        }
    }
    fn mark_cleanup_failed(&self) {
        // Cancellation may already have installed a newer cycle. It must not
        // report a clean lock after cleanup of this old callback became unknown.
        let slot = match self.ticket.access.0.closing.lock() {
            Ok(slot) => slot,
            Err(error) => error.into_inner(),
        };
        if let Some(cycle) = slot.as_ref() {
            cycle.cleanup_failed.store(true, Ordering::SeqCst);
        }
    }
    fn run<T>(
        &mut self,
        action: impl FnOnce(&mut dyn Backend) -> Result<T, ControlOwnerError>,
    ) -> Result<T, ControlError> {
        if !self.current() {
            self.fail();
            return Err(ControlError::Stale);
        }
        let attempted = {
            match self.ticket.access.0.backend.try_lock() {
                Ok(mut backend) => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    action(backend.as_mut())
                }))
                .map_err(|_| ControlError::Unavailable),
                Err(_) => Err(ControlError::Unavailable),
            }
        };
        let result = match attempted {
            Ok(result) => result,
            Err(error) => {
                self.mark_cleanup_failed();
                self.fail();
                return Err(error);
            }
        };
        if !self.current() {
            self.fail();
            return Err(ControlError::Stale);
        }
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                self.fail();
                Err(ControlError::Owner(error))
            }
        }
    }
    pub(crate) fn prepare(&mut self) -> Result<ControlPreview, ControlError> {
        if self.phase != Phase::New || !matches!(self.kind, Kind::CreateEmpty | Kind::Unlock) {
            self.fail();
            return Err(ControlError::Invalid);
        }
        let kind = self.kind;
        let value = self.run(|backend| backend.control_prepare(kind))?;
        self.phase = Phase::Prepared;
        Ok(value)
    }
    pub(crate) fn confirm(&mut self, id: &str) -> Result<(), ControlError> {
        if self.phase != Phase::Prepared || !matches!(self.kind, Kind::CreateEmpty | Kind::Unlock) {
            self.fail();
            return Err(ControlError::Invalid);
        }
        self.run(|backend| backend.control_confirm(id))?;
        self.phase = Phase::Confirmed;
        Ok(())
    }
    pub(crate) fn inspect_recovery(&mut self) -> Result<RecoveryPreview, ControlError> {
        if self.phase != Phase::New || self.kind != Kind::Recovery {
            self.fail();
            return Err(ControlError::Invalid);
        }
        let value = self.run(|backend| backend.control_inspect())?;
        self.phase = Phase::Inspected;
        Ok(value)
    }
    pub(crate) fn reconcile(&mut self, id: &str) -> Result<(), ControlError> {
        if self.phase != Phase::Inspected || self.kind != Kind::Recovery {
            self.fail();
            return Err(ControlError::Invalid);
        }
        self.run(|backend| backend.control_reconcile(id))?;
        self.phase = Phase::Reconciled;
        Ok(())
    }
    pub(crate) fn resume(self) -> Result<(), ControlError> {
        self.resume_inner(|| {})
    }
    #[cfg(test)]
    pub(crate) fn resume_paused(
        self,
        entered: std::sync::mpsc::Sender<()>,
        release: std::sync::mpsc::Receiver<()>,
    ) -> Result<(), ControlError> {
        self.resume_inner(move || {
            entered.send(()).unwrap();
            release.recv().unwrap();
        })
    }
    fn resume_inner(mut self, after_resume: impl FnOnce()) -> Result<(), ControlError> {
        if !matches!(self.kind, Kind::CreateEmpty | Kind::Unlock)
            || self.phase != Phase::Confirmed
            || !self.current()
        {
            return Err(ControlError::Invalid);
        }
        let (mut slot, healthy) = self
            .ticket
            .current_slot()
            .map_err(|_| ControlError::Stale)?;
        if !healthy {
            return Err(ControlError::Unavailable);
        }
        let backend = self
            .ticket
            .access
            .0
            .backend
            .try_lock()
            .map_err(|_| ControlError::Unavailable)?;
        if !backend.control_is_open() {
            return Err(ControlError::Unavailable);
        }
        drop(backend);
        if !self.ticket.access.purge_retained() {
            return Err(ControlError::Unavailable);
        }
        // Serialize count release, resume and slot removal against cancellation
        // and every SourceAccess admission path. No callback or retained plan
        // survives this consumed owner; no old result becomes current again.
        drop(self.lease.take());
        if self
            .ticket
            .access
            .0
            .clients
            .resume(&self.ticket.access.0.gate, &self.ticket.cycle.drain)
            .is_err()
        {
            drop(slot);
            return Err(ControlError::Unavailable);
        }
        after_resume();
        *slot = None;
        self.finished = true;
        Ok(())
    }
}
impl Drop for Control {
    fn drop(&mut self) {
        if !self.finished {
            self.close_owner();
        }
        // Key/plan cleanup precedes the last actual count release. When a
        // cleanup cannot be proved, the current close cycle remains unhealthy.
        drop(self.lease.take());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panicking_backend_callback_marks_cycle_unhealthy_and_keeps_admission_closed() {
        let gate = Gate::new();
        let access = SourceAccess::close_protocol_fixture(gate.clone());
        let ticket = access.request_close().unwrap();
        assert_eq!(ticket.release_key(), Ok(true));
        let mut control = ticket.start_control(Kind::CreateEmpty).unwrap();
        let result = control.run::<()>(|_| panic!("synthetic control callback failure"));
        assert_eq!(result, Err(ControlError::Unavailable));
        let current = access.request_close().unwrap();
        assert_eq!(current.is_quiescent(), Ok(false));
        drop(control);
        assert_eq!(current.is_quiescent(), Ok(true));
        assert_eq!(current.release_key(), Ok(false));
        assert!(matches!(
            current.start_control(Kind::CreateEmpty),
            Err(ControlError::Unavailable)
        ));
        assert!(access.begin(Lane::Read).is_err());
        assert!(gate.begin(Lane::Model).is_err());
    }
}
