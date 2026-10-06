//! Closed decoder for OpenAI Responses API completion envelopes.

use crate::{Diagnostic, json::preflight};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const MAX_ID_BYTES: usize = 128;
const MAX_MODEL_BYTES: usize = 128;
const MAX_CONTENT_BYTES: usize = 128 * 1024;

/// Provider-reported completed response. Its content remains untrusted data.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Response {
    pub(crate) id: String,
    pub(crate) model: String,
    pub(crate) content: String,
    pub(crate) usage: Option<Usage>,
}

/// Optional provider-reported token metrics. Absence remains unknown.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Usage {
    pub(crate) input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) total_tokens: u64,
    #[serde(default)]
    pub(crate) cached_input_tokens: Option<u64>,
    #[serde(default)]
    pub(crate) cache_write_input_tokens: Option<u64>,
    #[serde(default)]
    pub(crate) reasoning_output_tokens: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderUsage {
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
    #[serde(default)]
    input_tokens_details: Option<InputTokenDetails>,
    #[serde(default)]
    output_tokens_details: Option<OutputTokenDetails>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputTokenDetails {
    cached_tokens: u64,
    #[serde(default)]
    cache_write_tokens: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputTokenDetails {
    reasoning_tokens: u64,
}

#[derive(Deserialize)]
struct Envelope {
    object: String,
    id: String,
    model: String,
    status: String,
    #[serde(deserialize_with = "required_value")]
    error: Value,
    #[serde(deserialize_with = "required_value")]
    incomplete_details: Value,
    output: Vec<Value>,
    #[serde(default)]
    usage: Option<ProviderUsage>,
}

/// Decode the strict, completed subset of a Responses API response.
pub(crate) fn response(raw: &[u8], max_output_tokens: u64) -> Result<Response, Diagnostic> {
    preflight(raw)?;
    let envelope: Envelope =
        serde_json::from_slice(raw).map_err(|_| Diagnostic::ResponseInvalid)?;
    let Envelope {
        object,
        id,
        model,
        status,
        error,
        incomplete_details,
        output,
        usage,
    } = envelope;

    if object != "response" || !prefixed_id(&id, "resp_") || !printable(&model, MAX_MODEL_BYTES) {
        return Err(Diagnostic::ResponseInvalid);
    }
    if status == "incomplete" || !incomplete_details.is_null() {
        return Err(Diagnostic::ResponseIncomplete);
    }
    if status != "completed" || !error.is_null() {
        return Err(Diagnostic::ResponseInvalid);
    }
    let content = output_content(&output)?;
    let usage = usage
        .map(|usage| normalize_usage(usage, max_output_tokens))
        .transpose()?;
    Ok(Response {
        id,
        model,
        content,
        usage,
    })
}

fn required_value<'de, D>(deserializer: D) -> Result<Value, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Value::deserialize(deserializer)
}

fn normalize_usage(usage: ProviderUsage, max_output_tokens: u64) -> Result<Usage, Diagnostic> {
    let total = usage
        .input_tokens
        .checked_add(usage.output_tokens)
        .ok_or(Diagnostic::ResponseInvalid)?;
    if usage.total_tokens != total
        || usage.output_tokens > max_output_tokens
        || usage
            .input_tokens_details
            .as_ref()
            .is_some_and(|details| details.cached_tokens > usage.input_tokens)
        || usage
            .input_tokens_details
            .as_ref()
            .and_then(|details| details.cache_write_tokens)
            .is_some_and(|written| written > usage.input_tokens)
        || usage
            .output_tokens_details
            .as_ref()
            .is_some_and(|details| details.reasoning_tokens > usage.output_tokens)
    {
        return Err(Diagnostic::ResponseInvalid);
    }
    Ok(Usage {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        total_tokens: usage.total_tokens,
        cached_input_tokens: usage
            .input_tokens_details
            .as_ref()
            .map(|details| details.cached_tokens),
        cache_write_input_tokens: usage
            .input_tokens_details
            .as_ref()
            .and_then(|details| details.cache_write_tokens),
        reasoning_output_tokens: usage
            .output_tokens_details
            .as_ref()
            .map(|details| details.reasoning_tokens),
    })
}

fn output_content(output: &[Value]) -> Result<String, Diagnostic> {
    let mut message = None;
    for item in output {
        let object = object(item)?;
        let item_type = string(object, "type")?;
        match item_type {
            "message" => {
                if message.is_some() {
                    return Err(Diagnostic::ResponseInvalid);
                }
                message = Some(message_content(object)?);
            }
            "reasoning" => reasoning(object)?,
            "refusal" => return Err(Diagnostic::ResponseRefused),
            _ => return Err(Diagnostic::ResponseInvalid),
        }
    }
    message.ok_or(Diagnostic::ResponseInvalid)
}

fn message_content(item: &Map<String, Value>) -> Result<String, Diagnostic> {
    closed(item, &["id", "type", "status", "role", "content"])?;
    if !prefixed_id(string(item, "id")?, "msg_")
        || string(item, "type")? != "message"
        || string(item, "status")? != "completed"
        || string(item, "role")? != "assistant"
    {
        return Err(Diagnostic::ResponseInvalid);
    }
    let content = array(item, "content")?;
    if content.len() != 1 {
        return Err(Diagnostic::ResponseInvalid);
    }
    text_part(&content[0])
}

fn reasoning(item: &Map<String, Value>) -> Result<(), Diagnostic> {
    closed_optional(item, &["id", "type", "summary"], "status")?;
    if !printable(string(item, "id")?, MAX_ID_BYTES)
        || string(item, "type")? != "reasoning"
        || !array(item, "summary")?.is_empty()
    {
        return Err(Diagnostic::ResponseInvalid);
    }
    if let Some(status) = item.get("status") {
        if status.as_str() != Some("completed") {
            return Err(Diagnostic::ResponseInvalid);
        }
    }
    Ok(())
}

fn text_part(value: &Value) -> Result<String, Diagnostic> {
    let part = object(value)?;
    match part.get("type").and_then(Value::as_str) {
        Some("refusal") => {
            closed(part, &["type", "refusal"])?;
            string(part, "refusal")?;
            return Err(Diagnostic::ResponseRefused);
        }
        Some("output_text") => {}
        _ => return Err(Diagnostic::ResponseInvalid),
    }
    closed_optional(part, &["type", "text", "annotations"], "logprobs")?;
    if !array(part, "annotations")?.is_empty()
        || part
            .get("logprobs")
            .is_some_and(|value| !matches!(value, Value::Array(values) if values.is_empty()))
    {
        return Err(Diagnostic::ResponseInvalid);
    }
    let text = string(part, "text")?;
    if text.len() > MAX_CONTENT_BYTES {
        return Err(Diagnostic::ResponseTooLarge);
    }
    Ok(text.to_owned())
}

fn object(value: &Value) -> Result<&Map<String, Value>, Diagnostic> {
    value.as_object().ok_or(Diagnostic::ResponseInvalid)
}

fn string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, Diagnostic> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or(Diagnostic::ResponseInvalid)
}

fn array<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a Vec<Value>, Diagnostic> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or(Diagnostic::ResponseInvalid)
}

fn closed(object: &Map<String, Value>, required: &[&str]) -> Result<(), Diagnostic> {
    if object.len() != required.len() || required.iter().any(|key| !object.contains_key(*key)) {
        return Err(Diagnostic::ResponseInvalid);
    }
    Ok(())
}

fn closed_optional(
    object: &Map<String, Value>,
    required: &[&str],
    optional: &str,
) -> Result<(), Diagnostic> {
    if object.len() < required.len()
        || object.len() > required.len() + 1
        || required.iter().any(|key| !object.contains_key(*key))
        || object
            .keys()
            .any(|key| key != optional && !required.contains(&key.as_str()))
    {
        return Err(Diagnostic::ResponseInvalid);
    }
    Ok(())
}

fn prefixed_id(value: &str, prefix: &str) -> bool {
    value.starts_with(prefix) && value.len() > prefix.len() && printable(value, MAX_ID_BYTES)
}

fn printable(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
