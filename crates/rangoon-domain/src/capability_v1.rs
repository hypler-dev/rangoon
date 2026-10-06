//! Versioned capability DTOs with explicit birth and revision provenance.

use crate::{
    Authority, SourceSpan,
    capability::{self as legacy, ContentReview},
    composition::{InputReference, Operation},
};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "rangoon.capability.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Origin {
    Source {
        source_id: String,
        fragment_id: String,
        source_name: String,
        span: SourceSpan,
        original_text: String,
    },
    Composition {
        composition_id: String,
        operation: Operation,
        output_index: u32,
        inputs: Vec<InputReference>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum OriginSummary {
    Source {
        source_id: String,
        fragment_id: String,
    },
    Composition {
        composition_id: String,
        operation: Operation,
        output_index: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RevisionProvenance {
    Ordinary {},
    Composition {
        application_id: String,
        composition_id: String,
        output_index: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Revision {
    pub id: String,
    pub parent_revision_id: Option<String>,
    pub title: String,
    pub content: String,
    pub sha256: String,
    pub created_at_ms: i64,
    pub review: Option<ContentReview>,
    pub provenance: RevisionProvenance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevisionSummary {
    pub id: String,
    pub parent_revision_id: Option<String>,
    pub title: String,
    pub sha256: String,
    pub created_at_ms: i64,
    pub review: Option<ContentReview>,
    pub provenance: RevisionProvenance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityDetail {
    pub schema_version: String,
    pub id: String,
    pub origin: Origin,
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
    pub origin: OriginSummary,
    pub latest_revision_id: String,
    pub title: String,
    pub reviewed: bool,
    pub revision_count: u32,
}

impl<'de> Deserialize<'de> for Revision {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Wire {
            id: String,
            parent_revision_id: RequiredNullableString,
            title: String,
            content: String,
            sha256: String,
            created_at_ms: i64,
            review: RequiredNullableReview,
            provenance: RevisionProvenance,
        }
        let value = Wire::deserialize(deserializer)?;
        Ok(Self {
            id: value.id,
            parent_revision_id: value.parent_revision_id.0,
            title: value.title,
            content: value.content,
            sha256: value.sha256,
            created_at_ms: value.created_at_ms,
            review: value.review.0,
            provenance: value.provenance,
        })
    }
}

impl<'de> Deserialize<'de> for RevisionSummary {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Wire {
            id: String,
            parent_revision_id: RequiredNullableString,
            title: String,
            sha256: String,
            created_at_ms: i64,
            review: RequiredNullableReview,
            provenance: RevisionProvenance,
        }
        let value = Wire::deserialize(deserializer)?;
        Ok(Self {
            id: value.id,
            parent_revision_id: value.parent_revision_id.0,
            title: value.title,
            sha256: value.sha256,
            created_at_ms: value.created_at_ms,
            review: value.review.0,
            provenance: value.provenance,
        })
    }
}

struct RequiredNullableString(Option<String>);
struct RequiredNullableReview(Option<ContentReview>);

impl<'de> Deserialize<'de> for RequiredNullableString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
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
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RequiredNullableString(Some(value.into())))
            }
            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RequiredNullableString(Some(value)))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

impl<'de> Deserialize<'de> for RequiredNullableReview {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = RequiredNullableReview;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a required content review or null")
            }
            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RequiredNullableReview(None))
            }
            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RequiredNullableReview(None))
            }
            fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                ContentReview::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                    .map(|review| RequiredNullableReview(Some(review)))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

impl From<legacy::Revision> for Revision {
    fn from(value: legacy::Revision) -> Self {
        Self {
            id: value.id,
            parent_revision_id: value.parent_revision_id,
            title: value.title,
            content: value.content,
            sha256: value.sha256,
            created_at_ms: value.created_at_ms,
            review: value.review,
            provenance: RevisionProvenance::Ordinary {},
        }
    }
}

impl From<legacy::RevisionSummary> for RevisionSummary {
    fn from(value: legacy::RevisionSummary) -> Self {
        Self {
            id: value.id,
            parent_revision_id: value.parent_revision_id,
            title: value.title,
            sha256: value.sha256,
            created_at_ms: value.created_at_ms,
            review: value.review,
            provenance: RevisionProvenance::Ordinary {},
        }
    }
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
            provenance: value.provenance.clone(),
        }
    }
}

impl From<legacy::CapabilityDetail> for CapabilityDetail {
    fn from(value: legacy::CapabilityDetail) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.into(),
            id: value.id,
            origin: Origin::Source {
                source_id: value.source_id,
                fragment_id: value.fragment_id,
                source_name: value.source_name,
                span: value.span,
                original_text: value.original_text,
            },
            latest_revision_id: value.latest_revision_id,
            revision: value.revision.into(),
            history: value.history.into_iter().map(Into::into).collect(),
            authority: value.authority,
        }
    }
}

impl From<legacy::CapabilitySummary> for CapabilitySummary {
    fn from(value: legacy::CapabilitySummary) -> Self {
        Self {
            id: value.id,
            origin: OriginSummary::Source {
                source_id: value.source_id,
                fragment_id: value.fragment_id,
            },
            latest_revision_id: value.latest_revision_id,
            title: value.title,
            reviewed: value.reviewed,
            revision_count: value.revision_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{
        CapabilityDetail as LegacyDetail, CapabilitySummary as LegacySummary, Reviewer,
        Revision as LegacyRevision, RevisionSummary as LegacyRevisionSummary,
    };

    fn provenance() -> RevisionProvenance {
        RevisionProvenance::Ordinary {}
    }
    fn revision() -> Revision {
        Revision {
            id: "revision:a".into(),
            parent_revision_id: None,
            title: "Rules".into(),
            content: "\u{feff}exact\r\n".into(),
            sha256: "a".repeat(64),
            created_at_ms: 1,
            review: None,
            provenance: provenance(),
        }
    }

    #[test]
    fn v1_wire_is_closed_and_requires_nullable_and_provenance_fields() {
        let encoded = serde_json::to_string(&revision()).unwrap();
        assert!(serde_json::from_str::<Revision>(&encoded).is_ok());
        for field in ["provenance", "parentRevisionId", "review"] {
            let mut missing: serde_json::Value = serde_json::from_str(&encoded).unwrap();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<Revision>(missing).is_err());
        }
        let summary = RevisionSummary::from(&revision());
        for field in ["provenance", "parentRevisionId", "review"] {
            let mut missing = serde_json::to_value(&summary).unwrap();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<RevisionSummary>(missing).is_err());
        }
        assert!(
            serde_json::from_str::<RevisionProvenance>(r#"{"kind":"ordinary","extra":true}"#)
                .is_err()
        );
        assert!(serde_json::from_str::<Authority>(r#""granted""#).is_err());
    }

    #[test]
    fn projection_preserves_legacy_bytes_ids_and_reviews() {
        let review = ContentReview {
            reviewer: Reviewer::LocalOperator,
            reviewed_at_ms: 9,
        };
        let old_revision = LegacyRevision {
            id: "revision:root".into(),
            parent_revision_id: None,
            title: "Rules".into(),
            content: "\u{feff}a\r\n".into(),
            sha256: "b".repeat(64),
            created_at_ms: 3,
            review: Some(review.clone()),
        };
        let old_summary = LegacyRevisionSummary::from(&old_revision);
        let old = LegacyDetail {
            schema_version: legacy::SCHEMA_VERSION.into(),
            id: "capability:root".into(),
            source_id: "source:x".into(),
            fragment_id: "fragment:y".into(),
            source_name: "AGENTS.md".into(),
            span: SourceSpan {
                start_byte: 0,
                end_byte: 6,
                start_line: 1,
                end_line: 1,
            },
            original_text: "\u{feff}a\r\n".into(),
            latest_revision_id: old_revision.id.clone(),
            revision: old_revision,
            history: vec![old_summary],
            authority: Authority::None,
        };
        let projected: CapabilityDetail = old.into();
        assert_eq!(projected.schema_version, SCHEMA_VERSION);
        assert_eq!(projected.revision.content, "\u{feff}a\r\n");
        assert_eq!(projected.revision.review, Some(review));
        assert!(matches!(projected.origin, Origin::Source { .. }));
        assert!(matches!(
            projected.revision.provenance,
            RevisionProvenance::Ordinary {}
        ));
        assert_eq!(
            RevisionSummary::from(&projected.revision).provenance,
            RevisionProvenance::Ordinary {}
        );
    }

    #[test]
    fn summary_projection_is_source_origin_without_authority_field() {
        let old = LegacySummary {
            id: "capability:root".into(),
            source_id: "source:x".into(),
            fragment_id: "fragment:y".into(),
            latest_revision_id: "revision:z".into(),
            title: "Rules".into(),
            reviewed: false,
            revision_count: 1,
        };
        let projected: CapabilitySummary = old.into();
        assert!(matches!(projected.origin, OriginSummary::Source { .. }));
        assert!(serde_json::from_str::<Origin>(r#"{"kind":"unknown"}"#).is_err());
    }
}
