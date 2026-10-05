//! Pure, bounded composition preview. It has no storage or execution authority.
#![forbid(unsafe_code)]

use rangoon_domain::capability::{revision_id, valid_content, valid_title};
use rangoon_domain::{Authority, byte_digest};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};

pub const DRAFT_SCHEMA: &str = "rangoon.composition-draft.v0";
pub const PREVIEW_SCHEMA: &str = "rangoon.composition-core-preview.v0";
pub const TRANSFORMATION_VERSION: &str = "0.1.0";
pub const MAX_DRAFT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_INPUTS: usize = 16;
pub const MAX_OUTPUTS: usize = 16;
pub const MAX_PIECES: usize = 256;
pub const MAX_EXCLUSIONS: usize = 256;
pub const MAX_DUPLICATIONS: usize = 256;
pub const MAX_CONFLICTS: usize = 128;
pub const MAX_SEGMENTS: usize = 4096;
pub const MAX_CONTENT_BYTES: usize = 256 * 1024;
pub const MAX_TOTAL_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_EXPLANATION_BYTES: usize = 1024;
const MAX_JSON_ARRAY_ITEMS: usize = 4096;
const MAX_JSON_DEPTH: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Decompose,
    Merge,
    Split,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum InputReference {
    Source {
        source_id: String,
        sha256: String,
    },
    Revision {
        capability_id: String,
        revision_id: String,
        sha256: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputRange {
    pub input_index: u32,
    pub start_byte: u64,
    pub end_byte: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutputRecipe {
    pub title: String,
    pub pieces: Vec<Piece>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Piece {
    Copy {
        range: InputRange,
    },
    Authored {
        content: String,
        reason: String,
    },
    Replace {
        range: InputRange,
        content: String,
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Annotation {
    pub range: InputRange,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Conflict {
    pub id: u32,
    pub title: String,
    pub ranges: Vec<InputRange>,
    pub context: String,
    pub resolution: Option<String>,
}

impl<'de> Deserialize<'de> for Conflict {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct WireConflict {
            id: u32,
            title: String,
            ranges: Vec<InputRange>,
            context: String,
            resolution: RequiredNullableString,
        }
        let value = WireConflict::deserialize(deserializer)?;
        Ok(Self {
            id: value.id,
            title: value.title,
            ranges: value.ranges,
            context: value.context,
            resolution: value.resolution.0,
        })
    }
}

struct RequiredNullableString(Option<String>);
impl<'de> Deserialize<'de> for RequiredNullableString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct NullableVisitor;
        impl<'de> serde::de::Visitor<'de> for NullableVisitor {
            type Value = RequiredNullableString;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a required string or null")
            }
            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RequiredNullableString(None))
            }
            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RequiredNullableString(None))
            }
            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RequiredNullableString(Some(value)))
            }
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RequiredNullableString(Some(value.into())))
            }
        }
        deserializer.deserialize_any(NullableVisitor)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Draft {
    pub schema_version: String,
    pub operation: Operation,
    pub inputs: Vec<InputReference>,
    pub outputs: Vec<OutputRecipe>,
    pub exclusions: Vec<Annotation>,
    pub duplications: Vec<Annotation>,
    pub conflicts: Vec<Conflict>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedInput {
    Source {
        source_id: String,
        sha256: String,
        content: String,
    },
    Revision {
        capability_id: String,
        revision_id: String,
        sha256: String,
        content: String,
    },
}

impl ResolvedInput {
    fn reference(&self) -> InputReference {
        match self {
            Self::Source {
                source_id, sha256, ..
            } => InputReference::Source {
                source_id: source_id.clone(),
                sha256: sha256.clone(),
            },
            Self::Revision {
                capability_id,
                revision_id,
                sha256,
                ..
            } => InputReference::Revision {
                capability_id: capability_id.clone(),
                revision_id: revision_id.clone(),
                sha256: sha256.clone(),
            },
        }
    }
    fn content(&self) -> &str {
        match self {
            Self::Source { content, .. } | Self::Revision { content, .. } => content,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposeError {
    Json(String),
    InvalidDraft(&'static str),
    InvalidInput(usize, &'static str),
    InvalidRange(InputRange),
    Limit(&'static str),
    IdentityMismatch(usize),
    DuplicateInput,
    DuplicateConflictId,
    DuplicateConflictRange,
    OverlappingAnnotations(&'static str),
    DuplicateAcknowledgmentMismatch,
}

impl std::fmt::Display for ComposeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ComposeError {}

/// Decode a closed wire shape after rejecting payload and duplicate-key attacks.
pub fn decode_draft_json(bytes: &[u8]) -> Result<Draft, ComposeError> {
    if bytes.len() > MAX_DRAFT_BYTES {
        return Err(ComposeError::Limit("draft_bytes"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ComposeError::Json("utf8".into()))?;
    reject_duplicate_keys(text)?;
    let draft = serde_json::from_str(text).map_err(|_| ComposeError::Json("schema".into()))?;
    validate_draft_shape(&draft)?;
    Ok(draft)
}

/// Pure, deterministic materialization over host-resolved immutable inputs.
pub fn preview(draft: &Draft, inputs: &[ResolvedInput]) -> Result<Preview, ComposeError> {
    validate_draft_shape(draft)?;
    validate_resolved_inputs(draft, inputs)?;
    let normalized = normalized_draft(draft)?;
    let composition_id = composition_id(&normalized)?;
    let contents: Vec<&str> = inputs.iter().map(ResolvedInput::content).collect();
    validate_ranges(&normalized, &contents)?;
    let mut diagnostics = Vec::new();
    let (outputs, mappings) =
        materialize(&normalized, &contents, &composition_id, &mut diagnostics)?;
    let coverage = coverage(&normalized, &contents, &mut diagnostics)?;
    for conflict in &normalized.conflicts {
        if conflict.resolution.is_none() {
            for range in &conflict.ranges {
                diagnostics.push(Diagnostic {
                    code: DiagnosticCode::UnresolvedConflict,
                    input_index: Some(range.input_index),
                    output_index: None,
                    range: Some(*range),
                    conflict_id: Some(conflict.id),
                });
            }
        }
    }
    let saveable = diagnostics.is_empty();
    Ok(Preview {
        schema_version: PREVIEW_SCHEMA.into(),
        composition_id,
        saveable,
        outputs,
        mappings,
        coverage,
        diagnostics,
        authority: Authority::None,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub schema_version: String,
    pub composition_id: String,
    pub saveable: bool,
    pub outputs: Vec<ProposedOutput>,
    pub mappings: Vec<PieceMapping>,
    pub coverage: Vec<CoverageSegment>,
    pub diagnostics: Vec<Diagnostic>,
    pub authority: Authority,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedOutput {
    pub output_index: u32,
    pub title: String,
    pub content: String,
    pub sha256: String,
    pub capability_id: String,
    pub revision_id: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MappingKind {
    Copy,
    Authored,
    Replace,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PieceMapping {
    pub output_index: u32,
    pub piece_index: u32,
    pub output_start_byte: u64,
    pub output_end_byte: u64,
    pub kind: MappingKind,
    pub input_range: Option<InputRange>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    Copied,
    Duplicated,
    Replaced,
    Excluded,
    Unassigned,
    Mixed,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageSegment {
    pub range: InputRange,
    pub disposition: Disposition,
    pub references: Vec<CoverageReference>,
    pub duplication_acknowledged: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageReference {
    pub output_index: u32,
    pub piece_index: u32,
    pub kind: MappingKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    InvalidOutput,
    Unassigned,
    MixedDisposition,
    UnacknowledgedDuplicate,
    UnresolvedConflict,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub input_index: Option<u32>,
    pub output_index: Option<u32>,
    pub range: Option<InputRange>,
    pub conflict_id: Option<u32>,
}

/// Identity accepts only a structurally valid closed draft and has no clock or I/O input.
pub fn composition_id(draft: &Draft) -> Result<String, ComposeError> {
    validate_draft_shape(draft)?;
    let bytes = identity_envelope_bytes(draft)?;
    let mut framed = b"rangoon.composition.v0\0".to_vec();
    framed.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    framed.extend_from_slice(&bytes);
    Ok(format!("composition:{}", byte_digest(&framed)))
}

/// Exact compact UTF-8 bytes hashed by [`composition_id`].
pub fn identity_envelope_bytes(draft: &Draft) -> Result<Vec<u8>, ComposeError> {
    validate_draft_shape(draft)?;
    let normalized = normalized_draft(draft)?;
    serde_json::to_vec(&IdentityEnvelope {
        transformation_version: TRANSFORMATION_VERSION,
        draft: &normalized,
    })
    .map_err(|_| ComposeError::InvalidDraft("encode"))
}

pub fn composed_capability_id(composition_id: &str, output_index: u32) -> String {
    let mut framed = b"rangoon.composed-capability.v0\0".to_vec();
    framed.extend_from_slice(&(composition_id.len() as u64).to_be_bytes());
    framed.extend_from_slice(composition_id.as_bytes());
    framed.extend_from_slice(&output_index.to_be_bytes());
    format!("capability:{}", byte_digest(&framed))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IdentityEnvelope<'a> {
    transformation_version: &'static str,
    draft: &'a Draft,
}

fn validate_draft_shape(draft: &Draft) -> Result<(), ComposeError> {
    if draft.schema_version != DRAFT_SCHEMA {
        return Err(ComposeError::InvalidDraft("schema_version"));
    }
    if draft.inputs.is_empty() || draft.inputs.len() > MAX_INPUTS {
        return Err(ComposeError::Limit("inputs"));
    }
    if draft.outputs.is_empty() || draft.outputs.len() > MAX_OUTPUTS {
        return Err(ComposeError::Limit("outputs"));
    }
    if draft.exclusions.len() > MAX_EXCLUSIONS
        || draft.duplications.len() > MAX_DUPLICATIONS
        || draft.conflicts.len() > MAX_CONFLICTS
    {
        return Err(ComposeError::Limit("annotations"));
    }
    let mut pieces = 0usize;
    for output in &draft.outputs {
        pieces = pieces
            .checked_add(output.pieces.len())
            .ok_or(ComposeError::Limit("pieces"))?;
        if pieces > MAX_PIECES {
            return Err(ComposeError::Limit("pieces"));
        }
    }
    if draft
        .conflicts
        .iter()
        .any(|conflict| conflict.ranges.len() < 2 || conflict.ranges.len() > MAX_INPUTS)
    {
        return Err(ComposeError::InvalidDraft("conflict"));
    }
    ensure_serialized_draft_bound(draft)?;
    match draft.operation {
        Operation::Decompose
            if draft.inputs.len() != 1
                || !matches!(draft.inputs[0], InputReference::Source { .. }) =>
        {
            return Err(ComposeError::InvalidDraft("decompose_inputs"));
        }
        Operation::Merge
            if draft.inputs.len() < 2
                || draft.outputs.len() != 1
                || draft
                    .inputs
                    .iter()
                    .any(|r| !matches!(r, InputReference::Revision { .. })) =>
        {
            return Err(ComposeError::InvalidDraft("merge_shape"));
        }
        Operation::Split
            if draft.inputs.len() != 1
                || draft.outputs.len() < 2
                || !matches!(draft.inputs[0], InputReference::Revision { .. }) =>
        {
            return Err(ComposeError::InvalidDraft("split_shape"));
        }
        _ => {}
    }
    let mut seen = HashSet::new();
    for r in &draft.inputs {
        if !valid_reference(r) {
            return Err(ComposeError::InvalidDraft("input_reference"));
        }
        if !seen.insert(input_key(r)) {
            return Err(ComposeError::DuplicateInput);
        }
    }
    let mut conflict_ids = HashSet::new();
    for conflict in &draft.conflicts {
        if conflict.id == 0 {
            return Err(ComposeError::InvalidDraft("conflict_id"));
        }
        if !conflict_ids.insert(conflict.id) {
            return Err(ComposeError::DuplicateConflictId);
        }
        if !valid_title(&conflict.title)
            || !valid_explanation(&conflict.context)
            || conflict.ranges.len() < 2
            || conflict.ranges.len() > MAX_INPUTS
            || conflict
                .resolution
                .as_deref()
                .is_some_and(|x| !valid_explanation(x))
        {
            return Err(ComposeError::InvalidDraft("conflict"));
        }
        let mut unique_ranges = BTreeSet::new();
        for range in &conflict.ranges {
            if !unique_ranges.insert(*range) {
                return Err(ComposeError::DuplicateConflictRange);
            }
        }
    }
    for output in &draft.outputs {
        for piece in &output.pieces {
            match piece {
                Piece::Authored { content, reason }
                | Piece::Replace {
                    content, reason, ..
                } if content.len() > MAX_CONTENT_BYTES
                    || content.contains('\0')
                    || !valid_explanation(reason) =>
                {
                    return Err(ComposeError::InvalidDraft("piece"));
                }
                Piece::Copy { .. } | Piece::Authored { .. } | Piece::Replace { .. } => {}
            }
        }
    }
    for a in draft.exclusions.iter().chain(&draft.duplications) {
        if !valid_explanation(&a.reason) {
            return Err(ComposeError::InvalidDraft("annotation"));
        }
    }
    no_overlap(&draft.exclusions, "exclusions")?;
    no_overlap(&draft.duplications, "duplications")?;
    Ok(())
}

fn input_key(reference: &InputReference) -> String {
    match reference {
        InputReference::Source { source_id, .. } => format!("source\0{source_id}"),
        InputReference::Revision {
            capability_id,
            revision_id,
            ..
        } => format!("revision\0{capability_id}\0{revision_id}"),
    }
}

fn ensure_serialized_draft_bound(draft: &Draft) -> Result<(), ComposeError> {
    let mut writer = CappedWriter {
        len: 0,
        exceeded: false,
    };
    if serde_json::to_writer(&mut writer, draft).is_err() && writer.exceeded {
        return Err(ComposeError::Limit("draft_bytes"));
    }
    Ok(())
}

struct CappedWriter {
    len: usize,
    exceeded: bool,
}
impl std::io::Write for CappedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let Some(next) = self.len.checked_add(bytes.len()) else {
            self.exceeded = true;
            return Err(std::io::Error::other("draft limit"));
        };
        if next > MAX_DRAFT_BYTES {
            self.exceeded = true;
            return Err(std::io::Error::other("draft limit"));
        }
        self.len = next;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn valid_explanation(value: &str) -> bool {
    !value.trim().is_empty()
        && value.trim() == value
        && value.len() <= MAX_EXPLANATION_BYTES
        && !value.contains('\0')
}
fn valid_reference(reference: &InputReference) -> bool {
    match reference {
        InputReference::Source { source_id, sha256 } => {
            valid_hash_id(source_id, "source:") && valid_hash(sha256)
        }
        InputReference::Revision {
            capability_id,
            revision_id,
            sha256,
        } => {
            valid_hash_id(capability_id, "capability:")
                && valid_hash_id(revision_id, "revision:")
                && valid_hash(sha256)
        }
    }
}
fn valid_hash_id(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(valid_hash)
}
fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn validate_resolved_inputs(draft: &Draft, inputs: &[ResolvedInput]) -> Result<(), ComposeError> {
    if inputs.len() != draft.inputs.len() {
        return Err(ComposeError::InvalidDraft("resolved_inputs"));
    }
    let mut total = 0usize;
    for actual in inputs {
        if actual.content().len() > MAX_CONTENT_BYTES {
            return Err(ComposeError::Limit("input_content_bytes"));
        }
        total = total
            .checked_add(actual.content().len())
            .ok_or(ComposeError::Limit("input_bytes"))?;
    }
    if total > MAX_TOTAL_BYTES {
        return Err(ComposeError::Limit("input_bytes"));
    }
    for (index, (expected, actual)) in draft.inputs.iter().zip(inputs).enumerate() {
        if expected != &actual.reference()
            || byte_digest(actual.content().as_bytes()) != expected_sha(expected)
        {
            return Err(ComposeError::IdentityMismatch(index));
        }
    }
    Ok(())
}
fn expected_sha(input: &InputReference) -> &str {
    match input {
        InputReference::Source { sha256, .. } | InputReference::Revision { sha256, .. } => sha256,
    }
}

fn validate_ranges(draft: &Draft, contents: &[&str]) -> Result<(), ComposeError> {
    let mut all_conflict_ranges = BTreeSet::new();
    let check = |range: InputRange| -> Result<(), ComposeError> {
        let Some(content) = contents.get(range.input_index as usize) else {
            return Err(ComposeError::InvalidRange(range));
        };
        if range.start_byte >= range.end_byte || range.end_byte > content.len() as u64 {
            return Err(ComposeError::InvalidRange(range));
        }
        let (start, end) = (range.start_byte as usize, range.end_byte as usize);
        if !content.is_char_boundary(start) || !content.is_char_boundary(end) {
            return Err(ComposeError::InvalidRange(range));
        }
        Ok(())
    };
    for output in &draft.outputs {
        for piece in &output.pieces {
            if let Piece::Copy { range } | Piece::Replace { range, .. } = piece {
                check(*range)?;
            }
        }
    }
    for a in draft.exclusions.iter().chain(&draft.duplications) {
        check(a.range)?;
    }
    for conflict in &draft.conflicts {
        for range in &conflict.ranges {
            check(*range)?;
            if !all_conflict_ranges.insert((conflict.id, *range)) {
                return Err(ComposeError::DuplicateConflictRange);
            }
        }
    }
    let mut replacements = Vec::new();
    for output in &draft.outputs {
        for piece in &output.pieces {
            if let Piece::Replace { range, .. } = piece {
                replacements.push(*range);
            }
        }
    }
    no_overlapping_ranges(&replacements, "replacements")?;
    no_overlap(&draft.exclusions, "exclusions")?;
    no_overlap(&draft.duplications, "duplications")?;
    let mut ids = HashSet::new();
    for conflict in &draft.conflicts {
        if !ids.insert(conflict.id) {
            return Err(ComposeError::DuplicateConflictId);
        }
        let mut ranges = BTreeSet::new();
        for range in &conflict.ranges {
            if !ranges.insert(*range) {
                return Err(ComposeError::DuplicateConflictRange);
            }
        }
    }
    Ok(())
}
fn no_overlapping_ranges(ranges: &[InputRange], name: &'static str) -> Result<(), ComposeError> {
    for (index, left) in ranges.iter().enumerate() {
        for right in &ranges[index + 1..] {
            if left.input_index == right.input_index
                && left.start_byte < right.end_byte
                && right.start_byte < left.end_byte
            {
                return Err(ComposeError::OverlappingAnnotations(name));
            }
        }
    }
    Ok(())
}
fn no_overlap(items: &[Annotation], name: &'static str) -> Result<(), ComposeError> {
    let ranges: Vec<_> = items.iter().map(|item| item.range).collect();
    no_overlapping_ranges(&ranges, name)
}

fn normalized_draft(draft: &Draft) -> Result<Draft, ComposeError> {
    let mut n = draft.clone();
    n.exclusions.sort_by(annotation_order);
    n.duplications.sort_by(annotation_order);
    for conflict in &mut n.conflicts {
        conflict.ranges.sort();
    }
    n.conflicts.sort_by_key(|c| c.id);
    Ok(n)
}
fn annotation_order(a: &Annotation, b: &Annotation) -> std::cmp::Ordering {
    (
        a.range.input_index,
        a.range.start_byte,
        a.range.end_byte,
        a.reason.as_bytes(),
    )
        .cmp(&(
            b.range.input_index,
            b.range.start_byte,
            b.range.end_byte,
            b.reason.as_bytes(),
        ))
}

fn materialize(
    draft: &Draft,
    contents: &[&str],
    composition_id: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(Vec<ProposedOutput>, Vec<PieceMapping>), ComposeError> {
    let output_lengths = preflight_output_lengths(draft)?;
    let mut outputs = Vec::with_capacity(draft.outputs.len());
    let mut mappings = Vec::new();
    for (output_index, recipe) in draft.outputs.iter().enumerate() {
        let mut content = String::with_capacity(output_lengths[output_index]);
        for (piece_index, piece) in recipe.pieces.iter().enumerate() {
            let start = content.len() as u64;
            let (text, kind, input_range) = match piece {
                Piece::Copy { range } => (
                    &contents[range.input_index as usize]
                        [range.start_byte as usize..range.end_byte as usize],
                    MappingKind::Copy,
                    Some(*range),
                ),
                Piece::Authored { content, .. } => (content.as_str(), MappingKind::Authored, None),
                Piece::Replace { range, content, .. } => {
                    (content.as_str(), MappingKind::Replace, Some(*range))
                }
            };
            content.push_str(text);
            mappings.push(PieceMapping {
                output_index: output_index as u32,
                piece_index: piece_index as u32,
                output_start_byte: start,
                output_end_byte: content.len() as u64,
                kind,
                input_range,
            });
        }
        if !valid_title(&recipe.title) || !valid_content(&content) {
            diagnostics.push(Diagnostic {
                code: DiagnosticCode::InvalidOutput,
                input_index: None,
                output_index: Some(output_index as u32),
                range: None,
                conflict_id: None,
            });
        }
        let capability = composed_capability_id(composition_id, output_index as u32);
        outputs.push(ProposedOutput {
            output_index: output_index as u32,
            title: recipe.title.clone(),
            sha256: byte_digest(content.as_bytes()),
            revision_id: revision_id(&capability, None, &recipe.title, &content),
            capability_id: capability,
            content,
        });
    }
    Ok((outputs, mappings))
}

fn preflight_output_lengths(draft: &Draft) -> Result<Vec<usize>, ComposeError> {
    let mut total = 0usize;
    let mut lengths = Vec::with_capacity(draft.outputs.len());
    for output in &draft.outputs {
        let mut length = 0usize;
        for piece in &output.pieces {
            let part = match piece {
                Piece::Copy { range } => (range.end_byte - range.start_byte) as usize,
                Piece::Authored { content, .. } | Piece::Replace { content, .. } => content.len(),
            };
            length = length
                .checked_add(part)
                .ok_or(ComposeError::Limit("output_bytes"))?;
            if length > MAX_CONTENT_BYTES {
                return Err(ComposeError::Limit("output_content_bytes"));
            }
        }
        total = total
            .checked_add(length)
            .ok_or(ComposeError::Limit("output_bytes"))?;
        if total > MAX_TOTAL_BYTES {
            return Err(ComposeError::Limit("output_bytes"));
        }
        lengths.push(length);
    }
    Ok(lengths)
}

fn coverage(
    draft: &Draft,
    contents: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Vec<CoverageSegment>, ComposeError> {
    let mut result = Vec::new();
    for (input_index, content) in contents.iter().enumerate() {
        if content.is_empty() {
            continue;
        }
        let mut endpoints = BTreeSet::from([0u64, content.len() as u64]);
        let mut pieces = Vec::new();
        for (oi, output) in draft.outputs.iter().enumerate() {
            for (pi, piece) in output.pieces.iter().enumerate() {
                if let Piece::Copy { range } | Piece::Replace { range, .. } = piece {
                    if range.input_index == input_index as u32 {
                        endpoints.insert(range.start_byte);
                        endpoints.insert(range.end_byte);
                        pieces.push((
                            range,
                            oi as u32,
                            pi as u32,
                            matches!(piece, Piece::Copy { .. }),
                        ));
                    }
                }
            }
        }
        for annotation in draft.exclusions.iter().chain(&draft.duplications) {
            if annotation.range.input_index == input_index as u32 {
                endpoints.insert(annotation.range.start_byte);
                endpoints.insert(annotation.range.end_byte);
            }
        }
        for conflict in &draft.conflicts {
            for range in &conflict.ranges {
                if range.input_index == input_index as u32 {
                    endpoints.insert(range.start_byte);
                    endpoints.insert(range.end_byte);
                }
            }
        }
        let points: Vec<u64> = endpoints.into_iter().collect();
        for pair in points.windows(2) {
            if result.len() >= MAX_SEGMENTS {
                return Err(ComposeError::Limit("coverage_segments"));
            }
            let range = InputRange {
                input_index: input_index as u32,
                start_byte: pair[0],
                end_byte: pair[1],
            };
            let refs: Vec<_> = pieces
                .iter()
                .filter(|(r, _, _, _)| {
                    r.start_byte <= range.start_byte && r.end_byte >= range.end_byte
                })
                .map(|(_, oi, pi, copy)| CoverageReference {
                    output_index: *oi,
                    piece_index: *pi,
                    kind: if *copy {
                        MappingKind::Copy
                    } else {
                        MappingKind::Replace
                    },
                })
                .collect();
            let copies = refs.iter().filter(|r| r.kind == MappingKind::Copy).count();
            let replaced = refs.iter().any(|r| r.kind == MappingKind::Replace);
            let excluded = draft.exclusions.iter().any(|a| covers(a.range, range));
            let acknowledged = draft.duplications.iter().any(|a| covers(a.range, range));
            let disposition = if (copies > 0 && (replaced || excluded)) || (replaced && excluded) {
                Disposition::Mixed
            } else if copies > 1 {
                Disposition::Duplicated
            } else if copies == 1 {
                Disposition::Copied
            } else if replaced {
                Disposition::Replaced
            } else if excluded {
                Disposition::Excluded
            } else {
                Disposition::Unassigned
            };
            match disposition {
                Disposition::Unassigned => diagnostics.push(Diagnostic {
                    code: DiagnosticCode::Unassigned,
                    input_index: Some(range.input_index),
                    output_index: None,
                    range: Some(range),
                    conflict_id: None,
                }),
                Disposition::Mixed => diagnostics.push(Diagnostic {
                    code: DiagnosticCode::MixedDisposition,
                    input_index: Some(range.input_index),
                    output_index: None,
                    range: Some(range),
                    conflict_id: None,
                }),
                Disposition::Duplicated if !acknowledged => diagnostics.push(Diagnostic {
                    code: DiagnosticCode::UnacknowledgedDuplicate,
                    input_index: Some(range.input_index),
                    output_index: None,
                    range: Some(range),
                    conflict_id: None,
                }),
                _ => {}
            }
            result.push(CoverageSegment {
                range,
                disposition,
                references: refs,
                duplication_acknowledged: acknowledged,
            });
        }
    }
    for acknowledgment in &draft.duplications {
        if !acknowledgment_covers_only_duplicates(acknowledgment, &result) {
            return Err(ComposeError::DuplicateAcknowledgmentMismatch);
        }
    }
    Ok(result)
}
fn acknowledgment_covers_only_duplicates(
    acknowledgment: &Annotation,
    coverage: &[CoverageSegment],
) -> bool {
    let mut cursor = acknowledgment.range.start_byte;
    for segment in coverage.iter().filter(|segment| {
        segment.range.input_index == acknowledgment.range.input_index
            && segment.range.start_byte >= acknowledgment.range.start_byte
            && segment.range.end_byte <= acknowledgment.range.end_byte
    }) {
        if segment.range.start_byte != cursor || segment.disposition != Disposition::Duplicated {
            return false;
        }
        cursor = segment.range.end_byte;
    }
    cursor == acknowledgment.range.end_byte
}
fn covers(outer: InputRange, inner: InputRange) -> bool {
    outer.input_index == inner.input_index
        && outer.start_byte <= inner.start_byte
        && outer.end_byte >= inner.end_byte
}

/// A small JSON scanner used only to reject duplicate object members before Serde loses them.
fn reject_duplicate_keys(input: &str) -> Result<(), ComposeError> {
    let mut p = JsonParser {
        bytes: input.as_bytes(),
        at: 0,
        depth: 0,
        piece_count: 0,
    };
    p.value()?;
    p.ws();
    if p.at != p.bytes.len() {
        return Err(ComposeError::Json("trailing".into()));
    }
    Ok(())
}
struct JsonParser<'a> {
    bytes: &'a [u8],
    at: usize,
    depth: usize,
    piece_count: usize,
}
impl<'a> JsonParser<'a> {
    fn ws(&mut self) {
        while self
            .bytes
            .get(self.at)
            .is_some_and(|b| b.is_ascii_whitespace())
        {
            self.at += 1;
        }
    }
    fn value(&mut self) -> Result<(), ComposeError> {
        self.ws();
        self.depth += 1;
        if self.depth > MAX_JSON_DEPTH {
            return Err(ComposeError::Limit("json_depth"));
        }
        let result = match self.bytes.get(self.at) {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'\"') => self.string().map(|_| ()),
            Some(b'-' | b'0'..=b'9') => self.atom(),
            Some(b't') | Some(b'f') | Some(b'n') => self.atom(),
            _ => Err(ComposeError::Json("token".into())),
        };
        self.depth -= 1;
        result
    }
    fn object(&mut self) -> Result<(), ComposeError> {
        self.at += 1;
        self.ws();
        let mut names = HashSet::new();
        let mut count = 0usize;
        if self.bytes.get(self.at) == Some(&b'}') {
            self.at += 1;
            return Ok(());
        }
        loop {
            count += 1;
            if count > MAX_JSON_ARRAY_ITEMS {
                return Err(ComposeError::Limit("json_members"));
            }
            self.ws();
            let name = self.string()?;
            if !names.insert(name.clone()) {
                return Err(ComposeError::Json("duplicate_key".into()));
            }
            self.ws();
            if self.take(b':').is_err() {
                return Err(ComposeError::Json("colon".into()));
            }
            match name.as_str() {
                "inputs" => self.array_limited(MAX_INPUTS, "inputs", false)?,
                "outputs" => self.array_limited(MAX_OUTPUTS, "outputs", false)?,
                "exclusions" => self.array_limited(MAX_EXCLUSIONS, "exclusions", false)?,
                "duplications" => self.array_limited(MAX_DUPLICATIONS, "duplications", false)?,
                "conflicts" => self.array_limited(MAX_CONFLICTS, "conflicts", false)?,
                "ranges" => self.array_limited(MAX_INPUTS, "conflict_ranges", false)?,
                "pieces" => self.array_limited(MAX_PIECES, "pieces", true)?,
                _ => self.value()?,
            }
            self.ws();
            if self.take(b'}').is_ok() {
                return Ok(());
            }
            if self.take(b',').is_err() {
                return Err(ComposeError::Json("object".into()));
            }
        }
    }
    fn array(&mut self) -> Result<(), ComposeError> {
        self.array_limited(MAX_JSON_ARRAY_ITEMS, "json_array", false)
    }
    fn array_limited(
        &mut self,
        limit: usize,
        name: &'static str,
        aggregate_pieces: bool,
    ) -> Result<(), ComposeError> {
        self.ws();
        if self.bytes.get(self.at) != Some(&b'[') {
            return Err(ComposeError::Json("array".into()));
        }
        self.at += 1;
        self.ws();
        let mut count = 0usize;
        if self.bytes.get(self.at) == Some(&b']') {
            self.at += 1;
            return Ok(());
        }
        loop {
            count += 1;
            if count > limit {
                return Err(ComposeError::Limit(name));
            }
            if aggregate_pieces {
                self.piece_count += 1;
                if self.piece_count > MAX_PIECES {
                    return Err(ComposeError::Limit("pieces"));
                }
            }
            self.value()?;
            self.ws();
            if self.take(b']').is_ok() {
                return Ok(());
            }
            if self.take(b',').is_err() {
                return Err(ComposeError::Json("array".into()));
            }
        }
    }
    fn string(&mut self) -> Result<String, ComposeError> {
        if self.take(b'\"').is_err() {
            return Err(ComposeError::Json("string".into()));
        }
        let start = self.at;
        let mut escaped = false;
        while let Some(&b) = self.bytes.get(self.at) {
            self.at += 1;
            if escaped {
                escaped = false;
                continue;
            }
            if b == b'\\' {
                escaped = true;
                continue;
            }
            if b == b'\"' {
                let raw = std::str::from_utf8(&self.bytes[start - 1..self.at])
                    .map_err(|_| ComposeError::Json("utf8".into()))?;
                return serde_json::from_str(raw).map_err(|_| ComposeError::Json("string".into()));
            }
            if b < 0x20 {
                return Err(ComposeError::Json("control".into()));
            }
        }
        Err(ComposeError::Json("eof".into()))
    }
    fn atom(&mut self) -> Result<(), ComposeError> {
        let start = self.at;
        while self
            .bytes
            .get(self.at)
            .is_some_and(|b| !b.is_ascii_whitespace() && !matches!(b, b',' | b']' | b'}'))
        {
            self.at += 1;
        }
        serde_json::from_slice::<serde_json::Value>(&self.bytes[start..self.at])
            .map(|_| ())
            .map_err(|_| ComposeError::Json("atom".into()))
    }
    fn take(&mut self, byte: u8) -> Result<(), ()> {
        self.ws();
        if self.bytes.get(self.at) == Some(&byte) {
            self.at += 1;
            Ok(())
        } else {
            Err(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn hash(text: &str) -> String {
        byte_digest(text.as_bytes())
    }
    fn source(content: &str) -> (InputReference, ResolvedInput) {
        let source_id = format!("source:{}", "a".repeat(64));
        let sha256 = hash(content);
        (
            InputReference::Source {
                source_id: source_id.clone(),
                sha256: sha256.clone(),
            },
            ResolvedInput::Source {
                source_id,
                sha256,
                content: content.into(),
            },
        )
    }
    fn range(start_byte: u64, end_byte: u64) -> InputRange {
        InputRange {
            input_index: 0,
            start_byte,
            end_byte,
        }
    }
    fn decompose(content: &str, outputs: Vec<OutputRecipe>) -> (Draft, Vec<ResolvedInput>) {
        let (reference, resolved) = source(content);
        (
            Draft {
                schema_version: DRAFT_SCHEMA.into(),
                operation: Operation::Decompose,
                inputs: vec![reference],
                outputs,
                exclusions: vec![],
                duplications: vec![],
                conflicts: vec![],
            },
            vec![resolved],
        )
    }
    fn copy(title: &str, start_byte: u64, end_byte: u64) -> OutputRecipe {
        OutputRecipe {
            title: title.into(),
            pieces: vec![Piece::Copy {
                range: range(start_byte, end_byte),
            }],
        }
    }

    #[test]
    fn independent_golden_fixture_pins_wire_ids_and_materialization() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../fixtures/composition/decompose-v0.json"
        ))
        .unwrap();
        let draft = decode_draft_json(serde_json::to_string(&fixture["draft"]).unwrap().as_bytes())
            .unwrap();
        let pretty = serde_json::to_string_pretty(&fixture["draft"]).unwrap();
        assert_eq!(decode_draft_json(pretty.as_bytes()).unwrap(), draft);
        let mixed_whitespace = pretty.replace(": [", ": \r\n\t[");
        assert_eq!(
            decode_draft_json(mixed_whitespace.as_bytes()).unwrap(),
            draft
        );
        let reference = &fixture["resolvedInputs"][0]["reference"];
        let resolved = ResolvedInput::Source {
            source_id: reference["sourceId"].as_str().unwrap().into(),
            sha256: reference["sha256"].as_str().unwrap().into(),
            content: fixture["resolvedInputs"][0]["content"]
                .as_str()
                .unwrap()
                .into(),
        };
        assert_eq!(
            identity_envelope_bytes(&draft).unwrap(),
            fixture["expectedEnvelope"].as_str().unwrap().as_bytes()
        );
        assert_eq!(
            identity_envelope_bytes(&draft).unwrap().len() as u64,
            fixture["expectedEnvelopeByteLength"].as_u64().unwrap()
        );
        let result = preview(&draft, &[resolved]).unwrap();
        assert!(result.saveable);
        assert_eq!(
            result.composition_id,
            fixture["expectedCompositionId"].as_str().unwrap()
        );
        for (actual, expected) in result
            .outputs
            .iter()
            .zip(fixture["expectedOutputs"].as_array().unwrap())
        {
            assert_eq!(actual.title, expected["title"].as_str().unwrap());
            assert_eq!(actual.content, expected["content"].as_str().unwrap());
            assert_eq!(actual.sha256, expected["sha256"].as_str().unwrap());
            assert_eq!(
                actual.capability_id,
                expected["capabilityId"].as_str().unwrap()
            );
            assert_eq!(actual.revision_id, expected["revisionId"].as_str().unwrap());
        }
    }

    #[test]
    fn exact_unicode_bom_and_crlf_offsets_materialize_unchanged() {
        let content = "\u{feff}é\r\n終\n";
        let (draft, inputs) = decompose(content, vec![copy("Exact", 0, content.len() as u64)]);
        let result = preview(&draft, &inputs).unwrap();
        assert!(result.saveable);
        assert_eq!(result.outputs[0].content, content);
        let mut bad = draft.clone();
        bad.outputs[0].pieces = vec![Piece::Copy {
            range: range(1, content.len() as u64),
        }];
        assert!(matches!(
            preview(&bad, &inputs),
            Err(ComposeError::InvalidRange(_))
        ));
    }

    #[test]
    fn complete_coverage_handles_copy_duplicate_replace_and_exclusion() {
        let (mut draft, inputs) = decompose(
            "abcdef",
            vec![OutputRecipe {
                title: "Result".into(),
                pieces: vec![
                    Piece::Copy { range: range(0, 1) },
                    Piece::Copy { range: range(1, 2) },
                    Piece::Copy { range: range(1, 2) },
                    Piece::Replace {
                        range: range(2, 3),
                        content: "X".into(),
                        reason: "clarify".into(),
                    },
                ],
            }],
        );
        draft.duplications.push(Annotation {
            range: range(1, 2),
            reason: "shared".into(),
        });
        draft.exclusions.push(Annotation {
            range: range(3, 6),
            reason: "not needed".into(),
        });
        let result = preview(&draft, &inputs).unwrap();
        assert!(result.saveable);
        assert_eq!(result.outputs[0].content, "abbX");
        assert_eq!(
            result
                .coverage
                .iter()
                .map(|x| x.disposition)
                .collect::<Vec<_>>(),
            vec![
                Disposition::Copied,
                Disposition::Duplicated,
                Disposition::Replaced,
                Disposition::Excluded
            ]
        );
        assert!(result.coverage[1].duplication_acknowledged);
    }

    #[test]
    fn incomplete_mixed_and_unacknowledged_coverage_block_save() {
        let (draft, inputs) = decompose("abc", vec![copy("Result", 0, 1)]);
        let result = preview(&draft, &inputs).unwrap();
        assert!(!result.saveable);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::Unassigned)
        );
        let (draft, inputs) = decompose(
            "abc",
            vec![OutputRecipe {
                title: "Result".into(),
                pieces: vec![
                    Piece::Copy { range: range(0, 1) },
                    Piece::Replace {
                        range: range(0, 1),
                        content: "X".into(),
                        reason: "why".into(),
                    },
                    Piece::Copy { range: range(1, 3) },
                ],
            }],
        );
        let result = preview(&draft, &inputs).unwrap();
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::MixedDisposition)
        );
        let (draft, inputs) = decompose(
            "abc",
            vec![OutputRecipe {
                title: "Result".into(),
                pieces: vec![
                    Piece::Copy { range: range(0, 1) },
                    Piece::Copy { range: range(0, 1) },
                    Piece::Copy { range: range(1, 3) },
                ],
            }],
        );
        let result = preview(&draft, &inputs).unwrap();
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::UnacknowledgedDuplicate)
        );
    }

    #[test]
    fn unresolved_conflict_is_diagnostic_with_exact_ranges() {
        let (mut draft, inputs) = decompose("abc", vec![copy("Result", 0, 3)]);
        draft.conflicts.push(Conflict {
            id: 7,
            title: "Order".into(),
            ranges: vec![range(0, 1), range(1, 3)],
            context: "Instructions disagree".into(),
            resolution: None,
        });
        let result = preview(&draft, &inputs).unwrap();
        assert!(!result.saveable);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::UnresolvedConflict && d.conflict_id == Some(7))
        );
        let unresolved: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagnosticCode::UnresolvedConflict)
            .collect();
        assert_eq!(unresolved.len(), 2);
        assert_eq!(unresolved[0].input_index, Some(0));
        assert_eq!(unresolved[0].range, Some(range(0, 1)));
        assert_eq!(unresolved[1].input_index, Some(0));
        assert_eq!(unresolved[1].range, Some(range(1, 3)));
        draft.conflicts[0].resolution = Some("Use first rule".into());
        assert!(preview(&draft, &inputs).unwrap().saveable);
    }

    #[test]
    fn ordered_input_identity_and_resolved_content_are_bound() {
        let one = "one";
        let two = "two";
        let c1 = format!("capability:{}", "1".repeat(64));
        let c2 = format!("capability:{}", "2".repeat(64));
        let r1 = format!("revision:{}", "3".repeat(64));
        let r2 = format!("revision:{}", "4".repeat(64));
        let i1 = InputReference::Revision {
            capability_id: c1.clone(),
            revision_id: r1.clone(),
            sha256: hash(one),
        };
        let i2 = InputReference::Revision {
            capability_id: c2.clone(),
            revision_id: r2.clone(),
            sha256: hash(two),
        };
        let output = OutputRecipe {
            title: "Both".into(),
            pieces: vec![
                Piece::Copy { range: range(0, 3) },
                Piece::Authored {
                    content: " ".into(),
                    reason: "separator".into(),
                },
                Piece::Copy {
                    range: InputRange {
                        input_index: 1,
                        start_byte: 0,
                        end_byte: 3,
                    },
                },
            ],
        };
        let draft = Draft {
            schema_version: DRAFT_SCHEMA.into(),
            operation: Operation::Merge,
            inputs: vec![i1.clone(), i2.clone()],
            outputs: vec![output],
            exclusions: vec![],
            duplications: vec![],
            conflicts: vec![],
        };
        let inputs = vec![
            ResolvedInput::Revision {
                capability_id: c1,
                revision_id: r1,
                sha256: hash(one),
                content: one.into(),
            },
            ResolvedInput::Revision {
                capability_id: c2,
                revision_id: r2,
                sha256: hash(two),
                content: two.into(),
            },
        ];
        let id = composition_id(&draft).unwrap();
        let mut reversed = draft.clone();
        reversed.inputs.swap(0, 1);
        assert_ne!(id, composition_id(&reversed).unwrap());
        let mut duplicate = draft.clone();
        duplicate.inputs[1] = InputReference::Revision {
            capability_id: match &duplicate.inputs[0] {
                InputReference::Revision { capability_id, .. } => capability_id.clone(),
                InputReference::Source { .. } => unreachable!(),
            },
            revision_id: match &duplicate.inputs[0] {
                InputReference::Revision { revision_id, .. } => revision_id.clone(),
                InputReference::Source { .. } => unreachable!(),
            },
            sha256: "f".repeat(64),
        };
        assert!(matches!(
            composition_id(&duplicate),
            Err(ComposeError::DuplicateInput)
        ));
        let mut changed = inputs.clone();
        if let ResolvedInput::Revision { content, .. } = &mut changed[0] {
            *content = "ONE".into();
        }
        assert!(matches!(
            preview(&draft, &changed),
            Err(ComposeError::IdentityMismatch(0))
        ));
    }

    #[test]
    fn decoder_rejects_closed_wire_failures_and_resource_limits() {
        let good = r#"{"schemaVersion":"rangoon.composition-draft.v0","operation":"decompose","inputs":[{"kind":"source","sourceId":"source:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}],"outputs":[{"title":"Result","pieces":[{"kind":"copy","range":{"inputIndex":0,"startByte":0,"endByte":1}}]}],"exclusions":[],"duplications":[],"conflicts":[]}"#;
        assert!(decode_draft_json(good.as_bytes()).is_ok());
        assert!(matches!(
            decode_draft_json(
                format!("{}{}", &good[..good.len() - 1], ",\"extra\":true}").as_bytes()
            ),
            Err(ComposeError::Json(_))
        ));
        let duplicate = good.replacen(
            "\"operation\":\"decompose\"",
            "\"operation\":\"decompose\",\"operation\":\"split\"",
            1,
        );
        assert!(matches!(
            decode_draft_json(duplicate.as_bytes()),
            Err(ComposeError::Json(_))
        ));
        let missing_nullable = good.replace(
            "\"conflicts\":[]",
            "\"conflicts\":[{\"id\":1,\"title\":\"A\",\"ranges\":[{\"inputIndex\":0,\"startByte\":0,\"endByte\":1},{\"inputIndex\":0,\"startByte\":1,\"endByte\":2}],\"context\":\"x\"}]",
        );
        assert!(matches!(
            decode_draft_json(missing_nullable.as_bytes()),
            Err(ComposeError::Json(_))
        ));
        let huge = vec![b' '; MAX_DRAFT_BYTES + 1];
        assert!(matches!(
            decode_draft_json(&huge),
            Err(ComposeError::Limit("draft_bytes"))
        ));
        let wrong_schema = good.replace(DRAFT_SCHEMA, "rangoon.composition-draft.v9");
        assert!(matches!(
            decode_draft_json(wrong_schema.as_bytes()),
            Err(ComposeError::InvalidDraft("schema_version"))
        ));
        let (mut bad, resolved) = decompose("a", vec![copy("Result", 0, 1)]);
        bad.outputs[0].pieces = vec![Piece::Copy {
            range: InputRange {
                input_index: 1,
                start_byte: 0,
                end_byte: 1,
            },
        }];
        assert!(matches!(
            preview(&bad, &resolved),
            Err(ComposeError::InvalidRange(_))
        ));
        let (mut direct, _) = decompose("a", vec![copy("Result", 0, 1)]);
        direct.outputs[0].pieces = (0..=MAX_PIECES)
            .map(|_| Piece::Authored {
                content: String::new(),
                reason: "editing".into(),
            })
            .collect();
        assert!(matches!(
            composition_id(&direct),
            Err(ComposeError::Limit("pieces"))
        ));
    }

    #[test]
    fn decoder_preflights_semantic_sequence_limits_before_serde_allocates() {
        let good: Value = serde_json::from_str(r#"{"schemaVersion":"rangoon.composition-draft.v0","operation":"decompose","inputs":[{"kind":"source","sourceId":"source:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}],"outputs":[{"title":"Result","pieces":[]}],"exclusions":[],"duplications":[],"conflicts":[]}"#).unwrap();
        let encoded = |value: &Value| serde_json::to_vec(value).unwrap();
        let mut max_outputs = good.clone();
        max_outputs["outputs"] = Value::Array(vec![good["outputs"][0].clone(); MAX_OUTPUTS]);
        assert!(decode_draft_json(&encoded(&max_outputs)).is_ok());

        let mut too_many_inputs = good.clone();
        too_many_inputs["inputs"] = Value::Array(vec![good["inputs"][0].clone(); MAX_INPUTS + 1]);
        assert!(matches!(
            decode_draft_json(&encoded(&too_many_inputs)),
            Err(ComposeError::Limit("inputs"))
        ));
        let mut too_many_outputs = good.clone();
        too_many_outputs["outputs"] =
            Value::Array(vec![good["outputs"][0].clone(); MAX_OUTPUTS + 1]);
        assert!(matches!(
            decode_draft_json(&encoded(&too_many_outputs)),
            Err(ComposeError::Limit("outputs"))
        ));
        for (field, limit) in [
            ("exclusions", MAX_EXCLUSIONS),
            ("duplications", MAX_DUPLICATIONS),
            ("conflicts", MAX_CONFLICTS),
        ] {
            let mut too_many = good.clone();
            too_many[field] = Value::Array(vec![Value::Null; limit + 1]);
            assert!(matches!(
                decode_draft_json(&encoded(&too_many)),
                Err(ComposeError::Limit(_))
            ));
        }
        let mut too_many_ranges = good.clone();
        too_many_ranges["conflicts"] = serde_json::json!([{"id":1,"title":"A","ranges": [null, null, null, null, null, null, null, null, null, null, null, null, null, null, null, null, null],"context":"x","resolution":null}]);
        assert!(matches!(
            decode_draft_json(&encoded(&too_many_ranges)),
            Err(ComposeError::Limit("conflict_ranges"))
        ));
        let mut too_many_pieces = good.clone();
        too_many_pieces["outputs"] = Value::Array(
            (0..MAX_OUTPUTS)
                .map(|_| serde_json::json!({"title":"R","pieces": vec![Value::Null; 17]}))
                .collect(),
        );
        assert!(matches!(
            decode_draft_json(&encoded(&too_many_pieces)),
            Err(ComposeError::Limit("pieces"))
        ));
    }

    #[test]
    fn resource_limits_are_checked_before_hashing_or_output_assembly() {
        let (mut excessive, _) = decompose("a", vec![copy("Result", 0, 1)]);
        excessive.outputs[0].title = "x".repeat(MAX_DRAFT_BYTES + 1);
        excessive.inputs = vec![excessive.inputs[0].clone(); MAX_INPUTS + 1];
        assert!(matches!(
            composition_id(&excessive),
            Err(ComposeError::Limit("inputs"))
        ));

        let (draft, _) = decompose("a", vec![copy("Result", 0, 1)]);
        let oversized_input = ResolvedInput::Source {
            source_id: format!("source:{}", "a".repeat(64)),
            sha256: "0".repeat(64),
            content: "x".repeat(MAX_CONTENT_BYTES + 1),
        };
        assert!(matches!(
            preview(&draft, &[oversized_input]),
            Err(ComposeError::Limit("input_content_bytes"))
        ));

        let (draft, inputs) = decompose(
            "a",
            vec![OutputRecipe {
                title: "Result".into(),
                pieces: vec![
                    Piece::Copy { range: range(0, 1) },
                    Piece::Authored {
                        content: "x".repeat(MAX_CONTENT_BYTES),
                        reason: "editing".into(),
                    },
                ],
            }],
        );
        assert!(matches!(
            preview(&draft, &inputs),
            Err(ComposeError::Limit("output_content_bytes"))
        ));

        let (mut draft, _) = decompose("a", vec![copy("Result", 0, 1)]);
        draft.outputs[0].pieces = vec![Piece::Authored {
            content: "x".repeat(MAX_DRAFT_BYTES + 1),
            reason: "editing".into(),
        }];
        assert!(matches!(
            composition_id(&draft),
            Err(ComposeError::Limit("draft_bytes"))
        ));
    }

    #[test]
    fn overlapping_replacements_are_fixed_errors() {
        let (draft, inputs) = decompose(
            "abcd",
            vec![OutputRecipe {
                title: "Result".into(),
                pieces: vec![
                    Piece::Replace {
                        range: range(0, 3),
                        content: "one".into(),
                        reason: "editing".into(),
                    },
                    Piece::Replace {
                        range: range(2, 4),
                        content: "two".into(),
                        reason: "editing".into(),
                    },
                ],
            }],
        );
        assert!(matches!(
            preview(&draft, &inputs),
            Err(ComposeError::OverlappingAnnotations("replacements"))
        ));
    }

    #[test]
    fn duplication_acknowledgment_accepts_a_conflict_split_region() {
        let (mut draft, inputs) = decompose(
            "abcd",
            vec![OutputRecipe {
                title: "Result".into(),
                pieces: vec![
                    Piece::Copy { range: range(0, 4) },
                    Piece::Copy { range: range(0, 4) },
                ],
            }],
        );
        draft.duplications.push(Annotation {
            range: range(0, 4),
            reason: "shared".into(),
        });
        draft.conflicts.push(Conflict {
            id: 1,
            title: "Order".into(),
            ranges: vec![range(0, 2), range(2, 4)],
            context: "Needs decision".into(),
            resolution: Some("Preserve both".into()),
        });
        let result = preview(&draft, &inputs).unwrap();
        assert!(result.saveable);
        assert_eq!(result.coverage.len(), 2);
        assert!(
            result
                .coverage
                .iter()
                .all(|segment| segment.duplication_acknowledged)
        );
    }

    #[test]
    fn invalid_output_is_editable_diagnostic_not_a_fake_success() {
        let (draft, inputs) = decompose(
            "abc",
            vec![OutputRecipe {
                title: "Result".into(),
                pieces: vec![Piece::Replace {
                    range: range(0, 3),
                    content: " ".into(),
                    reason: "editing".into(),
                }],
            }],
        );
        let result = preview(&draft, &inputs).unwrap();
        assert!(!result.saveable);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::InvalidOutput)
        );
        assert_eq!(result.authority, Authority::None);
    }
}
