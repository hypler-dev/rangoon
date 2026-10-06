//! Pure, bounded model-assistance contracts. This crate has no transport or authority.
#![forbid(unsafe_code)]

mod json;
mod pack;
mod response;

use serde::Serialize;
use std::fmt;

pub use pack::ContextPack;
pub use response::ValidatedResponse;

/// Closed failures for untrusted request and response data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Diagnostic {
    InputInvalid,
    InputUnavailable,
    IdentityMismatch,
    RangeInvalid,
    InputLimit,
    PackOverBudget,
    ResponseInvalid,
    ResponseLimit,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InputInvalid => "input is invalid",
            Self::InputUnavailable => "input is unavailable",
            Self::IdentityMismatch => "input identity does not match",
            Self::RangeInvalid => "input range is invalid",
            Self::InputLimit => "input exceeds a limit",
            Self::PackOverBudget => "context pack exceeds its budget",
            Self::ResponseInvalid => "response is invalid",
            Self::ResponseLimit => "response exceeds a limit",
        })
    }
}

impl std::error::Error for Diagnostic {}

/// Builds a deterministic, provider-neutral analysis body from resolved inputs.
pub fn prepare_pack(raw_json: &[u8]) -> Result<ContextPack, Diagnostic> {
    pack::prepare(raw_json)
}

/// Validates a model response against one immutable local context pack.
pub fn validate_response(
    pack: &ContextPack,
    raw_json: &[u8],
) -> Result<ValidatedResponse, Diagnostic> {
    response::validate(pack, raw_json)
}
