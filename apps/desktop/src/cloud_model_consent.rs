//! Native-owned, one-shot consent bookkeeping. This module cannot send requests.
use rangoon_model_cloud::Cancellation;
use rangoon_model_session::Dependency;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

pub const REVIEW_WINDOW: &str = "cloud-model-confirmation";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub generation: String,
    pub run_id: String,
    pub request_id: String,
    pub origin: String,
    pub model: String,
    pub purpose: Purpose,
    pub method: String,
    pub path: String,
    pub credential_revision: String,
    pub body_sha256: String,
    pub body_bytes: usize,
    pub body_json: String,
    pub pack_id: Option<String>,
    pub inputs: Vec<Dependency>,
}
#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Analysis,
    ModelCheck,
}
impl Review {
    pub fn send_label(&self) -> &'static str {
        match self.purpose {
            Purpose::Analysis => "Send text and credential",
            Purpose::ModelCheck => "Check with credential",
        }
    }
    pub fn question(&self) -> String {
        let disclosure = match self.purpose {
            Purpose::Analysis => format!("Send {} selected saved records ({} body bytes) and the stored credential to OpenAI?", self.inputs.len(), self.body_bytes),
            Purpose::ModelCheck => "Send the stored credential and model identifier to OpenAI? No selected source text is sent.".to_owned(),
        };
        format!(
            "{disclosure}\n\nOrigin: {}\nModel: {}\n{} {}\nCredential revision: {}\nRequest: {}\n\nCost, retention and processing location are unknown. store:false is not Zero Data Retention. Cancellation cannot recall transmitted data. Results are advisory and cannot approve or execute anything.",
            self.origin,
            self.model,
            self.method,
            self.path,
            self.credential_revision,
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
        (decision.schema_version == "rangoon.cloud-review-decision.v1"
            && decision
                .run_id
                .strip_prefix("cloud-run:")
                .is_some_and(digest)
            && digest(&decision.request_id))
        .then_some(decision)
    }
}

struct Pending<T> {
    review: Review,
    cancellation: Cancellation,
    sender: Option<oneshot::Sender<bool>>,
    prompting: bool,
    cleanup_requested: bool,
    _owner: Arc<T>,
}

pub struct Consent<T = ()>(Mutex<Option<Pending<T>>>);
impl<T> Default for Consent<T> {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

impl<T> Consent<T> {
    pub fn install(
        &self,
        review: Review,
        cancellation: Cancellation,
        owner: Arc<T>,
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
            _owner: owner,
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

fn cancel<T>(pending: &mut Pending<T>) {
    pending.cancellation.cancel();
    if !pending.prompting
        && let Some(sender) = pending.sender.take()
    {
        let _ = sender.send(false);
    }
}
