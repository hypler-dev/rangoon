//! Cloud-only, memory-only request custody over validated saved records.
//!
//! This service never reads credentials or sends requests. Native consent, full
//! OS-envelope comparison and cross-provider operation serialization are caller
//! obligations. A transmission lease is not transfer permission.
use crate::Dependency;
use crate::{
    ActiveView, Cancel, Diagnostic, Freshness, OperationKind, Send, decode, handle_with,
    inputs::{self, Resolved, Selection},
    random_handle, valid_digest, valid_handle,
};
use rangoon_model_assistance::ContextPack;
use rangoon_model_cloud::{Cancellation, CloudProfile, PreparedRequest};
use rangoon_store::Workspace;
use serde::Serialize;
use std::sync::{Arc, Mutex, MutexGuard};

struct Packed {
    request: PreparedRequest,
    pack: ContextPack,
    inputs: Vec<Resolved>,
    credential_revision: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub schema_version: &'static str,
    pub generation: String,
    pub profile: Option<CloudProfile>,
    pub active: Option<ActiveView>,
    pub persistence: &'static str,
    pub processing_location: &'static str,
    pub retention: &'static str,
    pub authority: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedView {
    pub schema_version: &'static str,
    pub generation: String,
    pub prepared_id: String,
    pub request_id: String,
    pub origin: String,
    pub model: String,
    pub body_json: String,
    pub body_bytes: usize,
    pub body_sha256: String,
    pub credential_revision: String,
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
    profile: Option<CloudProfile>,
    active: Option<Active>,
    prepared: Option<Staged>,
}

/// Clones share custody and one active-operation slot. No state is persisted.
#[derive(Clone, Default)]
pub struct CloudSession(Arc<Mutex<State>>);

impl CloudSession {
    fn lock(&self) -> Result<MutexGuard<'_, State>, Diagnostic> {
        self.0.lock().map_err(|_| Diagnostic::SessionUnavailable)
    }

    pub fn inspect(&self) -> Result<SessionView, Diagnostic> {
        Ok(view(&*self.lock()?))
    }

    /// Invalid configuration preserves the current profile and preparation.
    pub fn configure(&self, raw: &[u8]) -> Result<SessionView, Diagnostic> {
        let profile = CloudProfile::parse(raw).map_err(|_| Diagnostic::InvalidProfile)?;
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
        if input.schema_version != "rangoon.cloud-cancel.v1"
            || !valid_handle(&input.run_id, "cloud-run:")
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

    /// Every new selection attempt discards the previous preparation, including busy failures.
    pub fn begin_prepare(
        &self,
        raw: &[u8],
        credential_revision: &str,
    ) -> Result<Preparation, Diagnostic> {
        self.begin_resolution(raw)?
            .with_credential_revision(credential_revision)
    }

    /// Acquire and validate before the native caller reads its fixed OS slot.
    /// No credential lookup is performed by the portable session.
    pub fn begin_resolution(&self, raw: &[u8]) -> Result<Resolution, Diagnostic> {
        let operation = self.start(OperationKind::Prepare)?;
        let selection = Selection::parse_cloud(raw)?;
        Ok(Resolution {
            operation,
            selection,
        })
    }

    /// The caller still needs credential custody and independent native consent.
    pub fn begin_check(&self) -> Result<Operation, Diagnostic> {
        self.start(OperationKind::Check)
    }

    /// Claim consumes the exact handle. The caller must still obtain native consent.
    pub fn begin_send(&self, raw: &[u8]) -> Result<Transmission, Diagnostic> {
        let input: Send = decode(raw, 512)?;
        if input.schema_version != "rangoon.cloud-send.v1"
            || !valid_handle(&input.prepared_id, "cloud-prepared:")
            || !valid_digest(&input.request_id)
        {
            return Err(Diagnostic::InvalidRequest);
        }
        let id = random_handle("cloud-run:")?;
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
        let id = handle_with("cloud-run:", random)?;
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
        schema_version: "rangoon.cloud-session.v1",
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
    session: &CloudSession,
    state: &mut State,
    profile: CloudProfile,
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
    session: CloudSession,
    generation: u64,
    id: String,
    profile: CloudProfile,
    cancellation: Cancellation,
}

impl Operation {
    pub fn run_id(&self) -> &str {
        &self.id
    }
    pub fn generation(&self) -> String {
        self.generation.to_string()
    }
    pub fn profile(&self) -> &CloudProfile {
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
            || state
                .active
                .as_ref()
                .is_none_or(|a| a.view.run_id != self.id)
            || state
                .profile
                .as_ref()
                .is_none_or(|p| p.profile_sha256() != self.profile.profile_sha256())
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

pub struct Resolution {
    operation: Operation,
    selection: Selection,
}
impl Resolution {
    pub fn operation(&self) -> &Operation {
        &self.operation
    }

    /// Bind only the revision obtained by the native host. Clear/cancel while
    /// the OS callback was running must not revive this resolution.
    pub fn with_credential_revision(self, revision: &str) -> Result<Preparation, Diagnostic> {
        self.operation.ensure_current()?;
        if !valid_digest(revision) {
            return Err(Diagnostic::InvalidRequest);
        }
        Ok(Preparation {
            operation: self.operation,
            selection: self.selection,
            credential_revision: revision.to_owned(),
        })
    }
}

pub struct Preparation {
    operation: Operation,
    selection: Selection,
    credential_revision: String,
}
impl Preparation {
    pub fn operation(&self) -> &Operation {
        &self.operation
    }

    /// Validated saved-record reads only; no network or database writes.
    pub fn prepare(self, store: &Workspace) -> Result<PreparedView, Diagnostic> {
        self.operation.ensure_current()?;
        let profile = self.operation.profile();
        let (pack, inputs) = inputs::prepare_pack(
            store,
            &self.selection,
            profile.profile_id(),
            profile.profile_sha256(),
            profile.model(),
            profile.max_output_tokens(),
        )?;
        let request = PreparedRequest::new(profile, pack.clone(), &self.credential_revision)
            .map_err(|error| match error {
                rangoon_model_cloud::Diagnostic::RequestOverBudget => Diagnostic::PackOverBudget,
                _ => Diagnostic::PackInvalid,
            })?;
        let packed = Packed {
            request,
            pack,
            inputs,
            credential_revision: self.credential_revision.clone(),
        };
        self.operation.ensure_current()?;
        let id = random_handle("cloud-prepared:")?;
        let result = PreparedView {
            schema_version: "rangoon.cloud-prepared.v1",
            generation: self.operation.generation(),
            prepared_id: id.clone(),
            request_id: packed.request.request_id().to_owned(),
            origin: self.operation.profile.origin().to_owned(),
            model: self.operation.profile.model().to_owned(),
            body_json: packed.request.body_json().to_owned(),
            body_bytes: packed.request.body_json().len(),
            body_sha256: rangoon_domain::byte_digest(packed.request.body_json().as_bytes()),
            credential_revision: packed.credential_revision.clone(),
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
    pub fn credential_revision(&self) -> &str {
        &self.packed.credential_revision
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

#[cfg(test)]
mod tests {
    use super::*;
    const PROFILE: &[u8] = br#"{"schemaVersion":"rangoon.cloud-profile-request.v1","profileId":"cloud","model":"fixture-v1","maxOutputTokens":1024}"#;

    #[test]
    fn entropy_failure_installs_no_operation() {
        let session = CloudSession::default();
        session.configure(PROFILE).unwrap();
        assert!(matches!(
            session.start_with(OperationKind::Prepare, |_| Err(
                Diagnostic::SessionUnavailable
            )),
            Err(Diagnostic::SessionUnavailable)
        ));
        assert!(session.inspect().unwrap().active.is_none());
    }

    #[test]
    fn generation_overflow_cancels_and_invalidates() {
        let session = CloudSession::default();
        session.configure(PROFILE).unwrap();
        let operation = session.start(OperationKind::Prepare).unwrap();
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
