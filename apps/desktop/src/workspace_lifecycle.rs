//! Shared native admission and drain custody. This does not lock storage, erase
//! retained data, cancel operations, or grant execution authority.
//!
//! Custody cannot be cloned or serialized, and internal state is never exposed.
//! ```compile_fail
//! use rangoon_desktop::workspace_lifecycle::{Gate, Lane};
//! let lease = Gate::new().begin(Lane::Read).unwrap();
//! let duplicate = lease.clone();
//! ```
//! ```compile_fail
//! use rangoon_desktop::workspace_lifecycle::Gate;
//! let drain = Gate::new().request_drain().unwrap();
//! let duplicate = drain.clone();
//! ```
//! ```compile_fail
//! use rangoon_desktop::workspace_lifecycle::Gate;
//! println!("{:?}", Gate::new());
//! ```
//! ```compile_fail
//! use rangoon_desktop::workspace_lifecycle::{Gate, Lane};
//! println!("{:?}", Gate::new().begin(Lane::Read).unwrap());
//! ```
//! ```compile_fail
//! use rangoon_desktop::workspace_lifecycle::Gate;
//! println!("{:?}", Gate::new().request_drain().unwrap());
//! ```
//! ```compile_fail
//! use rangoon_desktop::workspace_lifecycle::Gate;
//! serde_json::to_string(&Gate::new()).unwrap();
//! ```
//! ```compile_fail
//! use rangoon_desktop::workspace_lifecycle::{Gate, Lane};
//! serde_json::to_string(&Gate::new().begin(Lane::Read).unwrap()).unwrap();
//! ```
//! ```compile_fail
//! use rangoon_desktop::workspace_lifecycle::Gate;
//! serde_json::to_string(&Gate::new().request_drain().unwrap()).unwrap();
//! ```
use std::sync::{Arc, Mutex, MutexGuard};

const MAX_READERS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    Read,
    Exclusive,
    Model,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Busy,
    Closed,
    Stale,
    Unavailable,
    Exhausted,
}

#[derive(Clone, Default)]
pub struct Gate(Arc<Mutex<State>>);

#[derive(Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Open,
    Draining,
    Exhausted,
}

#[derive(Default)]
struct State {
    phase: Phase,
    generation: u64,
    readers: usize,
    exclusive: bool,
    model: bool,
}
impl State {
    fn idle(&self) -> bool {
        self.readers == 0 && !self.exclusive && !self.model
    }
    fn advance(&mut self) -> Result<(), Error> {
        match self.generation.checked_add(1) {
            Some(next) => {
                self.generation = next;
                Ok(())
            }
            None => {
                self.phase = Phase::Exhausted;
                Err(Error::Exhausted)
            }
        }
    }
}

/// Move this owner into the actual worker or retained callback. An IPC waiter
/// dropping does not release a lease still owned by its callback.
pub struct Lease {
    gate: Gate,
    lane: Lane,
    generation: u64,
}

/// Trusted host custody only. Dropping this token never reopens admission.
pub struct Drain {
    gate: Gate,
    generation: u64,
}

impl Gate {
    pub fn new() -> Self {
        Self::default()
    }
    fn state(&self) -> Result<MutexGuard<'_, State>, Error> {
        self.0.lock().map_err(|_| Error::Unavailable)
    }
    pub fn begin(&self, lane: Lane) -> Result<Lease, Error> {
        let mut state = self.state()?;
        if state.phase != Phase::Open {
            return Err(Error::Closed);
        }
        match lane {
            Lane::Read if state.readers < MAX_READERS => state.readers += 1,
            Lane::Exclusive if !state.exclusive => state.exclusive = true,
            Lane::Model if !state.model => state.model = true,
            _ => return Err(Error::Busy),
        }
        Ok(Lease {
            gate: self.clone(),
            lane,
            generation: state.generation,
        })
    }
    pub fn request_drain(&self) -> Result<Drain, Error> {
        let mut state = self.state()?;
        match state.phase {
            Phase::Draining => return Err(Error::Closed),
            Phase::Exhausted => return Err(Error::Exhausted),
            Phase::Open => {}
        }
        state.advance()?;
        state.phase = Phase::Draining;
        Ok(Drain {
            gate: self.clone(),
            generation: state.generation,
        })
    }
    pub fn resume(&self, drain: &Drain) -> Result<(), Error> {
        if !Arc::ptr_eq(&self.0, &drain.gate.0) {
            return Err(Error::Stale);
        }
        let mut state = self.state()?;
        if state.phase == Phase::Exhausted {
            return Err(Error::Exhausted);
        }
        if state.phase != Phase::Draining || state.generation != drain.generation {
            return Err(Error::Stale);
        }
        if !state.idle() {
            return Err(Error::Busy);
        }
        state.advance()?;
        state.phase = Phase::Open;
        Ok(())
    }
    #[cfg(test)]
    fn seed_generation(&self, value: u64) -> Result<(), Error> {
        let mut state = self.state()?;
        if state.phase != Phase::Open || !state.idle() {
            return Err(Error::Busy);
        }
        state.generation = value;
        Ok(())
    }
    #[cfg(test)]
    fn poison_for_test(&self) {
        let gate = self.clone();
        let _ = std::thread::spawn(move || {
            let _state = gate.0.lock().unwrap();
            panic!("synthetic admission poison");
        })
        .join();
    }
}

impl Lease {
    pub fn is_current(&self) -> Result<bool, Error> {
        let state = self.gate.state()?;
        Ok(state.phase == Phase::Open && state.generation == self.generation)
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        // Poison is sticky. Never repair or clear custody after an unknown
        // state transition; admission remains unavailable until restart.
        if let Ok(mut state) = self.gate.state() {
            match self.lane {
                Lane::Read => state.readers -= 1,
                Lane::Exclusive => state.exclusive = false,
                Lane::Model => state.model = false,
            }
        }
    }
}
impl Drain {
    pub fn is_quiescent(&self) -> Result<bool, Error> {
        let state = self.gate.state()?;
        if state.phase == Phase::Exhausted {
            return Err(Error::Exhausted);
        }
        if state.phase != Phase::Draining || state.generation != self.generation {
            return Err(Error::Stale);
        }
        Ok(state.idle())
    }
}

#[cfg(test)]
#[path = "workspace_lifecycle_tests.rs"]
mod tests;
