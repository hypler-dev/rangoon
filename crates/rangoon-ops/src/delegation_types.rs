//! Immutable inspection DTOs; reported delegation links never convey authority.
use crate::relationship_types::ReferenceLink;
use serde::Serialize;

pub const DELEGATION_SCHEMA: &str = "rangoon.ops.delegations.v1";
pub const MAX_DELEGATION_BYTES: usize = 1_048_576;
pub const MAX_DELEGATION_REFERENCES: usize = 8_192;
pub const MAX_DELEGATION_MATCHES: usize = 8_192;
pub const MAX_DELEGATION_PATH_STEPS: usize = 8_192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelegationError {
    SerializationFailed,
    StructuralLimit,
    Limit,
}
impl DelegationError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::SerializationFailed => "delegation_serialization_failed",
            Self::StructuralLimit => "delegation_structural_limit",
            Self::Limit => "delegation_limit",
        }
    }
}
impl std::fmt::Display for DelegationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for DelegationError {}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubjectTuple {
    pub system_id: String,
    pub canonical_subject_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegationDocument {
    pub(crate) schema_version: &'static str,
    pub(crate) input_canonical_digest: String,
    pub(crate) structural_projection_digest: String,
    pub(crate) authority: &'static str,
    pub(crate) authenticity: &'static str,
    pub(crate) execution: &'static str,
    pub(crate) semantic_assessment: &'static str,
    pub(crate) authoritative_eligibility: &'static str,
    pub(crate) authoritative_effective_evaluation_id: Option<String>,
    pub(crate) subdelegation_admission: &'static str,
    pub(crate) diagnostic_counts: DelegationCounts,
    pub(crate) subjects: Vec<SubjectInspection>,
    pub(crate) event_assessments: Vec<DelegationEvent>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubjectInspection {
    pub system_id: String,
    pub canonical_subject_id: String,
    pub actor_record_ids: Vec<String>,
    pub reported_actor_kinds: Vec<String>,
    pub matching_event_ordinals: Vec<usize>,
    pub assessment: &'static str,
    pub diagnostics: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DelegationEvent {
    pub input_ordinal: usize,
    pub event_id: String,
    pub record_id: String,
    pub record_kind: String,
    pub assessment: &'static str,
    pub reported_state: Option<String>,
    pub reported_verification: Option<String>,
    pub reported_freshness: Option<String>,
    pub subject: Option<SubjectTuple>,
    pub delegator_subject: Option<SubjectTuple>,
    pub delegate_subject: Option<SubjectTuple>,
    pub changed_by_subject: Option<SubjectTuple>,
    pub ancestor_record_ids: Vec<String>,
    pub diagnostics: Vec<&'static str>,
    pub references: Vec<ReferenceLink>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct DelegationCounts {
    pub structural_incomplete: usize,
    pub reference_missing: usize,
    pub reference_ambiguous: usize,
    pub reference_kind_mismatch: usize,
    pub subject_kind_conflict: usize,
    pub system_mismatch: usize,
    pub self_delegation: usize,
    pub self_expansion: usize,
    pub parent_actor_mismatch: usize,
    pub scope_incomplete: usize,
    pub scope_expansion: usize,
    pub expiry_incomplete: usize,
    pub expiry_expansion: usize,
    pub attenuation_missing: usize,
    pub delegation_cycle: usize,
    pub subject_cycle: usize,
}
impl DelegationCounts {
    pub(crate) fn from_array(c: [usize; 16]) -> Self {
        Self {
            structural_incomplete: c[0],
            reference_missing: c[1],
            reference_ambiguous: c[2],
            reference_kind_mismatch: c[3],
            subject_kind_conflict: c[4],
            system_mismatch: c[5],
            self_delegation: c[6],
            self_expansion: c[7],
            parent_actor_mismatch: c[8],
            scope_incomplete: c[9],
            scope_expansion: c[10],
            expiry_incomplete: c[11],
            expiry_expansion: c[12],
            attenuation_missing: c[13],
            delegation_cycle: c[14],
            subject_cycle: c[15],
        }
    }
}

pub struct DelegationInspection {
    pub(crate) canonical: Vec<u8>,
    pub(crate) document: DelegationDocument,
}
impl DelegationInspection {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub fn canonical_digest(&self) -> String {
        rangoon_domain::byte_digest(&self.canonical)
    }
    pub fn document(&self) -> &DelegationDocument {
        &self.document
    }
}
impl std::fmt::Debug for DelegationInspection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DelegationInspection")
            .field("canonicalDigest", &self.canonical_digest())
            .field("canonicalBytes", &self.canonical.len())
            .finish()
    }
}
