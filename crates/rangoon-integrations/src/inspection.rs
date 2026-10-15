//! Strict offline inspection with fixed authority and provenance separation.
use crate::{
    INPUT_SCHEMA, Inspection, InspectionError, MAX_ARRAY_ITEMS, MAX_BYTES, MAX_DEPTH, MAX_INTEGER,
    MAX_OBJECT_FIELDS, MAX_VALUES, OPERATIONS, OUTPUT_SCHEMA, fixture_profile,
};
use rangoon_domain::byte_digest;
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::io::Write;

const REQUEST_FIELDS: [&str; 6] = [
    "id",
    "operation",
    "input",
    "inputDigest",
    "configuration",
    "configurationDigest",
];
const OBSERVATION_FIELDS: [&str; 20] = [
    "requestId",
    "operation",
    "inputDigest",
    "configurationDigest",
    "state",
    "reason",
    "kind",
    "reportedOutcome",
    "evidenceReference",
    "policyRevision",
    "traceId",
    "resourceReference",
    "observedProductVersion",
    "observedProtocol",
    "observedAt",
    "receivedAt",
    "redaction",
    "originalDigest",
    "payload",
    "payloadDigest",
];
const KINDS: [&str; 6] = [
    "provider_configuration",
    "provider_decision",
    "provider_guardrail_result",
    "provider_approval",
    "provider_execution_outcome",
    "local_content_review",
];

pub fn inspect_fixture(input: &[u8]) -> Result<Inspection, InspectionError> {
    if input.len() > MAX_BYTES {
        return Err(InspectionError::InputLimit);
    }
    let source = std::str::from_utf8(input).map_err(|_| InspectionError::InvalidEncoding)?;
    let value = parse(source)?;
    let root = object(&value)?;
    closed(
        root,
        &["schemaVersion", "profile", "request", "observation"],
    )?;
    if string(field(root, "schemaVersion")?)? != INPUT_SCHEMA {
        return Err(InspectionError::UnsupportedSchema);
    }
    let profile = field(root, "profile")?;
    closed(
        object(profile)?,
        &[
            "instanceId",
            "providerFamily",
            "productTarget",
            "productVersion",
            "protocol",
            "adapterId",
            "adapterVersion",
            "adapterDigest",
        ],
    )?;
    if *profile != fixture_profile() {
        return Err(InspectionError::UnsupportedProfile);
    }
    let request = object(field(root, "request")?)?;
    closed(request, &REQUEST_FIELDS)?;
    token(field(request, "id")?)?;
    let operation = string(field(request, "operation")?)?;
    one(field(request, "operation")?, &OPERATIONS)?;
    digest(field(request, "inputDigest")?)?;
    if string(field(request, "inputDigest")?)? != byte_digest(&canonical(field(request, "input")?)?)
    {
        return Err(InspectionError::DigestMismatch);
    }
    let configuration = field(request, "configuration")?;
    let config_digest = field(request, "configurationDigest")?;
    if configuration.is_null() {
        if !config_digest.is_null() {
            return Err(InspectionError::InvalidRecord);
        }
    } else {
        let config = string(configuration)?;
        digest(config_digest)?;
        if string(config_digest)? != byte_digest(config.as_bytes()) {
            return Err(InspectionError::DigestMismatch);
        }
    }
    if operation == "connection_check"
        && (!field(request, "input")?.is_null() || !configuration.is_null())
    {
        return Err(InspectionError::InvalidRecord);
    }
    let observation = object(field(root, "observation")?)?;
    closed(observation, &OBSERVATION_FIELDS)?;
    token(field(observation, "requestId")?)?;
    one(field(observation, "operation")?, &OPERATIONS)?;
    digest(field(observation, "inputDigest")?)?;
    optional(field(observation, "configurationDigest")?, digest)?;
    for (reported, expected) in [
        ("requestId", "id"),
        ("operation", "operation"),
        ("inputDigest", "inputDigest"),
        ("configurationDigest", "configurationDigest"),
    ] {
        if field(observation, reported)? != field(request, expected)? {
            return Err(InspectionError::ContextMismatch);
        }
    }
    let state = string(field(observation, "state")?)?;
    let reason = match state {
        "unavailable" => "adapter_unavailable",
        "unsupported" => "operation_unsupported",
        "disconnected" => "not_connected",
        "error" => "inspection_failed",
        "inspected" => "observation_only",
        _ => return Err(InspectionError::InvalidRecord),
    };
    if string(field(observation, "reason")?)? != reason {
        return Err(InspectionError::InvalidRecord);
    }
    if state != "inspected" {
        for name in &OBSERVATION_FIELDS[6..] {
            if *name == "redaction" {
                one(field(observation, name)?, &["omitted"])?;
            } else if !field(observation, name)?.is_null() {
                return Err(InspectionError::InvalidRecord);
            }
        }
    } else {
        inspect_observation(observation, operation, configuration)?;
    }
    let configuration_validation = if state == "inspected" && operation == "configuration_preview" {
        "local_shape"
    } else {
        "unavailable"
    };
    let output = json!({
        "schemaVersion": OUTPUT_SCHEMA, "source": "offline_fixture",
        "profile": profile, "request": field(root, "request")?,
        "observation": field(root, "observation")?, "authority": "none",
        "execution": "unavailable", "authenticity": "unverified",
        "compatibility": "unknown", "connection": "unknown", "networkAttempted": false,
        "configurationValidation": configuration_validation,
    });
    let canonical = canonical(&output)?;
    let output_source =
        std::str::from_utf8(&canonical).map_err(|_| InspectionError::SerializationFailed)?;
    scan_raw(output_source).map_err(|_| InspectionError::OutputLimit)?;
    Ok(Inspection {
        digest: byte_digest(&canonical),
        canonical,
        state: state.to_owned(),
    })
}

fn inspect_observation(
    o: &Map<String, Value>,
    operation: &str,
    configuration: &Value,
) -> Result<(), InspectionError> {
    let kind = string(field(o, "kind")?)?;
    one(field(o, "kind")?, &KINDS)?;
    match operation {
        "configuration_preview" if kind == "provider_configuration" && !configuration.is_null() => {
        }
        "authorization_evaluation" if kind == "provider_decision" => {}
        "guardrail_evaluation" if kind == "provider_guardrail_result" => {}
        "evidence_read" => {}
        _ => return Err(InspectionError::InvalidRecord),
    }
    for name in &OBSERVATION_FIELDS[7..16] {
        optional(field(o, name)?, token)?;
    }
    for (name, pin) in [
        ("observedProductVersion", "1"),
        ("observedProtocol", "rangoon.fixture.evidence.v1"),
    ] {
        let v = field(o, name)?;
        if !v.is_null() && string(v)? != pin {
            return Err(InspectionError::CompatibilityMismatch);
        }
    }
    let redaction = string(field(o, "redaction")?)?;
    one(field(o, "redaction")?, &["none", "redacted", "omitted"])?;
    if operation == "configuration_preview" && redaction != "none" {
        return Err(InspectionError::InvalidRecord);
    }
    if redaction == "none" {
        if !field(o, "originalDigest")?.is_null() {
            return Err(InspectionError::InvalidRecord);
        }
    } else {
        digest(field(o, "originalDigest")?)?;
    }
    let payload = field(o, "payload")?;
    if redaction == "omitted" {
        if !payload.is_null() || !field(o, "payloadDigest")?.is_null() {
            return Err(InspectionError::InvalidRecord);
        }
    } else {
        if payload.is_null() {
            return Err(InspectionError::InvalidRecord);
        }
        digest(field(o, "payloadDigest")?)?;
        if string(field(o, "payloadDigest")?)? != byte_digest(&canonical(payload)?) {
            return Err(InspectionError::DigestMismatch);
        }
    }
    if operation == "configuration_preview" && parse(string(configuration)?)? != *payload {
        return Err(InspectionError::ContextMismatch);
    }
    Ok(())
}

fn parse(source: &str) -> Result<Value, InspectionError> {
    scan_raw(source)?;
    serde_json::from_str(source).map_err(|_| InspectionError::InvalidJson)
}

// Sorting explicitly keeps canonical identities independent from Map feature flags.
fn sorted(v: &Value) -> Value {
    match v {
        Value::Object(o) => {
            let mut keys: Vec<_> = o.keys().collect();
            keys.sort_unstable();
            let mut result = Map::new();
            for key in keys {
                result.insert(key.clone(), sorted(&o[key]));
            }
            Value::Object(result)
        }
        Value::Array(a) => Value::Array(a.iter().map(sorted).collect()),
        other => other.clone(),
    }
}
fn canonical(value: &Value) -> Result<Vec<u8>, InspectionError> {
    let mut sink = BoundedSink {
        bytes: Vec::new(),
        exceeded: false,
    };
    if serde_json::to_writer(&mut sink, &sorted(value)).is_err() {
        return Err(if sink.exceeded {
            InspectionError::OutputLimit
        } else {
            InspectionError::SerializationFailed
        });
    }
    Ok(sink.bytes)
}
struct BoundedSink {
    bytes: Vec<u8>,
    exceeded: bool,
}
impl Write for BoundedSink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_BYTES.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(std::io::Error::other("output_limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn token(v: &Value) -> Result<(), InspectionError> {
    let s = string(v)?;
    if s.is_empty()
        || s.len() > 128
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b':' | b'-'))
    {
        Err(InspectionError::InvalidRecord)
    } else {
        Ok(())
    }
}
fn digest(v: &Value) -> Result<(), InspectionError> {
    let s = string(v)?;
    if s.len() != 64
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    {
        Err(InspectionError::InvalidRecord)
    } else {
        Ok(())
    }
}
fn optional(
    v: &Value,
    f: fn(&Value) -> Result<(), InspectionError>,
) -> Result<(), InspectionError> {
    if v.is_null() { Ok(()) } else { f(v) }
}
fn one(v: &Value, allowed: &[&str]) -> Result<(), InspectionError> {
    if allowed.contains(&string(v)?) {
        Ok(())
    } else {
        Err(InspectionError::InvalidRecord)
    }
}
fn object(v: &Value) -> Result<&Map<String, Value>, InspectionError> {
    v.as_object().ok_or(InspectionError::InvalidRecord)
}
fn string(v: &Value) -> Result<&str, InspectionError> {
    v.as_str().ok_or(InspectionError::InvalidRecord)
}
fn field<'a>(o: &'a Map<String, Value>, name: &str) -> Result<&'a Value, InspectionError> {
    o.get(name).ok_or(InspectionError::InvalidRecord)
}
fn closed(o: &Map<String, Value>, fields: &[&str]) -> Result<(), InspectionError> {
    if o.len() != fields.len() || fields.iter().any(|k| !o.contains_key(*k)) {
        Err(InspectionError::InvalidRecord)
    } else {
        Ok(())
    }
}

fn scan_raw(source: &str) -> Result<(), InspectionError> {
    let mut s = Scanner {
        b: source.as_bytes(),
        p: 0,
        values: 0,
    };
    s.ws();
    s.value(1)?;
    s.ws();
    if s.p != s.b.len() {
        return Err(InspectionError::InvalidJson);
    }
    Ok(())
}
struct Scanner<'a> {
    b: &'a [u8],
    p: usize,
    values: usize,
}
impl Scanner<'_> {
    fn ws(&mut self) {
        while self.p < self.b.len() && self.b[self.p].is_ascii_whitespace() {
            self.p += 1
        }
    }
    fn value(&mut self, d: usize) -> Result<(), InspectionError> {
        if d > MAX_DEPTH {
            return Err(InspectionError::DepthLimit);
        }
        self.values += 1;
        if self.values > MAX_VALUES {
            return Err(InspectionError::ValueLimit);
        }
        self.ws();
        match self.b.get(self.p) {
            Some(b'{') => self.obj(d),
            Some(b'[') => self.arr(d),
            Some(b'\"') => {
                self.str()?;
                Ok(())
            }
            Some(b't') => self.word(b"true"),
            Some(b'f') => self.word(b"false"),
            Some(b'n') => self.word(b"null"),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(InspectionError::InvalidJson),
        }
    }
    fn obj(&mut self, d: usize) -> Result<(), InspectionError> {
        self.p += 1;
        self.ws();
        let mut keys = BTreeSet::new();
        let mut n = 0;
        if self.b.get(self.p) == Some(&b'}') {
            self.p += 1;
            return Ok(());
        }
        loop {
            self.ws();
            let start = self.p;
            self.str()?;
            let key: String = serde_json::from_slice(&self.b[start..self.p])
                .map_err(|_| InspectionError::InvalidJson)?;
            if keys.contains(&key) {
                return Err(InspectionError::DuplicateField);
            }
            n += 1;
            if n > MAX_OBJECT_FIELDS {
                return Err(InspectionError::CollectionLimit);
            }
            keys.insert(key);
            self.ws();
            if self.b.get(self.p) != Some(&b':') {
                return Err(InspectionError::InvalidJson);
            }
            self.p += 1;
            self.value(d + 1)?;
            self.ws();
            match self.b.get(self.p) {
                Some(b',') => self.p += 1,
                Some(b'}') => {
                    self.p += 1;
                    return Ok(());
                }
                _ => return Err(InspectionError::InvalidJson),
            }
        }
    }
    fn arr(&mut self, d: usize) -> Result<(), InspectionError> {
        self.p += 1;
        self.ws();
        let mut n = 0;
        if self.b.get(self.p) == Some(&b']') {
            self.p += 1;
            return Ok(());
        }
        loop {
            n += 1;
            if n > MAX_ARRAY_ITEMS {
                return Err(InspectionError::CollectionLimit);
            }
            self.value(d + 1)?;
            self.ws();
            match self.b.get(self.p) {
                Some(b',') => self.p += 1,
                Some(b']') => {
                    self.p += 1;
                    return Ok(());
                }
                _ => return Err(InspectionError::InvalidJson),
            }
        }
    }
    fn str(&mut self) -> Result<(), InspectionError> {
        if self.b.get(self.p) != Some(&b'\"') {
            return Err(InspectionError::InvalidJson);
        }
        self.p += 1;
        while let Some(&c) = self.b.get(self.p) {
            match c {
                b'\"' => {
                    self.p += 1;
                    return Ok(());
                }
                b'\\' => {
                    self.p += 1;
                    match self.b.get(self.p) {
                        Some(b'\"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => {
                            self.p += 1
                        }
                        Some(b'u') => {
                            self.p += 1;
                            for _ in 0..4 {
                                if !self.b.get(self.p).is_some_and(uhex) {
                                    return Err(InspectionError::InvalidJson);
                                }
                                self.p += 1
                            }
                        }
                        _ => return Err(InspectionError::InvalidJson),
                    }
                }
                0..=31 => return Err(InspectionError::InvalidJson),
                _ => self.p += 1,
            }
        }
        Err(InspectionError::InvalidJson)
    }
    fn word(&mut self, w: &[u8]) -> Result<(), InspectionError> {
        if self.b.get(self.p..self.p + w.len()) == Some(w) {
            self.p += w.len();
            Ok(())
        } else {
            Err(InspectionError::InvalidJson)
        }
    }
    fn number(&mut self) -> Result<(), InspectionError> {
        let start = self.p;
        if self.b.get(self.p) == Some(&b'-') {
            self.p += 1
        }
        if self.b.get(self.p) == Some(&b'0') {
            self.p += 1
        } else {
            let p = self.p;
            while self.b.get(self.p).is_some_and(|c| c.is_ascii_digit()) {
                self.p += 1
            }
            if p == self.p {
                return Err(InspectionError::InvalidJson);
            }
        }
        if self.b.get(self.p) == Some(&b'.') {
            self.p += 1;
            let p = self.p;
            while self.b.get(self.p).is_some_and(|c| c.is_ascii_digit()) {
                self.p += 1
            }
            if p == self.p {
                return Err(InspectionError::InvalidJson);
            }
        }
        if matches!(self.b.get(self.p), Some(b'e' | b'E')) {
            self.p += 1;
            if matches!(self.b.get(self.p), Some(b'+' | b'-')) {
                self.p += 1
            }
            let p = self.p;
            while self.b.get(self.p).is_some_and(|c| c.is_ascii_digit()) {
                self.p += 1
            }
            if p == self.p {
                return Err(InspectionError::InvalidJson);
            }
        }
        if self.p == start {
            Err(InspectionError::InvalidJson)
        } else {
            let number: serde_json::Number = serde_json::from_slice(&self.b[start..self.p])
                .map_err(|_| InspectionError::InvalidRecord)?;
            if number
                .as_i64()
                .is_some_and(|n| n.unsigned_abs() <= MAX_INTEGER)
                || number.as_u64().is_some_and(|n| n <= MAX_INTEGER)
            {
                Ok(())
            } else {
                Err(InspectionError::InvalidRecord)
            }
        }
    }
}
fn uhex(c: &u8) -> bool {
    c.is_ascii_hexdigit()
}
