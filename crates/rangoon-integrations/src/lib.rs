//! Offline provider metadata and immutable synthetic evidence inspection.
//! Provider observations never issue Rangoon approval or execution authority.
#![forbid(unsafe_code)]

mod descriptor;
mod inspection;
pub mod openshell_runtime;

pub use descriptor::{candidate_inventory, fixture_profile};
pub use inspection::inspect_fixture;

#[cfg(test)]
mod conformance_tests;

pub const INPUT_SCHEMA: &str = "rangoon.integrations.fixture.v1";
pub const OUTPUT_SCHEMA: &str = "rangoon.integrations.inspection.v1";
pub const MAX_BYTES: usize = 262_144;
pub const MAX_DEPTH: usize = 16;
pub const MAX_VALUES: usize = 4_096;
pub const MAX_OBJECT_FIELDS: usize = 32;
pub const MAX_ARRAY_ITEMS: usize = 128;
pub const MAX_INTEGER: u64 = 9_007_199_254_740_991;
pub const OPERATIONS: [&str; 5] = [
    "configuration_preview",
    "connection_check",
    "evidence_read",
    "authorization_evaluation",
    "guardrail_evaluation",
];

/// Fixed payload-free failures. No partial result or parser message is exposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectionError {
    InputLimit,
    InvalidEncoding,
    InvalidJson,
    DuplicateField,
    DepthLimit,
    ValueLimit,
    CollectionLimit,
    UnsupportedSchema,
    UnsupportedProfile,
    InvalidRecord,
    ContextMismatch,
    CompatibilityMismatch,
    DigestMismatch,
    OutputLimit,
    SerializationFailed,
}
impl InspectionError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InputLimit => "input_limit",
            Self::InvalidEncoding => "invalid_encoding",
            Self::InvalidJson => "invalid_json",
            Self::DuplicateField => "duplicate_field",
            Self::DepthLimit => "depth_limit",
            Self::ValueLimit => "value_limit",
            Self::CollectionLimit => "collection_limit",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::UnsupportedProfile => "unsupported_profile",
            Self::InvalidRecord => "invalid_record",
            Self::ContextMismatch => "context_mismatch",
            Self::CompatibilityMismatch => "compatibility_mismatch",
            Self::DigestMismatch => "digest_mismatch",
            Self::OutputLimit => "output_limit",
            Self::SerializationFailed => "serialization_failed",
        }
    }
}
impl std::fmt::Display for InspectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for InspectionError {}

/// A read-only structure check of caller-supplied offline fixture bytes.
#[derive(Debug)]
pub struct Inspection {
    canonical: Vec<u8>,
    digest: String,
    state: String,
}
impl Inspection {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub fn canonical_digest(&self) -> &str {
        &self.digest
    }
    pub fn state(&self) -> &str {
        &self.state
    }
}
