//! Read-only reported relationships; these DTOs cannot convey execution authority.
use serde::Serialize;

pub const RELATIONSHIP_SCHEMA: &str = "rangoon.ops.relationships.v1";
pub const MAX_RELATIONSHIP_BYTES: usize = 1_048_576;
pub const MAX_RELATIONSHIP_REFERENCES: usize = 8_192;
pub const MAX_RELATIONSHIP_MATCHES: usize = 8_192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationshipError {
    SerializationFailed,
    StructuralLimit,
    Limit,
}
impl RelationshipError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::SerializationFailed => "relationship_serialization_failed",
            Self::StructuralLimit => "relationship_structural_limit",
            Self::Limit => "relationship_limit",
        }
    }
}
impl std::fmt::Display for RelationshipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for RelationshipError {}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipDocument {
    pub(crate) schema_version: &'static str,
    pub(crate) input_canonical_digest: String,
    pub(crate) structural_projection_digest: String,
    pub(crate) authority: &'static str,
    pub(crate) authenticity: &'static str,
    pub(crate) execution: &'static str,
    pub(crate) semantic_assessment: &'static str,
    pub(crate) authoritative_eligibility: &'static str,
    pub(crate) authoritative_effective_evaluation_id: Option<String>,
    pub(crate) diagnostic_counts: RelationshipCounts,
    pub(crate) event_links: Vec<EventLink>,
    pub(crate) chains: Vec<ReportedChain>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EventLink {
    pub input_ordinal: usize,
    pub event_id: String,
    pub record_id: String,
    pub record_kind: String,
    pub assessment: &'static str,
    pub diagnostics: Vec<&'static str>,
    pub references: Vec<ReferenceLink>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReferenceLink {
    pub field: &'static str,
    pub index: Option<usize>,
    pub record_id: String,
    pub expected_kind: &'static str,
    pub status: &'static str,
    pub matching_event_ordinals: Vec<usize>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReportedChain {
    pub request_id: String,
    pub engine_id: String,
    pub request_event_ordinals: Vec<usize>,
    pub evaluation_ids: Vec<String>,
    pub reported_chain_status: &'static str,
    pub reported_chain_tip_id: Option<String>,
    pub reported_result: Option<String>,
    pub diagnostics: Vec<&'static str>,
}
#[derive(Clone, Debug, Serialize)]
pub(crate) struct RelationshipCounts {
    pub structural_incomplete: usize,
    pub reference_missing: usize,
    pub reference_ambiguous: usize,
    pub reference_kind_mismatch: usize,
    pub context_mismatch: usize,
    pub gateway_mismatch: usize,
    pub context_digest_mismatch: usize,
    pub gateway_policy_mismatch: usize,
    pub evaluation_selector_mismatch: usize,
    pub lineage_fence_mismatch: usize,
    pub evaluation_sequence_mismatch: usize,
    pub evaluation_branch: usize,
    pub evaluation_cycle: usize,
    pub evaluation_incomplete_chain: usize,
    pub binding_mismatch: usize,
    pub non_tip_evaluation: usize,
}
impl RelationshipCounts {
    pub(crate) fn from_array(c: [usize; 16]) -> Self {
        Self {
            structural_incomplete: c[0],
            reference_missing: c[1],
            reference_ambiguous: c[2],
            reference_kind_mismatch: c[3],
            context_mismatch: c[4],
            gateway_mismatch: c[5],
            context_digest_mismatch: c[6],
            gateway_policy_mismatch: c[7],
            evaluation_selector_mismatch: c[8],
            lineage_fence_mismatch: c[9],
            evaluation_sequence_mismatch: c[10],
            evaluation_branch: c[11],
            evaluation_cycle: c[12],
            evaluation_incomplete_chain: c[13],
            binding_mismatch: c[14],
            non_tip_evaluation: c[15],
        }
    }
}

pub struct Relationships {
    pub(crate) canonical: Vec<u8>,
    pub(crate) document: RelationshipDocument,
}
impl Relationships {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub fn canonical_digest(&self) -> String {
        rangoon_domain::byte_digest(&self.canonical)
    }
    pub fn document(&self) -> &RelationshipDocument {
        &self.document
    }
}
impl std::fmt::Debug for Relationships {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Relationships")
            .field("canonicalDigest", &self.canonical_digest())
            .field("canonicalBytes", &self.canonical.len())
            .finish()
    }
}
