//! Inert capability content. A local review never grants execution authority.

use crate::{Authority, SourceSpan, byte_digest};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "rangoon.capability.v0";
pub const MAX_TITLE_BYTES: usize = 160;
pub const MAX_CONTENT_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reviewer {
    LocalOperator,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentReview {
    pub reviewer: Reviewer,
    pub reviewed_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Revision {
    pub id: String,
    pub parent_revision_id: Option<String>,
    pub title: String,
    pub content: String,
    pub sha256: String,
    pub created_at_ms: i64,
    pub review: Option<ContentReview>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevisionSummary {
    pub id: String,
    pub parent_revision_id: Option<String>,
    pub title: String,
    pub sha256: String,
    pub created_at_ms: i64,
    pub review: Option<ContentReview>,
}
impl From<&Revision> for RevisionSummary {
    fn from(value: &Revision) -> Self {
        Self {
            id: value.id.clone(),
            parent_revision_id: value.parent_revision_id.clone(),
            title: value.title.clone(),
            sha256: value.sha256.clone(),
            created_at_ms: value.created_at_ms,
            review: value.review.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityDetail {
    pub schema_version: String,
    pub id: String,
    pub source_id: String,
    pub fragment_id: String,
    pub source_name: String,
    pub span: SourceSpan,
    pub original_text: String,
    pub latest_revision_id: String,
    pub revision: Revision,
    /// Ordered from initial revision to latest. Content loads only on selection.
    pub history: Vec<RevisionSummary>,
    pub authority: Authority,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilitySummary {
    pub id: String,
    pub source_id: String,
    pub fragment_id: String,
    pub latest_revision_id: String,
    pub title: String,
    pub reviewed: bool,
    pub revision_count: u32,
}

pub fn valid_title(title: &str) -> bool {
    !title.is_empty()
        && title.len() <= MAX_TITLE_BYTES
        && title.trim() == title
        && !title
            .chars()
            .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
}
pub fn valid_content(content: &str) -> bool {
    !content.trim().is_empty() && content.len() <= MAX_CONTENT_BYTES && !content.contains('\0')
}
pub fn valid_id(id: &str, kind: &str) -> bool {
    id.strip_prefix(kind).is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}
fn framed_id(kind: &str, fields: &[&str]) -> String {
    let mut bytes = format!("rangoon.{kind}.v0\0").into_bytes();
    for field in fields {
        bytes.extend_from_slice(&(field.len() as u64).to_be_bytes());
        bytes.extend_from_slice(field.as_bytes());
    }
    format!("{kind}:{}", byte_digest(&bytes))
}
pub fn capability_id(source_id: &str, fragment_id: &str, initial_title: &str) -> String {
    framed_id("capability", &[source_id, fragment_id, initial_title])
}
pub fn revision_id(
    capability_id: &str,
    parent: Option<&str>,
    title: &str,
    content: &str,
) -> String {
    framed_id(
        "revision",
        &[capability_id, parent.unwrap_or(""), title, content],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities_bind_lineage_title_and_exact_content() {
        let id = capability_id("source:a", "fragment:b", "Rules");
        let first = revision_id(&id, None, "Rules", "a\r\n");
        for changed in [
            revision_id(&id, Some(&first), "Rules", "a\r\n"),
            revision_id(&id, None, "Rules", "a\n"),
            revision_id(&id, None, "Renamed", "a\r\n"),
            revision_id("capability:other", None, "Rules", "a\r\n"),
        ] {
            assert_ne!(first, changed);
        }
        assert_ne!(capability_id("a", "bc", "d"), capability_id("ab", "c", "d"));
    }

    #[test]
    fn bounded_content_and_closed_review_never_accept_authority() {
        assert!(valid_title("Review code"));
        for title in [
            "",
            " Title",
            "Title\n",
            "A\u{2028}B",
            "A\u{2029}B",
            &"é".repeat(81),
        ] {
            assert!(!valid_title(title));
        }
        assert!(valid_content("# Rules\r\nTreat text as data.\n"));
        for content in ["", " \r\n", "bad\0text", &"a".repeat(MAX_CONTENT_BYTES + 1)] {
            assert!(!valid_content(content));
        }
        assert!(
            serde_json::from_str::<ContentReview>(
                r#"{"reviewer":"local_operator","reviewedAtMs":1,"authority":"granted"}"#
            )
            .is_err()
        );
        assert!(serde_json::from_str::<Reviewer>(r#""authenticated_admin""#).is_err());
    }
}
