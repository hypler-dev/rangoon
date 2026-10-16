//! Reported lifecycle inspection DTOs. These observations never issue authority.

use crate::relationship_types::ReferenceLink;
use serde::Serialize;

pub const LIFECYCLE_SCHEMA: &str = "rangoon.ops.lifecycle.v1";
pub const MAX_LIFECYCLE_BYTES: usize = 1_048_576;
pub const MAX_LIFECYCLE_REFERENCES: usize = 8_192;
pub const MAX_LIFECYCLE_MATCHES: usize = 8_192;
pub const MAX_LIFECYCLE_PATH_STEPS: usize = 8_192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleError {
    SerializationFailed,
    StructuralLimit,
    RelationshipLimit,
    DelegationLimit,
    ApprovalLimit,
    Limit,
}

impl LifecycleError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::SerializationFailed => "lifecycle_serialization_failed",
            Self::StructuralLimit => "lifecycle_structural_limit",
            Self::RelationshipLimit => "lifecycle_relationship_limit",
            Self::DelegationLimit => "lifecycle_delegation_limit",
            Self::ApprovalLimit => "lifecycle_approval_limit",
            Self::Limit => "lifecycle_limit",
        }
    }
}

impl std::fmt::Display for LifecycleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for LifecycleError {}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleDocument {
    pub(crate) schema_version: &'static str,
    pub(crate) input_canonical_digest: String,
    pub(crate) structural_projection_digest: String,
    pub(crate) relationship_digest: String,
    pub(crate) delegation_digest: String,
    pub(crate) approval_digest: String,
    pub(crate) authority: &'static str,
    pub(crate) authenticity: &'static str,
    pub(crate) execution: &'static str,
    pub(crate) semantic_assessment: &'static str,
    pub(crate) authoritative_eligibility: &'static str,
    pub(crate) authoritative_outcome: &'static str,
    pub(crate) qualified_coverage: &'static str,
    pub(crate) diagnostic_counts: Vec<LifecycleCount>,
    pub(crate) event_assessments: Vec<LifecycleEvent>,
    pub(crate) reported_chains: Vec<LifecycleChain>,
    pub(crate) coverage_assessments: Vec<CoverageAssessment>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct LifecycleCount {
    pub code: &'static str,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LifecycleEvent {
    pub input_ordinal: usize,
    pub event_id: String,
    pub record_id: String,
    pub record_kind: String,
    pub assessment: &'static str,
    pub reported_state: Option<String>,
    pub reported_attempt_id: Option<String>,
    pub diagnostics: Vec<&'static str>,
    pub references: Vec<ReferenceLink>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LifecycleChain {
    pub kind: &'static str,
    pub group_id: String,
    pub matching_event_ordinals: Vec<usize>,
    pub record_ids: Vec<String>,
    pub reported_chain_status: &'static str,
    pub reported_tip_record_id: Option<String>,
    pub reported_tip_state: Option<String>,
    pub diagnostics: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReportedChecks {
    pub policy_decision: String,
    pub approval_duties: String,
    pub agent_delegation: String,
    pub expiry_revocation: String,
    pub execution_consumption: String,
    pub replay_protection: String,
    pub outcome_evidence: String,
    pub reconciliation: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoverageAssessment {
    pub input_ordinal: usize,
    pub record_id: String,
    pub reported_level: String,
    pub reported_freshness: String,
    pub observed_ms: Option<u64>,
    pub expires_ms: Option<u64>,
    pub target_os: String,
    pub checks: ReportedChecks,
    pub assessment: &'static str,
    pub diagnostics: Vec<&'static str>,
    pub references: Vec<ReferenceLink>,
    pub qualified_level: &'static str,
}

pub struct LifecycleInspection {
    pub(crate) canonical: Vec<u8>,
    pub(crate) document: LifecycleDocument,
}

impl LifecycleInspection {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }

    pub fn canonical_digest(&self) -> String {
        rangoon_domain::byte_digest(&self.canonical)
    }

    pub fn document(&self) -> &LifecycleDocument {
        &self.document
    }
}

impl std::fmt::Debug for LifecycleInspection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LifecycleInspection")
            .field("canonicalDigest", &self.canonical_digest())
            .field("canonicalBytes", &self.canonical.len())
            .finish()
    }
}
