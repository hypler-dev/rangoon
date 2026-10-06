//! An explicit source-free metadata request, never implicit model discovery.
use crate::{CloudProfile, Diagnostic, request::valid_revision};
use rangoon_domain::byte_digest;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckRequest {
    request_id: String,
    binding: Binding,
    #[serde(skip_serializing)]
    pub(crate) profile: CloudProfile,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Binding {
    schema_version: &'static str,
    adapter: &'static str,
    profile_sha256: String,
    origin: &'static str,
    method: &'static str,
    path: String,
    credential_revision: String,
    body_sha256: String,
    body_bytes: usize,
}
impl CheckRequest {
    pub fn new(profile: &CloudProfile, credential_revision: &str) -> Result<Self, Diagnostic> {
        if !valid_revision(credential_revision) {
            return Err(Diagnostic::InvalidCredential);
        }
        let binding = Binding {
            schema_version: "rangoon.cloud-check-request.v1",
            adapter: crate::profile::ADAPTER,
            profile_sha256: profile.profile_sha256().to_owned(),
            origin: profile.origin(),
            method: "GET",
            path: format!("/v1/models/{}", profile.model()),
            credential_revision: credential_revision.to_owned(),
            body_sha256: byte_digest(b""),
            body_bytes: 0,
        };
        let bytes = serde_json::to_vec(&binding).map_err(|_| Diagnostic::InvalidProfile)?;
        let mut framed = b"rangoon.cloud-check-request.v1\0".to_vec();
        framed.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        framed.extend_from_slice(&bytes);
        Ok(Self {
            request_id: byte_digest(&framed),
            binding,
            profile: profile.clone(),
        })
    }
    pub fn request_id(&self) -> &str {
        &self.request_id
    }
    pub fn path(&self) -> &str {
        &self.binding.path
    }
    pub fn credential_revision(&self) -> &str {
        &self.binding.credential_revision
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub(crate) schema_version: &'static str,
    pub(crate) request_id: String,
    pub(crate) profile_sha256: String,
    pub(crate) credential_revision: String,
    pub(crate) response_sha256: String,
    pub(crate) observed_model: String,
    pub(crate) owned_by: String,
    pub(crate) created: u64,
    pub(crate) shutdown_date: Option<String>,
    pub(crate) observation: &'static str,
    pub(crate) inference_compatibility: &'static str,
    pub(crate) processing_location: &'static str,
    pub(crate) retention: &'static str,
    pub(crate) cost: &'static str,
    pub(crate) authority: &'static str,
}
