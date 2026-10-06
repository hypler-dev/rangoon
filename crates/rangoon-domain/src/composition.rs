//! Closed composition references shared by pure recipe and storage contracts.

use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_wire_is_closed_and_uses_existing_field_names() {
        let source: InputReference =
            serde_json::from_str(r#"{"kind":"source","sourceId":"source:a","sha256":"b"}"#)
                .unwrap();
        assert_eq!(
            serde_json::to_string(&source).unwrap(),
            r#"{"kind":"source","sourceId":"source:a","sha256":"b"}"#
        );
        assert!(
            serde_json::from_str::<InputReference>(
                r#"{"kind":"source","sourceId":"source:a","sha256":"b","extra":true}"#
            )
            .is_err()
        );
        assert!(serde_json::from_str::<Operation>(r#""unknown""#).is_err());
    }
}
