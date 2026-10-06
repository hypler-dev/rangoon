//! Closed decoder for source-free cloud model metadata.

use crate::{Diagnostic, json::preflight};
use serde::Deserialize;

const RAW_LIMIT: usize = 65_536;
const MAX_TEXT_BYTES: usize = 128;

/// Provider-reported metadata for the explicitly requested model.
///
/// This metadata remains untrusted and grants no compatibility or authority.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Metadata {
    pub(crate) id: String,
    pub(crate) owned_by: String,
    pub(crate) created: u64,
    pub(crate) shutdown_date: Option<String>,
}

#[derive(Deserialize)]
struct Envelope {
    object: String,
    id: String,
    owned_by: String,
    created: u64,
    #[serde(default)]
    shutdown_date: Option<String>,
}

/// Decode one bounded model-metadata response for the selected model identity.
pub(crate) fn response(raw: &[u8], expected_model: &str) -> Result<Metadata, Diagnostic> {
    if raw.len() > RAW_LIMIT {
        return Err(Diagnostic::ResponseTooLarge);
    }
    preflight(raw)?;
    let envelope: Envelope =
        serde_json::from_slice(raw).map_err(|_| Diagnostic::ResponseInvalid)?;
    if envelope.object != "model"
        || envelope.id != expected_model
        || !printable(&envelope.id)
        || !printable(&envelope.owned_by)
        || envelope
            .shutdown_date
            .as_deref()
            .is_some_and(|date| !valid_date(date))
    {
        return Err(Diagnostic::ResponseInvalid);
    }
    Ok(Metadata {
        id: envelope.id,
        owned_by: envelope.owned_by,
        created: envelope.created,
        shutdown_date: envelope.shutdown_date,
    })
}

fn printable(value: &str) -> bool {
    (1..=MAX_TEXT_BYTES).contains(&value.len())
        && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

fn valid_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let Some(year) = digits(&bytes[0..4]) else {
        return false;
    };
    let Some(month) = digits(&bytes[5..7]) else {
        return false;
    };
    let Some(day) = digits(&bytes[8..10]) else {
        return false;
    };
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return false;
    }
    let maximum_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year(year) => 29,
        2 => 28,
        _ => return false,
    };
    (1..=maximum_day).contains(&day)
}

fn digits(bytes: &[u8]) -> Option<u16> {
    bytes.iter().try_fold(0u16, |value, byte| {
        byte.is_ascii_digit()
            .then(|| value * 10 + u16::from(byte - b'0'))
    })
}

fn leap_year(year: u16) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid(extra: &str) -> String {
        format!(r#"{{"object":"model","id":"gpt-fixture","owned_by":"system","created":1{extra}}}"#)
    }

    fn with_owner(owner: &str) -> String {
        format!(r#"{{"object":"model","id":"gpt-fixture","owned_by":"{owner}","created":1}}"#)
    }

    fn with_id(id: &str) -> String {
        format!(r#"{{"object":"model","id":"{id}","owned_by":"system","created":1}}"#)
    }

    #[test]
    fn decodes_expected_model_and_ignores_bounded_top_metadata() {
        let decoded = response(
            valid(",\"shutdown_date\":\"2024-02-29\",\"provider\":{\"trace\":true}").as_bytes(),
            "gpt-fixture",
        )
        .unwrap();
        assert_eq!(
            decoded,
            Metadata {
                id: "gpt-fixture".to_owned(),
                owned_by: "system".to_owned(),
                created: 1,
                shutdown_date: Some("2024-02-29".to_owned()),
            }
        );
        assert_eq!(
            response(valid(",\"shutdown_date\":null").as_bytes(), "gpt-fixture")
                .unwrap()
                .shutdown_date,
            None
        );
    }

    #[test]
    fn rejects_missing_bad_typed_duplicate_and_mismatched_identity_fields() {
        for raw in [
            br#"{"id":"gpt-fixture","owned_by":"system","created":1}"#.as_slice(),
            br#"{"object":"model","id":"gpt-fixture","owned_by":"system"}"#.as_slice(),
            br#"{"object":"model","id":"gpt-fixture","owned_by":"system","created":-1}"#.as_slice(),
            br#"{"object":"model","id":"gpt-fixture","owned_by":"system","created":"1"}"#.as_slice(),
            br#"{"object":"model","id":"gpt-fixture","owned_by":"system","created":1,"shutdown_date":1}"#.as_slice(),
            br#"{"object":"model","id":"gpt-fixture","\u0069d":"other","owned_by":"system","created":1}"#.as_slice(),
            br#"{"object":"model","id":"other","owned_by":"system","created":1}"#.as_slice(),
            br#"{"object":"response","id":"gpt-fixture","owned_by":"system","created":1}"#.as_slice(),
        ] {
            assert_eq!(response(raw, "gpt-fixture"), Err(Diagnostic::ResponseInvalid));
        }
    }

    #[test]
    fn enforces_graphic_owner_and_identity_bounds() {
        let exact_owner = with_owner(&"x".repeat(128));
        assert!(response(exact_owner.as_bytes(), "gpt-fixture").is_ok());
        let excess_owner = with_owner(&"x".repeat(129));
        assert_eq!(
            response(excess_owner.as_bytes(), "gpt-fixture"),
            Err(Diagnostic::ResponseInvalid)
        );
        for raw in [
            with_owner(""),
            with_owner("has space"),
            with_id(&"g".repeat(129)),
        ] {
            let expected = if raw.contains(&"g".repeat(129)) {
                "g".repeat(129)
            } else {
                "gpt-fixture".to_owned()
            };
            assert_eq!(
                response(raw.as_bytes(), &expected),
                Err(Diagnostic::ResponseInvalid)
            );
        }
    }

    #[test]
    fn validates_real_gregorian_shutdown_dates() {
        for date in [
            "0000-01-01",
            "1900-02-29",
            "2023-02-29",
            "2024-13-01",
            "2024-04-31",
            "2024-2-29",
        ] {
            assert_eq!(
                response(
                    valid(&format!(",\"shutdown_date\":\"{date}\"")).as_bytes(),
                    "gpt-fixture"
                ),
                Err(Diagnostic::ResponseInvalid)
            );
        }
        for date in ["0001-01-01", "2000-02-29", "2024-02-29"] {
            assert!(
                response(
                    valid(&format!(",\"shutdown_date\":\"{date}\"")).as_bytes(),
                    "gpt-fixture"
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn enforces_preflight_depth_and_check_raw_limit() {
        let mut nested = "null".to_owned();
        for _ in 0..16 {
            nested = format!(r#"{{"next":{nested}}}"#);
        }
        assert_eq!(
            response(
                valid(&format!(",\"metadata\":{nested}")).as_bytes(),
                "gpt-fixture"
            ),
            Err(Diagnostic::ResponseTooLarge)
        );
        let mut exact = valid("").into_bytes();
        exact.resize(RAW_LIMIT, b' ');
        assert!(response(&exact, "gpt-fixture").is_ok());
        exact.push(b' ');
        assert_eq!(
            response(&exact, "gpt-fixture"),
            Err(Diagnostic::ResponseTooLarge)
        );
    }
}
