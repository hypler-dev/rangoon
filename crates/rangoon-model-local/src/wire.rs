//! Closed, bounded decoders for untrusted loopback responses.

use crate::Diagnostic;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;

const VERSION_RAW_LIMIT: usize = 1_024;
const CHAT_RAW_LIMIT: usize = 1_048_576;
const MAX_DEPTH: usize = 4;
const MAX_KEYS: usize = 32;
const MAX_VALUES: usize = 128;
const MAX_KEY_BYTES: usize = 64;
const MAX_STRING_BYTES: usize = 128;
const MAX_CONTENT_BYTES: usize = 131_072;

/// Completed chat envelope. Its content is still untrusted model-authored data.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Chat {
    pub(crate) model: String,
    pub(crate) created_at: String,
    pub(crate) content: String,
    pub(crate) usage: Usage,
}

/// Optional, server-reported metrics. Absence remains unknown.
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Usage {
    pub(crate) total_duration: Option<u64>,
    pub(crate) load_duration: Option<u64>,
    pub(crate) prompt_eval_count: Option<u64>,
    pub(crate) prompt_eval_cached_count: Option<u64>,
    pub(crate) prompt_eval_duration: Option<u64>,
    pub(crate) eval_count: Option<u64>,
    pub(crate) eval_duration: Option<u64>,
}

/// Decode the exact closed `/api/version` response schema.
pub(crate) fn version(raw: &[u8]) -> Result<String, Diagnostic> {
    preflight(raw, VERSION_RAW_LIMIT)?;
    let response: VersionResponse =
        serde_json::from_slice(raw).map_err(|_| Diagnostic::ResponseInvalid)?;
    if response.version.len() > 64 {
        return Err(Diagnostic::ResponseTooLarge);
    }
    if !printable_nonblank(&response.version, 64) {
        return Err(Diagnostic::ResponseInvalid);
    }
    Ok(response.version)
}

/// Decode the exact closed completed `/api/chat` response schema.
pub(crate) fn chat(
    raw: &[u8],
    expected_model: &str,
    max_output_tokens: u64,
) -> Result<Chat, Diagnostic> {
    preflight(raw, CHAT_RAW_LIMIT)?;
    let response: ChatResponse =
        serde_json::from_slice(raw).map_err(|_| Diagnostic::ResponseInvalid)?;
    if !response.done || response.done_reason != "stop" {
        return Err(Diagnostic::ResponseIncomplete);
    }
    if response.model != expected_model
        || !printable_nonblank(&response.created_at, 128)
        || response.message.role != "assistant"
        || response
            .message
            .thinking
            .as_deref()
            .is_some_and(|thinking| !thinking.is_empty())
        || response
            .eval_count
            .is_some_and(|count| count > max_output_tokens)
    {
        return Err(Diagnostic::ResponseInvalid);
    }
    Ok(Chat {
        model: response.model,
        created_at: response.created_at,
        content: response.message.content,
        usage: Usage {
            total_duration: response.total_duration,
            load_duration: response.load_duration,
            prompt_eval_count: response.prompt_eval_count,
            prompt_eval_cached_count: response.prompt_eval_cached_count,
            prompt_eval_duration: response.prompt_eval_duration,
            eval_count: response.eval_count,
            eval_duration: response.eval_duration,
        },
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionResponse {
    version: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChatResponse {
    model: String,
    created_at: String,
    message: Message,
    done: bool,
    done_reason: String,
    #[serde(default, deserialize_with = "optional_not_null")]
    total_duration: Option<u64>,
    #[serde(default, deserialize_with = "optional_not_null")]
    load_duration: Option<u64>,
    #[serde(default, deserialize_with = "optional_not_null")]
    prompt_eval_count: Option<u64>,
    #[serde(default, deserialize_with = "optional_not_null")]
    prompt_eval_cached_count: Option<u64>,
    #[serde(default, deserialize_with = "optional_not_null")]
    prompt_eval_duration: Option<u64>,
    #[serde(default, deserialize_with = "optional_not_null")]
    eval_count: Option<u64>,
    #[serde(default, deserialize_with = "optional_not_null")]
    eval_duration: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Message {
    role: String,
    content: String,
    #[serde(default, deserialize_with = "optional_not_null")]
    thinking: Option<String>,
}

/// `Option<T>` normally accepts JSON null. Missing fields use `default`; supplied
/// fields must deserialize as their concrete type.
fn optional_not_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

fn printable_nonblank(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
        && !value.bytes().all(|byte| byte == b' ')
}

/// Validate grammar and resource bounds before typed deserialization. Only decoded
/// object keys are allocated, so content is never allocated by this pass.
fn preflight(raw: &[u8], raw_limit: usize) -> Result<(), Diagnostic> {
    if raw.len() > raw_limit {
        return Err(Diagnostic::ResponseTooLarge);
    }
    std::str::from_utf8(raw).map_err(|_| Diagnostic::ResponseInvalid)?;
    let mut parser = Parser {
        raw,
        cursor: 0,
        values: 0,
    };
    parser.value(0, false)?;
    parser.space();
    if parser.cursor != raw.len() {
        return Err(Diagnostic::ResponseInvalid);
    }
    Ok(())
}

struct Parser<'a> {
    raw: &'a [u8],
    cursor: usize,
    values: usize,
}

impl Parser<'_> {
    fn invalid<T>(&self) -> Result<T, Diagnostic> {
        Err(Diagnostic::ResponseInvalid)
    }

    fn too_large<T>(&self) -> Result<T, Diagnostic> {
        Err(Diagnostic::ResponseTooLarge)
    }

    fn space(&mut self) {
        while self
            .raw
            .get(self.cursor)
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
        {
            self.cursor += 1;
        }
    }

    fn take(&mut self, expected: u8) -> Result<(), Diagnostic> {
        self.space();
        if self.raw.get(self.cursor) != Some(&expected) {
            return self.invalid();
        }
        self.cursor += 1;
        Ok(())
    }

    fn value(&mut self, depth: usize, content: bool) -> Result<(), Diagnostic> {
        self.space();
        self.values += 1;
        if self.values > MAX_VALUES {
            return self.too_large();
        }
        match self.raw.get(self.cursor) {
            Some(b'{') => {
                if depth >= MAX_DEPTH {
                    return self.too_large();
                }
                self.object(depth + 1)
            }
            Some(b'[') => self.invalid(),
            Some(b'"') => {
                self.string(if content {
                    MAX_CONTENT_BYTES
                } else {
                    MAX_STRING_BYTES
                })?;
                Ok(())
            }
            Some(b'0'..=b'9') => self.number(),
            Some(b't') => self.literal(b"true"),
            Some(b'f') => self.literal(b"false"),
            Some(b'n') => self.literal(b"null"),
            _ => self.invalid(),
        }
    }

    fn object(&mut self, depth: usize) -> Result<(), Diagnostic> {
        self.take(b'{')?;
        self.space();
        if self.raw.get(self.cursor) == Some(&b'}') {
            self.cursor += 1;
            return Ok(());
        }
        let mut keys = HashSet::new();
        loop {
            if keys.len() >= MAX_KEYS {
                return self.too_large();
            }
            self.space();
            let start = self.cursor;
            self.string(MAX_KEY_BYTES)?;
            let key: String = serde_json::from_slice(&self.raw[start..self.cursor])
                .map_err(|_| Diagnostic::ResponseInvalid)?;
            if !keys.insert(key.clone()) {
                return self.invalid();
            }
            self.take(b':')?;
            let content = depth == 1 && key == "message";
            if content {
                self.message_object(depth)?;
            } else {
                self.value(depth, false)?;
            }
            self.space();
            match self.raw.get(self.cursor) {
                Some(b'}') => {
                    self.cursor += 1;
                    return Ok(());
                }
                Some(b',') => self.cursor += 1,
                _ => return self.invalid(),
            }
        }
    }

    fn message_object(&mut self, depth: usize) -> Result<(), Diagnostic> {
        self.space();
        self.values += 1;
        if self.values > MAX_VALUES {
            return self.too_large();
        }
        if self.raw.get(self.cursor) != Some(&b'{') {
            return self.invalid();
        }
        if depth >= MAX_DEPTH {
            return self.too_large();
        }
        self.take(b'{')?;
        self.space();
        if self.raw.get(self.cursor) == Some(&b'}') {
            self.cursor += 1;
            return Ok(());
        }
        let mut keys = HashSet::new();
        loop {
            if keys.len() >= MAX_KEYS {
                return self.too_large();
            }
            self.space();
            let start = self.cursor;
            self.string(MAX_KEY_BYTES)?;
            let key: String = serde_json::from_slice(&self.raw[start..self.cursor])
                .map_err(|_| Diagnostic::ResponseInvalid)?;
            if !keys.insert(key.clone()) {
                return self.invalid();
            }
            self.take(b':')?;
            self.value(depth + 1, key == "content")?;
            self.space();
            match self.raw.get(self.cursor) {
                Some(b'}') => {
                    self.cursor += 1;
                    return Ok(());
                }
                Some(b',') => self.cursor += 1,
                _ => return self.invalid(),
            }
        }
    }

    /// Scan JSON string and count decoded UTF-8 bytes without allocating it.
    fn string(&mut self, limit: usize) -> Result<(), Diagnostic> {
        if self.raw.get(self.cursor) != Some(&b'"') {
            return self.invalid();
        }
        self.cursor += 1;
        let mut decoded = 0usize;
        loop {
            match self.raw.get(self.cursor).copied() {
                Some(b'"') => {
                    self.cursor += 1;
                    return Ok(());
                }
                Some(b'\\') => {
                    self.cursor += 1;
                    let width = match self.raw.get(self.cursor).copied() {
                        Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => {
                            self.cursor += 1;
                            1
                        }
                        Some(b'u') => {
                            self.cursor += 1;
                            let first = self.hex4()?;
                            let scalar = if (0xd800..=0xdbff).contains(&first) {
                                if self.raw.get(self.cursor..self.cursor + 2) != Some(b"\\u") {
                                    return self.invalid();
                                }
                                self.cursor += 2;
                                let second = self.hex4()?;
                                if !(0xdc00..=0xdfff).contains(&second) {
                                    return self.invalid();
                                }
                                0x1_0000 + ((u32::from(first) - 0xd800) << 10) + u32::from(second)
                                    - 0xdc00
                            } else if (0xdc00..=0xdfff).contains(&first) {
                                return self.invalid();
                            } else {
                                u32::from(first)
                            };
                            char::from_u32(scalar)
                                .ok_or(Diagnostic::ResponseInvalid)?
                                .len_utf8()
                        }
                        _ => return self.invalid(),
                    };
                    decoded = decoded
                        .checked_add(width)
                        .ok_or(Diagnostic::ResponseTooLarge)?;
                }
                Some(0..=31) | None => return self.invalid(),
                Some(byte) => {
                    let width = if byte < 0x80 {
                        1
                    } else {
                        // `preflight` already validated the entire buffer as UTF-8. Read
                        // only the leading byte here so a long Unicode string stays linear.
                        match byte {
                            0xc2..=0xdf => 2,
                            0xe0..=0xef => 3,
                            0xf0..=0xf4 => 4,
                            _ => return self.invalid(),
                        }
                    };
                    self.cursor += width;
                    decoded = decoded
                        .checked_add(width)
                        .ok_or(Diagnostic::ResponseTooLarge)?;
                }
            }
            if decoded > limit {
                return self.too_large();
            }
        }
    }

    fn hex4(&mut self) -> Result<u16, Diagnostic> {
        let mut value = 0u16;
        for _ in 0..4 {
            let digit = match self.raw.get(self.cursor).copied() {
                Some(byte @ b'0'..=b'9') => byte - b'0',
                Some(byte @ b'a'..=b'f') => byte - b'a' + 10,
                Some(byte @ b'A'..=b'F') => byte - b'A' + 10,
                _ => return self.invalid(),
            };
            value = (value << 4) | u16::from(digit);
            self.cursor += 1;
        }
        Ok(value)
    }

    fn number(&mut self) -> Result<(), Diagnostic> {
        let start = self.cursor;
        while self.raw.get(self.cursor).is_some_and(u8::is_ascii_digit) {
            self.cursor += 1;
            if self.cursor - start > 20 {
                return self.too_large();
            }
        }
        let token = &self.raw[start..self.cursor];
        if token.len() > 1 && token[0] == b'0' {
            return self.invalid();
        }
        std::str::from_utf8(token)
            .ok()
            .and_then(|token| token.parse::<u64>().ok())
            .ok_or(Diagnostic::ResponseInvalid)?;
        Ok(())
    }

    fn literal(&mut self, token: &[u8]) -> Result<(), Diagnostic> {
        if self.raw.get(self.cursor..self.cursor + token.len()) != Some(token) {
            return self.invalid();
        }
        self.cursor += token.len();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &str = "example:tag";

    fn valid_chat(content: &str) -> String {
        format!(
            r#"{{"model":"{MODEL}","created_at":"2026-10-06T00:00:00Z","message":{{"role":"assistant","content":"{content}"}},"done":true,"done_reason":"stop"}}"#
        )
    }

    #[test]
    fn version_is_closed_printable_and_bounded() {
        assert_eq!(version(br#"{"version":"0.12.0"}"#), Ok("0.12.0".to_owned()));
        assert_eq!(
            version(br#"{"version":" "}"#),
            Err(Diagnostic::ResponseInvalid)
        );
        assert_eq!(
            version(br#"{"version":"x","extra":0}"#),
            Err(Diagnostic::ResponseInvalid)
        );
        let exact = format!(r#"{{"version":"{}"}}"#, "v".repeat(64));
        assert!(version(exact.as_bytes()).is_ok());
        let excess = format!(r#"{{"version":"{}"}}"#, "v".repeat(65));
        assert_eq!(
            version(excess.as_bytes()),
            Err(Diagnostic::ResponseTooLarge)
        );
    }

    #[test]
    fn chat_keeps_decoded_content_and_reports_camel_case_usage() {
        let raw = br#"{"model":"example:tag","created_at":"2026-10-06T00:00:00Z","message":{"role":"assistant","content":"keep \ud83d\ude00\nexact","thinking":""},"done":true,"done_reason":"stop","eval_count":4,"total_duration":9}"#;
        let decoded = chat(raw, MODEL, 4).unwrap();
        assert_eq!(decoded.content, "keep 😀\nexact");
        assert_eq!(decoded.usage.eval_count, Some(4));
        assert_eq!(
            serde_json::to_string(&decoded.usage).unwrap(),
            r#"{"totalDuration":9,"loadDuration":null,"promptEvalCount":null,"promptEvalCachedCount":null,"promptEvalDuration":null,"evalCount":4,"evalDuration":null}"#
        );
    }

    #[test]
    fn incomplete_model_and_unknown_tool_fields_fail_closed() {
        assert_eq!(
            chat(
                valid_chat("ok")
                    .replace("\"done\":true", "\"done\":false")
                    .as_bytes(),
                MODEL,
                1
            ),
            Err(Diagnostic::ResponseIncomplete)
        );
        assert_eq!(
            chat(
                valid_chat("ok")
                    .replace("\"stop\"", "\"length\"")
                    .as_bytes(),
                MODEL,
                1
            ),
            Err(Diagnostic::ResponseIncomplete)
        );
        assert_eq!(
            chat(valid_chat("ok").as_bytes(), "other:tag", 1),
            Err(Diagnostic::ResponseInvalid)
        );
        assert_eq!(
            chat(
                valid_chat("ok")
                    .replace("\"done\":true", "\"tools\":[],\"done\":true")
                    .as_bytes(),
                MODEL,
                1
            ),
            Err(Diagnostic::ResponseInvalid)
        );
    }

    #[test]
    fn optional_fields_reject_null_and_nonempty_thinking() {
        assert_eq!(
            chat(
                valid_chat("ok")
                    .replace("\"done\":true", "\"eval_count\":null,\"done\":true")
                    .as_bytes(),
                MODEL,
                1
            ),
            Err(Diagnostic::ResponseInvalid)
        );
        assert_eq!(
            chat(
                valid_chat("ok")
                    .replace(
                        "\"content\":\"ok\"",
                        "\"content\":\"ok\",\"thinking\":\"reasoning\""
                    )
                    .as_bytes(),
                MODEL,
                1
            ),
            Err(Diagnostic::ResponseInvalid)
        );
        assert_eq!(
            chat(
                valid_chat("ok")
                    .replace("\"done\":true", "\"eval_count\":2,\"done\":true")
                    .as_bytes(),
                MODEL,
                1
            ),
            Err(Diagnostic::ResponseInvalid)
        );
    }

    #[test]
    fn scanner_rejects_duplicate_escapes_arrays_depth_and_bad_numbers() {
        for raw in [
            br#"{"version":"a","\u0076ersion":"b"}"#.as_slice(),
            br#"{"version":[]}"#.as_slice(),
            br#"{"version":1e2}"#.as_slice(),
            br#"{"version":"\ud800"}"#.as_slice(),
        ] {
            assert_eq!(version(raw), Err(Diagnostic::ResponseInvalid));
        }
        assert_eq!(
            version(br#"{"a":{"b":{"c":{"d":{"e":0}}}}}"#),
            Err(Diagnostic::ResponseTooLarge)
        );
        assert_eq!(
            version(br#"{"version":100000000000000000000}"#),
            Err(Diagnostic::ResponseTooLarge)
        );
    }

    #[test]
    fn scanner_enforces_exact_and_one_over_content_and_raw_limits() {
        let exact = valid_chat(&"x".repeat(MAX_CONTENT_BYTES));
        assert!(chat(exact.as_bytes(), MODEL, 1).is_ok());
        let excess = valid_chat(&"x".repeat(MAX_CONTENT_BYTES + 1));
        assert_eq!(
            chat(excess.as_bytes(), MODEL, 1),
            Err(Diagnostic::ResponseTooLarge)
        );
        assert_eq!(
            version(&vec![b' '; VERSION_RAW_LIMIT]),
            Err(Diagnostic::ResponseInvalid)
        );
        assert_eq!(
            version(&vec![b' '; VERSION_RAW_LIMIT + 1]),
            Err(Diagnostic::ResponseTooLarge)
        );
    }

    #[test]
    fn scanner_measures_generic_keys_and_long_unicode_in_linear_bytes() {
        let exact_string = format!(r#"{{"version":"{}"}}"#, "x".repeat(MAX_STRING_BYTES));
        assert!(preflight(exact_string.as_bytes(), VERSION_RAW_LIMIT).is_ok());
        let excess_string = format!(r#"{{"version":"{}"}}"#, "x".repeat(MAX_STRING_BYTES + 1));
        assert_eq!(
            preflight(excess_string.as_bytes(), VERSION_RAW_LIMIT),
            Err(Diagnostic::ResponseTooLarge)
        );

        let exact_key = format!(r#"{{"{}":0}}"#, "k".repeat(MAX_KEY_BYTES));
        assert!(preflight(exact_key.as_bytes(), VERSION_RAW_LIMIT).is_ok());
        let excess_key = format!(r#"{{"{}":0}}"#, "k".repeat(MAX_KEY_BYTES + 1));
        assert_eq!(
            preflight(excess_key.as_bytes(), VERSION_RAW_LIMIT),
            Err(Diagnostic::ResponseTooLarge)
        );

        let exact_unicode = valid_chat(&"😀".repeat(MAX_CONTENT_BYTES / 4));
        assert!(chat(exact_unicode.as_bytes(), MODEL, 1).is_ok());
        let excess_unicode = valid_chat(&"😀".repeat(MAX_CONTENT_BYTES / 4 + 1));
        assert_eq!(
            chat(excess_unicode.as_bytes(), MODEL, 1),
            Err(Diagnostic::ResponseTooLarge)
        );
    }
}
