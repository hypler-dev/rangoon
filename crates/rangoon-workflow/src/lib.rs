//! Portable workflow-definition validation. This crate has no I/O or execution authority.
#![forbid(unsafe_code)]

use rangoon_domain::capability::{valid_id, valid_title};
use rangoon_domain::{Authority, byte_digest};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

pub mod records;

pub const DEFINITION_SCHEMA: &str = "rangoon.workflow-definition.v1";
pub const REPORT_SCHEMA: &str = "rangoon.workflow-validation.v1";
pub const MAX_INPUT_BYTES: usize = 128 * 1024;
pub const MAX_NODES: usize = 128;
pub const MAX_CONTROL_EDGES: usize = 256;
pub const MAX_DATA_EDGES: usize = 1024;
pub const MAX_PORTS: usize = 8;
pub const MAX_DIAGNOSTICS: usize = 128;
pub const MAX_INSPECTION_BYTES: usize = 256 * 1024;
const MAX_CONTAINER_DEPTH: usize = 32;
const MAX_SCANNER_RECURSION_DEPTH: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowError {
    InputLimit,
    InvalidEncoding,
    InvalidJson,
    UnsupportedSchema,
    CollectionLimit,
    DepthLimit,
    CanonicalLimit,
    InspectionLimit,
    SerializationFailed,
}

impl WorkflowError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InputLimit => "input_limit",
            Self::InvalidEncoding => "invalid_encoding",
            Self::InvalidJson => "invalid_json",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::CollectionLimit => "collection_limit",
            Self::DepthLimit => "depth_limit",
            Self::CanonicalLimit => "canonical_limit",
            Self::InspectionLimit => "inspection_limit",
            Self::SerializationFailed => "serialization_failed",
        }
    }
}
impl std::fmt::Display for WorkflowError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}
impl std::error::Error for WorkflowError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowDefinition {
    pub schema_version: String,
    pub title: String,
    pub nodes: Vec<WorkflowNode>,
    pub control_edges: Vec<ControlEdge>,
    pub data_edges: Vec<DataEdge>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowNode {
    pub id: String,
    pub title: String,
    pub operation: Operation,
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Operation {
    Input,
    Capability {
        capability_id: String,
        revision_id: String,
    },
    Check {
        check: Check,
    },
    Branch,
    Checkpoint {
        prompt: String,
    },
    Output,
}

impl<'de> Deserialize<'de> for Operation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct InputWire {
            kind: String,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct CapabilityWire {
            kind: String,
            capability_id: String,
            revision_id: String,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct CheckWire {
            kind: String,
            check: Check,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct BranchWire {
            kind: String,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct CheckpointWire {
            kind: String,
            prompt: String,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct OutputWire {
            kind: String,
        }
        let value = serde_json::Value::deserialize(deserializer)?;
        let kind = value
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| serde::de::Error::custom("operation kind"))?;
        match kind {
            "input" => {
                let wire: InputWire =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                if wire.kind == "input" {
                    Ok(Self::Input)
                } else {
                    Err(serde::de::Error::custom("operation kind"))
                }
            }
            "capability" => {
                let wire: CapabilityWire =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                if wire.kind == "capability" {
                    Ok(Self::Capability {
                        capability_id: wire.capability_id,
                        revision_id: wire.revision_id,
                    })
                } else {
                    Err(serde::de::Error::custom("operation kind"))
                }
            }
            "check" => {
                let wire: CheckWire =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                if wire.kind == "check" {
                    Ok(Self::Check { check: wire.check })
                } else {
                    Err(serde::de::Error::custom("operation kind"))
                }
            }
            "branch" => {
                let wire: BranchWire =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                if wire.kind == "branch" {
                    Ok(Self::Branch)
                } else {
                    Err(serde::de::Error::custom("operation kind"))
                }
            }
            "checkpoint" => {
                let wire: CheckpointWire =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                if wire.kind == "checkpoint" {
                    Ok(Self::Checkpoint {
                        prompt: wire.prompt,
                    })
                } else {
                    Err(serde::de::Error::custom("operation kind"))
                }
            }
            "output" => {
                let wire: OutputWire =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                if wire.kind == "output" {
                    Ok(Self::Output)
                } else {
                    Err(serde::de::Error::custom("operation kind"))
                }
            }
            _ => Err(serde::de::Error::custom("operation kind")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Check {
    NonEmpty,
    ContainsText { needle: String },
}

impl<'de> Deserialize<'de> for Check {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct NonEmptyWire {
            kind: String,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct ContainsTextWire {
            kind: String,
            needle: String,
        }
        let value = serde_json::Value::deserialize(deserializer)?;
        let kind = value
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| serde::de::Error::custom("check kind"))?;
        match kind {
            "non_empty" => {
                let wire: NonEmptyWire =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                if wire.kind == "non_empty" {
                    Ok(Self::NonEmpty)
                } else {
                    Err(serde::de::Error::custom("check kind"))
                }
            }
            "contains_text" => {
                let wire: ContainsTextWire =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                if wire.kind == "contains_text" {
                    Ok(Self::ContainsText {
                        needle: wire.needle,
                    })
                } else {
                    Err(serde::de::Error::custom("check kind"))
                }
            }
            _ => Err(serde::de::Error::custom("check kind")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Port {
    pub name: String,
    pub data_type: DataType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    Text,
    Json,
    Boolean,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlEdge {
    pub from_node: String,
    pub outlet: ControlOutlet,
    pub to_node: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlOutlet {
    Next,
    True,
    False,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataEdge {
    pub from_node: String,
    pub from_port: String,
    pub to_node: String,
    pub to_port: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    InvalidTitle,
    InvalidNodeCount,
    InvalidNodeId,
    DuplicateNodeId,
    InvalidInputPortName,
    InvalidOutputPortName,
    DuplicateInputPort,
    DuplicateOutputPort,
    InvalidOperationPorts,
    InvalidReference,
    InvalidCheckNeedle,
    InvalidCheckpointPrompt,
    EntryCount,
    OutputMissing,
    MissingControlEndpoint,
    DuplicateControlEdge,
    SelfControlEdge,
    InvalidControlOutlet,
    ControlCardinality,
    EntryIncoming,
    Cycle,
    UnreachableNode,
    NoOutputPath,
    MissingDataEndpoint,
    MissingOutputPort,
    MissingInputPort,
    DuplicateDataEdge,
    SelfDataEdge,
    TypeMismatch,
    InputCardinality,
    DataNotDominating,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionDiagnostic {
    pub code: DiagnosticCode,
    pub node_index: Option<u32>,
    pub control_edge_index: Option<u32>,
    pub data_edge_index: Option<u32>,
    pub port_index: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyReference {
    pub capability_id: String,
    pub revision_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceStatus {
    Unverified,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub schema_version: String,
    pub structurally_valid: bool,
    pub definition_id: Option<String>,
    pub diagnostics: Vec<DefinitionDiagnostic>,
    pub diagnostics_truncated: bool,
    pub topological_order: Vec<String>,
    pub dependencies: Vec<DependencyReference>,
    pub reference_status: ReferenceStatus,
    pub execution_status: ExecutionStatus,
    pub authority: Authority,
}

/// The decoded definition and its immutable inspection result. Fields are private deliberately.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionInspection {
    definition: WorkflowDefinition,
    report: ValidationReport,
}

impl DefinitionInspection {
    pub fn definition(&self) -> &WorkflowDefinition {
        &self.definition
    }
    pub fn report(&self) -> &ValidationReport {
        &self.report
    }

    /// Compact canonical wire bytes for storage or draft comparison; validity remains in the report.
    pub fn canonical_definition_bytes(&self) -> Result<Vec<u8>, WorkflowError> {
        canonical_definition_bytes(&self.definition)
    }

    /// Serializes the read-only inspection under the contract's bounded output budget.
    pub fn serialized_bytes(&self) -> Result<Vec<u8>, WorkflowError> {
        let bytes = serde_json::to_vec(self).map_err(|_| WorkflowError::SerializationFailed)?;
        if bytes.len() > MAX_INSPECTION_BYTES {
            return Err(WorkflowError::InspectionLimit);
        }
        Ok(bytes)
    }
}

/// Decodes one closed V1 JSON definition and reports all deterministic structural failures.
pub fn inspect_definition(bytes: &[u8]) -> Result<DefinitionInspection, WorkflowError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(WorkflowError::InputLimit);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| WorkflowError::InvalidEncoding)?;
    scan_depth(text)?;
    scan_json(text)?;
    let definition: WorkflowDefinition =
        serde_json::from_str(text).map_err(|_| WorkflowError::InvalidJson)?;
    if definition.schema_version != DEFINITION_SCHEMA {
        return Err(WorkflowError::UnsupportedSchema);
    }
    Ok(validate_definition(definition))
}

fn canonical_definition_bytes(definition: &WorkflowDefinition) -> Result<Vec<u8>, WorkflowError> {
    let bytes = serde_json::to_vec(definition).map_err(|_| WorkflowError::SerializationFailed)?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(WorkflowError::CanonicalLimit);
    }
    Ok(bytes)
}

fn definition_id(canonical: &[u8]) -> String {
    let mut framed = Vec::with_capacity(DEFINITION_SCHEMA.len() + 1 + 8 + canonical.len());
    framed.extend_from_slice(DEFINITION_SCHEMA.as_bytes());
    framed.push(0);
    framed.extend_from_slice(&(canonical.len() as u64).to_be_bytes());
    framed.extend_from_slice(canonical);
    format!("workflow-definition:{}", byte_digest(&framed))
}

struct Validation {
    diagnostics: Vec<DefinitionDiagnostic>,
    truncated: bool,
}
impl Validation {
    fn push(
        &mut self,
        code: DiagnosticCode,
        node: Option<usize>,
        control: Option<usize>,
        data: Option<usize>,
        port: Option<usize>,
    ) {
        if self.diagnostics.len() == MAX_DIAGNOSTICS {
            self.truncated = true;
            return;
        }
        self.diagnostics.push(DefinitionDiagnostic {
            code,
            node_index: node.map(|value| value as u32),
            control_edge_index: control.map(|value| value as u32),
            data_edge_index: data.map(|value| value as u32),
            port_index: port.map(|value| value as u32),
        });
    }
    fn invalid(&self) -> bool {
        !self.diagnostics.is_empty() || self.truncated
    }
}

fn validate_definition(definition: WorkflowDefinition) -> DefinitionInspection {
    let mut validation = Validation {
        diagnostics: Vec::new(),
        truncated: false,
    };
    if !valid_title(&definition.title) {
        validation.push(DiagnosticCode::InvalidTitle, None, None, None, None);
    }
    if !(2..=MAX_NODES).contains(&definition.nodes.len()) {
        validation.push(DiagnosticCode::InvalidNodeCount, None, None, None, None);
    }

    let mut ids = HashMap::new();
    let mut ids_usable = true;
    let mut dependencies = Vec::new();
    let mut dependency_seen = HashSet::new();
    let mut input_namespaces = Vec::with_capacity(definition.nodes.len());
    let mut output_namespaces = Vec::with_capacity(definition.nodes.len());
    for (node_index, node) in definition.nodes.iter().enumerate() {
        if !valid_identifier(&node.id) {
            validation.push(
                DiagnosticCode::InvalidNodeId,
                Some(node_index),
                None,
                None,
                None,
            );
            ids_usable = false;
        }
        if ids.insert(node.id.as_str(), node_index).is_some() {
            validation.push(
                DiagnosticCode::DuplicateNodeId,
                Some(node_index),
                None,
                None,
                None,
            );
            ids_usable = false;
        }
        if !valid_title(&node.title) {
            validation.push(
                DiagnosticCode::InvalidTitle,
                Some(node_index),
                None,
                None,
                None,
            );
        }
        validate_operation(
            &mut validation,
            node_index,
            node,
            &mut dependencies,
            &mut dependency_seen,
        );
        input_namespaces.push(validate_ports(
            &mut validation,
            node_index,
            &node.inputs,
            true,
        ));
        output_namespaces.push(validate_ports(
            &mut validation,
            node_index,
            &node.outputs,
            false,
        ));
    }

    let control = validate_control(&definition, &ids, ids_usable, &mut validation);
    validate_data(
        &definition,
        &ids,
        ids_usable,
        &input_namespaces,
        &output_namespaces,
        control.as_ref(),
        &mut validation,
    );

    let structurally_valid = !validation.invalid();
    let (definition_id, topological_order) = if structurally_valid {
        let canonical =
            canonical_definition_bytes(&definition).expect("bounded serializable definition");
        let order = control
            .expect("valid definition has control topology")
            .order;
        (
            Some(definition_id(&canonical)),
            order
                .into_iter()
                .map(|index| definition.nodes[index].id.clone())
                .collect(),
        )
    } else {
        (None, Vec::new())
    };
    DefinitionInspection {
        definition,
        report: ValidationReport {
            schema_version: REPORT_SCHEMA.into(),
            structurally_valid,
            definition_id,
            diagnostics: validation.diagnostics,
            diagnostics_truncated: validation.truncated,
            topological_order,
            dependencies,
            reference_status: ReferenceStatus::Unverified,
            execution_status: ExecutionStatus::Unavailable,
            authority: Authority::None,
        },
    }
}

fn valid_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=48).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn validate_ports(validation: &mut Validation, node: usize, ports: &[Port], inputs: bool) -> bool {
    let mut seen = HashSet::new();
    let mut usable = true;
    for (port_index, port) in ports.iter().enumerate() {
        if !valid_identifier(&port.name) {
            validation.push(
                if inputs {
                    DiagnosticCode::InvalidInputPortName
                } else {
                    DiagnosticCode::InvalidOutputPortName
                },
                Some(node),
                None,
                None,
                Some(port_index),
            );
            usable = false;
        }
        if !seen.insert(port.name.as_str()) {
            validation.push(
                if inputs {
                    DiagnosticCode::DuplicateInputPort
                } else {
                    DiagnosticCode::DuplicateOutputPort
                },
                Some(node),
                None,
                None,
                Some(port_index),
            );
            usable = false;
        }
    }
    usable
}

fn validate_operation(
    validation: &mut Validation,
    index: usize,
    node: &WorkflowNode,
    dependencies: &mut Vec<DependencyReference>,
    seen: &mut HashSet<(String, String)>,
) {
    let mut detail = None;
    let operation_ports_valid = match &node.operation {
        Operation::Input => node.inputs.is_empty() && (1..=MAX_PORTS).contains(&node.outputs.len()),
        Operation::Capability {
            capability_id,
            revision_id,
        } => {
            let valid =
                valid_id(capability_id, "capability:") && valid_id(revision_id, "revision:");
            if valid && seen.insert((capability_id.clone(), revision_id.clone())) {
                dependencies.push(DependencyReference {
                    capability_id: capability_id.clone(),
                    revision_id: revision_id.clone(),
                });
            }
            if !valid {
                detail = Some(DiagnosticCode::InvalidReference);
            }
            (1..=MAX_PORTS).contains(&node.inputs.len())
                && (1..=MAX_PORTS).contains(&node.outputs.len())
        }
        Operation::Check { check } => {
            let valid_check = match check {
                Check::NonEmpty => true,
                Check::ContainsText { needle } => {
                    !needle.is_empty() && needle.len() <= 1024 && !needle.contains('\0')
                }
            };
            if !valid_check {
                detail = Some(DiagnosticCode::InvalidCheckNeedle);
            }
            exact_ports(&node.inputs, "value", DataType::Text)
                && exact_ports(&node.outputs, "passed", DataType::Boolean)
        }
        Operation::Branch => {
            exact_ports(&node.inputs, "condition", DataType::Boolean) && node.outputs.is_empty()
        }
        Operation::Checkpoint { prompt } => {
            if prompt.trim().is_empty() || prompt.len() > 1024 || prompt.contains('\0') {
                detail = Some(DiagnosticCode::InvalidCheckpointPrompt);
            }
            node.inputs.is_empty() && node.outputs.is_empty()
        }
        Operation::Output => {
            (1..=MAX_PORTS).contains(&node.inputs.len()) && node.outputs.is_empty()
        }
    };
    if !operation_ports_valid {
        validation.push(
            DiagnosticCode::InvalidOperationPorts,
            Some(index),
            None,
            None,
            None,
        );
    }
    if let Some(code) = detail {
        validation.push(code, Some(index), None, None, None);
    }
}

fn exact_ports(ports: &[Port], name: &str, data_type: DataType) -> bool {
    matches!(ports, [Port { name: actual, data_type: actual_type }] if actual == name && *actual_type == data_type)
}

#[derive(Clone)]
struct ControlTopology {
    order: Vec<usize>,
    predecessors: Vec<Vec<usize>>,
}

fn validate_control(
    definition: &WorkflowDefinition,
    ids: &HashMap<&str, usize>,
    ids_usable: bool,
    validation: &mut Validation,
) -> Option<ControlTopology> {
    let count = definition.nodes.len();
    let mut usable = ids_usable;
    let mut adjacency = vec![Vec::new(); count];
    let mut predecessors = vec![Vec::new(); count];
    let mut seen = HashSet::new();
    let entries: Vec<_> = definition
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| matches!(node.operation, Operation::Input).then_some(index))
        .collect();
    if entries.len() != 1 {
        validation.push(DiagnosticCode::EntryCount, None, None, None, None);
    }
    let outputs: Vec<_> = definition
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| matches!(node.operation, Operation::Output).then_some(index))
        .collect();
    if outputs.is_empty() {
        validation.push(DiagnosticCode::OutputMissing, None, None, None, None);
    }
    if !ids_usable {
        return None;
    }
    for (edge_index, edge) in definition.control_edges.iter().enumerate() {
        let Some(&from) = ids.get(edge.from_node.as_str()) else {
            validation.push(
                DiagnosticCode::MissingControlEndpoint,
                None,
                Some(edge_index),
                None,
                None,
            );
            usable = false;
            continue;
        };
        let Some(&to) = ids.get(edge.to_node.as_str()) else {
            validation.push(
                DiagnosticCode::MissingControlEndpoint,
                None,
                Some(edge_index),
                None,
                None,
            );
            usable = false;
            continue;
        };
        if from == to {
            validation.push(
                DiagnosticCode::SelfControlEdge,
                None,
                Some(edge_index),
                None,
                None,
            );
            usable = false;
            continue;
        }
        if !seen.insert((from, edge.outlet, to)) {
            validation.push(
                DiagnosticCode::DuplicateControlEdge,
                None,
                Some(edge_index),
                None,
                None,
            );
            usable = false;
            continue;
        }
        if !outlet_allowed(&definition.nodes[from].operation, edge.outlet) {
            validation.push(
                DiagnosticCode::InvalidControlOutlet,
                None,
                Some(edge_index),
                None,
                None,
            );
            usable = false;
            continue;
        }
        adjacency[from].push(to);
        predecessors[to].push(from);
    }

    for (index, node) in definition.nodes.iter().enumerate() {
        let outgoing = definition
            .control_edges
            .iter()
            .filter(|edge| {
                ids.get(edge.from_node.as_str()) == Some(&index)
                    && outlet_allowed(&node.operation, edge.outlet)
            })
            .count();
        let required = match node.operation {
            Operation::Branch => 2,
            Operation::Output => 0,
            _ => 1,
        };
        if outgoing != required {
            validation.push(
                DiagnosticCode::ControlCardinality,
                Some(index),
                None,
                None,
                None,
            );
        }
        if matches!(node.operation, Operation::Branch) {
            let true_count = definition
                .control_edges
                .iter()
                .filter(|edge| {
                    ids.get(edge.from_node.as_str()) == Some(&index)
                        && edge.outlet == ControlOutlet::True
                })
                .count();
            let false_count = definition
                .control_edges
                .iter()
                .filter(|edge| {
                    ids.get(edge.from_node.as_str()) == Some(&index)
                        && edge.outlet == ControlOutlet::False
                })
                .count();
            if true_count != 1 || false_count != 1 {
                validation.push(
                    DiagnosticCode::ControlCardinality,
                    Some(index),
                    None,
                    None,
                    None,
                );
            }
        }
    }
    if !usable || entries.len() != 1 {
        return None;
    }
    let entry = entries[0];
    if !predecessors[entry].is_empty() {
        validation.push(DiagnosticCode::EntryIncoming, Some(entry), None, None, None);
    }

    let order = stable_topological_order(&adjacency, &predecessors);
    let Some(order) = order else {
        validation.push(DiagnosticCode::Cycle, None, None, None, None);
        return None;
    };
    let reached = reachable(entry, &adjacency);
    for (index, is_reached) in reached.iter().enumerate() {
        if !is_reached {
            validation.push(
                DiagnosticCode::UnreachableNode,
                Some(index),
                None,
                None,
                None,
            );
        }
    }
    let mut reversed = vec![Vec::new(); count];
    for (from, targets) in adjacency.iter().enumerate() {
        for &to in targets {
            reversed[to].push(from);
        }
    }
    let mut can_reach_output = vec![false; count];
    let mut queue = VecDeque::new();
    for output in outputs {
        can_reach_output[output] = true;
        queue.push_back(output);
    }
    while let Some(node) = queue.pop_front() {
        for &previous in &reversed[node] {
            if !can_reach_output[previous] {
                can_reach_output[previous] = true;
                queue.push_back(previous);
            }
        }
    }
    for (index, reaches) in can_reach_output.iter().enumerate() {
        if !reaches {
            validation.push(DiagnosticCode::NoOutputPath, Some(index), None, None, None);
        }
    }
    Some(ControlTopology {
        order,
        predecessors,
    })
}

fn outlet_allowed(operation: &Operation, outlet: ControlOutlet) -> bool {
    match operation {
        Operation::Branch => matches!(outlet, ControlOutlet::True | ControlOutlet::False),
        Operation::Output => false,
        _ => outlet == ControlOutlet::Next,
    }
}
fn stable_topological_order(
    adjacency: &[Vec<usize>],
    predecessors: &[Vec<usize>],
) -> Option<Vec<usize>> {
    let mut indegrees: Vec<_> = predecessors.iter().map(Vec::len).collect();
    let mut used = vec![false; adjacency.len()];
    let mut order = Vec::with_capacity(adjacency.len());
    for _ in 0..adjacency.len() {
        let next = (0..adjacency.len()).find(|&index| !used[index] && indegrees[index] == 0)?;
        used[next] = true;
        order.push(next);
        for &target in &adjacency[next] {
            indegrees[target] -= 1;
        }
    }
    Some(order)
}
fn reachable(entry: usize, adjacency: &[Vec<usize>]) -> Vec<bool> {
    let mut result = vec![false; adjacency.len()];
    result[entry] = true;
    let mut queue = VecDeque::from([entry]);
    while let Some(node) = queue.pop_front() {
        for &next in &adjacency[node] {
            if !result[next] {
                result[next] = true;
                queue.push_back(next);
            }
        }
    }
    result
}

fn validate_data(
    definition: &WorkflowDefinition,
    ids: &HashMap<&str, usize>,
    ids_usable: bool,
    input_namespaces: &[bool],
    output_namespaces: &[bool],
    topology: Option<&ControlTopology>,
    validation: &mut Validation,
) {
    if !ids_usable {
        return;
    }
    let mut producers = HashMap::<(usize, &str), usize>::new();
    let mut seen = HashSet::new();
    let mut ambiguous_destinations = HashSet::<(usize, &str)>::new();
    let dominators = topology.and_then(|topology| {
        definition
            .nodes
            .iter()
            .position(|node| matches!(node.operation, Operation::Input))
            .map(|entry| dominators(entry, &topology.predecessors, &topology.order))
    });
    for (edge_index, edge) in definition.data_edges.iter().enumerate() {
        let Some(&from) = ids.get(edge.from_node.as_str()) else {
            validation.push(
                DiagnosticCode::MissingDataEndpoint,
                None,
                None,
                Some(edge_index),
                None,
            );
            continue;
        };
        let Some(&to) = ids.get(edge.to_node.as_str()) else {
            validation.push(
                DiagnosticCode::MissingDataEndpoint,
                None,
                None,
                Some(edge_index),
                None,
            );
            continue;
        };
        if from == to {
            validation.push(
                DiagnosticCode::SelfDataEdge,
                None,
                None,
                Some(edge_index),
                None,
            );
            continue;
        }
        if !seen.insert((from, edge.from_port.as_str(), to, edge.to_port.as_str())) {
            validation.push(
                DiagnosticCode::DuplicateDataEdge,
                None,
                None,
                Some(edge_index),
                None,
            );
            continue;
        }
        let source = if output_namespaces[from] {
            let source = definition.nodes[from]
                .outputs
                .iter()
                .find(|port| port.name == edge.from_port);
            if source.is_none() {
                validation.push(
                    DiagnosticCode::MissingOutputPort,
                    None,
                    None,
                    Some(edge_index),
                    None,
                );
            }
            source
        } else {
            None
        };
        let destination = if input_namespaces[to] {
            let destination = definition.nodes[to]
                .inputs
                .iter()
                .find(|port| port.name == edge.to_port);
            if destination.is_none() {
                validation.push(
                    DiagnosticCode::MissingInputPort,
                    None,
                    None,
                    Some(edge_index),
                    None,
                );
            }
            destination
        } else {
            None
        };
        if !output_namespaces[from] || !input_namespaces[to] {
            if !output_namespaces[from] && destination.is_some() {
                ambiguous_destinations.insert((to, edge.to_port.as_str()));
            }
            continue;
        }
        let Some(source) = source else {
            continue;
        };
        let Some(destination) = destination else {
            continue;
        };
        if source.data_type != destination.data_type {
            validation.push(
                DiagnosticCode::TypeMismatch,
                None,
                None,
                Some(edge_index),
                None,
            );
        }
        if producers
            .insert((to, edge.to_port.as_str()), edge_index)
            .is_some()
        {
            validation.push(
                DiagnosticCode::InputCardinality,
                None,
                None,
                Some(edge_index),
                None,
            );
        }
        if dominators.as_ref().is_some_and(|matrix| !matrix[to][from]) {
            validation.push(
                DiagnosticCode::DataNotDominating,
                None,
                None,
                Some(edge_index),
                None,
            );
        }
    }
    for (node_index, node) in definition.nodes.iter().enumerate() {
        if !input_namespaces[node_index] {
            continue;
        }
        for (port_index, port) in node.inputs.iter().enumerate() {
            if !ambiguous_destinations.contains(&(node_index, port.name.as_str()))
                && !producers.contains_key(&(node_index, port.name.as_str()))
            {
                validation.push(
                    DiagnosticCode::InputCardinality,
                    Some(node_index),
                    None,
                    None,
                    Some(port_index),
                );
            }
        }
    }
}

fn dominators(entry: usize, predecessors: &[Vec<usize>], order: &[usize]) -> Vec<Vec<bool>> {
    let count = predecessors.len();
    let mut result = vec![vec![true; count]; count];
    result[entry] = vec![false; count];
    result[entry][entry] = true;
    for &node in order {
        if node == entry {
            continue;
        }
        if predecessors[node].is_empty() {
            result[node] = vec![false; count];
            continue;
        }
        let mut current = vec![true; count];
        for &previous in &predecessors[node] {
            for (index, current_value) in current.iter_mut().enumerate() {
                *current_value &= result[previous][index];
            }
        }
        current[node] = true;
        result[node] = current;
    }
    result
}

#[derive(Clone, Copy)]
enum ScanError {
    Invalid,
    Limit,
}
impl From<ScanError> for WorkflowError {
    fn from(value: ScanError) -> Self {
        match value {
            ScanError::Invalid => Self::InvalidJson,
            ScanError::Limit => Self::CollectionLimit,
        }
    }
}
fn scan_json(input: &str) -> Result<(), WorkflowError> {
    let mut scanner = JsonScanner {
        bytes: input.as_bytes(),
        at: 0,
        depth: 0,
    };
    scanner.value(None).map_err(WorkflowError::from)?;
    scanner.ws();
    if scanner.at != scanner.bytes.len() {
        return Err(WorkflowError::InvalidJson);
    }
    Ok(())
}

/// Counts containers without interpreting JSON syntax so malformed input stays an invalid-json error.
fn scan_depth(input: &str) -> Result<(), WorkflowError> {
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for &byte in input.as_bytes() {
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
                if depth > MAX_CONTAINER_DEPTH {
                    return Err(WorkflowError::DepthLimit);
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}
struct JsonScanner<'a> {
    bytes: &'a [u8],
    at: usize,
    depth: usize,
}
impl JsonScanner<'_> {
    fn ws(&mut self) {
        while self
            .bytes
            .get(self.at)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            self.at += 1;
        }
    }
    fn value(&mut self, array_limit: Option<usize>) -> Result<(), ScanError> {
        self.ws();
        self.depth += 1;
        if self.depth > MAX_SCANNER_RECURSION_DEPTH {
            return Err(ScanError::Limit);
        }
        let result = match self.bytes.get(self.at) {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(array_limit.unwrap_or(MAX_DATA_EDGES)),
            Some(b'"') => self.string().map(|_| ()),
            Some(b'-' | b'0'..=b'9' | b't' | b'f' | b'n') => self.atom(),
            _ => Err(ScanError::Invalid),
        };
        self.depth -= 1;
        result
    }
    fn object(&mut self) -> Result<(), ScanError> {
        self.at += 1;
        self.ws();
        let mut names = HashSet::new();
        let mut count = 0usize;
        if self.take(b'}') {
            return Ok(());
        }
        loop {
            count += 1;
            if count > MAX_DATA_EDGES {
                return Err(ScanError::Limit);
            }
            let name = self.string()?;
            if !names.insert(name.clone()) {
                return Err(ScanError::Invalid);
            }
            if !self.take(b':') {
                return Err(ScanError::Invalid);
            }
            let limit = match name.as_str() {
                "nodes" => Some(MAX_NODES),
                "controlEdges" => Some(MAX_CONTROL_EDGES),
                "dataEdges" => Some(MAX_DATA_EDGES),
                "inputs" | "outputs" => Some(MAX_PORTS),
                _ => None,
            };
            self.value(limit)?;
            if self.take(b'}') {
                return Ok(());
            }
            if !self.take(b',') {
                return Err(ScanError::Invalid);
            }
        }
    }
    fn array(&mut self, limit: usize) -> Result<(), ScanError> {
        self.at += 1;
        self.ws();
        let mut count = 0usize;
        if self.take(b']') {
            return Ok(());
        }
        loop {
            count += 1;
            if count > limit {
                return Err(ScanError::Limit);
            }
            self.value(None)?;
            if self.take(b']') {
                return Ok(());
            }
            if !self.take(b',') {
                return Err(ScanError::Invalid);
            }
        }
    }
    fn string(&mut self) -> Result<String, ScanError> {
        if !self.take(b'"') {
            return Err(ScanError::Invalid);
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
                    .map_err(|_| ScanError::Invalid);
            }
            if byte < 0x20 {
                return Err(ScanError::Invalid);
            }
        }
        Err(ScanError::Invalid)
    }
    fn atom(&mut self) -> Result<(), ScanError> {
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
            .map_err(|_| ScanError::Invalid)
    }
    fn take(&mut self, expected: u8) -> bool {
        self.ws();
        if self.bytes.get(self.at) == Some(&expected) {
            self.at += 1;
            true
        } else {
            false
        }
    }
}
