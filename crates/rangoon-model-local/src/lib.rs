//! Explicit, bounded loopback model transport. Results have no execution authority.
//!
//! This library does not establish operator consent, source freshness, endpoint
//! authentication, or local-only inference. Those are separate application boundaries.
#![forbid(unsafe_code)]

mod profile;
mod request;
mod transport;
mod wire;

pub use profile::LocalProfile;
pub use request::PreparedRequest;
use serde::Serialize;
use std::fmt;
pub use transport::{Cancellation, CheckResult, Completion, LocalClient};

/// Closed diagnostics deliberately omit source text and remote error bodies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Diagnostic {
    InvalidProfile,
    ProfileMismatch,
    RequestOverBudget,
    Busy,
    Cancelled,
    TimedOut,
    ConnectionFailed,
    RedirectRejected,
    RemoteRejected,
    ResponseTooLarge,
    ResponseInvalid,
    ResponseIncomplete,
    ProposalInvalid,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidProfile => "local profile is invalid",
            Self::ProfileMismatch => "context target does not match the local profile",
            Self::RequestOverBudget => "final request exceeds its byte budget",
            Self::Busy => "local model client is busy",
            Self::Cancelled => "local model request was cancelled",
            Self::TimedOut => "local model request timed out",
            Self::ConnectionFailed => "local model connection failed",
            Self::RedirectRejected => "local model redirect was rejected",
            Self::RemoteRejected => "local model server rejected the request",
            Self::ResponseTooLarge => "local model response exceeds a limit",
            Self::ResponseInvalid => "local model response is invalid",
            Self::ResponseIncomplete => "local model response is incomplete",
            Self::ProposalInvalid => "local model proposal is invalid",
        })
    }
}

impl std::error::Error for Diagnostic {}
