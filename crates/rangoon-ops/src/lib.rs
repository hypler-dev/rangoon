//! Bounded operations observations. Decoding cannot establish identity or issue authority.
#![forbid(unsafe_code)]

use rangoon_domain::byte_digest;
use serde::Serialize;
use std::collections::BTreeMap;

mod decode;
mod projection;
mod projection_types;
mod records;

pub use decode::decode_window;
pub use projection::project_window;
pub use projection_types::{
    MAX_PROJECTION_BYTES, PROJECTION_SCHEMA, Projection, ProjectionDocument, ProjectionError,
};

pub const WINDOW_SCHEMA: &str = "rangoon.ops.window.v1";
pub const REDUCER_VERSION: &str = "rangoon.ops.reducer.v1";
pub const INVENTORY_SCHEMA: &str = "rangoon.ops.inventory.v1";
pub const MAX_INPUT_BYTES: usize = 2_097_152;
pub const MAX_CANONICAL_BYTES: usize = 2_097_152;
pub const MAX_DEPTH: usize = 32;
pub const MAX_VALUES: usize = 32_768;
pub const MAX_OBJECT_FIELDS: usize = 64;
pub const MAX_RAW_ARRAY: usize = 1_024;
pub const MAX_PRODUCERS: usize = 32;
pub const MAX_EVENTS: usize = 1_024;
pub const MAX_NODES: usize = 128;
pub const MAX_EDGES: usize = 512;
pub const MAX_LIST: usize = 32;
pub const MAX_INTEGER: u64 = 9_007_199_254_740_991;
pub const RECORD_KINDS: [&str; 19] = [
    "actor",
    "resource",
    "tool",
    "workflow",
    "engine",
    "policy_revision",
    "gateway_revision",
    "assignment",
    "delegation",
    "request",
    "evaluation",
    "approval_plan",
    "approval_decision",
    "authorization",
    "attempt",
    "suspension",
    "metric",
    "evidence",
    "coverage",
];

/// Fixed, payload-free decoding diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    InputLimit,
    InvalidEncoding,
    InvalidJson,
    DuplicateField,
    DepthLimit,
    ValueLimit,
    CollectionLimit,
    UnsupportedSchema,
    InvalidRecord,
    CanonicalLimit,
    SerializationFailed,
}

impl DecodeError {
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
            Self::InvalidRecord => "invalid_record",
            Self::CanonicalLimit => "canonical_limit",
            Self::SerializationFailed => "serialization_failed",
        }
    }
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}
impl std::error::Error for DecodeError {}

/// Read-only shape inventory. Counts and reported records establish no authenticity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    schema_version: &'static str,
    canonical_digest: String,
    canonical_bytes: usize,
    producer_count: usize,
    event_count: usize,
    topology_node_count: usize,
    topology_edge_count: usize,
    record_counts: BTreeMap<String, usize>,
    authority: &'static str,
    authenticity: &'static str,
    execution: &'static str,
}

impl Inventory {
    pub fn canonical_digest(&self) -> &str {
        &self.canonical_digest
    }
    pub const fn canonical_bytes(&self) -> usize {
        self.canonical_bytes
    }
    pub const fn producer_count(&self) -> usize {
        self.producer_count
    }
    pub const fn event_count(&self) -> usize {
        self.event_count
    }
    pub const fn topology_node_count(&self) -> usize {
        self.topology_node_count
    }
    pub const fn topology_edge_count(&self) -> usize {
        self.topology_edge_count
    }
    pub fn record_counts(&self) -> &BTreeMap<String, usize> {
        &self.record_counts
    }
    pub const fn authority(&self) -> &'static str {
        self.authority
    }
    pub const fn authenticity(&self) -> &'static str {
        self.authenticity
    }
    pub const fn execution(&self) -> &'static str {
        self.execution
    }
}

/// An immutable, shape-checked observation window, never an authorization capability.
pub struct DecodedWindow {
    canonical: Vec<u8>,
    inventory: Inventory,
}

impl DecodedWindow {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub fn canonical_digest(&self) -> &str {
        self.inventory.canonical_digest()
    }
    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    pub(crate) fn from_parts(
        canonical: Vec<u8>,
        producer_count: usize,
        event_count: usize,
        topology_node_count: usize,
        topology_edge_count: usize,
        counts: [usize; 19],
    ) -> Self {
        let inventory = Inventory {
            schema_version: INVENTORY_SCHEMA,
            canonical_digest: byte_digest(&canonical),
            canonical_bytes: canonical.len(),
            producer_count,
            event_count,
            topology_node_count,
            topology_edge_count,
            record_counts: RECORD_KINDS
                .into_iter()
                .zip(counts)
                .map(|(kind, count)| (kind.to_owned(), count))
                .collect(),
            authority: "none",
            authenticity: "unverified",
            execution: "unavailable",
        };
        Self {
            canonical,
            inventory,
        }
    }
}

impl std::fmt::Debug for DecodedWindow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DecodedWindow")
            .field("inventory", &self.inventory)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod record_tests;

#[cfg(test)]
mod projection_tests;
