//! Explicit fixed-origin cloud analysis transport with no application authority.
//!
//! Native callers must independently enforce consent, saved-record freshness and
//! current OS credential custody. Constructing these types performs no I/O.
#![forbid(unsafe_code)]

mod check;
mod check_wire;
mod json;
mod profile;
mod request;
mod transport;
mod wire;

pub use check::{CheckRequest, CheckResult};
pub use profile::CloudProfile;
pub use request::PreparedRequest;
use serde::Serialize;
use std::fmt;
pub use transport::{Cancellation, CloudClient, Completion, Credential};

/// Closed diagnostics never include credentials, source or provider error text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Diagnostic {
    InvalidProfile,
    ProfileMismatch,
    InvalidCredential,
    CredentialChanged,
    RequestOverBudget,
    Busy,
    Cancelled,
    TimedOut,
    ConnectionFailed,
    AddressRejected,
    TlsRejected,
    RedirectRejected,
    RemoteRejected,
    ResponseTooLarge,
    ResponseInvalid,
    ResponseIncomplete,
    ResponseRefused,
    ProposalInvalid,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidProfile => "cloud profile is invalid",
            Self::ProfileMismatch => "context target does not match cloud profile",
            Self::InvalidCredential => "cloud credential is invalid",
            Self::CredentialChanged => "cloud credential revision changed",
            Self::RequestOverBudget => "cloud request exceeds byte budget",
            Self::Busy => "cloud model client is busy",
            Self::Cancelled => "cloud model request was cancelled",
            Self::TimedOut => "cloud model request timed out",
            Self::ConnectionFailed => "cloud model connection failed",
            Self::AddressRejected => "cloud model address was rejected",
            Self::TlsRejected => "cloud model TLS verification failed",
            Self::RedirectRejected => "cloud model redirect was rejected",
            Self::RemoteRejected => "cloud model provider rejected request",
            Self::ResponseTooLarge => "cloud model response exceeds a limit",
            Self::ResponseInvalid => "cloud model response is invalid",
            Self::ResponseIncomplete => "cloud model response is incomplete",
            Self::ResponseRefused => "cloud model refused request",
            Self::ProposalInvalid => "cloud model proposal is invalid",
        })
    }
}
impl std::error::Error for Diagnostic {}

/// Stable secret-free diagnostic envelope for callers that need serialization.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    schema_version: &'static str,
    adapter: &'static str,
    code: Diagnostic,
    authority: &'static str,
}
impl Diagnostic {
    pub fn receipt(self) -> Failure {
        Failure {
            schema_version: "rangoon.cloud-diagnostic.v1",
            adapter: profile::ADAPTER,
            code: self,
            authority: "none",
        }
    }
}
