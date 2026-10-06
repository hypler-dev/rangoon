use crate::Diagnostic;
use rangoon_domain::byte_digest;
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

pub(crate) const ADAPTER: &str = "ollama-loopback.v1";

/// An immutable, explicitly configured numeric-loopback connection target.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalProfile {
    config: Config,
    profile_sha256: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    schema_version: &'static str,
    adapter: &'static str,
    profile_id: String,
    host: String,
    port: u16,
    model: String,
    max_output_tokens: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    schema_version: String,
    profile_id: String,
    host: String,
    port: u16,
    model: String,
    max_output_tokens: u64,
}

impl LocalProfile {
    /// Accepts a closed profile request without opening a connection.
    pub fn parse(raw: &[u8]) -> Result<Self, Diagnostic> {
        if raw.len() > 1024 {
            return Err(Diagnostic::InvalidProfile);
        }
        let input: Input = serde_json::from_slice(raw).map_err(|_| Diagnostic::InvalidProfile)?;
        if input.schema_version != "rangoon.local-profile-request.v1"
            || !(1..=64).contains(&input.profile_id.len())
            || !input
                .profile_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
            || !matches!(input.host.as_str(), "127.0.0.1" | "::1")
            || input.port == 0
            || !valid_model(&input.model)
            || !(1..=32_768).contains(&input.max_output_tokens)
        {
            return Err(Diagnostic::InvalidProfile);
        }
        let config = Config {
            schema_version: "rangoon.local-profile.v1",
            adapter: ADAPTER,
            profile_id: input.profile_id,
            host: input.host,
            port: input.port,
            model: input.model,
            max_output_tokens: input.max_output_tokens,
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
    pub fn origin(&self) -> String {
        format!("http://{}", self.host_header())
    }

    pub(crate) fn socket_addr(&self) -> SocketAddr {
        match self.config.host.as_str() {
            "127.0.0.1" => SocketAddr::from((Ipv4Addr::LOCALHOST, self.config.port)),
            // The only other constructible host is the IPv6 loopback literal.
            _ => SocketAddr::from((Ipv6Addr::LOCALHOST, self.config.port)),
        }
    }

    pub(crate) fn host_header(&self) -> String {
        self.socket_addr().to_string()
    }
}

fn valid_model(model: &str) -> bool {
    fn segment(value: &str) -> bool {
        value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
            && value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    }
    (1..=128).contains(&model.len())
        && model
            .split_once(':')
            .is_some_and(|(name, tag)| name.split('/').all(segment) && segment(tag))
}
