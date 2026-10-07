//! Portable immutable workflow revisions. This module has no persistence or execution authority.

use super::{DefinitionInspection, WorkflowDefinition, inspect_definition};
use rangoon_domain::byte_digest;
use rangoon_domain::capability::valid_id;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const REVISION_SCHEMA: &str = "rangoon.workflow-revision.v1";
const MAX_RECORD_BYTES: usize = 160 * 1024;
const MAX_LAYOUT_BYTES: usize = 16 * 1024;
const MAX_POSITIONS: usize = 128;
const COORDINATE_LIMIT: i64 = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveIntent {
    Draft,
    Validated,
}

impl SaveIntent {
    fn wire(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Validated => "validated",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodePosition {
    pub node_index: u32,
    pub x: i64,
    pub y: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowLayout {
    positions: Vec<NodePosition>,
}
impl WorkflowLayout {
    pub fn positions(&self) -> &[NodePosition] {
        &self.positions
    }
    pub fn serialized_bytes(&self) -> Result<Vec<u8>, RecordError> {
        let bytes = serde_json::to_vec(self).map_err(|_| RecordError::SerializationFailed)?;
        if bytes.len() > MAX_LAYOUT_BYTES {
            return Err(RecordError::SerializationLimit);
        }
        Ok(bytes)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRevision {
    schema_version: String,
    id: String,
    workflow_id: String,
    parent_revision_id: Option<String>,
    intent: SaveIntent,
    definition: WorkflowDefinition,
    layout: WorkflowLayout,
    #[serde(skip)]
    inspection: DefinitionInspection,
}
impl WorkflowRevision {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn workflow_id(&self) -> &str {
        &self.workflow_id
    }
    pub fn parent_revision_id(&self) -> Option<&str> {
        self.parent_revision_id.as_deref()
    }
    pub fn intent(&self) -> SaveIntent {
        self.intent
    }
    pub fn layout(&self) -> &WorkflowLayout {
        &self.layout
    }
    pub fn inspection(&self) -> &DefinitionInspection {
        &self.inspection
    }
    pub fn serialized_bytes(&self) -> Result<Vec<u8>, RecordError> {
        let bytes = serde_json::to_vec(self).map_err(|_| RecordError::SerializationFailed)?;
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(RecordError::SerializationLimit);
        }
        Ok(bytes)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordError {
    InputLimit,
    InvalidEncoding,
    InvalidJson,
    UnsupportedSchema,
    InvalidWorkflowId,
    InvalidParentId,
    InvalidRevisionId,
    DefinitionInvalid,
    LayoutInvalid,
    DepthLimit,
    ValidatedDefinitionRequired,
    IdentityMismatch,
    SerializationLimit,
    SerializationFailed,
}
impl RecordError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InputLimit => "input_limit",
            Self::InvalidEncoding => "invalid_encoding",
            Self::InvalidJson => "invalid_json",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::InvalidWorkflowId => "invalid_workflow_id",
            Self::InvalidParentId => "invalid_parent_id",
            Self::InvalidRevisionId => "invalid_revision_id",
            Self::DefinitionInvalid => "definition_invalid",
            Self::LayoutInvalid => "layout_invalid",
            Self::DepthLimit => "depth_limit",
            Self::ValidatedDefinitionRequired => "validated_definition_required",
            Self::IdentityMismatch => "identity_mismatch",
            Self::SerializationLimit => "serialization_limit",
            Self::SerializationFailed => "serialization_failed",
        }
    }
}
impl std::fmt::Display for RecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for RecordError {}

pub fn workflow_id_from_nonce(nonce: &[u8; 32]) -> String {
    let mut framed = b"rangoon.workflow-id.v1\0".to_vec();
    framed.extend_from_slice(nonce);
    format!("workflow:{}", byte_digest(&framed))
}

pub fn prepare_revision(
    workflow_id: &str,
    parent_revision_id: Option<&str>,
    intent: SaveIntent,
    definition_bytes: &[u8],
    layout_bytes: &[u8],
) -> Result<WorkflowRevision, RecordError> {
    if !valid_id(workflow_id, "workflow:") {
        return Err(RecordError::InvalidWorkflowId);
    }
    if parent_revision_id.is_some_and(|value| !valid_id(value, "workflow-revision:")) {
        return Err(RecordError::InvalidParentId);
    }
    let inspection =
        inspect_definition(definition_bytes).map_err(|_| RecordError::DefinitionInvalid)?;
    if intent == SaveIntent::Validated && !inspection.report().structurally_valid {
        return Err(RecordError::ValidatedDefinitionRequired);
    }
    let layout = decode_layout(layout_bytes, inspection.definition().nodes.len())?;
    let definition = inspection.definition().clone();
    build_revision(
        workflow_id.into(),
        parent_revision_id.map(str::to_owned),
        intent,
        definition,
        layout,
        inspection,
    )
}

pub fn decode_revision(bytes: &[u8]) -> Result<WorkflowRevision, RecordError> {
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(RecordError::InputLimit);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| RecordError::InvalidEncoding)?;
    scan_depth(text)?;
    scan_strict(text)?;
    let root: serde_json::Value =
        serde_json::from_str(text).map_err(|_| RecordError::InvalidJson)?;
    if !root
        .as_object()
        .is_some_and(|object| object.contains_key("parentRevisionId"))
    {
        return Err(RecordError::InvalidJson);
    }
    let wire: RevisionWire = serde_json::from_str(text).map_err(|_| RecordError::InvalidJson)?;
    if wire.schema_version != REVISION_SCHEMA {
        return Err(RecordError::UnsupportedSchema);
    }
    if !valid_id(&wire.workflow_id, "workflow:") {
        return Err(RecordError::InvalidWorkflowId);
    }
    if wire
        .parent_revision_id
        .0
        .as_deref()
        .is_some_and(|value| !valid_id(value, "workflow-revision:"))
    {
        return Err(RecordError::InvalidParentId);
    }
    if !valid_id(&wire.id, "workflow-revision:") {
        return Err(RecordError::InvalidRevisionId);
    }
    let definition_bytes =
        serde_json::to_vec(&wire.definition).map_err(|_| RecordError::SerializationFailed)?;
    let inspection =
        inspect_definition(&definition_bytes).map_err(|_| RecordError::DefinitionInvalid)?;
    if wire.intent == SaveIntent::Validated && !inspection.report().structurally_valid {
        return Err(RecordError::ValidatedDefinitionRequired);
    }
    let layout = validate_layout(wire.layout.positions, inspection.definition().nodes.len())?;
    let revision = build_revision(
        wire.workflow_id,
        wire.parent_revision_id.0,
        wire.intent,
        wire.definition,
        layout,
        inspection,
    )?;
    if revision.id != wire.id {
        return Err(RecordError::IdentityMismatch);
    }
    Ok(revision)
}

fn build_revision(
    workflow_id: String,
    parent_revision_id: Option<String>,
    intent: SaveIntent,
    definition: WorkflowDefinition,
    layout: WorkflowLayout,
    inspection: DefinitionInspection,
) -> Result<WorkflowRevision, RecordError> {
    let definition_bytes = inspection
        .canonical_definition_bytes()
        .map_err(|_| RecordError::DefinitionInvalid)?;
    let layout_bytes = layout.serialized_bytes()?;
    let id = revision_id(
        &workflow_id,
        parent_revision_id.as_deref(),
        intent,
        &definition_bytes,
        &layout_bytes,
    );
    let revision = WorkflowRevision {
        schema_version: REVISION_SCHEMA.into(),
        id,
        workflow_id,
        parent_revision_id,
        intent,
        definition,
        layout,
        inspection,
    };
    revision.serialized_bytes()?;
    Ok(revision)
}

fn revision_id(
    workflow: &str,
    parent: Option<&str>,
    intent: SaveIntent,
    definition: &[u8],
    layout: &[u8],
) -> String {
    let mut framed = b"rangoon.workflow-revision.v1\0".to_vec();
    for field in [
        workflow.as_bytes(),
        parent.unwrap_or("").as_bytes(),
        intent.wire().as_bytes(),
        definition,
        layout,
    ] {
        framed.extend_from_slice(&(field.len() as u64).to_be_bytes());
        framed.extend_from_slice(field);
    }
    format!("workflow-revision:{}", byte_digest(&framed))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RevisionWire {
    schema_version: String,
    id: String,
    workflow_id: String,
    parent_revision_id: RequiredNullableString,
    intent: SaveIntent,
    definition: WorkflowDefinition,
    layout: LayoutWire,
}
struct RequiredNullableString(Option<String>);
impl<'de> Deserialize<'de> for RequiredNullableString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<String>::deserialize(deserializer).map(Self)
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LayoutWire {
    positions: Vec<PositionWire>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PositionWire {
    node_index: u32,
    x: i64,
    y: i64,
}

fn decode_layout(bytes: &[u8], node_count: usize) -> Result<WorkflowLayout, RecordError> {
    if bytes.len() > MAX_LAYOUT_BYTES {
        return Err(RecordError::InputLimit);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| RecordError::InvalidEncoding)?;
    scan_depth(text)?;
    scan_strict(text)?;
    let layout: LayoutWire = serde_json::from_str(text).map_err(|_| RecordError::InvalidJson)?;
    validate_layout(layout.positions, node_count)
}
fn validate_layout(
    positions: Vec<PositionWire>,
    node_count: usize,
) -> Result<WorkflowLayout, RecordError> {
    if positions.len() > MAX_POSITIONS {
        return Err(RecordError::LayoutInvalid);
    }
    let mut seen = HashSet::new();
    let mut positions = positions
        .into_iter()
        .map(|position| NodePosition {
            node_index: position.node_index,
            x: position.x,
            y: position.y,
        })
        .collect::<Vec<_>>();
    for position in &positions {
        if position.node_index as usize >= node_count
            || !seen.insert(position.node_index)
            || !(-COORDINATE_LIMIT..=COORDINATE_LIMIT).contains(&position.x)
            || !(-COORDINATE_LIMIT..=COORDINATE_LIMIT).contains(&position.y)
        {
            return Err(RecordError::LayoutInvalid);
        }
    }
    positions.sort_by_key(|position| position.node_index);
    Ok(WorkflowLayout { positions })
}
fn scan_depth(text: &str) -> Result<(), RecordError> {
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for &byte in text.as_bytes() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
            continue;
        }
        match byte {
            b'"' => quoted = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > 32 {
                    return Err(RecordError::DepthLimit);
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}
fn scan_strict(input: &str) -> Result<(), RecordError> {
    let mut scanner = RecordScanner {
        bytes: input.as_bytes(),
        at: 0,
    };
    scanner.value()?;
    scanner.ws();
    if scanner.at == scanner.bytes.len() {
        Ok(())
    } else {
        Err(RecordError::InvalidJson)
    }
}
struct RecordScanner<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl RecordScanner<'_> {
    fn ws(&mut self) {
        while self
            .bytes
            .get(self.at)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            self.at += 1;
        }
    }
    fn value(&mut self) -> Result<(), RecordError> {
        self.ws();
        match self.bytes.get(self.at) {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(|_| ()),
            Some(b'-' | b'0'..=b'9' | b't' | b'f' | b'n') => self.atom(),
            _ => Err(RecordError::InvalidJson),
        }
    }
    fn object(&mut self) -> Result<(), RecordError> {
        self.at += 1;
        let mut keys = HashSet::new();
        if self.take(b'}') {
            return Ok(());
        }
        loop {
            self.ws();
            let key = self.string()?;
            if !keys.insert(key) {
                return Err(RecordError::InvalidJson);
            }
            if !self.take(b':') {
                return Err(RecordError::InvalidJson);
            }
            self.value()?;
            if self.take(b'}') {
                return Ok(());
            }
            if !self.take(b',') {
                return Err(RecordError::InvalidJson);
            }
        }
    }
    fn array(&mut self) -> Result<(), RecordError> {
        self.at += 1;
        if self.take(b']') {
            return Ok(());
        }
        loop {
            self.value()?;
            if self.take(b']') {
                return Ok(());
            }
            if !self.take(b',') {
                return Err(RecordError::InvalidJson);
            }
        }
    }
    fn string(&mut self) -> Result<String, RecordError> {
        if !self.take(b'"') {
            return Err(RecordError::InvalidJson);
        }
        let start = self.at - 1;
        let mut escaped = false;
        while let Some(&byte) = self.bytes.get(self.at) {
            self.at += 1;
            if escaped {
                escaped = false;
                continue;
            }
            if byte == b'\\' {
                escaped = true;
                continue;
            }
            if byte == b'"' {
                return serde_json::from_slice(&self.bytes[start..self.at])
                    .map_err(|_| RecordError::InvalidJson);
            }
            if byte < 0x20 {
                return Err(RecordError::InvalidJson);
            }
        }
        Err(RecordError::InvalidJson)
    }
    fn atom(&mut self) -> Result<(), RecordError> {
        let start = self.at;
        while self
            .bytes
            .get(self.at)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b',' | b']' | b'}'))
        {
            self.at += 1;
        }
        serde_json::from_slice::<serde_json::Value>(&self.bytes[start..self.at])
            .map(|_| ())
            .map_err(|_| RecordError::InvalidJson)
    }
    fn take(&mut self, byte: u8) -> bool {
        self.ws();
        if self.bytes.get(self.at) == Some(&byte) {
            self.at += 1;
            true
        } else {
            false
        }
    }
}
