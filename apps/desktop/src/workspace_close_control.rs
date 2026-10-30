//! Trusted native supervisor. The production encrypted factory is not mounted.
use super::*;
use crate::workspace_source_access::{ControlError, ControlKind, SourceControl};
use rangoon_desktop::workspace_session::{Preview, RecoveryPreview};

type CleanupCallback = Box<dyn FnOnce(bool) -> bool + Send>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    New,
    Prepared,
    Confirmed,
    Inspected,
    Reconciled,
    Failed,
}

/// Store only the actual source owner here, never a Flight or AppHandle. This
/// avoids a coordinator -> Flight -> coordinator reference cycle.
pub(super) struct Retained {
    serial: u64,
    active: bool,
    stage: Stage,
    kind: ControlKind,
    control: Option<SourceControl>,
}

pub(crate) struct Flight {
    close: WorkspaceClose,
    serial: u64,
    stage: Stage,
    control: Option<SourceControl>,
    cleanup: Option<CleanupCallback>,
    disarmed: bool,
}

/// Borrowed custody for bounded native startup work. It cannot outlive the
/// Flight, be decoded from IPC, or resume the content registry.
#[cfg(feature = "encrypted-workspace")]
pub(crate) struct StartupPermit<'a> {
    flight: &'a Flight,
}

/// Retained native plan identity. Neither a request ID nor a standalone grant:
/// every use still requires the same live coordinator and exact control cycle.
#[cfg(feature = "encrypted-workspace")]
pub(crate) struct StartupBinding {
    coordinator: std::sync::Weak<Mutex<State>>,
    serial: u64,
}

#[cfg(feature = "encrypted-workspace")]
impl StartupPermit<'_> {
    pub(crate) fn binding(&self) -> Result<StartupBinding, ControlError> {
        self.checkpoint()?;
        Ok(StartupBinding {
            coordinator: Arc::downgrade(&self.flight.close.state),
            serial: self.flight.serial,
        })
    }
    pub(crate) fn check_binding(&self, binding: &StartupBinding) -> Result<(), ControlError> {
        if binding.serial != self.flight.serial
            || !binding
                .coordinator
                .upgrade()
                .is_some_and(|coordinator| Arc::ptr_eq(&coordinator, &self.flight.close.state))
        {
            return Err(ControlError::Stale);
        }
        self.checkpoint()
    }
    pub(crate) fn checkpoint(&self) -> Result<(), ControlError> {
        if self.flight.stage != Stage::New || !self.flight.current() {
            return Err(ControlError::Stale);
        }
        let control = self.flight.control.as_ref().ok_or(ControlError::Stale)?;
        control.startup_checkpoint()?;
        // C0 cleanup allowed this control to be created. Source-bearing startup
        // work also needs the fresh C1 renderer acknowledgment; caller-supplied
        // identities and the old completed cycle cannot stand in for it.
        let ready = self.flight.close.state.lock().is_ok_and(|state| {
            !state.exiting
                && !state.failed
                && state.control.as_ref().is_some_and(|retained| {
                    retained.serial == self.flight.serial && retained.active
                })
                && state.attempt.as_ref().is_some_and(|attempt| {
                    attempt.progress.lock().is_ok_and(|progress| {
                        progress.healthy
                            && progress.initial_done
                            && !progress.finishing
                            && progress.acknowledged
                    })
                })
        });
        if !ready {
            return Err(ControlError::Unavailable);
        }
        control.startup_checkpoint()
    }
}

fn attempt(
    ticket: CloseTicket,
    id: Option<String>,
    epoch: Option<String>,
    healthy: bool,
) -> Arc<Attempt> {
    Arc::new(Attempt {
        ticket,
        id,
        epoch,
        progress: Mutex::new(Progress {
            initial_done: true,
            finishing: false,
            native_done: false,
            healthy,
            acknowledged: false,
        }),
    })
}
fn random(bytes: &mut [u8]) -> Result<(), ()> {
    getrandom::fill(bytes).map_err(|_| ())
}

impl WorkspaceClose {
    /// Observation for the bounded native waiter only. A true value is not a
    /// permit: the retained Flight still rechecks its exact serial and lease.
    #[cfg(feature = "encrypted-workspace")]
    pub(crate) fn startup_acknowledged(&self) -> Result<bool, ControlError> {
        let state = self.state.lock().map_err(|_| ControlError::Unavailable)?;
        if state.exiting
            || state.failed
            || self.source.failure_latched()
            || !state.control.as_ref().is_some_and(|retained| {
                retained.kind == ControlKind::Startup
                    && retained.stage == Stage::New
                    && retained.active
            })
        {
            return Err(ControlError::Unavailable);
        }
        let attempt = state.attempt.as_ref().ok_or(ControlError::Unavailable)?;
        let progress = attempt
            .progress
            .lock()
            .map_err(|_| ControlError::Unavailable)?;
        if !progress.healthy || !progress.initial_done || progress.finishing {
            return Err(ControlError::Unavailable);
        }
        Ok(progress.acknowledged)
    }

    /// Native startup may begin only from the coordinator's actual completed
    /// close. Returned identifiers match requests; they do not replace custody.
    #[cfg(feature = "encrypted-workspace")]
    pub(crate) fn locked_startup_cycle(&self) -> Result<(String, String), ControlError> {
        let state = self.state.lock().map_err(|_| ControlError::Unavailable)?;
        if state.exiting
            || state.control.is_some()
            || self.source.failure_latched()
            || !healthy_attempt(&state)
        {
            return Err(ControlError::Unavailable);
        }
        let attempt = state.attempt.as_ref().ok_or(ControlError::Unavailable)?;
        let progress = attempt
            .progress
            .lock()
            .map_err(|_| ControlError::Unavailable)?;
        if !progress.native_done || !progress.acknowledged {
            return Err(ControlError::Unavailable);
        }
        Ok((
            attempt.id.clone().ok_or(ControlError::Unavailable)?,
            attempt.epoch.clone().ok_or(ControlError::Unavailable)?,
        ))
    }
    /// Call only after native consent policy and main-window identity are
    /// qualified. This private seam does not establish either by itself.
    #[cfg(test)]
    pub(crate) fn start_control(
        &self,
        kind: ControlKind,
        cleanup: impl FnOnce(bool) -> bool + Send + 'static,
    ) -> Result<Flight, ControlError> {
        self.start_control_inner(None, kind, cleanup, random)
    }
    #[cfg(test)]
    fn start_control_with_entropy(
        &self,
        kind: ControlKind,
        cleanup: impl FnOnce(bool) -> bool + Send + 'static,
        entropy: impl FnOnce(&mut [u8]) -> Result<(), ()>,
    ) -> Result<Flight, ControlError> {
        self.start_control_inner(None, kind, cleanup, entropy)
    }
    pub(crate) fn start_control_for(
        &self,
        id: &str,
        epoch: &str,
        kind: ControlKind,
        cleanup: impl FnOnce(bool) -> bool + Send + 'static,
    ) -> Result<Flight, ControlError> {
        self.start_control_inner(Some((id, epoch)), kind, cleanup, random)
    }
    fn start_control_inner(
        &self,
        expected: Option<(&str, &str)>,
        kind: ControlKind,
        cleanup: impl FnOnce(bool) -> bool + Send + 'static,
        entropy: impl FnOnce(&mut [u8]) -> Result<(), ()>,
    ) -> Result<Flight, ControlError> {
        let mut state = self.state.lock().map_err(|_| ControlError::Unavailable)?;
        if expected.is_some_and(|(id, epoch)| !matches_attempt(&state, id, epoch)) {
            return Err(ControlError::Stale);
        }
        if state.exiting || state.failed || self.source.failure_latched() || state.control.is_some()
        {
            return Err(ControlError::Unavailable);
        }
        let old = state.attempt.clone().ok_or(ControlError::Invalid)?;
        let mut progress = old.progress.lock().map_err(|_| ControlError::Unavailable)?;
        if !progress.healthy
            || !progress.initial_done
            || !progress.native_done
            || !progress.acknowledged
            || progress.finishing
        {
            return Err(ControlError::Unavailable);
        }
        let (id, epoch) = next_identity(&mut state, entropy);
        if id.is_none() || epoch.is_none() {
            state.failed = true;
            return Err(ControlError::Unavailable);
        }
        let serial = state.epoch;
        // Reserve Closing before start_control can replace its source cycle and
        // fail. Never retain an old native_done/ack proof after that mutation.
        progress.native_done = false;
        progress.acknowledged = false;
        state.control = Some(Retained {
            serial,
            active: true,
            stage: Stage::New,
            kind,
            control: None,
        });
        #[cfg(test)]
        let source_result = if state.fail_control_after_swap {
            old.ticket.fail_control_after_swap_for_test(kind)
        } else {
            old.ticket.start_control(kind)
        };
        #[cfg(not(test))]
        let source_result = old.ticket.start_control(kind);
        let control = match source_result {
            Ok(control) => control,
            Err(error) => {
                state.failed = true;
                state.control = None;
                if let Ok(ticket) = self.source.request_close() {
                    state.attempt = Some(attempt(ticket, id, epoch, false));
                }
                return Err(error);
            }
        };
        let ticket = match self.source.request_close() {
            Ok(ticket) => ticket,
            Err(()) => {
                state.failed = true;
                state.control = None;
                drop(progress);
                drop(state);
                drop(control);
                return Err(ControlError::Unavailable);
            }
        };
        state.attempt = Some(attempt(ticket, id, epoch, true));
        Ok(Flight {
            close: self.clone(),
            serial,
            stage: Stage::New,
            control: Some(control),
            cleanup: Some(Box::new(cleanup)),
            disarmed: false,
        })
    }
    pub(crate) fn take_control_ready(
        &self,
        id: &str,
        epoch: &str,
        cleanup: impl FnOnce(bool) -> bool + Send + 'static,
    ) -> Result<Flight, ControlError> {
        self.take_control_inner(id, epoch, true, cleanup)
    }
    pub(crate) fn take_control(
        &self,
        id: &str,
        epoch: &str,
        cleanup: impl FnOnce(bool) -> bool + Send + 'static,
    ) -> Result<Flight, ControlError> {
        self.take_control_inner(id, epoch, false, cleanup)
    }
    fn take_control_inner(
        &self,
        id: &str,
        epoch: &str,
        require_ready: bool,
        cleanup: impl FnOnce(bool) -> bool + Send + 'static,
    ) -> Result<Flight, ControlError> {
        let mut state = self.state.lock().map_err(|_| ControlError::Unavailable)?;
        if state.exiting
            || !healthy_attempt(&state)
            || self.source.failure_latched()
            || !matches_attempt(&state, id, epoch)
        {
            return Err(ControlError::Stale);
        }
        // Exact C0 completed native cleanup before creating this New control.
        // C1 itself cannot become terminal while its ClosedLease is retained.
        if require_ready
            && !state.attempt.as_ref().is_some_and(|attempt| {
                attempt.progress.lock().is_ok_and(|progress| {
                    progress.healthy
                        && progress.initial_done
                        && !progress.finishing
                        && progress.acknowledged
                })
            })
        {
            return Err(ControlError::Unavailable);
        }
        let retained = state.control.as_mut().ok_or(ControlError::Stale)?;
        if require_ready && (retained.stage != Stage::New || retained.kind == ControlKind::Recovery)
        {
            return Err(ControlError::Invalid);
        }
        if retained.active {
            return Err(ControlError::Busy);
        }
        let control = retained.control.take().ok_or(ControlError::Stale)?;
        retained.active = true;
        Ok(Flight {
            close: self.clone(),
            serial: retained.serial,
            stage: retained.stage,
            control: Some(control),
            cleanup: Some(Box::new(cleanup)),
            disarmed: false,
        })
    }
    pub(crate) fn inspect_control(&self) -> (View, Option<ControlKind>, &'static str) {
        if !self.source.encrypted_close_available() {
            return (self.snapshot(None), None, "unavailable");
        }
        let (state, poisoned) = match self.state.lock() {
            Ok(state) => (state, false),
            Err(error) => (error.into_inner(), true),
        };
        let view = self.snapshot_locked(&state, poisoned, None);
        let (kind, phase) = match view.state {
            Phase::Unavailable => (None, "unavailable"),
            Phase::RecoveryRequired => (None, "recovery_required"),
            Phase::Open => (None, "open"),
            Phase::Locked => (None, "locked"),
            Phase::Closing => match state.control.as_ref() {
                Some(retained) if retained.kind == ControlKind::Recovery => {
                    (None, "recovery_required")
                }
                Some(retained) => (
                    Some(retained.kind),
                    if retained.active {
                        "active"
                    } else {
                        "awaiting_confirmation"
                    },
                ),
                None => (None, "closing"),
            },
        };
        (view, kind, phase)
    }
    pub(crate) fn cancel_control(
        &self,
        id: &str,
        epoch: &str,
        cleanup: impl FnOnce(bool) -> bool,
    ) -> View {
        self.cancel_control_inner(Some((id, epoch)), cleanup)
    }
    pub(super) fn cancel_any_control(&self, cleanup: impl FnOnce(bool) -> bool) -> View {
        self.cancel_control_inner(None, cleanup)
    }
    fn cancel_control_inner(
        &self,
        expected: Option<(&str, &str)>,
        cleanup: impl FnOnce(bool) -> bool,
    ) -> View {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(error) => {
                drop(error.into_inner());
                return self.poisoned_cleanup(cleanup);
            }
        };
        if state.failed || expected.is_some_and(|(id, epoch)| !matches_attempt(&state, id, epoch)) {
            drop(state);
            return self.rejected();
        }
        if state.control.is_none() {
            drop(state);
            return if expected.is_none() {
                self.begin(cleanup)
            } else {
                self.rejected()
            };
        }
        let prior_healthy = healthy_attempt(&state);
        state.failed |= !prior_healthy;
        let old = state.attempt.clone();
        // State owns the serial check and synchronous source transition. An old
        // cancel after success never starts or renews another source drain.
        let renewed = old.as_ref().and_then(|a| a.ticket.renew_control().ok());
        let retained = state.control.take().and_then(|mut c| c.control.take());
        let (id, epoch) = next_identity(&mut state, random);
        match renewed {
            Some(ticket) => {
                let healthy = prior_healthy && id.is_some() && epoch.is_some();
                state.failed |= !healthy;
                state.attempt = Some(attempt(ticket, id, epoch, healthy));
            }
            None => state.failed = true,
        }
        drop(state);
        // Control cleanup locks Owner before its lease drops. Never do this
        // under State; its finalizer may need current-cycle bookkeeping.
        drop(retained);
        self.inspect(cleanup)
    }
    pub(super) fn purge_poisoned_control(&self) {
        let retained = {
            // Called only after a coordinator or progress mutex failed. A
            // healthy State mutex does not make poisoned progress operational.
            let mut state = match self.state.lock() {
                Ok(state) => state,
                Err(error) => error.into_inner(),
            };
            state.failed = true;
            if state.control.is_some()
                && let Ok(ticket) = self.source.request_close()
            {
                let _ = ticket.renew_control();
            }
            state.control.take().and_then(|mut c| c.control.take())
        };
        drop(retained);
    }
    #[cfg(test)]
    pub(crate) fn fail_source_swap_for_test(&self) {
        self.state.lock().unwrap().fail_control_after_swap = true;
    }
    #[cfg(test)]
    pub(crate) fn exhaust_control_epoch_for_test(&self) {
        self.state.lock().unwrap().epoch = u64::MAX;
    }
    #[cfg(test)]
    pub(crate) fn fail_control_entropy_for_test(
        &self,
        kind: ControlKind,
    ) -> Result<Flight, ControlError> {
        self.start_control_with_entropy(kind, |_| true, |_| Err(()))
    }
    #[cfg(test)]
    pub(crate) fn poison_control_progress_for_test(&self) {
        let current = self.state.lock().unwrap().attempt.clone().unwrap();
        let _ = std::thread::spawn(move || {
            let _held = current.progress.lock().unwrap();
            panic!("synthetic control-progress poison");
        })
        .join();
    }
    #[cfg(test)]
    pub(crate) fn poison_control_state_for_test(&self) {
        let close = self.clone();
        let _ = std::thread::spawn(move || {
            let _held = close.state.lock().unwrap();
            panic!("synthetic control-state poison");
        })
        .join();
    }
}
fn matches_attempt(state: &State, id: &str, epoch: &str) -> bool {
    state
        .attempt
        .as_ref()
        .is_some_and(|a| a.id.as_deref() == Some(id) && a.epoch.as_deref() == Some(epoch))
}
fn healthy_attempt(state: &State) -> bool {
    !state.failed
        && state.attempt.as_ref().is_some_and(|a| {
            a.progress
                .lock()
                .is_ok_and(|p| p.healthy && p.initial_done && !p.finishing)
        })
}
impl Flight {
    #[cfg(feature = "encrypted-workspace")]
    pub(crate) fn check_bound_identity(&mut self, expected: &str) -> Result<(), ControlError> {
        if self.stage != Stage::New {
            return Err(ControlError::Invalid);
        }
        self.apply(|control| control.check_bound_identity(expected))
    }
    #[cfg(feature = "encrypted-workspace")]
    pub(crate) fn startup_cycle(&self) -> Result<(String, String), ControlError> {
        if self.stage != Stage::New || !self.current() {
            return Err(ControlError::Stale);
        }
        self.control
            .as_ref()
            .ok_or(ControlError::Stale)?
            .startup_checkpoint()?;
        let state = self
            .close
            .state
            .lock()
            .map_err(|_| ControlError::Unavailable)?;
        let attempt = state.attempt.as_ref().ok_or(ControlError::Stale)?;
        Ok((
            attempt.id.clone().ok_or(ControlError::Unavailable)?,
            attempt.epoch.clone().ok_or(ControlError::Unavailable)?,
        ))
    }
    #[cfg(feature = "encrypted-workspace")]
    pub(crate) fn startup_permit(&self) -> Result<StartupPermit<'_>, ControlError> {
        let permit = StartupPermit { flight: self };
        permit.checkpoint()?;
        Ok(permit)
    }
    #[cfg(feature = "encrypted-workspace")]
    pub(crate) fn bind_locked_owner(
        &mut self,
        proof: crate::workspace_startup::BindingProof,
    ) -> Result<(), ControlError> {
        if self.stage != Stage::New {
            return Err(ControlError::Invalid);
        }
        self.apply(|control| control.bind_locked_owner(proof))
    }
    fn current(&self) -> bool {
        self.close.state.lock().is_ok_and(|s| {
            !s.exiting
                && healthy_attempt(&s)
                && s.control
                    .as_ref()
                    .is_some_and(|c| c.serial == self.serial && c.active)
        })
    }
    fn apply<T>(
        &mut self,
        action: impl FnOnce(&mut SourceControl) -> Result<T, ControlError>,
    ) -> Result<T, ControlError> {
        if !self.current() {
            self.stage = Stage::Failed;
            return Err(ControlError::Stale);
        }
        let result = action(self.control.as_mut().ok_or(ControlError::Stale)?);
        if !self.current() {
            self.stage = Stage::Failed;
            return Err(ControlError::Stale);
        }
        if result.is_err() {
            self.stage = Stage::Failed;
        }
        result
    }
    pub(crate) fn prepare(&mut self) -> Result<Preview, ControlError> {
        let result = self.apply(SourceControl::prepare)?;
        self.stage = Stage::Prepared;
        Ok(result)
    }
    pub(crate) fn confirm(&mut self, id: &str) -> Result<(), ControlError> {
        self.apply(|control| control.confirm(id))?;
        self.stage = Stage::Confirmed;
        Ok(())
    }
    pub(crate) fn inspect_recovery(&mut self) -> Result<RecoveryPreview, ControlError> {
        let result = self.apply(SourceControl::inspect_recovery)?;
        self.stage = Stage::Inspected;
        Ok(result)
    }
    pub(crate) fn reconcile(&mut self, id: &str) -> Result<(), ControlError> {
        self.apply(|control| control.reconcile(id))?;
        self.stage = Stage::Reconciled;
        Ok(())
    }
    pub(crate) fn retain(mut self) -> Result<(), ControlError> {
        if !matches!(self.stage, Stage::New | Stage::Prepared | Stage::Inspected) {
            return Err(ControlError::Invalid);
        }
        let mut state = self
            .close
            .state
            .lock()
            .map_err(|_| ControlError::Unavailable)?;
        if state.exiting || !healthy_attempt(&state) {
            return Err(ControlError::Unavailable);
        }
        let retained = state
            .control
            .as_mut()
            .filter(|c| c.serial == self.serial && c.active)
            .ok_or(ControlError::Stale)?;
        retained.control = self.control.take();
        retained.active = false;
        retained.stage = self.stage;
        // State retains Control only. This Flight must not clear or finalize it.
        self.disarmed = true;
        self.cleanup = None;
        Ok(())
    }
    pub(crate) fn resume(mut self) -> Result<View, ControlError> {
        if self.stage != Stage::Confirmed {
            return Err(ControlError::Invalid);
        }
        let mut state = self
            .close
            .state
            .lock()
            .map_err(|_| ControlError::Unavailable)?;
        if state.exiting
            || !healthy_attempt(&state)
            || !state
                .control
                .as_ref()
                .is_some_and(|c| c.serial == self.serial && c.active)
        {
            return Err(ControlError::Stale);
        }
        self.control.take().ok_or(ControlError::Stale)?.resume()?;
        state.control = None;
        state.attempt = None;
        self.disarmed = true;
        self.cleanup = None;
        drop(state);
        Ok(self.close.snapshot(None))
    }
}
impl Drop for Flight {
    fn drop(&mut self) {
        if self.disarmed {
            return;
        }
        // Drop actual Owner/key custody before taking the coordinator mutex.
        drop(self.control.take());
        match self.close.state.lock() {
            Ok(mut state) => {
                if state
                    .control
                    .as_ref()
                    .is_some_and(|c| c.serial == self.serial)
                {
                    state.control = None;
                }
            }
            Err(error) => {
                error.into_inner().failed = true;
            }
        }
        if let Some(cleanup) = self.cleanup.take() {
            let _ = self.close.inspect(cleanup);
        }
    }
}
