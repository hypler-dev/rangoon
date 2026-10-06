use crate::Diagnostic;
use rangoon_domain::byte_digest;
use serde::{Deserialize, Serialize};

pub(crate) const ADAPTER: &str = "openai-responses.v1";

/// An immutable, explicitly configured fixed-origin cloud target.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudProfile {
    config: Config,
    profile_sha256: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    schema_version: &'static str,
    adapter: &'static str,
    profile_id: String,
    model: String,
    max_output_tokens: u64,
    origin: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    schema_version: String,
    profile_id: String,
    model: String,
    max_output_tokens: u64,
}

impl CloudProfile {
    /// Accepts a closed profile request without opening a connection.
    pub fn parse(raw: &[u8]) -> Result<Self, Diagnostic> {
        if raw.len() > 1024 {
            return Err(Diagnostic::InvalidProfile);
        }
        let input: Input = serde_json::from_slice(raw).map_err(|_| Diagnostic::InvalidProfile)?;
        if input.schema_version != "rangoon.cloud-profile-request.v1"
            || !(1..=64).contains(&input.profile_id.len())
            || !input
                .profile_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
            || !valid_model(&input.model)
            || !(1..=32_768).contains(&input.max_output_tokens)
        {
            return Err(Diagnostic::InvalidProfile);
        }
        let config = Config {
            schema_version: "rangoon.cloud-profile.v1",
            adapter: ADAPTER,
            profile_id: input.profile_id,
            model: input.model,
            max_output_tokens: input.max_output_tokens,
            origin: "https://api.openai.com",
        };
        let canonical = serde_json::to_vec(&config).map_err(|_| Diagnostic::InvalidProfile)?;
        Ok(Self {
            config,
            profile_sha256: byte_digest(&canonical),
        })
    }

    pub fn profile_id(&self) -> &str {
        &self.config.profile_id
    }
    pub fn profile_sha256(&self) -> &str {
        &self.profile_sha256
    }
    pub fn model(&self) -> &str {
        &self.config.model
    }
    pub fn max_output_tokens(&self) -> u64 {
        self.config.max_output_tokens
    }
    pub fn origin(&self) -> &'static str {
        self.config.origin
    }
}

fn valid_model(model: &str) -> bool {
    (1..=128).contains(&model.len())
        && model
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && model
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
}
