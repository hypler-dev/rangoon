//! Experimental source-analysis records. These records cannot authorize actions.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub mod capability;

pub const SCHEMA_VERSION: &str = "rangoon.source-analysis.v0";
pub const ANALYZER_VERSION: &str = "0.1.0";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisReport {
    pub schema_version: String,
    pub analyzer_version: String,
    pub source: SourceSnapshot,
    pub fragments: Vec<SourceFragment>,
    pub diagnostics: Vec<Diagnostic>,
    pub authority: Authority,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceSnapshot {
    pub id: String,
    pub display_name: String,
    pub format: SourceFormat,
    pub sha256: String,
    pub byte_length: u64,
    pub line_count: u32,
    /// Original UTF-8 content. No newline, Unicode, or BOM normalization occurs.
    pub content: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    AgentsMarkdown,
    ClaudeMarkdown,
    SkillMarkdown,
    GenericMarkdown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceFragment {
    pub id: String,
    pub kind: FragmentKind,
    pub heading: Option<Heading>,
    pub span: SourceSpan,
    pub text: String,
    pub review_state: ReviewState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FragmentKind {
    Preamble,
    Section,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Heading {
    pub level: u8,
    pub title: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceSpan {
    /// Zero-based inclusive offset in the original UTF-8 byte sequence.
    pub start_byte: u64,
    /// Zero-based exclusive offset in the original UTF-8 byte sequence.
    pub end_byte: u64,
    /// One-based lines. A final newline does not add a phantom empty line.
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Unreviewed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Authority {
    None,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub message: String,
    pub line: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    EmptySource,
    MarkdownSubset,
    UnclosedFence,
}

/// Original-byte identity, distinct from any future semantic digest.
pub fn byte_digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Versioned, length-framed source identity binds the display name and bytes.
/// This is an application identity, not an authority token or signed statement.
pub fn source_id(display_name: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rangoon.source.v0\0");
    hasher.update((display_name.len() as u64).to_be_bytes());
    hasher.update(display_name.as_bytes());
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
    format!("source:{}", hex(&hasher.finalize()))
}

pub fn fragment_id(source_id: &str, span: SourceSpan) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rangoon.fragment.v0\0");
    hasher.update((source_id.len() as u64).to_be_bytes());
    hasher.update(source_id.as_bytes());
    hasher.update(span.start_byte.to_be_bytes());
    hasher.update(span.end_byte.to_be_bytes());
    format!("fragment:{}", hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 15) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_byte_digest_matches_sha256_reference() {
        assert_eq!(
            byte_digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_ne!(byte_digest(b"a\n"), byte_digest(b"a\r\n"));
    }

    #[test]
    fn source_identity_binds_name_and_exact_bytes() {
        assert_eq!(
            source_id("AGENTS.md", b"abc"),
            source_id("AGENTS.md", b"abc")
        );
        assert_ne!(
            source_id("AGENTS.md", b"abc"),
            source_id("CLAUDE.md", b"abc")
        );
        assert_ne!(
            source_id("AGENTS.md", b"abc"),
            source_id("AGENTS.md", b"abd")
        );
        assert_ne!(source_id("a", b"bc"), source_id("ab", b"c"));
    }

    #[test]
    fn data_records_reject_authority_and_review_promotion() {
        assert!(serde_json::from_str::<Authority>("\"granted\"").is_err());
        assert!(serde_json::from_str::<ReviewState>("\"approved\"").is_err());
        let span = r#"{"startByte":0,"endByte":1,"startLine":1,"endLine":1,"execute":true}"#;
        assert!(serde_json::from_str::<SourceSpan>(span).is_err());
    }
}
