//! Pure application of a composition recipe to explicit capability destinations.

use crate::{
    Authority, ComposeError, Draft, MAX_DRAFT_BYTES, MAX_OUTPUTS, ResolvedInput, byte_digest,
    ensure_serialized_json_bound, preview as core_preview, reject_duplicate_keys,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const REQUEST_SCHEMA: &str = "rangoon.composition-application-request.v0";
pub const PREVIEW_SCHEMA: &str = "rangoon.composition-application-preview.v0";
pub const APPLICATION_SCHEMA: &str = "rangoon.composition-application.v0";
pub const MAX_REQUEST_BYTES: usize = MAX_DRAFT_BYTES + 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub schema_version: String,
    pub draft: Draft,
    pub targets: Vec<Target>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Target {
    New {},
    Append {
        capability_id: String,
        expected_revision_id: String,
    },
}

/// Host-resolved current heads, supplied in append-target order only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetHead {
    pub capability_id: String,
    pub revision_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationError {
    Core(ComposeError),
    Json(&'static str),
    InvalidRequest(&'static str),
    Limit(&'static str),
    InvalidTarget {
        output_index: u32,
        reason: &'static str,
    },
    DuplicateDestination {
        output_index: u32,
    },
}

impl std::fmt::Display for ApplicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for ApplicationError {}
impl From<ComposeError> for ApplicationError {
    fn from(value: ComposeError) -> Self {
        Self::Core(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationPreview {
    pub schema_version: String,
    pub core: crate::Preview,
    pub application_id: String,
    pub applied_outputs: Vec<AppliedOutput>,
    pub saveable: bool,
    pub authority: Authority,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedOutput {
    pub output_index: u32,
    pub kind: AppliedKind,
    pub capability_id: String,
    pub parent_revision_id: Option<String>,
    pub revision_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppliedKind {
    New,
    Append,
}

/// Decode one closed request without allowing a renderer to supply unbounded targets.
pub fn decode_request_json(bytes: &[u8]) -> Result<Request, ApplicationError> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(ApplicationError::Limit("request_bytes"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ApplicationError::Json("utf8"))?;
    reject_duplicate_keys(text).map_err(map_json_error)?;
    let request: Request =
        serde_json::from_str(text).map_err(|_| ApplicationError::Json("schema"))?;
    validate_request_shape(&request)?;
    Ok(request)
}

/// Pure deterministic preview. Host state is checked only by its supplied pinned heads.
pub fn preview(
    request: &Request,
    resolved_inputs: &[ResolvedInput],
    target_heads: &[TargetHead],
) -> Result<ApplicationPreview, ApplicationError> {
    validate_request_shape(request)?;
    validate_target_heads(&request.targets, target_heads)?;
    let core = core_preview(&request.draft, resolved_inputs)?;
    validate_actual_destinations(&core.composition_id, &request.targets)?;
    let envelope = encode_application_envelope(&core.composition_id, &request.targets)?;
    let application_id = hash_application_envelope(&envelope);
    let mut applied_outputs = Vec::with_capacity(core.outputs.len());
    for (index, (output, target)) in core.outputs.iter().zip(&request.targets).enumerate() {
        let output_index = index as u32;
        match target {
            Target::New {} => applied_outputs.push(AppliedOutput {
                output_index,
                kind: AppliedKind::New,
                capability_id: output.capability_id.clone(),
                parent_revision_id: None,
                revision_id: output.revision_id.clone(),
            }),
            Target::Append {
                capability_id,
                expected_revision_id,
            } => applied_outputs.push(AppliedOutput {
                output_index,
                kind: AppliedKind::Append,
                capability_id: capability_id.clone(),
                parent_revision_id: Some(expected_revision_id.clone()),
                revision_id: composition_revision_id(
                    capability_id,
                    expected_revision_id,
                    &application_id,
                    output_index,
                    &output.title,
                    &output.content,
                )?,
            }),
        }
    }
    Ok(ApplicationPreview {
        schema_version: PREVIEW_SCHEMA.into(),
        saveable: core.saveable,
        core,
        application_id,
        applied_outputs,
        authority: Authority::None,
    })
}

/// Bind a structurally valid recipe to exactly one distinct destination per output.
/// This does not prove input existence, current heads, coverage or saveability.
pub fn application_identity_envelope_bytes(
    draft: &Draft,
    targets: &[Target],
) -> Result<Vec<u8>, ApplicationError> {
    validate_target_shape(draft, targets)?;
    let composition_id = crate::composition_id(draft)?;
    validate_actual_destinations(&composition_id, targets)?;
    encode_application_envelope(&composition_id, targets)
}

fn encode_application_envelope(
    composition_id: &str,
    targets: &[Target],
) -> Result<Vec<u8>, ApplicationError> {
    serde_json::to_vec(&ApplicationEnvelope {
        schema_version: APPLICATION_SCHEMA,
        composition_id,
        targets,
    })
    .map_err(|_| ApplicationError::InvalidRequest("encode"))
}

pub fn application_id(draft: &Draft, targets: &[Target]) -> Result<String, ApplicationError> {
    let envelope = application_identity_envelope_bytes(draft, targets)?;
    Ok(hash_application_envelope(&envelope))
}

fn hash_application_envelope(envelope: &[u8]) -> String {
    let mut bytes = b"rangoon.composition-application.v0\0".to_vec();
    bytes.extend_from_slice(&(envelope.len() as u64).to_be_bytes());
    bytes.extend_from_slice(envelope);
    format!("composition-application:{}", byte_digest(&bytes))
}

/// Revision identity for an append destination; ordinary revision IDs remain unchanged.
pub fn composition_revision_id(
    capability_id: &str,
    expected_parent_revision_id: &str,
    application_id: &str,
    output_index: u32,
    title: &str,
    content: &str,
) -> Result<String, ApplicationError> {
    if output_index as usize >= MAX_OUTPUTS {
        return Err(invalid_target(output_index, "output_index"));
    }
    if !valid_hash_id(capability_id, "capability:") {
        return Err(invalid_target(output_index, "capability_id"));
    }
    if !valid_hash_id(expected_parent_revision_id, "revision:") {
        return Err(invalid_target(output_index, "expected_revision_id"));
    }
    if !valid_hash_id(application_id, "composition-application:") {
        return Err(invalid_target(output_index, "application_id"));
    }
    if title.len() > MAX_DRAFT_BYTES || content.len() > crate::MAX_CONTENT_BYTES {
        return Err(invalid_target(output_index, "output_bounds"));
    }
    let mut bytes = b"rangoon.composition-revision.v0\0".to_vec();
    for field in [capability_id, expected_parent_revision_id, application_id] {
        bytes.extend_from_slice(&(field.len() as u64).to_be_bytes());
        bytes.extend_from_slice(field.as_bytes());
    }
    bytes.extend_from_slice(&output_index.to_be_bytes());
    for field in [title, content] {
        bytes.extend_from_slice(&(field.len() as u64).to_be_bytes());
        bytes.extend_from_slice(field.as_bytes());
    }
    Ok(format!("revision:{}", byte_digest(&bytes)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplicationEnvelope<'a> {
    schema_version: &'static str,
    composition_id: &'a str,
    targets: &'a [Target],
}

fn validate_request_shape(request: &Request) -> Result<(), ApplicationError> {
    if request.schema_version != REQUEST_SCHEMA {
        return Err(ApplicationError::InvalidRequest("schema_version"));
    }
    validate_target_shape(&request.draft, &request.targets)?;
    crate::validate_draft_shape(&request.draft)?;
    ensure_serialized_json_bound(request, MAX_REQUEST_BYTES, "request_bytes")?;
    Ok(())
}

fn validate_target_shape(draft: &Draft, targets: &[Target]) -> Result<(), ApplicationError> {
    if targets.len() > MAX_OUTPUTS {
        return Err(ApplicationError::Limit("targets"));
    }
    if targets.is_empty() || targets.len() != draft.outputs.len() {
        return Err(ApplicationError::InvalidRequest("target_count"));
    }
    validate_targets(targets)
}

fn validate_actual_destinations(
    composition_id: &str,
    targets: &[Target],
) -> Result<(), ApplicationError> {
    let mut destinations = HashSet::new();
    for (index, target) in targets.iter().enumerate() {
        let capability_id = match target {
            Target::New {} => crate::composed_capability_id(composition_id, index as u32),
            Target::Append { capability_id, .. } => capability_id.clone(),
        };
        if !destinations.insert(capability_id) {
            return Err(ApplicationError::DuplicateDestination {
                output_index: index as u32,
            });
        }
    }
    Ok(())
}

fn validate_targets(targets: &[Target]) -> Result<(), ApplicationError> {
    let mut append_capabilities = HashSet::new();
    for (index, target) in targets.iter().enumerate() {
        let output_index = index as u32;
        if let Target::Append {
            capability_id,
            expected_revision_id,
        } = target
        {
            if !valid_hash_id(capability_id, "capability:") {
                return Err(invalid_target(output_index, "capability_id"));
            }
            if !valid_hash_id(expected_revision_id, "revision:") {
                return Err(invalid_target(output_index, "expected_revision_id"));
            }
            if !append_capabilities.insert(capability_id.as_str()) {
                return Err(ApplicationError::DuplicateDestination { output_index });
            }
        }
    }
    Ok(())
}

fn validate_target_heads(targets: &[Target], heads: &[TargetHead]) -> Result<(), ApplicationError> {
    let append_indices: Vec<_> = targets
        .iter()
        .enumerate()
        .filter_map(|(index, target)| matches!(target, Target::Append { .. }).then_some(index))
        .collect();
    if heads.len() > append_indices.len() {
        return Err(ApplicationError::InvalidRequest("target_head_count"));
    }
    if heads.len() < append_indices.len() {
        return Err(invalid_target(
            append_indices[heads.len()] as u32,
            "target_head_count",
        ));
    }
    for (head, output_index) in heads.iter().zip(append_indices) {
        let Target::Append {
            capability_id,
            expected_revision_id,
        } = &targets[output_index]
        else {
            unreachable!()
        };
        if !valid_hash_id(&head.capability_id, "capability:")
            || !valid_hash_id(&head.revision_id, "revision:")
            || head.capability_id != *capability_id
            || head.revision_id != *expected_revision_id
        {
            return Err(invalid_target(output_index as u32, "target_head"));
        }
    }
    Ok(())
}

fn valid_hash_id(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|value| {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}
fn invalid_target(output_index: u32, reason: &'static str) -> ApplicationError {
    ApplicationError::InvalidTarget {
        output_index,
        reason,
    }
}
fn map_json_error(error: ComposeError) -> ApplicationError {
    match error {
        ComposeError::Limit(label) => ApplicationError::Limit(label),
        _ => ApplicationError::Json("syntax"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_target_rejects_unknown_fields() {
        assert!(serde_json::from_str::<Target>(r#"{"kind":"new","extra":true}"#).is_err());
    }
}
