//! Experimental, offline inspection for the closed RUSL D0a candidate profile.
//!
//! This crate parses supplied bytes only. It performs no I/O, authorization,
//! execution, native-payload handling, or record-to-record mapping.
#![forbid(unsafe_code)]

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub const MAX_INPUT_BYTES: usize = 131_072;
const MAX_DEPTH: usize = 16;
const MAX_VALUES: usize = 256;
const MAX_OBJECT_FIELDS: usize = 32;
const MAX_ARRAY_ITEMS: usize = 64;
const PROFILE: &str = "rangoon.rusl.observe.v0.1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    CapabilityDescriptor,
    Observation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diagnostic {
    InputLimit,
    InvalidEncoding,
    InvalidJson,
    DuplicateField,
    DepthLimit,
    ValueLimit,
    CollectionLimit,
    UnsupportedVersion,
    UnsupportedKind,
    UnknownCoreField,
    InvalidReference,
    InvalidNativeReference,
    UnsupportedExtension,
    InvalidSemanticVariant,
    CanonicalizationFailed,
}

impl Diagnostic {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InputLimit => "input_limit",
            Self::InvalidEncoding => "invalid_encoding",
            Self::InvalidJson => "invalid_json",
            Self::DuplicateField => "duplicate_field",
            Self::DepthLimit => "depth_limit",
            Self::ValueLimit => "value_limit",
            Self::CollectionLimit => "collection_limit",
            Self::UnsupportedVersion => "unsupported_version",
            Self::UnsupportedKind => "unsupported_kind",
            Self::UnknownCoreField => "unknown_core_field",
            Self::InvalidReference => "invalid_reference",
            Self::InvalidNativeReference => "invalid_native_reference",
            Self::UnsupportedExtension => "unsupported_extension",
            Self::InvalidSemanticVariant => "invalid_semantic_variant",
            Self::CanonicalizationFailed => "canonicalization_failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inspection {
    kind: Kind,
    canonical: Vec<u8>,
    identity: String,
}

impl Inspection {
    pub const fn kind(&self) -> Kind {
        self.kind
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub fn identity_sha256(&self) -> &str {
        &self.identity
    }
}

/// Strictly inspects one D0a candidate document from supplied bytes.
pub fn inspect(bytes: &[u8]) -> Result<Inspection, Diagnostic> {
    preflight(bytes)?;
    let value: Value = serde_json::from_slice(bytes).map_err(|_| Diagnostic::InvalidJson)?;
    let root = object(&value)?;
    exact(
        root,
        &[
            "languageVersion",
            "kind",
            "id",
            "createdAt",
            "origin",
            "native",
            "semantic",
            "extensions",
            "translation",
        ],
    )?;
    if text(root, "languageVersion")? != "rangoon.rusl.v0.1" {
        return Err(Diagnostic::UnsupportedVersion);
    }
    let kind = match text(root, "kind")? {
        "capability_descriptor" => Kind::CapabilityDescriptor,
        "observation" => Kind::Observation,
        _ => return Err(Diagnostic::UnsupportedKind),
    };
    token(text(root, "id")?)?;
    timestamp(text(root, "createdAt")?)?;
    validate_origin(required_object(root, "origin")?, kind)?;
    validate_native(required_object(root, "native")?)?;
    validate_semantic(
        required_object(root, "semantic")?,
        kind,
        required_object(root, "origin")?,
    )?;
    empty_array(
        root.get("extensions").ok_or(Diagnostic::UnknownCoreField)?,
        Diagnostic::UnsupportedExtension,
    )?;
    validate_translation(required_object(root, "translation")?, kind)?;
    let canonical = canonical(&value)?;
    let mut hasher = Sha256::new();
    hasher.update(PROFILE.as_bytes());
    hasher.update([0]);
    hasher.update(&canonical);
    Ok(Inspection {
        kind,
        canonical,
        identity: hex(&hasher.finalize()),
    })
}

fn validate_origin(origin: &Map<String, Value>, kind: Kind) -> Result<(), Diagnostic> {
    if kind == Kind::Observation && !origin.contains_key("sourceRef") {
        return Err(Diagnostic::InvalidReference);
    }
    exact(
        origin,
        &[
            "systemId",
            "environmentId",
            "engineId",
            "product",
            "productVersion",
            "protocol",
            "protocolVersion",
            "adapterId",
            "adapterVersion",
            "adapterDigest",
            "sourceRef",
            "observedAt",
            "receivedAt",
            "authenticity",
            "qualificationRef",
        ],
    )?;
    descriptive(text(origin, "product")?)?;
    descriptive(text(origin, "protocol")?)?;
    if text(origin, "authenticity")? != "unverified" || !is_null(origin, "qualificationRef")? {
        return Err(Diagnostic::InvalidSemanticVariant);
    }
    match kind {
        Kind::CapabilityDescriptor => {
            for key in [
                "systemId",
                "environmentId",
                "engineId",
                "productVersion",
                "protocolVersion",
                "adapterId",
                "adapterVersion",
                "adapterDigest",
                "sourceRef",
                "observedAt",
                "receivedAt",
            ] {
                if !is_null(origin, key)? {
                    return Err(Diagnostic::InvalidReference);
                }
            }
        }
        Kind::Observation => {
            for key in [
                "systemId",
                "environmentId",
                "engineId",
                "productVersion",
                "protocolVersion",
                "adapterId",
                "adapterVersion",
            ] {
                token(text(origin, key)?)?;
            }
            let digest = text(origin, "adapterDigest")?;
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(Diagnostic::InvalidReference);
            }
            timestamp(text(origin, "observedAt")?)?;
            timestamp(text(origin, "receivedAt")?)?;
            let source = required_object(origin, "sourceRef")?;
            validate_reference(source)?;
            for key in ["systemId", "environmentId", "engineId"] {
                if text(source, key)? != text(origin, key)? {
                    return Err(Diagnostic::InvalidReference);
                }
            }
        }
    }
    Ok(())
}

fn validate_native(native: &Map<String, Value>) -> Result<(), Diagnostic> {
    exact(
        native,
        &[
            "state",
            "mediaType",
            "byteLength",
            "sha256",
            "hostRef",
            "redaction",
            "retention",
            "availableForRoundTrip",
        ],
    )?;
    if text(native, "state")? != "omitted"
        || text(native, "redaction")? != "omitted"
        || text(native, "retention")? != "none"
        || bool_value(native, "availableForRoundTrip")?
        || !is_null(native, "mediaType")?
        || !is_null(native, "byteLength")?
        || !is_null(native, "sha256")?
        || !is_null(native, "hostRef")?
    {
        return Err(Diagnostic::InvalidNativeReference);
    }
    Ok(())
}

fn validate_semantic(
    semantic: &Map<String, Value>,
    kind: Kind,
    origin: &Map<String, Value>,
) -> Result<(), Diagnostic> {
    match kind {
        Kind::CapabilityDescriptor => {
            exact(
                semantic,
                &[
                    "family",
                    "operation",
                    "effectClass",
                    "supportState",
                    "nativeOperationRef",
                ],
            )?;
            family(text(semantic, "family")?)?;
            token(text(semantic, "operation")?)?;
            one_of(
                text(semantic, "effectClass")?,
                &[
                    "read",
                    "write",
                    "delete",
                    "exec",
                    "network",
                    "inference",
                    "mixed",
                    "unknown",
                ],
            )?;
            if text(semantic, "supportState")? != "unassessed"
                || !is_null(semantic, "nativeOperationRef")?
            {
                return Err(Diagnostic::InvalidSemanticVariant);
            }
        }
        Kind::Observation => {
            let stage = text(semantic, "stage")?;
            match stage {
                "provider_policy_evaluation" => {
                    exact(
                        semantic,
                        &[
                            "family",
                            "stage",
                            "operationRef",
                            "nativeState",
                            "projectedDecision",
                        ],
                    )?;
                    if text(semantic, "family")? != "policy" {
                        return Err(Diagnostic::InvalidSemanticVariant);
                    }
                    one_of(
                        text(semantic, "projectedDecision")?,
                        &["allow", "deny", "unknown"],
                    )?;
                }
                "runtime_report" => {
                    exact(
                        semantic,
                        &[
                            "family",
                            "stage",
                            "operationRef",
                            "nativeState",
                            "reportedEventClass",
                        ],
                    )?;
                    if text(semantic, "family")? != "os_sandbox" {
                        return Err(Diagnostic::InvalidSemanticVariant);
                    }
                    one_of(
                        text(semantic, "reportedEventClass")?,
                        &[
                            "attempt",
                            "active",
                            "completed",
                            "failed",
                            "cancelled",
                            "unknown",
                        ],
                    )?;
                }
                _ => return Err(Diagnostic::InvalidSemanticVariant),
            }
            token(text(semantic, "nativeState")?)?;
            let operation = required_object(semantic, "operationRef")?;
            validate_reference(operation)?;
            let source = required_object(origin, "sourceRef")?;
            for key in ["systemId", "environmentId", "engineId", "contractVersion"] {
                if text(operation, key)? != text(source, key)? {
                    return Err(Diagnostic::InvalidReference);
                }
            }
        }
    }
    Ok(())
}

fn validate_translation(translation: &Map<String, Value>, kind: Kind) -> Result<(), Diagnostic> {
    exact(
        translation,
        &[
            "profileId",
            "direction",
            "status",
            "authority",
            "fieldMap",
            "loss",
            "prerequisites",
        ],
    )?;
    if text(translation, "profileId")? != PROFILE
        || text(translation, "direction")? != "native_to_rusl"
        || text(translation, "authority")? != "none"
    {
        return Err(Diagnostic::UnsupportedVersion);
    }
    let expected_status = match kind {
        Kind::CapabilityDescriptor => "unassessed",
        Kind::Observation => "observation_only",
    };
    if text(translation, "status")? != expected_status {
        return Err(Diagnostic::InvalidSemanticVariant);
    }
    empty_array(
        translation
            .get("fieldMap")
            .ok_or(Diagnostic::UnknownCoreField)?,
        Diagnostic::InvalidSemanticVariant,
    )?;
    sorted_tokens(
        translation
            .get("loss")
            .ok_or(Diagnostic::UnknownCoreField)?,
    )?;
    sorted_tokens(
        translation
            .get("prerequisites")
            .ok_or(Diagnostic::UnknownCoreField)?,
    )?;
    Ok(())
}

fn validate_reference(reference: &Map<String, Value>) -> Result<(), Diagnostic> {
    exact(
        reference,
        &[
            "systemId",
            "environmentId",
            "engineId",
            "contractVersion",
            "nativeId",
        ],
    )?;
    for key in [
        "systemId",
        "environmentId",
        "engineId",
        "contractVersion",
        "nativeId",
    ] {
        token(text(reference, key)?)?;
    }
    Ok(())
}

fn object(value: &Value) -> Result<&Map<String, Value>, Diagnostic> {
    value.as_object().ok_or(Diagnostic::InvalidJson)
}
fn required_object<'a>(
    map: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a Map<String, Value>, Diagnostic> {
    map.get(key)
        .and_then(Value::as_object)
        .ok_or(Diagnostic::InvalidReference)
}
fn text<'a>(map: &'a Map<String, Value>, key: &str) -> Result<&'a str, Diagnostic> {
    map.get(key)
        .and_then(Value::as_str)
        .ok_or(Diagnostic::InvalidSemanticVariant)
}
fn bool_value(map: &Map<String, Value>, key: &str) -> Result<bool, Diagnostic> {
    map.get(key)
        .and_then(Value::as_bool)
        .ok_or(Diagnostic::InvalidNativeReference)
}
fn is_null(map: &Map<String, Value>, key: &str) -> Result<bool, Diagnostic> {
    Ok(map.get(key).ok_or(Diagnostic::UnknownCoreField)?.is_null())
}

fn exact(map: &Map<String, Value>, keys: &[&str]) -> Result<(), Diagnostic> {
    if map.len() != keys.len() || map.keys().any(|key| !keys.contains(&key.as_str())) {
        return Err(Diagnostic::UnknownCoreField);
    }
    if keys.iter().any(|key| !map.contains_key(*key)) {
        return Err(Diagnostic::UnknownCoreField);
    }
    Ok(())
}

fn empty_array(value: &Value, error: Diagnostic) -> Result<(), Diagnostic> {
    match value.as_array() {
        Some(values) if values.is_empty() => Ok(()),
        _ => Err(error),
    }
}
fn one_of(value: &str, accepted: &[&str]) -> Result<(), Diagnostic> {
    if accepted.contains(&value) {
        Ok(())
    } else {
        Err(Diagnostic::InvalidSemanticVariant)
    }
}
fn family(value: &str) -> Result<(), Diagnostic> {
    one_of(value, &["policy", "os_sandbox"])
}
fn token(value: &str) -> Result<(), Diagnostic> {
    if (1..=128).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b':' | b'-'))
    {
        Ok(())
    } else {
        Err(Diagnostic::InvalidReference)
    }
}
fn descriptive(value: &str) -> Result<(), Diagnostic> {
    if (1..=4096).contains(&value.len())
        && value
            .chars()
            .all(|c| !matches!(c as u32, 0..=31 | 127..=159))
    {
        Ok(())
    } else {
        Err(Diagnostic::InvalidSemanticVariant)
    }
}
fn sorted_tokens(value: &Value) -> Result<(), Diagnostic> {
    let values = value.as_array().ok_or(Diagnostic::InvalidSemanticVariant)?;
    if values.len() > 16 {
        return Err(Diagnostic::CollectionLimit);
    }
    let mut previous = None;
    for value in values {
        let value = value.as_str().ok_or(Diagnostic::InvalidSemanticVariant)?;
        token(value)?;
        if previous.is_some_and(|prior: &str| prior >= value) {
            return Err(Diagnostic::InvalidSemanticVariant);
        }
        previous = Some(value);
    }
    Ok(())
}

fn timestamp(value: &str) -> Result<(), Diagnostic> {
    let bytes = value.as_bytes();
    if bytes.len() != 24
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'.'
        || bytes[23] != b'Z'
    {
        return Err(Diagnostic::InvalidSemanticVariant);
    }
    if ![
        (0, 4),
        (5, 7),
        (8, 10),
        (11, 13),
        (14, 16),
        (17, 19),
        (20, 23),
    ]
    .iter()
    .all(|&(start, end)| bytes[start..end].iter().all(u8::is_ascii_digit))
    {
        return Err(Diagnostic::InvalidSemanticVariant);
    }
    let number = |start: usize, len: usize| -> Option<u32> {
        std::str::from_utf8(&bytes[start..start + len])
            .ok()?
            .parse()
            .ok()
    };
    let (year, month, day, hour, minute, second) = (
        number(0, 4),
        number(5, 2),
        number(8, 2),
        number(11, 2),
        number(14, 2),
        number(17, 2),
    );
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) =
        (year, month, day, hour, minute, second)
    else {
        return Err(Diagnostic::InvalidSemanticVariant);
    };
    if year == 0 || month == 0 || month > 12 || hour > 23 || minute > 59 || second > 59 {
        return Err(Diagnostic::InvalidSemanticVariant);
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => unreachable!(),
    };
    if day == 0 || day > max_day {
        return Err(Diagnostic::InvalidSemanticVariant);
    }
    Ok(())
}

fn canonical(value: &Value) -> Result<Vec<u8>, Diagnostic> {
    let mut out = String::new();
    canonical_into(value, &mut out)?;
    Ok(out.into_bytes())
}

fn canonical_into(value: &Value, out: &mut String) -> Result<(), Diagnostic> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
        Value::String(value) => out.push_str(
            &serde_json::to_string(value).map_err(|_| Diagnostic::CanonicalizationFailed)?,
        ),
        Value::Array(values) => {
            out.push('[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                canonical_into(value, out)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort_unstable();
            out.push('{');
            for (index, key) in keys.into_iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                out.push_str(
                    &serde_json::to_string(key).map_err(|_| Diagnostic::CanonicalizationFailed)?,
                );
                out.push(':');
                canonical_into(&map[key], out)?;
            }
            out.push('}');
        }
        Value::Number(_) => return Err(Diagnostic::CanonicalizationFailed),
    }
    Ok(())
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

fn preflight(bytes: &[u8]) -> Result<(), Diagnostic> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(Diagnostic::InputLimit);
    }
    std::str::from_utf8(bytes).map_err(|_| Diagnostic::InvalidEncoding)?;
    let mut parser = Scanner {
        bytes,
        cursor: 0,
        values: 0,
    };
    parser.value(1)?;
    parser.space();
    if parser.cursor == bytes.len() {
        Ok(())
    } else {
        Err(Diagnostic::InvalidJson)
    }
}

struct Scanner<'a> {
    bytes: &'a [u8],
    cursor: usize,
    values: usize,
}
impl Scanner<'_> {
    fn space(&mut self) {
        while self
            .bytes
            .get(self.cursor)
            .is_some_and(|b| matches!(b, b' ' | b'\n' | b'\r' | b'\t'))
        {
            self.cursor += 1;
        }
    }
    fn value(&mut self, depth: usize) -> Result<(), Diagnostic> {
        self.space();
        if depth > MAX_DEPTH {
            return Err(Diagnostic::DepthLimit);
        }
        self.values += 1;
        if self.values > MAX_VALUES {
            return Err(Diagnostic::ValueLimit);
        }
        match self.bytes.get(self.cursor) {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => {
                self.string()?;
                Ok(())
            }
            Some(b't') => self.literal(b"true"),
            Some(b'f') => self.literal(b"false"),
            Some(b'n') => self.literal(b"null"),
            Some(b'-' | b'0'..=b'9') => {
                self.number()?;
                Ok(())
            }
            _ => Err(Diagnostic::InvalidJson),
        }
    }
    fn object(&mut self, depth: usize) -> Result<(), Diagnostic> {
        self.cursor += 1;
        self.space();
        if self.bytes.get(self.cursor) == Some(&b'}') {
            self.cursor += 1;
            return Ok(());
        }
        let mut keys = HashSet::new();
        loop {
            if keys.len() >= MAX_OBJECT_FIELDS {
                return Err(Diagnostic::CollectionLimit);
            }
            self.space();
            let start = self.cursor;
            self.string()?;
            let key: String = serde_json::from_slice(&self.bytes[start..self.cursor])
                .map_err(|_| Diagnostic::InvalidJson)?;
            if !keys.insert(key) {
                return Err(Diagnostic::DuplicateField);
            }
            self.space();
            if self.bytes.get(self.cursor) != Some(&b':') {
                return Err(Diagnostic::InvalidJson);
            }
            self.cursor += 1;
            self.value(depth + 1)?;
            self.space();
            match self.bytes.get(self.cursor) {
                Some(b'}') => {
                    self.cursor += 1;
                    return Ok(());
                }
                Some(b',') => self.cursor += 1,
                _ => return Err(Diagnostic::InvalidJson),
            }
        }
    }
    fn array(&mut self, depth: usize) -> Result<(), Diagnostic> {
        self.cursor += 1;
        self.space();
        if self.bytes.get(self.cursor) == Some(&b']') {
            self.cursor += 1;
            return Ok(());
        }
        let mut count = 0;
        loop {
            count += 1;
            if count > MAX_ARRAY_ITEMS {
                return Err(Diagnostic::CollectionLimit);
            }
            self.value(depth + 1)?;
            self.space();
            match self.bytes.get(self.cursor) {
                Some(b']') => {
                    self.cursor += 1;
                    return Ok(());
                }
                Some(b',') => self.cursor += 1,
                _ => return Err(Diagnostic::InvalidJson),
            }
        }
    }
    fn string(&mut self) -> Result<(), Diagnostic> {
        if self.bytes.get(self.cursor) != Some(&b'"') {
            return Err(Diagnostic::InvalidJson);
        }
        self.cursor += 1;
        loop {
            match self.bytes.get(self.cursor).copied() {
                Some(b'"') => {
                    self.cursor += 1;
                    return Ok(());
                }
                Some(b'\\') => {
                    self.cursor += 1;
                    match self.bytes.get(self.cursor).copied() {
                        Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => {
                            self.cursor += 1
                        }
                        Some(b'u') => {
                            self.cursor += 1;
                            self.hex4()?;
                        }
                        _ => return Err(Diagnostic::InvalidJson),
                    }
                }
                Some(0..=31) | None => return Err(Diagnostic::InvalidJson),
                Some(byte) if byte < 128 => self.cursor += 1,
                Some(byte) => {
                    let width = match byte {
                        0xc2..=0xdf => 2,
                        0xe0..=0xef => 3,
                        0xf0..=0xf4 => 4,
                        _ => return Err(Diagnostic::InvalidJson),
                    };
                    if self
                        .bytes
                        .get(self.cursor..self.cursor + width)
                        .is_none_or(|slice| std::str::from_utf8(slice).is_err())
                    {
                        return Err(Diagnostic::InvalidJson);
                    }
                    self.cursor += width;
                }
            }
        }
    }
    fn hex4(&mut self) -> Result<(), Diagnostic> {
        for _ in 0..4 {
            if !self
                .bytes
                .get(self.cursor)
                .is_some_and(u8::is_ascii_hexdigit)
            {
                return Err(Diagnostic::InvalidJson);
            }
            self.cursor += 1;
        }
        Ok(())
    }
    fn literal(&mut self, literal: &[u8]) -> Result<(), Diagnostic> {
        if self.bytes.get(self.cursor..self.cursor + literal.len()) == Some(literal) {
            self.cursor += literal.len();
            Ok(())
        } else {
            Err(Diagnostic::InvalidJson)
        }
    }
    fn number(&mut self) -> Result<(), Diagnostic> {
        let start = self.cursor;
        if self.bytes.get(self.cursor) == Some(&b'-') {
            self.cursor += 1;
        }
        match self.bytes.get(self.cursor) {
            Some(b'0') => self.cursor += 1,
            Some(b'1'..=b'9') => {
                self.cursor += 1;
                while self.bytes.get(self.cursor).is_some_and(u8::is_ascii_digit) {
                    self.cursor += 1;
                }
            }
            _ => return Err(Diagnostic::InvalidJson),
        }
        if self.bytes.get(self.cursor) == Some(&b'.') {
            self.cursor += 1;
            let fraction = self.cursor;
            while self.bytes.get(self.cursor).is_some_and(u8::is_ascii_digit) {
                self.cursor += 1;
            }
            if self.cursor == fraction {
                return Err(Diagnostic::InvalidJson);
            }
        }
        if matches!(self.bytes.get(self.cursor), Some(b'e' | b'E')) {
            self.cursor += 1;
            if matches!(self.bytes.get(self.cursor), Some(b'+' | b'-')) {
                self.cursor += 1;
            }
            let exponent = self.cursor;
            while self.bytes.get(self.cursor).is_some_and(u8::is_ascii_digit) {
                self.cursor += 1;
            }
            if self.cursor == exponent {
                return Err(Diagnostic::InvalidJson);
            }
        }
        if self.cursor == start {
            Err(Diagnostic::InvalidJson)
        } else {
            Ok(())
        }
    }
}
