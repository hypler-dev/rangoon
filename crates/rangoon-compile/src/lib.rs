//! Pure, deterministic instruction compilation. It has no runtime or I/O authority.
#![forbid(unsafe_code)]

use rangoon_domain::{
    Authority, byte_digest,
    capability::{self, ContentReview, valid_content, valid_id, valid_title},
    capability_v1::{Revision, RevisionProvenance},
};
use serde::{Deserialize, Serialize};

pub mod bundle;
pub use bundle::{BundleError, BundleInspection, EncodedBundle, encode_bundle, inspect_bundle};

const REPORT_SCHEMA: &str = "rangoon.compilation.v1";
const MANIFEST_SCHEMA: &str = "rangoon.instruction-candidate.v1";
const IDENTITY_SCHEMA: &str = "rangoon.compilation-identity.v1";
const MAX_TIMESTAMP_MS: i64 = 8_640_000_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    AgentsMdV1,
    ClaudeMdV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDescriptor {
    pub id: Profile,
    pub version: u32,
    pub artifact_path: String,
    pub documentation_url: String,
    pub documentation_retrieved_on: String,
    pub runtime_qualification: String,
    pub target_budget: String,
    pub semantic_equivalence: String,
}

impl Profile {
    pub fn descriptor(self) -> ProfileDescriptor {
        let (artifact_path, documentation_url) = match self {
            Self::AgentsMdV1 => (
                "AGENTS.md",
                "https://learn.chatgpt.com/docs/agent-configuration/agents-md",
            ),
            Self::ClaudeMdV1 => ("CLAUDE.md", "https://code.claude.com/docs/en/memory"),
        };
        ProfileDescriptor {
            id: self,
            version: 1,
            artifact_path: artifact_path.into(),
            documentation_url: documentation_url.into(),
            documentation_retrieved_on: "2026-10-06".into(),
            runtime_qualification: "untested".into(),
            target_budget: "unknown".into(),
            semantic_equivalence: "unverified".into(),
        }
    }

    pub fn canonical_descriptor_bytes(self) -> Result<Vec<u8>, CompileError> {
        canonical(&self.descriptor())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompileError {
    InvalidSelection,
    InvalidRequirements,
    SerializationFailed,
}

impl CompileError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidSelection => "invalid_selection",
            Self::InvalidRequirements => "invalid_requirements",
            Self::SerializationFailed => "serialization_failed",
        }
    }
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for CompileError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedRevision {
    pub capability_id: String,
    pub revision_id: String,
    pub parent_revision_id: Option<String>,
    pub title: String,
    pub sha256: String,
    pub provenance: RevisionProvenance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub path: String,
    pub content: String,
    pub sha256: String,
    pub byte_length: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactMetadata {
    pub path: String,
    pub sha256: String,
    pub byte_length: u64,
}

impl From<&Artifact> for ArtifactMetadata {
    fn from(value: &Artifact) -> Self {
        Self {
            path: value.path.clone(),
            sha256: value.sha256.clone(),
            byte_length: value.byte_length,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    InstructionSemanticsUnverified,
    TargetEnvironmentUnqualified,
    UnsupportedRequirement,
    PossibleIncludeSyntax,
    PossibleCommentElision,
}

impl DiagnosticCode {
    pub const fn explanation(self) -> &'static str {
        match self {
            Self::InstructionSemanticsUnverified => {
                "Exact bytes do not prove equivalent instruction meaning, dependency closure, or enforcement."
            }
            Self::TargetEnvironmentUnqualified => {
                "Discovery, other instruction files, loading transformations, and target budget are unqualified."
            }
            Self::UnsupportedRequirement => {
                "Text-only profiles do not support declared structured requirements."
            }
            Self::PossibleIncludeSyntax => {
                "Conservative ASCII @ scan blocks possible includes, including emails and code examples."
            }
            Self::PossibleCommentElision => {
                "Conservative <!-- scan blocks comments because target loading can omit them."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub detail: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    Blocked,
    ReviewRequired,
    Candidate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateManifest {
    pub schema_version: String,
    pub compilation_id: String,
    pub profile_digest: String,
    pub selected_revision: SelectedRevision,
    pub artifact: ArtifactMetadata,
    pub review_observation: Option<ContentReview>,
    pub diagnostics: Vec<Diagnostic>,
    pub authority: Authority,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub manifest: CandidateManifest,
    pub manifest_sha256: String,
    pub candidate_id: String,
}

impl Candidate {
    pub fn canonical_manifest_bytes(&self) -> Result<Vec<u8>, CompileError> {
        canonical(&self.manifest)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilationReport {
    pub schema_version: String,
    pub compilation_id: String,
    pub profile: ProfileDescriptor,
    pub profile_digest: String,
    pub selected_revision: SelectedRevision,
    pub artifact: Artifact,
    pub diagnostics: Vec<Diagnostic>,
    pub review_observation: Option<ContentReview>,
    pub readiness: Readiness,
    pub candidate: Option<Candidate>,
    pub authority: Authority,
}

impl CompilationReport {
    pub fn canonical_manifest_bytes(&self) -> Result<Option<Vec<u8>>, CompileError> {
        self.candidate
            .as_ref()
            .map(Candidate::canonical_manifest_bytes)
            .transpose()
    }
}

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

pub fn compile(
    capability_id: &str,
    revision: &Revision,
    profile: Profile,
    requirements: &[String],
) -> Result<CompilationReport, CompileError> {
    validate_selection(capability_id, revision)?;
    validate_requirements(requirements)?;

    let descriptor = profile.descriptor();
    let profile_digest = byte_digest(&canonical(&descriptor)?);
    let selected_revision = SelectedRevision {
        capability_id: capability_id.into(),
        revision_id: revision.id.clone(),
        parent_revision_id: revision.parent_revision_id.clone(),
        title: revision.title.clone(),
        sha256: revision.sha256.clone(),
        provenance: revision.provenance.clone(),
    };
    let artifact = Artifact {
        path: descriptor.artifact_path.clone(),
        content: revision.content.clone(),
        sha256: revision.sha256.clone(),
        byte_length: revision.content.len() as u64,
    };
    let diagnostics = diagnostics(profile, requirements, &revision.content);
    let identity = IdentityEnvelope {
        schema_version: IDENTITY_SCHEMA,
        profile_digest: &profile_digest,
        selected_revision: &selected_revision,
        artifact: ArtifactMetadata::from(&artifact),
        diagnostics: &diagnostics,
        authority: Authority::None,
    };
    let compilation_id = framed_identity("rangoon.compilation.v1\0", &canonical(&identity)?);
    let readiness = if diagnostics
        .iter()
        .any(|item| item.severity == Severity::Error)
    {
        Readiness::Blocked
    } else if revision.review.is_none() {
        Readiness::ReviewRequired
    } else {
        Readiness::Candidate
    };
    let candidate = (readiness == Readiness::Candidate)
        .then(|| {
            let manifest = CandidateManifest {
                schema_version: MANIFEST_SCHEMA.into(),
                compilation_id: compilation_id.clone(),
                profile_digest: profile_digest.clone(),
                selected_revision: selected_revision.clone(),
                artifact: ArtifactMetadata::from(&artifact),
                review_observation: revision.review.clone(),
                diagnostics: diagnostics.clone(),
                authority: Authority::None,
            };
            let manifest_sha256 = byte_digest(&canonical(&manifest)?);
            let candidate_id = candidate_id(&compilation_id, &manifest_sha256);
            Ok(Candidate {
                manifest,
                manifest_sha256,
                candidate_id,
            })
        })
        .transpose()?;
    Ok(CompilationReport {
        schema_version: REPORT_SCHEMA.into(),
        compilation_id,
        profile: descriptor,
        profile_digest,
        selected_revision,
        artifact,
        diagnostics,
        review_observation: revision.review.clone(),
        readiness,
        candidate,
        authority: Authority::None,
    })
}

pub(crate) fn canonical<T: Serialize>(value: &T) -> Result<Vec<u8>, CompileError> {
    serde_json::to_vec(value).map_err(|_| CompileError::SerializationFailed)
}

pub(crate) fn framed_identity(domain: &str, envelope: &[u8]) -> String {
    let mut bytes = domain.as_bytes().to_vec();
    bytes.extend_from_slice(&(envelope.len() as u64).to_be_bytes());
    bytes.extend_from_slice(envelope);
    format!("compilation:{}", byte_digest(&bytes))
}

pub(crate) fn candidate_id(compilation_id: &str, manifest_sha256: &str) -> String {
    let mut bytes = b"rangoon.instruction-candidate.v1\0".to_vec();
    for field in [compilation_id, manifest_sha256] {
        bytes.extend_from_slice(&(field.len() as u64).to_be_bytes());
        bytes.extend_from_slice(field.as_bytes());
    }
    format!("candidate:{}", byte_digest(&bytes))
}

fn validate_selection(capability_id: &str, revision: &Revision) -> Result<(), CompileError> {
    if !valid_id(capability_id, "capability:")
        || !valid_id(&revision.id, "revision:")
        || revision
            .parent_revision_id
            .as_deref()
            .is_some_and(|id| !valid_id(id, "revision:"))
        || !valid_title(&revision.title)
        || !valid_content(&revision.content)
        || revision.sha256 != byte_digest(revision.content.as_bytes())
        || !valid_timestamp(revision.created_at_ms)
        || revision
            .review
            .as_ref()
            .is_some_and(|review| !valid_timestamp(review.reviewed_at_ms))
    {
        return Err(CompileError::InvalidSelection);
    }
    match &revision.provenance {
        RevisionProvenance::Ordinary {} => {
            if revision.id
                != capability::revision_id(
                    capability_id,
                    revision.parent_revision_id.as_deref(),
                    &revision.title,
                    &revision.content,
                )
            {
                return Err(CompileError::InvalidSelection);
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
                return Err(CompileError::InvalidSelection);
            }
        }
    }
    Ok(())
}

fn validate_requirements(requirements: &[String]) -> Result<(), CompileError> {
    if requirements.len() > 32 {
        return Err(CompileError::InvalidRequirements);
    }
    let mut seen = std::collections::BTreeSet::new();
    for requirement in requirements {
        if requirement.is_empty()
            || requirement.len() > 64
            || !requirement.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'-' | b'.')
            })
            || !seen.insert(requirement)
        {
            return Err(CompileError::InvalidRequirements);
        }
    }
    Ok(())
}

pub(crate) fn valid_timestamp(value: i64) -> bool {
    (0..=MAX_TIMESTAMP_MS).contains(&value)
}

pub(crate) fn diagnostics(
    profile: Profile,
    requirements: &[String],
    content: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = vec![
        Diagnostic {
            code: DiagnosticCode::InstructionSemanticsUnverified,
            severity: Severity::Warning,
            detail: None,
        },
        Diagnostic {
            code: DiagnosticCode::TargetEnvironmentUnqualified,
            severity: Severity::Warning,
            detail: None,
        },
    ];
    diagnostics.extend(requirements.iter().cloned().map(|detail| Diagnostic {
        code: DiagnosticCode::UnsupportedRequirement,
        severity: Severity::Error,
        detail: Some(detail),
    }));
    if profile == Profile::ClaudeMdV1 && content.contains('@') {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::PossibleIncludeSyntax,
            severity: Severity::Error,
            detail: None,
        });
    }
    if profile == Profile::ClaudeMdV1 && content.contains("<!--") {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::PossibleCommentElision,
            severity: Severity::Error,
            detail: None,
        });
    }
    diagnostics
}

#[cfg(test)]
#[path = "bundle_tests.rs"]
mod bundle_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use rangoon_domain::capability::{ContentReview, Reviewer};

    fn id(prefix: &str, byte: char) -> String {
        format!("{prefix}:{}", byte.to_string().repeat(64))
    }

    fn revision(content: &str) -> (String, Revision) {
        let capability_id = id("capability", 'a');
        let title = "Exact rules".to_string();
        let revision = Revision {
            id: capability::revision_id(&capability_id, None, &title, content),
            parent_revision_id: None,
            title,
            content: content.into(),
            sha256: byte_digest(content.as_bytes()),
            created_at_ms: 0,
            review: None,
            provenance: RevisionProvenance::Ordinary {},
        };
        (capability_id, revision)
    }

    #[test]
    fn descriptors_are_pinned_and_byte_exact() {
        assert_eq!(
            String::from_utf8(Profile::AgentsMdV1.canonical_descriptor_bytes().unwrap()).unwrap(),
            r#"{"id":"agents_md_v1","version":1,"artifactPath":"AGENTS.md","documentationUrl":"https://learn.chatgpt.com/docs/agent-configuration/agents-md","documentationRetrievedOn":"2026-10-06","runtimeQualification":"untested","targetBudget":"unknown","semanticEquivalence":"unverified"}"#
        );
        assert_eq!(
            byte_digest(&Profile::AgentsMdV1.canonical_descriptor_bytes().unwrap()),
            "e6485c07fa7134403582747e4930c6330b9db94aee094195c26621791475b8e2"
        );
        assert_eq!(
            byte_digest(&Profile::ClaudeMdV1.canonical_descriptor_bytes().unwrap()),
            "f9f8cc107e092ad8c62f4cbee0bdd1ea041002674d44a152bf600a7c9b5753a3"
        );
    }

    #[test]
    fn exact_bytes_and_review_change_only_candidate_evidence() {
        let (capability_id, mut value) = revision("\u{feff}Rule\r\n😀\n");
        let first = compile(&capability_id, &value, Profile::AgentsMdV1, &[]).unwrap();
        assert_eq!(first.artifact.content, value.content);
        assert_eq!(first.artifact.byte_length, value.content.len() as u64);
        assert_eq!(first.readiness, Readiness::ReviewRequired);
        value.review = Some(ContentReview {
            reviewer: Reviewer::LocalOperator,
            reviewed_at_ms: 1,
        });
        let reviewed = compile(&capability_id, &value, Profile::AgentsMdV1, &[]).unwrap();
        assert_eq!(first.compilation_id, reviewed.compilation_id);
        assert_eq!(reviewed.readiness, Readiness::Candidate);
        assert_ne!(first.candidate, reviewed.candidate);
        assert_eq!(
            reviewed
                .candidate
                .as_ref()
                .unwrap()
                .manifest
                .artifact
                .sha256,
            value.sha256
        );
    }

    #[test]
    fn diagnostics_are_fixed_order_and_static_rules_block_candidates() {
        let (capability_id, mut value) = revision("mail@example.test <!-- note -->");
        value.review = Some(ContentReview {
            reviewer: Reviewer::LocalOperator,
            reviewed_at_ms: 1,
        });
        let report = compile(
            &capability_id,
            &value,
            Profile::ClaudeMdV1,
            &["network".into()],
        )
        .unwrap();
        assert_eq!(report.readiness, Readiness::Blocked);
        assert_eq!(report.candidate, None);
        assert_eq!(
            report
                .diagnostics
                .iter()
                .map(|item| item.code)
                .collect::<Vec<_>>(),
            vec![
                DiagnosticCode::InstructionSemanticsUnverified,
                DiagnosticCode::TargetEnvironmentUnqualified,
                DiagnosticCode::UnsupportedRequirement,
                DiagnosticCode::PossibleIncludeSyntax,
                DiagnosticCode::PossibleCommentElision
            ]
        );
        assert!(
            DiagnosticCode::PossibleIncludeSyntax
                .explanation()
                .contains("emails")
        );
    }

    #[test]
    fn rejects_selection_and_requirement_boundaries() {
        let (capability_id, value) = revision("x");
        for requirements in [
            vec!["UPPER".into()],
            vec!["a".repeat(65)],
            vec!["same".into(), "same".into()],
            vec!["x".to_string(); 33],
        ] {
            assert_eq!(
                compile(&capability_id, &value, Profile::AgentsMdV1, &requirements),
                Err(CompileError::InvalidRequirements)
            );
        }
        let mut bad = value.clone();
        bad.sha256 = "0".repeat(64);
        assert_eq!(
            compile(&capability_id, &bad, Profile::AgentsMdV1, &[]),
            Err(CompileError::InvalidSelection)
        );
        let mut bad = value.clone();
        bad.created_at_ms = -1;
        assert_eq!(
            compile(&capability_id, &bad, Profile::AgentsMdV1, &[]),
            Err(CompileError::InvalidSelection)
        );
        let mut bad = value.clone();
        bad.created_at_ms = MAX_TIMESTAMP_MS + 1;
        assert_eq!(
            compile(&capability_id, &bad, Profile::AgentsMdV1, &[]),
            Err(CompileError::InvalidSelection)
        );
        let mut bad = value.clone();
        bad.title = "x".repeat(161);
        assert_eq!(
            compile(&capability_id, &bad, Profile::AgentsMdV1, &[]),
            Err(CompileError::InvalidSelection)
        );
        let mut bad = value.clone();
        bad.id = id("revision", 'f');
        assert_eq!(
            compile(&capability_id, &bad, Profile::AgentsMdV1, &[]),
            Err(CompileError::InvalidSelection)
        );
        let mut bad = value.clone();
        bad.parent_revision_id = Some("revision:not-a-digest".into());
        assert_eq!(
            compile(&capability_id, &bad, Profile::AgentsMdV1, &[]),
            Err(CompileError::InvalidSelection)
        );
        assert_eq!(
            compile("capability:not-a-digest", &value, Profile::AgentsMdV1, &[]),
            Err(CompileError::InvalidSelection)
        );
    }

    #[test]
    fn title_and_content_bounds_are_applied_to_compiler_inputs() {
        let (capability_id, value) = revision("x");
        for title in [
            " x".to_string(),
            "x\n".to_string(),
            "x\u{0001}".to_string(),
            "x".repeat(161),
        ] {
            let mut invalid = value.clone();
            invalid.title = title;
            assert_eq!(
                compile(&capability_id, &invalid, Profile::AgentsMdV1, &[]),
                Err(CompileError::InvalidSelection)
            );
        }
        let title = "t".repeat(160);
        let content = "x".repeat(capability::MAX_CONTENT_BYTES);
        let valid = Revision {
            id: capability::revision_id(&capability_id, None, &title, &content),
            parent_revision_id: None,
            title,
            sha256: byte_digest(content.as_bytes()),
            content,
            created_at_ms: 0,
            review: None,
            provenance: RevisionProvenance::Ordinary {},
        };
        assert!(compile(&capability_id, &valid, Profile::AgentsMdV1, &[]).is_ok());
        for content in [
            " \r\n".to_string(),
            "bad\0content".to_string(),
            "x".repeat(capability::MAX_CONTENT_BYTES + 1),
        ] {
            let mut invalid = value.clone();
            invalid.content = content;
            assert_eq!(
                compile(&capability_id, &invalid, Profile::AgentsMdV1, &[]),
                Err(CompileError::InvalidSelection)
            );
        }
    }

    #[test]
    fn review_times_and_composition_reference_shape_are_bounded_without_clock_ordering() {
        let (capability_id, mut value) = revision("x");
        value.created_at_ms = 100;
        value.review = Some(ContentReview {
            reviewer: Reviewer::LocalOperator,
            reviewed_at_ms: 5,
        });
        assert_eq!(
            compile(&capability_id, &value, Profile::AgentsMdV1, &[])
                .unwrap()
                .readiness,
            Readiness::Candidate
        );
        for reviewed_at_ms in [-1, MAX_TIMESTAMP_MS + 1] {
            let mut invalid = value.clone();
            invalid.review.as_mut().unwrap().reviewed_at_ms = reviewed_at_ms;
            assert_eq!(
                compile(&capability_id, &invalid, Profile::AgentsMdV1, &[]),
                Err(CompileError::InvalidSelection)
            );
        }
        value.provenance = RevisionProvenance::Composition {
            application_id: "composition-application:not-a-digest".into(),
            composition_id: id("composition", 'c'),
            output_index: 0,
        };
        assert_eq!(
            compile(&capability_id, &value, Profile::AgentsMdV1, &[]),
            Err(CompileError::InvalidSelection)
        );
        value.provenance = RevisionProvenance::Composition {
            application_id: id("composition-application", 'b'),
            composition_id: "composition:not-a-digest".into(),
            output_index: 0,
        };
        assert_eq!(
            compile(&capability_id, &value, Profile::AgentsMdV1, &[]),
            Err(CompileError::InvalidSelection)
        );
    }

    #[test]
    fn requirement_boundaries_and_order_bind_static_identity() {
        let (capability_id, value) = revision("x");
        let requirements: Vec<_> = (0..32).map(|index| format!("req{index}")).collect();
        assert!(compile(&capability_id, &value, Profile::AgentsMdV1, &requirements).is_ok());
        assert!(
            compile(
                &capability_id,
                &value,
                Profile::AgentsMdV1,
                &["a".repeat(64)]
            )
            .is_ok()
        );
        for requirements in [
            vec![String::new()],
            vec!["ümlaut".into()],
            (0..33).map(|index| format!("req{index}")).collect(),
        ] {
            assert_eq!(
                compile(&capability_id, &value, Profile::AgentsMdV1, &requirements),
                Err(CompileError::InvalidRequirements)
            );
        }
        let forward = compile(
            &capability_id,
            &value,
            Profile::AgentsMdV1,
            &["alpha".into(), "beta".into()],
        )
        .unwrap();
        let repeat = compile(
            &capability_id,
            &value,
            Profile::AgentsMdV1,
            &["alpha".into(), "beta".into()],
        )
        .unwrap();
        let reversed = compile(
            &capability_id,
            &value,
            Profile::AgentsMdV1,
            &["beta".into(), "alpha".into()],
        )
        .unwrap();
        assert_eq!(forward.compilation_id, repeat.compilation_id);
        assert_ne!(forward.compilation_id, reversed.compilation_id);
    }

    #[test]
    fn composition_reference_shape_accepts_fifteen_rejects_sixteen() {
        let (capability_id, mut value) = revision("x");
        value.provenance = RevisionProvenance::Composition {
            application_id: id("composition-application", 'b'),
            composition_id: id("composition", 'c'),
            output_index: 15,
        };
        assert!(compile(&capability_id, &value, Profile::AgentsMdV1, &[]).is_ok());
        if let RevisionProvenance::Composition { output_index, .. } = &mut value.provenance {
            *output_index = 16;
        }
        assert_eq!(
            compile(&capability_id, &value, Profile::AgentsMdV1, &[]),
            Err(CompileError::InvalidSelection)
        );
    }

    #[test]
    fn golden_vectors_match_full_values_and_canonical_report_bytes() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/compilation/instruction-v1.json"
        ))
        .unwrap();
        assert_eq!(fixture["schemaVersion"], "rangoon.compilation-vectors.v1");
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 10);
        for case in cases {
            let revision: Revision = serde_json::from_value(case["revision"].clone()).unwrap();
            let profile: Profile = serde_json::from_value(case["profile"].clone()).unwrap();
            let requirements: Vec<String> =
                serde_json::from_value(case["requirements"].clone()).unwrap();
            let report = compile(
                case["capabilityId"].as_str().unwrap(),
                &revision,
                profile,
                &requirements,
            )
            .unwrap();
            assert_eq!(
                serde_json::to_value(&report).unwrap(),
                case["expected"],
                "{}",
                case["name"]
            );
            assert_eq!(
                serde_json::to_string(&report).unwrap(),
                case["expectedCanonicalJson"].as_str().unwrap(),
                "{}",
                case["name"]
            );
        }
    }

    #[test]
    fn profile_is_closed() {
        assert!(serde_json::from_str::<Profile>(r#""unknown_profile""#).is_err());
    }
}
