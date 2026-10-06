//! Application-owned, memory-only model request custody and saved-input resolution.
//!
//! This service performs no network calls or native dialogs. An operation lease is
//! not operator consent: the native caller must implement the reviewed dialog and
//! workspace-guard sequence before giving the retained request to transport.
#![forbid(unsafe_code)]

mod inputs;
pub use inputs::{Dependency, Selector, Task};
use inputs::{Packed, Selection};
use rangoon_model_assistance::ContextPack;
use rangoon_model_local::{Cancellation, LocalProfile, PreparedRequest};
use rangoon_store::Workspace;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Write,
    sync::{Arc, Mutex, MutexGuard},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Diagnostic {
    InvalidRequest,
    InvalidProfile,
    Unconfigured,
    Busy,
    SessionUnavailable,
    StalePrepared,
    RunNotFound,
    InputUnavailable,
    InputEmpty,
    InputStale,
    PackInvalid,
    PackOverBudget,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Prepare,
    Check,
    Send,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveView {
    pub run_id: String,
    pub kind: OperationKind,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub generation: String,
    pub profile: Option<LocalProfile>,
    pub active: Option<ActiveView>,
    pub persistence: &'static str,
    pub processing_location: &'static str,
    pub retention: &'static str,
    pub authority: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedView {
    pub generation: String,
    pub prepared_id: String,
    pub request_id: String,
    pub origin: String,
    pub model: String,
    pub body_json: String,
    pub body_bytes: usize,
    pub inputs: Vec<Dependency>,
    pub pack: ContextPack,
    pub processing_location: &'static str,
    pub retention: &'static str,
    pub authority: &'static str,
}

struct Active {
    view: ActiveView,
    cancellation: Cancellation,
}
struct Staged {
    id: String,
    packed: Arc<Packed>,
}
#[derive(Default)]
struct State {
    generation: u64,
    profile: Option<LocalProfile>,
    active: Option<Active>,
    prepared: Option<Staged>,
}

/// Clones share custody and one active-operation slot. No state is persisted.
#[derive(Clone, Default)]
pub struct LocalSession(Arc<Mutex<State>>);

impl LocalSession {
    fn lock(&self) -> Result<MutexGuard<'_, State>, Diagnostic> {
        self.0.lock().map_err(|_| Diagnostic::SessionUnavailable)
    }

    pub fn inspect(&self) -> Result<SessionView, Diagnostic> {
        Ok(view(&*self.lock()?))
    }

    /// Invalid configuration preserves the current profile and preparation.
    pub fn configure(&self, raw: &[u8]) -> Result<SessionView, Diagnostic> {
        let profile = LocalProfile::parse(raw).map_err(|_| Diagnostic::InvalidProfile)?;
        let mut state = self.lock()?;
        if state.active.is_some() {
            return Err(Diagnostic::Busy);
        }
        invalidate(&mut state)?;
        state.profile = Some(profile);
        Ok(view(&state))
    }

    /// Clear invalidates immediately, but an active lease owns its slot until drop.
    pub fn clear(&self) -> Result<SessionView, Diagnostic> {
        let mut state = self.lock()?;
        invalidate(&mut state)?;
        Ok(view(&state))
    }

    pub fn cancel(&self, raw: &[u8]) -> Result<(), Diagnostic> {
        let input: Cancel = decode(raw, 256)?;
        if input.schema_version != "rangoon.local-cancel.v1" || !valid_handle(&input.run_id, "run:")
        {
            return Err(Diagnostic::InvalidRequest);
        }
        let mut state = self.lock()?;
        let active = state
            .active
            .as_ref()
            .filter(|a| a.view.run_id == input.run_id)
            .ok_or(Diagnostic::RunNotFound)?;
        active.cancellation.cancel();
        state.prepared = None;
        Ok(())
    }

    pub fn begin_check(&self) -> Result<Operation, Diagnostic> {
        self.start(OperationKind::Check)
    }

    /// Every new selection attempt discards the previous preparation, including busy failures.
    pub fn begin_prepare(&self, raw: &[u8]) -> Result<Preparation, Diagnostic> {
        let operation = self.start(OperationKind::Prepare)?;
        let selection = Selection::parse(raw)?;
        Ok(Preparation {
            operation,
            selection,
        })
    }

    /// Claim consumes the exact handle. The caller must still obtain native consent.
    pub fn begin_send(&self, raw: &[u8]) -> Result<Transmission, Diagnostic> {
        let input: Send = decode(raw, 512)?;
        if input.schema_version != "rangoon.local-send.v1"
            || !valid_handle(&input.prepared_id, "prepared:")
            || !valid_digest(&input.request_id)
        {
            return Err(Diagnostic::InvalidRequest);
        }
        let id = random_handle("run:")?;
        let mut state = self.lock()?;
        if state.active.is_some() {
            return Err(Diagnostic::Busy);
        }
        let profile = state.profile.clone().ok_or(Diagnostic::Unconfigured)?;
        let staged = state
            .prepared
            .as_ref()
            .filter(|p| {
                p.id == input.prepared_id && p.packed.request.request_id() == input.request_id
            })
            .ok_or(Diagnostic::StalePrepared)?;
        let packed = Arc::clone(&staged.packed);
        state.prepared = None;
        let operation = install(self, &mut state, profile, id, OperationKind::Send);
        Ok(Transmission { operation, packed })
    }

    fn start(&self, kind: OperationKind) -> Result<Operation, Diagnostic> {
        self.start_with(kind, |bytes| {
            getrandom::fill(bytes).map_err(|_| Diagnostic::SessionUnavailable)
        })
    }

    fn start_with(
        &self,
        kind: OperationKind,
        random: impl FnOnce(&mut [u8]) -> Result<(), Diagnostic>,
    ) -> Result<Operation, Diagnostic> {
        let mut state = self.lock()?;
        // A new selection attempt must never leave an old selection sendable,
        // including when another operation prevents preparing its replacement.
        if kind == OperationKind::Prepare {
            state.prepared = None;
        }
        if state.active.is_some() {
            return Err(Diagnostic::Busy);
        }
        let profile = state.profile.clone().ok_or(Diagnostic::Unconfigured)?;
        let id = handle_with("run:", random)?;
        Ok(install(self, &mut state, profile, id, kind))
    }
}

fn invalidate(state: &mut State) -> Result<(), Diagnostic> {
    state.profile = None;
    state.prepared = None;
    if let Some(active) = &state.active {
        active.cancellation.cancel();
    }
    state.generation = state
        .generation
        .checked_add(1)
        .ok_or(Diagnostic::SessionUnavailable)?;
    Ok(())
}

fn view(state: &State) -> SessionView {
    SessionView {
        generation: state.generation.to_string(),
        profile: state.profile.clone(),
        active: state.active.as_ref().map(|a| a.view.clone()),
        persistence: "session_only",
        processing_location: "unknown",
        retention: "unknown",
        authority: "none",
    }
}

fn install(
    session: &LocalSession,
    state: &mut State,
    profile: LocalProfile,
    id: String,
    kind: OperationKind,
) -> Operation {
    let cancellation = Cancellation::new();
    state.active = Some(Active {
        view: ActiveView {
            run_id: id.clone(),
            kind,
        },
        cancellation: cancellation.clone(),
    });
    Operation {
        session: session.clone(),
        generation: state.generation,
        id,
        profile,
        cancellation,
    }
}

/// A linear lease: not cloneable, releases only its own operation on drop.
/// It never denotes user consent, record freshness, or endpoint authentication.
pub struct Operation {
    session: LocalSession,
    generation: u64,
    id: String,
    profile: LocalProfile,
    cancellation: Cancellation,
}

impl Operation {
    pub fn run_id(&self) -> &str {
        &self.id
    }
    pub fn generation(&self) -> String {
        self.generation.to_string()
    }
    pub fn profile(&self) -> &LocalProfile {
        &self.profile
    }
    pub fn cancellation(&self) -> &Cancellation {
        &self.cancellation
    }

    pub fn ensure_current(&self) -> Result<(), Diagnostic> {
        self.check(&*self.session.lock()?)
    }

    fn check(&self, state: &State) -> Result<(), Diagnostic> {
        if self.cancellation.is_cancelled()
            || self.generation != state.generation
            || !state
                .active
                .as_ref()
                .is_some_and(|a| a.view.run_id == self.id)
            || !state
                .profile
                .as_ref()
                .is_some_and(|p| p.profile_sha256() == self.profile.profile_sha256())
        {
            return Err(Diagnostic::Cancelled);
        }
        Ok(())
    }
}

impl Drop for Operation {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Ok(mut state) = self.session.lock() {
            if state
                .active
                .as_ref()
                .is_some_and(|a| a.view.run_id == self.id)
            {
                state.active = None;
            }
        }
    }
}

pub struct Preparation {
    operation: Operation,
    selection: Selection,
}
impl Preparation {
    pub fn operation(&self) -> &Operation {
        &self.operation
    }

    /// Validated saved-record reads only; no network or database writes.
    pub fn prepare(self, store: &Workspace) -> Result<PreparedView, Diagnostic> {
        self.operation.ensure_current()?;
        let packed = inputs::prepare(store, &self.selection, self.operation.profile())?;
        self.operation.ensure_current()?;
        let id = random_handle("prepared:")?;
        let result = PreparedView {
            generation: self.operation.generation(),
            prepared_id: id.clone(),
            request_id: packed.request.request_id().to_owned(),
            origin: self.operation.profile.origin(),
            model: self.operation.profile.model().to_owned(),
            body_json: packed.request.body_json().to_owned(),
            body_bytes: packed.request.body_json().len(),
            inputs: packed.inputs.iter().map(|r| r.dependency.clone()).collect(),
            pack: packed.pack.clone(),
            processing_location: "unknown",
            retention: "unknown",
            authority: "none",
        };
        let mut state = self.operation.session.lock()?;
        self.operation.check(&state)?;
        state.prepared = Some(Staged {
            id,
            packed: Arc::new(packed),
        });
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Current,
    Stale,
    Unavailable,
}

pub struct Transmission {
    operation: Operation,
    packed: Arc<Packed>,
}
impl Transmission {
    pub fn operation(&self) -> &Operation {
        &self.operation
    }
    pub fn request(&self) -> &PreparedRequest {
        &self.packed.request
    }
    pub fn input_count(&self) -> usize {
        self.packed.inputs.len()
    }
    /// Native confirmation metadata comes from custody, never a renderer echo.
    pub fn inputs(&self) -> impl Iterator<Item = &Dependency> {
        self.packed.inputs.iter().map(|input| &input.dependency)
    }
    pub fn pack_id(&self) -> &str {
        self.packed.pack.pack_id()
    }

    /// Run under the native workspace guard immediately before dispatch and again
    /// after response. This method does not acquire that guard or approve sending.
    pub fn freshness(&self, store: &Workspace) -> Result<Freshness, Diagnostic> {
        self.operation.ensure_current()?;
        let mut freshness = Freshness::Current;
        for original in &self.packed.inputs {
            match inputs::Resolved::read(store, &original.selector) {
                Ok(current) if !original.matches(&current) => freshness = Freshness::Stale,
                Ok(_) => {}
                Err(_) => {
                    freshness = Freshness::Unavailable;
                    break;
                }
            }
        }
        self.operation.ensure_current()?;
        Ok(freshness)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Send {
    schema_version: String,
    prepared_id: String,
    request_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Cancel {
    schema_version: String,
    run_id: String,
}

fn decode<T: serde::de::DeserializeOwned>(raw: &[u8], limit: usize) -> Result<T, Diagnostic> {
    if raw.len() > limit {
        return Err(Diagnostic::InvalidRequest);
    }
    serde_json::from_slice(raw).map_err(|_| Diagnostic::InvalidRequest)
}
fn valid_handle(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(valid_digest)
}
fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn random_handle(prefix: &str) -> Result<String, Diagnostic> {
    handle_with(prefix, |bytes| {
        getrandom::fill(bytes).map_err(|_| Diagnostic::SessionUnavailable)
    })
}
fn handle_with(
    prefix: &str,
    random: impl FnOnce(&mut [u8]) -> Result<(), Diagnostic>,
) -> Result<String, Diagnostic> {
    let mut bytes = [0; 32];
    random(&mut bytes)?;
    let mut result = String::with_capacity(prefix.len() + 64);
    result.push_str(prefix);
    for byte in bytes {
        write!(result, "{byte:02x}").map_err(|_| Diagnostic::SessionUnavailable)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    const PROFILE: &[u8] = br#"{"schemaVersion":"rangoon.local-profile-request.v1","profileId":"local","host":"127.0.0.1","port":11434,"model":"fixture:v1","maxOutputTokens":1024}"#;

    #[test]
    fn entropy_failure_leaves_no_active_operation() {
        let session = LocalSession::default();
        session.configure(PROFILE).unwrap();
        assert!(matches!(
            session.start_with(OperationKind::Check, |_| Err(
                Diagnostic::SessionUnavailable
            )),
            Err(Diagnostic::SessionUnavailable)
        ));
        assert!(session.inspect().unwrap().active.is_none());
    }

    #[test]
    fn generation_overflow_invalidates_and_cancels() {
        let session = LocalSession::default();
        session.configure(PROFILE).unwrap();
        let operation = session.begin_check().unwrap();
        session.lock().unwrap().generation = u64::MAX;
        assert!(matches!(
            session.clear(),
            Err(Diagnostic::SessionUnavailable)
        ));
        assert_eq!(operation.ensure_current(), Err(Diagnostic::Cancelled));
        assert!(session.inspect().unwrap().profile.is_none());
        drop(operation);
        assert!(matches!(
            session.configure(PROFILE),
            Err(Diagnostic::SessionUnavailable)
        ));
    }
}
