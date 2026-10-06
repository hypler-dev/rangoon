use crate::{
    Diagnostic,
    json::{WireKind, preflight},
    pack::{ContextPack, Range, Task},
};
use rangoon_domain::{byte_digest, composition::InputReference};
use serde::{Deserialize, Serialize};

const RESPONSE_SCHEMA: &str = "rangoon.analysis-proposals.v1";
const VALIDATED_SCHEMA: &str = "rangoon.validated-analysis.v1";
const MAX_RESPONSE_BYTES: usize = 128 * 1024;
const MAX_PROPOSALS: usize = 16;
const MAX_CITATIONS_PER_PROPOSAL: usize = 64;
const MAX_CITATIONS: usize = 256;
const MAX_AUTHORED_TEXT_PER_PROPOSAL: usize = 16 * 1024;
const MAX_AUTHORED_TEXT: usize = 64 * 1024;
const MAX_EXPLANATION_BYTES: usize = 1024;
const MAX_UNCERTAINTIES: usize = 32;
const MAX_UNCERTAINTY_BYTES: usize = 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatedResponse {
    schema_version: &'static str,
    pack_id: String,
    response_sha256: String,
    task: Task,
    proposals: Vec<Proposal>,
    uncertainties: Vec<String>,
    content_kind: &'static str,
    authority: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Response {
    schema_version: String,
    task: Task,
    proposals: Vec<Proposal>,
    uncertainties: Vec<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Proposal {
    kind: ProposalKind,
    title: String,
    authored_text: String,
    explanation: String,
    citations: Vec<Citation>,
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ProposalKind {
    Classification,
    Capability,
    Difference,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Citation {
    input: InputReference,
    start_byte: u64,
    end_byte: u64,
}

pub(super) fn validate(
    pack: &ContextPack,
    raw_json: &[u8],
) -> Result<ValidatedResponse, Diagnostic> {
    if raw_json.len() > MAX_RESPONSE_BYTES {
        return Err(Diagnostic::ResponseLimit);
    }
    preflight(raw_json, WireKind::Response)?;
    let response: Response =
        serde_json::from_slice(raw_json).map_err(|_| Diagnostic::ResponseInvalid)?;
    let context = pack.context();
    if response.schema_version != RESPONSE_SCHEMA || response.task != context.task {
        return Err(Diagnostic::ResponseInvalid);
    }
    if response.proposals.len() > MAX_PROPOSALS || response.uncertainties.len() > MAX_UNCERTAINTIES
    {
        return Err(Diagnostic::ResponseLimit);
    }

    let expected_kind = expected_kind(context.task);
    let mut total_citations = 0usize;
    let mut total_authored_text = 0usize;
    for proposal in &response.proposals {
        if proposal.kind != expected_kind
            || !valid_nonblank(&proposal.title, 120)
            || !valid_nonblank(&proposal.explanation, MAX_EXPLANATION_BYTES)
        {
            return Err(Diagnostic::ResponseInvalid);
        }
        if proposal.authored_text.len() > MAX_AUTHORED_TEXT_PER_PROPOSAL {
            return Err(Diagnostic::ResponseLimit);
        }
        total_authored_text = total_authored_text
            .checked_add(proposal.authored_text.len())
            .ok_or(Diagnostic::ResponseLimit)?;
        if total_authored_text > MAX_AUTHORED_TEXT {
            return Err(Diagnostic::ResponseLimit);
        }
        if proposal.citations.is_empty() || proposal.citations.len() > MAX_CITATIONS_PER_PROPOSAL {
            return Err(Diagnostic::ResponseInvalid);
        }
        total_citations = total_citations
            .checked_add(proposal.citations.len())
            .ok_or(Diagnostic::ResponseLimit)?;
        if total_citations > MAX_CITATIONS {
            return Err(Diagnostic::ResponseLimit);
        }
        for citation in &proposal.citations {
            validate_citation(citation, &context.inputs)?;
        }
    }
    if response
        .uncertainties
        .iter()
        .any(|uncertainty| uncertainty.is_empty() || uncertainty.len() > MAX_UNCERTAINTY_BYTES)
    {
        return Err(Diagnostic::ResponseInvalid);
    }

    Ok(ValidatedResponse {
        schema_version: VALIDATED_SCHEMA,
        pack_id: context.pack_id.clone(),
        response_sha256: byte_digest(raw_json),
        task: context.task,
        proposals: response.proposals,
        uncertainties: response.uncertainties,
        content_kind: "model_authored",
        authority: "none",
    })
}

fn expected_kind(task: Task) -> ProposalKind {
    match task {
        Task::ClassifyV1 => ProposalKind::Classification,
        Task::DecomposeV1 => ProposalKind::Capability,
        Task::CompareV1 => ProposalKind::Difference,
    }
}

fn valid_nonblank(value: &str, maximum_bytes: usize) -> bool {
    !value.is_empty() && value.len() <= maximum_bytes && !value.chars().all(char::is_whitespace)
}

fn validate_citation(
    citation: &Citation,
    inputs: &[crate::pack::ResolvedContext],
) -> Result<(), Diagnostic> {
    let Some(input) = inputs.iter().find(|input| input.input == citation.input) else {
        return Err(Diagnostic::ResponseInvalid);
    };
    let range = Range {
        start_byte: citation.start_byte,
        end_byte: citation.end_byte,
    };
    if range.start_byte >= range.end_byte || range.end_byte > input.content.len() as u64 {
        return Err(Diagnostic::ResponseInvalid);
    }
    let start = usize::try_from(range.start_byte).map_err(|_| Diagnostic::ResponseInvalid)?;
    let end = usize::try_from(range.end_byte).map_err(|_| Diagnostic::ResponseInvalid)?;
    if !input.content.is_char_boundary(start)
        || !input.content.is_char_boundary(end)
        || !input.selected_ranges.iter().any(|selected| {
            selected.start_byte <= range.start_byte && range.end_byte <= selected.end_byte
        })
    {
        return Err(Diagnostic::ResponseInvalid);
    }
    Ok(())
}
