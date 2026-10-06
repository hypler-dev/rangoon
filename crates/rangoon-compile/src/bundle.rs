//! Pure portable instruction-bundle encoding and inspection.

use crate::{
    Artifact, ArtifactMetadata, Candidate, CandidateManifest, CompilationReport, Diagnostic,
    DiagnosticCode, Profile, Readiness, SelectedRevision, Severity, candidate_id, canonical,
    diagnostics, framed_identity, valid_timestamp,
};
use rangoon_domain::{
    Authority, byte_digest,
    capability::{self, ContentReview, Reviewer, valid_content, valid_id, valid_title},
    capability_v1::RevisionProvenance,
};
use serde::{Deserialize, Serialize};

pub const MAGIC: &[u8] = b"RANGOON-INSTRUCTIONS-V1\n";
const TRAILER_BYTES: usize = 64;
pub const HEADER_BYTES: usize = MAGIC.len() + 16;
const MAX_MANIFEST_BYTES: usize = 32_768;
const MAX_ARTIFACT_BYTES: usize = 262_144;
pub const MAX_BUNDLE_BYTES: usize = 295_016;
const INSPECTION_SCHEMA: &str = "rangoon.instruction-bundle-inspection.v1";
const MANIFEST_SCHEMA: &str = "rangoon.instruction-candidate.v1";
const REPORT_SCHEMA: &str = "rangoon.compilation.v1";
const IDENTITY_SCHEMA: &str = "rangoon.compilation-identity.v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BundleError {
    TooLarge,
    Invalid,
    NotCandidate,
    SerializationFailed,
}

impl BundleError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::TooLarge => "bundle_too_large",
            Self::Invalid => "bundle_invalid",
            Self::NotCandidate => "bundle_not_candidate",
            Self::SerializationFailed => "bundle_serialization_failed",
        }
    }
}

impl std::fmt::Display for BundleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for BundleError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedBundle {
    pub bytes: Vec<u8>,
    pub inspection: BundleInspection,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleInspection {
    pub schema_version: String,
    pub bundle_id: String,
    pub sha256: String,
    pub byte_length: u64,
    pub compilation: CompilationReport,
    pub verification: String,
    pub authority: Authority,
}

/// Encode an already reviewed compiler candidate. This has no filesystem or runtime authority.
pub fn encode_bundle(report: &CompilationReport) -> Result<EncodedBundle, BundleError> {
    if report.readiness != Readiness::Candidate || report.candidate.is_none() {
        return Err(BundleError::NotCandidate);
    }
    let candidate = report.candidate.as_ref().ok_or(BundleError::NotCandidate)?;
    let manifest = candidate
        .canonical_manifest_bytes()
        .map_err(|_| BundleError::SerializationFailed)?;
    let artifact = report.artifact.content.as_bytes();
    if manifest.len() > MAX_MANIFEST_BYTES {
        return Err(BundleError::TooLarge);
    }
    if manifest.is_empty() || artifact.is_empty() {
        return Err(BundleError::Invalid);
    }
    if artifact.len() > MAX_ARTIFACT_BYTES {
        return Err(BundleError::TooLarge);
    }

    let total = checked_total(manifest.len(), artifact.len()).ok_or(BundleError::TooLarge)?;
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&(manifest.len() as u64).to_be_bytes());
    bytes.extend_from_slice(&(artifact.len() as u64).to_be_bytes());
    bytes.extend_from_slice(&manifest);
    bytes.extend_from_slice(artifact);
    bytes.extend_from_slice(byte_digest(&bytes).as_bytes());

    let inspection = inspect_bundle(&bytes)?;
    if inspection.compilation != *report {
        return Err(BundleError::Invalid);
    }
    Ok(EncodedBundle { bytes, inspection })
}

/// Inspect an untrusted bundle entirely in memory. A successful result is consistency evidence only.
pub fn inspect_bundle(bytes: &[u8]) -> Result<BundleInspection, BundleError> {
    if bytes.len() > MAX_BUNDLE_BYTES {
        return Err(BundleError::TooLarge);
    }
    if bytes.len() < HEADER_BYTES + TRAILER_BYTES + 2 || !bytes.starts_with(MAGIC) {
        return Err(BundleError::Invalid);
    }
    let manifest_length = read_length(&bytes[MAGIC.len()..MAGIC.len() + 8])?;
    let artifact_length = read_length(&bytes[MAGIC.len() + 8..HEADER_BYTES])?;
    if manifest_length == 0 || artifact_length == 0 {
        return Err(BundleError::Invalid);
    }
    if manifest_length > MAX_MANIFEST_BYTES || artifact_length > MAX_ARTIFACT_BYTES {
        return Err(BundleError::TooLarge);
    }
    let total = checked_total(manifest_length, artifact_length).ok_or(BundleError::TooLarge)?;
    if total != bytes.len() {
        return Err(BundleError::Invalid);
    }

    let manifest_end = HEADER_BYTES + manifest_length;
    let artifact_end = manifest_end + artifact_length;
    let trailer = &bytes[artifact_end..];
    if trailer.len() != TRAILER_BYTES
        || !trailer.iter().all(u8::is_ascii_hexdigit)
        || !trailer.iter().all(|byte| !byte.is_ascii_uppercase())
        || trailer != byte_digest(&bytes[..artifact_end]).as_bytes()
    {
        return Err(BundleError::Invalid);
    }
    let manifest_bytes = &bytes[HEADER_BYTES..manifest_end];
    let artifact_bytes = &bytes[manifest_end..artifact_end];
    let artifact = std::str::from_utf8(artifact_bytes).map_err(|_| BundleError::Invalid)?;
    if !valid_content(artifact) {
        return Err(BundleError::Invalid);
    }
    let wire: WireManifest =
        serde_json::from_slice(manifest_bytes).map_err(|_| BundleError::Invalid)?;
    if canonical(&wire).map_err(|_| BundleError::SerializationFailed)? != manifest_bytes {
        return Err(BundleError::Invalid);
    }
    let report = reconstruct_report(wire, artifact)?;
    if report
        .canonical_manifest_bytes()
        .map_err(|_| BundleError::SerializationFailed)?
        .as_deref()
        != Some(manifest_bytes)
    {
        return Err(BundleError::Invalid);
    }
    let bundle_sha256 = byte_digest(bytes);
    let mut identity = b"rangoon.instruction-bundle.v1\0".to_vec();
    identity.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    identity.extend_from_slice(bytes);
    Ok(BundleInspection {
        schema_version: INSPECTION_SCHEMA.into(),
        bundle_id: format!("instruction-bundle:{}", byte_digest(&identity)),
        sha256: bundle_sha256,
        byte_length: bytes.len() as u64,
        compilation: report,
        verification: "internal_consistency_only".into(),
        authority: Authority::None,
    })
}

fn checked_total(manifest_length: usize, artifact_length: usize) -> Option<usize> {
    HEADER_BYTES
        .checked_add(manifest_length)?
        .checked_add(artifact_length)?
        .checked_add(TRAILER_BYTES)
        .filter(|total| *total <= MAX_BUNDLE_BYTES)
}

fn read_length(bytes: &[u8]) -> Result<usize, BundleError> {
    let array: [u8; 8] = bytes.try_into().map_err(|_| BundleError::Invalid)?;
    usize::try_from(u64::from_be_bytes(array)).map_err(|_| BundleError::TooLarge)
}

fn reconstruct_report(wire: WireManifest, content: &str) -> Result<CompilationReport, BundleError> {
    if wire.schema_version != MANIFEST_SCHEMA || wire.authority != Authority::None {
        return Err(BundleError::Invalid);
    }
    let profile = profile_for_digest(&wire.profile_digest).ok_or(BundleError::Invalid)?;
    let descriptor = profile.descriptor();
    let selected = SelectedRevision {
        capability_id: wire.selected_revision.capability_id,
        revision_id: wire.selected_revision.revision_id,
        parent_revision_id: wire.selected_revision.parent_revision_id,
        title: wire.selected_revision.title,
        sha256: wire.selected_revision.sha256,
        provenance: wire.selected_revision.provenance,
    };
    let artifact = Artifact {
        path: wire.artifact.path,
        content: content.into(),
        sha256: wire.artifact.sha256,
        byte_length: wire.artifact.byte_length,
    };
    let review = wire.review_observation.ok_or(BundleError::Invalid)?;
    let diagnostics = wire
        .diagnostics
        .into_iter()
        .map(Into::into)
        .collect::<Vec<Diagnostic>>();
    validate_fields(
        &selected,
        &artifact,
        &review,
        profile,
        &wire.profile_digest,
        &diagnostics,
    )?;

    let computed_compilation_id =
        compilation_id(&wire.profile_digest, &selected, &artifact, &diagnostics)?;
    if wire.compilation_id != computed_compilation_id {
        return Err(BundleError::Invalid);
    }
    let manifest = CandidateManifest {
        schema_version: MANIFEST_SCHEMA.into(),
        compilation_id: computed_compilation_id.clone(),
        profile_digest: wire.profile_digest,
        selected_revision: selected.clone(),
        artifact: ArtifactMetadata::from(&artifact),
        review_observation: Some(review.clone()),
        diagnostics: diagnostics.clone(),
        authority: Authority::None,
    };
    let manifest_sha256 =
        byte_digest(&canonical(&manifest).map_err(|_| BundleError::SerializationFailed)?);
    let candidate = Candidate {
        candidate_id: candidate_id(&computed_compilation_id, &manifest_sha256),
        manifest,
        manifest_sha256,
    };
    Ok(CompilationReport {
        schema_version: REPORT_SCHEMA.into(),
        compilation_id: computed_compilation_id,
        profile: descriptor,
        profile_digest: candidate.manifest.profile_digest.clone(),
        selected_revision: selected,
        artifact,
        diagnostics,
        review_observation: Some(review),
        readiness: Readiness::Candidate,
        candidate: Some(candidate),
        authority: Authority::None,
    })
}

fn profile_for_digest(digest: &str) -> Option<Profile> {
    [Profile::AgentsMdV1, Profile::ClaudeMdV1]
        .into_iter()
        .find(|profile| {
            profile
                .canonical_descriptor_bytes()
                .ok()
                .is_some_and(|bytes| byte_digest(&bytes) == digest)
        })
}

fn validate_fields(
    selected: &SelectedRevision,
    artifact: &Artifact,
    review: &ContentReview,
    profile: Profile,
    profile_digest: &str,
    diagnostics_in_manifest: &[Diagnostic],
) -> Result<(), BundleError> {
    if artifact.path != profile.descriptor().artifact_path
        || artifact.byte_length != artifact.content.len() as u64
        || artifact.sha256 != byte_digest(artifact.content.as_bytes())
        || artifact.sha256 != selected.sha256
        || !valid_content(&artifact.content)
        || !valid_id(&selected.capability_id, "capability:")
        || !valid_id(&selected.revision_id, "revision:")
        || selected
            .parent_revision_id
            .as_deref()
            .is_some_and(|id| !valid_id(id, "revision:"))
        || !valid_title(&selected.title)
        || !valid_timestamp(review.reviewed_at_ms)
        || review.reviewer != Reviewer::LocalOperator
    {
        return Err(BundleError::Invalid);
    }
    match &selected.provenance {
        RevisionProvenance::Ordinary {} => {
            if selected.revision_id
                != capability::revision_id(
                    &selected.capability_id,
                    selected.parent_revision_id.as_deref(),
                    &selected.title,
                    &artifact.content,
                )
            {
                return Err(BundleError::Invalid);
            }
        }
        RevisionProvenance::Composition {
            application_id,
            composition_id,
            output_index,
        } => {
            if !valid_id(application_id, "composition-application:")
                || !valid_id(composition_id, "composition:")
                || *output_index > 15
            {
                return Err(BundleError::Invalid);
            }
        }
    }
    let expected_profile_digest = byte_digest(
        &profile
            .canonical_descriptor_bytes()
            .map_err(|_| BundleError::SerializationFailed)?,
    );
    if profile_digest != expected_profile_digest
        || diagnostics_in_manifest
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
        || diagnostics_in_manifest != diagnostics(profile, &[], &artifact.content)
    {
        return Err(BundleError::Invalid);
    }
    Ok(())
}

pub(crate) fn compilation_id(
    profile_digest: &str,
    selected_revision: &SelectedRevision,
    artifact: &Artifact,
    diagnostics: &[Diagnostic],
) -> Result<String, BundleError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct IdentityEnvelope<'a> {
        schema_version: &'static str,
        profile_digest: &'a str,
        selected_revision: &'a SelectedRevision,
        artifact: ArtifactMetadata,
        diagnostics: &'a [Diagnostic],
        authority: Authority,
    }
    let identity = IdentityEnvelope {
        schema_version: IDENTITY_SCHEMA,
        profile_digest,
        selected_revision,
        artifact: ArtifactMetadata::from(artifact),
        diagnostics,
        authority: Authority::None,
    };
    let bytes = canonical(&identity).map_err(|_| BundleError::SerializationFailed)?;
    Ok(framed_identity("rangoon.compilation.v1\0", &bytes))
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireManifest {
    schema_version: String,
    compilation_id: String,
    profile_digest: String,
    selected_revision: WireSelectedRevision,
    artifact: WireArtifactMetadata,
    review_observation: Option<ContentReview>,
    diagnostics: Vec<WireDiagnostic>,
    authority: Authority,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireSelectedRevision {
    capability_id: String,
    revision_id: String,
    parent_revision_id: Option<String>,
    title: String,
    sha256: String,
    provenance: RevisionProvenance,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireArtifactMetadata {
    path: String,
    sha256: String,
    byte_length: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireDiagnostic {
    code: WireDiagnosticCode,
    severity: WireSeverity,
    detail: Option<String>,
}

impl From<WireDiagnostic> for Diagnostic {
    fn from(value: WireDiagnostic) -> Self {
        Self {
            code: value.code.into(),
            severity: value.severity.into(),
            detail: value.detail,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireDiagnosticCode {
    InstructionSemanticsUnverified,
    TargetEnvironmentUnqualified,
    UnsupportedRequirement,
    PossibleIncludeSyntax,
    PossibleCommentElision,
}

impl From<WireDiagnosticCode> for DiagnosticCode {
    fn from(value: WireDiagnosticCode) -> Self {
        match value {
            WireDiagnosticCode::InstructionSemanticsUnverified => {
                Self::InstructionSemanticsUnverified
            }
            WireDiagnosticCode::TargetEnvironmentUnqualified => Self::TargetEnvironmentUnqualified,
            WireDiagnosticCode::UnsupportedRequirement => Self::UnsupportedRequirement,
            WireDiagnosticCode::PossibleIncludeSyntax => Self::PossibleIncludeSyntax,
            WireDiagnosticCode::PossibleCommentElision => Self::PossibleCommentElision,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireSeverity {
    Warning,
    Error,
}

impl From<WireSeverity> for Severity {
    fn from(value: WireSeverity) -> Self {
        match value {
            WireSeverity::Warning => Self::Warning,
            WireSeverity::Error => Self::Error,
        }
    }
}
