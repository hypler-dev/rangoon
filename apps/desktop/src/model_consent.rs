//! Native-owned, one-shot consent bookkeeping. This module cannot send requests.
use rangoon_model_local::Cancellation;
use rangoon_model_session::{Dependency, Transmission};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tokio::sync::oneshot;

pub const REVIEW_WINDOW: &str = "model-confirmation";
pub const SEND_LABEL: &str = "Send selected text";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub generation: String,
    pub run_id: String,
    pub request_id: String,
    pub origin: String,
    pub model: String,
    pub body_bytes: usize,
    pub body_json: String,
    pub pack_id: String,
    pub inputs: Vec<Dependency>,
}
impl Review {
    pub fn from_transmission(send: &Transmission) -> Self {
        Self {
            generation: send.operation().generation(),
            run_id: send.operation().run_id().to_owned(),
            request_id: send.request().request_id().to_owned(),
            origin: send.operation().profile().origin(),
            model: send.operation().profile().model().to_owned(),
            body_bytes: send.request().body_json().len(),
            body_json: send.request().body_json().to_owned(),
            pack_id: send.pack_id().to_owned(),
            inputs: send.inputs().cloned().collect(),
        }
    }

    pub fn question(&self) -> String {
        format!(
            "Send {} selected saved records ({} bytes) to {} using {}?\n\nThe endpoint may retain or forward this text. Results are advisory and cannot approve or execute anything.\n\nPack: {}\nRequest: {}",
            self.inputs.len(),
            self.body_bytes,
            self.origin,
            self.model,
            self.pack_id,
            self.request_id
        )
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Decision {
    schema_version: String,
    pub run_id: String,
    pub request_id: String,
}
impl Decision {
    pub fn parse(raw: &[u8]) -> Option<Self> {
        if raw.len() > 512 {
            return None;
        }
        let decision: Self = serde_json::from_slice(raw).ok()?;
        let digest = |s: &str| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        (decision.schema_version == "rangoon.local-review-decision.v1"
            && decision.run_id.strip_prefix("run:").is_some_and(digest)
            && digest(&decision.request_id))
        .then_some(decision)
    }
}

struct Pending {
    review: Review,
    cancellation: Cancellation,
    sender: Option<oneshot::Sender<bool>>,
    prompting: bool,
    cleanup_requested: bool,
}

#[derive(Default)]
pub struct Consent(Mutex<Option<Pending>>);

impl Consent {
    pub fn install(
        &self,
        review: Review,
        cancellation: Cancellation,
    ) -> Option<oneshot::Receiver<bool>> {
        let mut slot = self.0.lock().ok()?;
        if slot.is_some() || cancellation.is_cancelled() {
            return None;
        }
        let (sender, receiver) = oneshot::channel();
        *slot = Some(Pending {
            review,
            cancellation,
            sender: Some(sender),
            prompting: false,
            cleanup_requested: false,
        });
        Some(receiver)
    }

    pub fn inspect(&self) -> Option<Review> {
        let slot = self.0.lock().ok()?;
        let pending = slot.as_ref()?;
        (!pending.cancellation.is_cancelled() && pending.sender.is_some())
            .then(|| pending.review.clone())
    }

    /// Claim the OS prompt exactly once. No lock is retained by the caller.
    pub fn prompt(&self, decision: &Decision) -> Option<Review> {
        let mut slot = self.0.lock().ok()?;
        let pending = slot.as_mut()?;
        if pending.review.run_id != decision.run_id
            || pending.review.request_id != decision.request_id
            || pending.prompting
            || pending.sender.is_none()
            || pending.cancellation.is_cancelled()
        {
            return None;
        }
        pending.prompting = true;
        Some(pending.review.clone())
    }

    /// Called only after the OS prompt exits. True asks the native caller to
    /// close a window whose owner disappeared while that prompt was active.
    pub fn finish_prompt(&self, run_id: &str, approved: bool) -> bool {
        let Ok(mut slot) = self.0.lock() else {
            return false;
        };
        let Some(pending) = slot
            .as_mut()
            .filter(|p| p.review.run_id == run_id && p.prompting)
        else {
            return false;
        };
        pending.prompting = false;
        let allow = approved && !pending.cancellation.is_cancelled() && !pending.cleanup_requested;
        if !allow {
            pending.cancellation.cancel();
        }
        if let Some(sender) = pending.sender.take() {
            let _ = sender.send(allow);
        }
        let cleanup = pending.cleanup_requested;
        if cleanup {
            *slot = None;
        }
        cleanup
    }

    pub fn cancel_decision(&self, decision: &Decision) -> bool {
        let Ok(mut slot) = self.0.lock() else {
            return false;
        };
        let Some(pending) = slot.as_mut().filter(|p| {
            p.review.run_id == decision.run_id && p.review.request_id == decision.request_id
        }) else {
            return false;
        };
        cancel(pending);
        true
    }

    /// The optional run identity prevents stale commands from cancelling a new run.
    /// Return true while an OS prompt is active: its parent must not be destroyed.
    pub fn cancel_run(&self, run_id: Option<&str>) -> bool {
        let Ok(mut slot) = self.0.lock() else {
            return true;
        };
        let Some(pending) = slot
            .as_mut()
            .filter(|p| run_id.is_none_or(|id| p.review.run_id == id))
        else {
            return false;
        };
        cancel(pending);
        pending.prompting
    }

    /// RAII owner cleanup is identity-bound. A pending OS prompt retains its
    /// parent; finish_prompt performs deferred removal after the dialog exits.
    pub fn remove(&self, run_id: &str) -> bool {
        let Ok(mut slot) = self.0.lock() else {
            return false;
        };
        let Some(pending) = slot.as_mut().filter(|p| p.review.run_id == run_id) else {
            return false;
        };
        if pending.prompting {
            pending.cleanup_requested = true;
            cancel(pending);
            false
        } else {
            *slot = None;
            true
        }
    }
}

fn cancel(pending: &mut Pending) {
    pending.cancellation.cancel();
    if !pending.prompting
        && let Some(sender) = pending.sender.take()
    {
        let _ = sender.send(false);
    }
}
