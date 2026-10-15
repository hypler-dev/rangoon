//! Read-only reported approval observations; none of these DTOs grants authority.
use crate::delegation_types::SubjectTuple;
use crate::relationship_types::ReferenceLink;
use serde::Serialize;

pub const APPROVAL_SCHEMA: &str = "rangoon.ops.approvals.v1";
pub const MAX_APPROVAL_BYTES: usize = 1_048_576;
pub const MAX_APPROVAL_STAGES: usize = 1_024;
pub const MAX_APPROVAL_REFERENCES: usize = 8_192;
pub const MAX_APPROVAL_MATCHES: usize = 8_192;
pub const MAX_APPROVAL_PATH_STEPS: usize = 8_192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalError {
    SerializationFailed,
    StructuralLimit,
    RelationshipLimit,
    DelegationLimit,
    Limit,
}
impl ApprovalError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::SerializationFailed => "approval_serialization_failed",
            Self::StructuralLimit => "approval_structural_limit",
            Self::RelationshipLimit => "approval_relationship_limit",
            Self::DelegationLimit => "approval_delegation_limit",
            Self::Limit => "approval_limit",
        }
    }
}
impl std::fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for ApprovalError {}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalDocument {
    pub(crate) schema_version: &'static str,
    pub(crate) input_canonical_digest: String,
    pub(crate) structural_projection_digest: String,
    pub(crate) relationship_digest: String,
    pub(crate) delegation_digest: String,
    pub(crate) authority: &'static str,
    pub(crate) authenticity: &'static str,
    pub(crate) execution: &'static str,
    pub(crate) semantic_assessment: &'static str,
    pub(crate) authoritative_eligibility: &'static str,
    pub(crate) authoritative_effective_evaluation_id: Option<String>,
    pub(crate) authoritative_duty_satisfaction: &'static str,
    pub(crate) diagnostic_counts: Vec<ApprovalCount>,
    pub(crate) event_assessments: Vec<ApprovalEvent>,
    pub(crate) decision_chains: Vec<DecisionChain>,
}
#[derive(Clone, Debug, Serialize)]
pub(crate) struct ApprovalCount {
    pub code: &'static str,
    pub count: usize,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApprovalEvent {
    pub input_ordinal: usize,
    pub event_id: String,
    pub record_id: String,
    pub record_kind: String,
    pub assessment: &'static str,
    pub reported_decision: Option<String>,
    pub actor_subject: Option<SubjectTuple>,
    pub reported_decision_chain_tip_id: Option<String>,
    pub diagnostics: Vec<&'static str>,
    pub references: Vec<ReferenceLink>,
    pub stages: Vec<ApprovalStage>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApprovalStage {
    pub stage_input_index: usize,
    pub stage_id: String,
    pub mode: String,
    pub reported_quorum: Option<u64>,
    pub reported_human_required: bool,
    pub reported_expires_ms: Option<u64>,
    pub required_reported_subject_count: Option<u64>,
    pub reported_votes: Option<ReportedVotes>,
    pub authoritative_duty_satisfaction: &'static str,
    pub assessment: &'static str,
    pub diagnostics: Vec<&'static str>,
    pub reported_approver_subjects: Vec<SubjectTuple>,
    pub reported_escalation_subjects: Vec<SubjectTuple>,
    pub approver_references: Vec<ReferenceLink>,
    pub escalation_references: Vec<ReferenceLink>,
    pub dependency_references: Vec<StageReference>,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReportedVotes {
    pub approve_subjects: usize,
    pub deny_subjects: usize,
    pub abstain_subjects: usize,
    pub human_approve_subjects: usize,
    pub agent_approve_subjects: usize,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StageReference {
    pub index: usize,
    pub stage_id: String,
    pub status: &'static str,
    pub matching_stage_indices: Vec<usize>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DecisionChain {
    pub plan_id: String,
    pub evaluation_id: String,
    pub stage_id: String,
    pub subject: SubjectTuple,
    pub decision_record_ids: Vec<String>,
    pub matching_event_ordinals: Vec<usize>,
    pub reported_chain_status: &'static str,
    pub reported_chain_tip_id: Option<String>,
    pub reported_decision: Option<String>,
    pub reported_actor_kind: Option<String>,
    pub diagnostics: Vec<&'static str>,
}

pub struct ApprovalInspection {
    pub(crate) canonical: Vec<u8>,
    pub(crate) document: ApprovalDocument,
}
impl ApprovalInspection {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub fn canonical_digest(&self) -> String {
        rangoon_domain::byte_digest(&self.canonical)
    }
    pub fn document(&self) -> &ApprovalDocument {
        &self.document
    }
}
impl std::fmt::Debug for ApprovalInspection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApprovalInspection")
            .field("canonicalDigest", &self.canonical_digest())
            .field("canonicalBytes", &self.canonical.len())
            .finish()
    }
}
