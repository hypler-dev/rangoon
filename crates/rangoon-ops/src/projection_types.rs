//! Private, inert DTOs for deterministic structural projections.

use crate::Inventory;
use crate::records::{Correlation, Cursor, SnapshotScope};
use serde::Serialize;

pub const PROJECTION_SCHEMA: &str = "rangoon.ops.projection.v1";
pub const MAX_PROJECTION_BYTES: usize = 1_048_576;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionError {
    SerializationFailed,
    Limit,
}
impl ProjectionError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::SerializationFailed => "projection_serialization_failed",
            Self::Limit => "projection_limit",
        }
    }
}
impl std::fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for ProjectionError {}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionDocument {
    pub(crate) schema_version: &'static str,
    pub(crate) inventory: Inventory,
    pub(crate) replay: Replay,
    pub(crate) authority: &'static str,
    pub(crate) authenticity: &'static str,
    pub(crate) execution: &'static str,
    pub(crate) semantic_assessment: &'static str,
    pub(crate) authoritative_eligibility: &'static str,
    pub(crate) authoritative_effective_evaluation_id: Option<String>,
    pub(crate) structural_state: &'static str,
    pub(crate) snapshot_diagnostics: Vec<String>,
    pub(crate) diagnostic_counts: DiagnosticCounts,
    pub(crate) timeline: Vec<TimelineEntry>,
    pub(crate) nodes: Vec<ProjectedNode>,
    pub(crate) edges: Vec<ProjectedEdge>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Replay {
    pub canonical_digest: String,
    pub snapshot_id: String,
    pub reducer_version: String,
    pub as_of_ms: u64,
    pub scope: SnapshotScope,
    pub topology_revision_id: String,
    pub policy_revision_ids: Vec<String>,
    pub coverage_revision_ids: Vec<String>,
    pub prefixes: Vec<ReplayPrefix>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReplayPrefix {
    pub input_prefix_index: usize,
    pub producer_id: String,
    pub fence_id: String,
    pub watermark: Option<Cursor>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TimelineEntry {
    pub input_ordinal: usize,
    pub location: Location,
    pub event_id: String,
    pub producer_id: String,
    pub fence_id: String,
    pub sequence: u64,
    pub previous_event_id: Option<String>,
    pub observed_ms: Option<u64>,
    pub received_ms: Option<u64>,
    pub correlation: Option<Correlation>,
    pub record_id: String,
    pub record_kind: String,
    pub event_digest: String,
    pub reported_value: Option<String>,
    pub diagnostics: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub(crate) enum Location {
    Prefix {
        prefix_index: usize,
        event_index: usize,
    },
    Tail {
        event_index: usize,
    },
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectedNode {
    pub input_node_index: usize,
    pub id: String,
    pub kind: String,
    pub record_id: String,
    pub matching_event_ordinals: Vec<usize>,
    pub diagnostics: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectedEdge {
    pub input_edge_index: usize,
    pub id: String,
    pub kind: String,
    pub from: String,
    pub to: String,
    pub diagnostics: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
pub(crate) struct DiagnosticCounts {
    pub(crate) duplicate_observation: usize,
    pub(crate) event_identity_conflict: usize,
    pub(crate) sequence_conflict: usize,
    pub(crate) record_identity_conflict: usize,
    pub(crate) sequence_gap: usize,
    pub(crate) predecessor_missing: usize,
    pub(crate) predecessor_conflict: usize,
    pub(crate) prefix_fence_mismatch: usize,
    pub(crate) prefix_watermark_mismatch: usize,
    pub(crate) prefix_identity_conflict: usize,
    pub(crate) scope_mismatch: usize,
    pub(crate) correlation_mismatch: usize,
    pub(crate) duplicate_node: usize,
    pub(crate) node_record_missing: usize,
    pub(crate) node_record_kind_mismatch: usize,
    pub(crate) duplicate_edge: usize,
    pub(crate) edge_endpoint_missing: usize,
    pub(crate) edge_endpoint_ambiguous: usize,
}
impl DiagnosticCounts {
    pub(crate) fn from_array(counts: [usize; 18]) -> Self {
        Self {
            duplicate_observation: counts[0],
            event_identity_conflict: counts[1],
            sequence_conflict: counts[2],
            record_identity_conflict: counts[3],
            sequence_gap: counts[4],
            predecessor_missing: counts[5],
            predecessor_conflict: counts[6],
            prefix_fence_mismatch: counts[7],
            prefix_watermark_mismatch: counts[8],
            prefix_identity_conflict: counts[9],
            scope_mismatch: counts[10],
            correlation_mismatch: counts[11],
            duplicate_node: counts[12],
            node_record_missing: counts[13],
            node_record_kind_mismatch: counts[14],
            duplicate_edge: counts[15],
            edge_endpoint_missing: counts[16],
            edge_endpoint_ambiguous: counts[17],
        }
    }
}
pub struct Projection {
    pub(crate) canonical: Vec<u8>,
    pub(crate) document: ProjectionDocument,
}
impl Projection {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub fn canonical_digest(&self) -> String {
        rangoon_domain::byte_digest(&self.canonical)
    }
    pub fn document(&self) -> &ProjectionDocument {
        &self.document
    }
}
impl std::fmt::Debug for Projection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Projection")
            .field("canonicalDigest", &self.canonical_digest())
            .field("canonicalBytes", &self.canonical.len())
            .finish()
    }
}
