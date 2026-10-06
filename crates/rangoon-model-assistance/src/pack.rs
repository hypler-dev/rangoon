use crate::{
    Diagnostic,
    json::{WireKind, preflight},
};
use rangoon_domain::{byte_digest, composition::InputReference};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::{self, Write},
};

const REQUEST_SCHEMA: &str = "rangoon.context-pack-request.v1";
const BODY_SCHEMA: &str = "rangoon.analysis-body.v1";
const PACK_SCHEMA: &str = "rangoon.context-pack.v1";
const RESPONSE_SCHEMA: &str = "rangoon.analysis-proposals.v1";
const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_INPUTS: usize = 16;
const MAX_INPUT_BYTES: usize = 256 * 1024;
const MAX_TOTAL_INPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_SELECTIONS: usize = 256;
const MAX_REQUIRED_PROTECTED_RANGES: usize = 256;
const MAX_OMITTED_RANGES: usize = 272;
const MAX_BODY_BYTES: u64 = 256 * 1024;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextPack {
    schema_version: String,
    pack_id: String,
    body_json: String,
    body_sha256: String,
    body_bytes: u64,
    selected_bytes: u64,
    unique_text_bytes: u64,
    omitted_bytes: u64,
    token_accounting: &'static str,
    authority: &'static str,
    #[serde(skip_serializing)]
    context: PackContext,
}

#[derive(Clone)]
pub(super) struct PackContext {
    pub(super) task: Task,
    pub(super) inputs: Vec<ResolvedContext>,
    pub(super) pack_id: String,
    target: Target,
}

#[derive(Clone)]
pub(super) struct ResolvedContext {
    pub(super) input: InputReference,
    pub(super) content: String,
    pub(super) selected_ranges: Vec<Range>,
}

impl ContextPack {
    /// Exact immutable provider-neutral body, before any transport wrapper.
    pub fn body_json(&self) -> &str {
        &self.body_json
    }

    pub fn pack_id(&self) -> &str {
        &self.pack_id
    }

    pub fn profile_id(&self) -> &str {
        &self.context.target.profile_id
    }

    pub fn profile_sha256(&self) -> &str {
        &self.context.target.profile_sha256
    }

    pub fn model(&self) -> &str {
        &self.context.target.model
    }

    pub fn max_output_tokens(&self) -> u64 {
        self.context.target.max_output_tokens
    }

    pub(super) fn context(&self) -> &PackContext {
        &self.context
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Task {
    ClassifyV1,
    DecomposeV1,
    CompareV1,
}

impl Task {
    pub(super) fn template(self) -> &'static str {
        match self {
            Self::ClassifyV1 => include_str!("../templates/classify_v1.txt"),
            Self::DecomposeV1 => include_str!("../templates/decompose_v1.txt"),
            Self::CompareV1 => include_str!("../templates/compare_v1.txt"),
        }
    }

    fn template_header(self) -> &'static str {
        match self {
            Self::ClassifyV1 => "Task: classify_v1",
            Self::DecomposeV1 => "Task: decompose_v1",
            Self::CompareV1 => "Task: compare_v1",
        }
    }

    fn template_is_exact(self, template: &str) -> bool {
        template
            .strip_prefix(self.template_header())
            .is_some_and(|remainder| remainder.starts_with('\n'))
            && template.ends_with('\n')
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    task: Task,
    target: Target,
    max_body_bytes: u64,
    inputs: Vec<ResolvedInput>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Target {
    profile_id: String,
    profile_sha256: String,
    model: String,
    max_output_tokens: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResolvedInput {
    input: InputReference,
    scope: String,
    content: String,
    selections: Vec<Selection>,
    required_protected_ranges: Vec<Range>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Selection {
    start_byte: u64,
    end_byte: u64,
    protected: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Range {
    pub(super) start_byte: u64,
    pub(super) end_byte: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Body {
    schema_version: &'static str,
    task: Task,
    target: Target,
    max_body_bytes: u64,
    template_version: &'static str,
    template_sha256: String,
    instructions: &'static str,
    response_schema: &'static str,
    inputs: Vec<BodyInput>,
    blocks: Vec<Block>,
    token_accounting: &'static str,
    authority: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BodyInput {
    input: InputReference,
    scope: String,
    byte_length: u64,
    required_protected_ranges: Vec<Range>,
    selected_ranges: Vec<Range>,
    protected_ranges: Vec<Range>,
    omitted_ranges: Vec<Range>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Block {
    text: String,
    aliases: Vec<Alias>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Alias {
    input_index: u64,
    start_byte: u64,
    end_byte: u64,
    protected: bool,
}

pub(super) fn prepare(raw_json: &[u8]) -> Result<ContextPack, Diagnostic> {
    if raw_json.len() > MAX_REQUEST_BYTES {
        return Err(Diagnostic::InputLimit);
    }
    preflight(raw_json, WireKind::Request)?;
    let request: Request =
        serde_json::from_slice(raw_json).map_err(|_| Diagnostic::InputInvalid)?;
    validate_request_shape(&request)?;

    let mut total_content = 0usize;
    let mut total_selections = 0usize;
    let mut total_required_protected_ranges = 0usize;
    let mut ids = HashSet::new();
    let mut body_inputs = Vec::with_capacity(request.inputs.len());
    let mut contexts = Vec::with_capacity(request.inputs.len());
    let mut blocks: Vec<Block> = Vec::new();
    let mut seen_blocks = HashMap::<String, usize>::new();
    let mut selected_bytes = 0u64;
    let mut omitted_bytes = 0u64;

    for (index, resolved) in request.inputs.into_iter().enumerate() {
        validate_reference(&resolved.input)?;
        let immutable_id = immutable_id(&resolved.input).to_owned();
        if !ids.insert(immutable_id) {
            return Err(Diagnostic::IdentityMismatch);
        }
        if !valid_scope(&resolved.scope) {
            return Err(Diagnostic::InputInvalid);
        }
        if resolved.content.len() > MAX_INPUT_BYTES {
            return Err(Diagnostic::InputLimit);
        }
        total_content = total_content
            .checked_add(resolved.content.len())
            .ok_or(Diagnostic::InputLimit)?;
        if total_content > MAX_TOTAL_INPUT_BYTES {
            return Err(Diagnostic::InputLimit);
        }
        if byte_digest(resolved.content.as_bytes()) != reference_digest(&resolved.input) {
            return Err(Diagnostic::IdentityMismatch);
        }
        if resolved.selections.is_empty() {
            return Err(Diagnostic::RangeInvalid);
        }
        total_selections = total_selections
            .checked_add(resolved.selections.len())
            .ok_or(Diagnostic::InputLimit)?;
        if total_selections > MAX_SELECTIONS {
            return Err(Diagnostic::InputLimit);
        }
        total_required_protected_ranges = total_required_protected_ranges
            .checked_add(resolved.required_protected_ranges.len())
            .ok_or(Diagnostic::InputLimit)?;
        if total_required_protected_ranges > MAX_REQUIRED_PROTECTED_RANGES {
            return Err(Diagnostic::InputLimit);
        }

        let mut selections = resolved.selections;
        selections.sort_by_key(|selection| {
            (
                selection.start_byte,
                selection.end_byte,
                selection.protected,
            )
        });
        for selection in &selections {
            validate_selection(selection, &resolved.content)?;
        }
        let selected_ranges = union(selections.iter().copied().map(|selection| Range {
            start_byte: selection.start_byte,
            end_byte: selection.end_byte,
        }));
        let protected_ranges = union(
            selections
                .iter()
                .filter(|selection| selection.protected)
                .map(|selection| Range {
                    start_byte: selection.start_byte,
                    end_byte: selection.end_byte,
                }),
        );
        let mut required_protected_ranges = resolved.required_protected_ranges;
        required_protected_ranges.sort_by_key(|range| (range.start_byte, range.end_byte));
        for range in &required_protected_ranges {
            validate_range(*range, &resolved.content)?;
        }
        let required_protected_ranges = union(required_protected_ranges);
        if required_protected_ranges
            .iter()
            .any(|required| !covered_by(*required, &protected_ranges))
        {
            return Err(Diagnostic::RangeInvalid);
        }
        let omitted_ranges = complements(&selected_ranges, resolved.content.len() as u64);
        if omitted_ranges.len() > MAX_OMITTED_RANGES {
            return Err(Diagnostic::InputLimit);
        }

        selected_bytes = selected_bytes
            .checked_add(total_range_bytes(&selected_ranges))
            .ok_or(Diagnostic::InputLimit)?;
        omitted_bytes = omitted_bytes
            .checked_add(total_range_bytes(&omitted_ranges))
            .ok_or(Diagnostic::InputLimit)?;

        for range in &selected_ranges {
            let text =
                resolved.content[range.start_byte as usize..range.end_byte as usize].to_owned();
            let alias = Alias {
                input_index: index as u64,
                start_byte: range.start_byte,
                end_byte: range.end_byte,
                protected: protected_ranges
                    .iter()
                    .any(|protected| overlaps(*range, *protected)),
            };
            if let Some(&block) = seen_blocks.get(&text) {
                blocks[block].aliases.push(alias);
            } else {
                let block = blocks.len();
                seen_blocks.insert(text.clone(), block);
                blocks.push(Block {
                    text,
                    aliases: vec![alias],
                });
            }
        }

        contexts.push(ResolvedContext {
            input: resolved.input.clone(),
            content: resolved.content.clone(),
            selected_ranges: selected_ranges.clone(),
        });
        body_inputs.push(BodyInput {
            input: resolved.input,
            scope: resolved.scope,
            byte_length: resolved.content.len() as u64,
            required_protected_ranges,
            selected_ranges,
            protected_ranges,
            omitted_ranges,
        });
    }

    let unique_text_bytes = blocks.iter().try_fold(0u64, |total, block| {
        total
            .checked_add(block.text.len() as u64)
            .ok_or(Diagnostic::InputLimit)
    })?;
    let instructions = request.task.template();
    if !request.task.template_is_exact(instructions) {
        return Err(Diagnostic::InputInvalid);
    }
    let body = Body {
        schema_version: BODY_SCHEMA,
        task: request.task,
        target: request.target.clone(),
        max_body_bytes: request.max_body_bytes,
        template_version: "1",
        template_sha256: byte_digest(instructions.as_bytes()),
        instructions,
        response_schema: RESPONSE_SCHEMA,
        inputs: body_inputs,
        blocks,
        token_accounting: "unknown",
        authority: "none",
    };
    let body_bytes = capped_json(&body, request.max_body_bytes)?;
    let body_sha256 = byte_digest(&body_bytes);
    let pack_id = pack_id(&body_bytes);

    Ok(ContextPack {
        schema_version: PACK_SCHEMA.into(),
        pack_id: pack_id.clone(),
        body_json: String::from_utf8(body_bytes.clone()).map_err(|_| Diagnostic::PackOverBudget)?,
        body_sha256,
        body_bytes: body_bytes.len() as u64,
        selected_bytes,
        unique_text_bytes,
        omitted_bytes,
        token_accounting: "unknown",
        authority: "none",
        context: PackContext {
            task: request.task,
            inputs: contexts,
            pack_id,
            target: request.target,
        },
    })
}

fn validate_request_shape(request: &Request) -> Result<(), Diagnostic> {
    if request.schema_version != REQUEST_SCHEMA
        || request.inputs.is_empty()
        || request.inputs.len() > MAX_INPUTS
        || !(1..=MAX_BODY_BYTES).contains(&request.max_body_bytes)
        || !(1..=32_768).contains(&request.target.max_output_tokens)
        || !valid_profile_id(&request.target.profile_id)
        || !valid_digest(&request.target.profile_sha256)
        || !valid_model(&request.target.model)
    {
        return Err(Diagnostic::InputInvalid);
    }
    if request.task == Task::CompareV1
        && (request.inputs.len() != 2
            || request
                .inputs
                .iter()
                .any(|input| !matches!(input.input, InputReference::Revision { .. })))
    {
        return Err(Diagnostic::InputInvalid);
    }
    Ok(())
}

fn validate_reference(reference: &InputReference) -> Result<(), Diagnostic> {
    let valid = match reference {
        InputReference::Source { source_id, sha256 } => {
            valid_id(source_id, "source:") && valid_digest(sha256)
        }
        InputReference::Revision {
            capability_id,
            revision_id,
            sha256,
        } => {
            valid_id(capability_id, "capability:")
                && valid_id(revision_id, "revision:")
                && valid_digest(sha256)
        }
    };
    valid.then_some(()).ok_or(Diagnostic::InputInvalid)
}

fn immutable_id(reference: &InputReference) -> &str {
    match reference {
        InputReference::Source { source_id, .. } => source_id,
        InputReference::Revision { revision_id, .. } => revision_id,
    }
}

fn reference_digest(reference: &InputReference) -> &str {
    match reference {
        InputReference::Source { sha256, .. } | InputReference::Revision { sha256, .. } => sha256,
    }
}

fn validate_selection(selection: &Selection, content: &str) -> Result<(), Diagnostic> {
    validate_range(
        Range {
            start_byte: selection.start_byte,
            end_byte: selection.end_byte,
        },
        content,
    )
}

fn validate_range(range: Range, content: &str) -> Result<(), Diagnostic> {
    if range.start_byte >= range.end_byte || range.end_byte > content.len() as u64 {
        return Err(Diagnostic::RangeInvalid);
    }
    let start = usize::try_from(range.start_byte).map_err(|_| Diagnostic::RangeInvalid)?;
    let end = usize::try_from(range.end_byte).map_err(|_| Diagnostic::RangeInvalid)?;
    (content.is_char_boundary(start) && content.is_char_boundary(end))
        .then_some(())
        .ok_or(Diagnostic::RangeInvalid)
}

fn union(ranges: impl IntoIterator<Item = Range>) -> Vec<Range> {
    let mut result: Vec<Range> = Vec::new();
    for range in ranges {
        if let Some(last) = result.last_mut() {
            if range.start_byte <= last.end_byte {
                last.end_byte = last.end_byte.max(range.end_byte);
            } else {
                result.push(range);
            }
        } else {
            result.push(range);
        }
    }
    result
}

fn complements(selected: &[Range], length: u64) -> Vec<Range> {
    let mut omitted = Vec::new();
    let mut start = 0;
    for range in selected {
        if start < range.start_byte {
            omitted.push(Range {
                start_byte: start,
                end_byte: range.start_byte,
            });
        }
        start = range.end_byte;
    }
    if start < length {
        omitted.push(Range {
            start_byte: start,
            end_byte: length,
        });
    }
    omitted
}

fn total_range_bytes(ranges: &[Range]) -> u64 {
    ranges
        .iter()
        .map(|range| range.end_byte - range.start_byte)
        .sum()
}

fn overlaps(left: Range, right: Range) -> bool {
    left.start_byte < right.end_byte && right.start_byte < left.end_byte
}

fn covered_by(required: Range, ranges: &[Range]) -> bool {
    ranges
        .iter()
        .any(|range| range.start_byte <= required.start_byte && required.end_byte <= range.end_byte)
}

fn valid_id(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(valid_digest)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_profile_id(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn valid_model(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| (0x21..=0x7e).contains(&byte) && !matches!(byte, b'\\' | b'"'))
        && !value.contains("://")
        && !value.contains('@')
}

fn valid_scope(value: &str) -> bool {
    value.len() <= 512 && !value.chars().any(char::is_control)
}

fn capped_json(value: &impl Serialize, limit: u64) -> Result<Vec<u8>, Diagnostic> {
    let mut writer = CappedWriter {
        bytes: Vec::new(),
        limit: limit as usize,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| Diagnostic::PackOverBudget)?;
    Ok(writer.bytes)
}

fn pack_id(body: &[u8]) -> String {
    let mut framed = Vec::with_capacity(b"rangoon.context-pack.v1\0".len() + 8 + body.len());
    framed.extend_from_slice(b"rangoon.context-pack.v1\0");
    framed.extend_from_slice(&(body.len() as u64).to_be_bytes());
    framed.extend_from_slice(body);
    format!("pack:{}", byte_digest(&framed))
}

struct CappedWriter {
    bytes: Vec<u8>,
    limit: usize,
}

impl Write for CappedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("context pack exceeds its byte budget"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
