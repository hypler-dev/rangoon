use crate::{
    pack::{Alias, Block, BodyInput, ContextPack, Range, Task},
    response::{Citation, Proposal, ProposalKind, ValidatedResponse},
};
use rangoon_domain::byte_digest;
use serde::Serialize;
use std::{
    fmt,
    io::{self, Write},
};

const INSPECTION_SCHEMA: &str = "rangoon.proposal-inspection.v1";
const MAX_INSPECTION_BYTES: usize = 512 * 1024;

/// Closed failures for bounded proposal inspection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionDiagnostic {
    InspectionMismatch,
    ProposalNotFound,
    InspectionLimit,
}

impl fmt::Display for InspectionDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InspectionMismatch => "inspection inputs do not match",
            Self::ProposalNotFound => "proposal was not found",
            Self::InspectionLimit => "inspection exceeds its byte limit",
        })
    }
}

impl std::error::Error for InspectionDiagnostic {}

/// An immutable, serializable inspection of one already validated proposal.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalInspection {
    schema_version: &'static str,
    inspection_id: String,
    pack_id: String,
    pack_body_sha256: String,
    response_sha256: String,
    task: Task,
    template_version: &'static str,
    template_sha256: String,
    profile_id: String,
    profile_sha256: String,
    configured_model: String,
    max_output_tokens: u64,
    proposal_index: u32,
    proposal_count: u32,
    proposal: InspectionProposal,
    uncertainties: Vec<String>,
    inputs: Vec<InspectionInput>,
    blocks: Vec<InspectionBlock>,
    selected_bytes: u64,
    unique_text_bytes: u64,
    omitted_bytes: u64,
    token_accounting: &'static str,
    content_kind: &'static str,
    authority: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InspectionProposal {
    kind: ProposalKind,
    title: String,
    authored_text: String,
    explanation: String,
    citations: Vec<Citation>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InspectionInput {
    input: rangoon_domain::composition::InputReference,
    scope: String,
    byte_length: u64,
    required_protected_ranges: Vec<Range>,
    selected_ranges: Vec<Range>,
    protected_ranges: Vec<Range>,
    omitted_ranges: Vec<Range>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InspectionBlock {
    text: String,
    aliases: Vec<Alias>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InspectionPreimage {
    schema_version: &'static str,
    pack_id: String,
    pack_body_sha256: String,
    response_sha256: String,
    task: Task,
    template_version: &'static str,
    template_sha256: String,
    profile_id: String,
    profile_sha256: String,
    configured_model: String,
    max_output_tokens: u64,
    proposal_index: u32,
    proposal_count: u32,
    proposal: InspectionProposal,
    uncertainties: Vec<String>,
    inputs: Vec<InspectionInput>,
    blocks: Vec<InspectionBlock>,
    selected_bytes: u64,
    unique_text_bytes: u64,
    omitted_bytes: u64,
    token_accounting: &'static str,
    content_kind: &'static str,
    authority: &'static str,
}

pub(super) fn inspect(
    pack: &ContextPack,
    response: &ValidatedResponse,
    proposal_index: u32,
) -> Result<ProposalInspection, InspectionDiagnostic> {
    let context = pack.context();
    if response.pack_id != context.pack_id || response.task != context.task {
        return Err(InspectionDiagnostic::InspectionMismatch);
    }
    let proposal = response
        .proposals
        .get(usize::try_from(proposal_index).map_err(|_| InspectionDiagnostic::ProposalNotFound)?)
        .ok_or(InspectionDiagnostic::ProposalNotFound)?;
    let proposal_count = u32::try_from(response.proposals.len())
        .map_err(|_| InspectionDiagnostic::InspectionLimit)?;
    let inputs = context.body_inputs.iter().map(inspection_input).collect();
    let blocks = context.blocks.iter().map(inspection_block).collect();
    let preimage = InspectionPreimage {
        schema_version: INSPECTION_SCHEMA,
        pack_id: context.pack_id.clone(),
        pack_body_sha256: pack.body_sha256.clone(),
        response_sha256: response.response_sha256.clone(),
        task: context.task,
        template_version: "1",
        template_sha256: context.template_sha256.clone(),
        profile_id: context.target.profile_id.clone(),
        profile_sha256: context.target.profile_sha256.clone(),
        configured_model: context.target.model.clone(),
        max_output_tokens: context.target.max_output_tokens,
        proposal_index,
        proposal_count,
        proposal: inspection_proposal(proposal),
        uncertainties: response.uncertainties.clone(),
        inputs,
        blocks,
        selected_bytes: pack.selected_bytes,
        unique_text_bytes: pack.unique_text_bytes,
        omitted_bytes: pack.omitted_bytes,
        token_accounting: "unknown",
        content_kind: "model_authored",
        authority: "none",
    };
    let preimage_json = capped_json(&preimage)?;
    let inspection_id = inspection_id(&preimage_json);
    let inspection = ProposalInspection {
        schema_version: preimage.schema_version,
        inspection_id,
        pack_id: preimage.pack_id,
        pack_body_sha256: preimage.pack_body_sha256,
        response_sha256: preimage.response_sha256,
        task: preimage.task,
        template_version: preimage.template_version,
        template_sha256: preimage.template_sha256,
        profile_id: preimage.profile_id,
        profile_sha256: preimage.profile_sha256,
        configured_model: preimage.configured_model,
        max_output_tokens: preimage.max_output_tokens,
        proposal_index: preimage.proposal_index,
        proposal_count: preimage.proposal_count,
        proposal: preimage.proposal,
        uncertainties: preimage.uncertainties,
        inputs: preimage.inputs,
        blocks: preimage.blocks,
        selected_bytes: preimage.selected_bytes,
        unique_text_bytes: preimage.unique_text_bytes,
        omitted_bytes: preimage.omitted_bytes,
        token_accounting: preimage.token_accounting,
        content_kind: preimage.content_kind,
        authority: preimage.authority,
    };
    capped_json(&inspection)?;
    Ok(inspection)
}

fn inspection_proposal(proposal: &Proposal) -> InspectionProposal {
    InspectionProposal {
        kind: proposal.kind,
        title: proposal.title.clone(),
        authored_text: proposal.authored_text.clone(),
        explanation: proposal.explanation.clone(),
        citations: proposal.citations.clone(),
    }
}

fn inspection_input(input: &BodyInput) -> InspectionInput {
    InspectionInput {
        input: input.input.clone(),
        scope: input.scope.clone(),
        byte_length: input.byte_length,
        required_protected_ranges: input.required_protected_ranges.clone(),
        selected_ranges: input.selected_ranges.clone(),
        protected_ranges: input.protected_ranges.clone(),
        omitted_ranges: input.omitted_ranges.clone(),
    }
}

fn inspection_block(block: &Block) -> InspectionBlock {
    InspectionBlock {
        text: block.text.clone(),
        aliases: block.aliases.clone(),
    }
}

fn inspection_id(preimage: &[u8]) -> String {
    let mut framed = Vec::with_capacity(INSPECTION_SCHEMA.len() + 1 + 8 + preimage.len());
    framed.extend_from_slice(INSPECTION_SCHEMA.as_bytes());
    framed.push(0);
    framed.extend_from_slice(&(preimage.len() as u64).to_be_bytes());
    framed.extend_from_slice(preimage);
    format!("inspection:{}", byte_digest(&framed))
}

fn capped_json(value: &impl Serialize) -> Result<Vec<u8>, InspectionDiagnostic> {
    let mut writer = CappedWriter::default();
    serde_json::to_writer(&mut writer, value).map_err(|_| InspectionDiagnostic::InspectionLimit)?;
    Ok(writer.bytes)
}

#[derive(Default)]
struct CappedWriter {
    bytes: Vec<u8>,
}

impl Write for CappedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_INSPECTION_BYTES.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other(
                "proposal inspection exceeds its byte limit",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capped_writer_never_truncates() {
        let mut writer = CappedWriter {
            bytes: vec![b'x'; MAX_INSPECTION_BYTES],
        };
        assert!(writer.write(b"y").is_err());
        assert_eq!(writer.bytes.len(), MAX_INSPECTION_BYTES);
    }
}
