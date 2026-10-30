//! Bounded local conversion metadata. These records are not authorization.
use super::Error;
use rangoon_desktop::workspace_keys::WorkspaceId;
use rangoon_store::WorkflowRecoveryCounts;
use serde::{Deserialize, Serialize};

pub(super) const LIMIT: usize = 4096;
pub(super) const SCHEMA: &str = "rangoon.workspace-adoption.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Phase {
    Intent,
    Target,
    Copied,
    Reopened,
    Selected,
}
impl Phase {
    pub(super) const ALL: [Self; 5] = [
        Self::Intent,
        Self::Target,
        Self::Copied,
        Self::Reopened,
        Self::Selected,
    ];
    pub(super) fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|p| *p == self)
            .expect("closed phase")
    }
    pub(super) fn name(self) -> &'static str {
        [
            "intent.json",
            "target.json",
            "copied.json",
            "reopened.json",
            "selected.json",
        ][self.index()]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Inventory {
    sources: u32,
    capabilities: u32,
    revisions: u32,
    reviews: u32,
    recipes: u32,
    applications: u32,
    derivations: u32,
    workflows: u32,
    workflow_revisions: u32,
}
impl From<&WorkflowRecoveryCounts> for Inventory {
    fn from(value: &WorkflowRecoveryCounts) -> Self {
        Self {
            sources: value.sources,
            capabilities: value.capabilities,
            revisions: value.revisions,
            reviews: value.reviews,
            recipes: value.recipes,
            applications: value.applications,
            derivations: value.derivations,
            workflows: value.workflows,
            workflow_revisions: value.workflow_revisions,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Record {
    pub(super) schema_version: String,
    pub(super) phase: Phase,
    pub(super) attempt_id: String,
    pub(super) creator_process: String,
    pub(super) previous_digest: Option<String>,
    pub(super) source_identity: String,
    pub(super) source_archive: String,
    pub(super) source_schema: u32,
    pub(super) target_schema: u32,
    pub(super) inventory: Inventory,
    pub(super) destination: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) verifier_process: Option<String>,
}
impl std::fmt::Debug for Record {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AdoptionRecord(<redacted>)")
    }
}
pub(super) fn hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl Record {
    pub(super) fn encode(&self) -> Result<Vec<u8>, Error> {
        let bytes = serde_json::to_vec(self).map_err(|_| Error::RecoveryRequired)?;
        Self::decode(&bytes)?;
        Ok(bytes)
    }
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.is_empty() || bytes.len() > LIMIT {
            return Err(Error::RecoveryRequired);
        }
        // The typed pass rejects duplicates before any Value-based presence check.
        let value: Self = serde_json::from_slice(bytes).map_err(|_| Error::RecoveryRequired)?;
        let presence: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| Error::RecoveryRequired)?;
        let object = presence.as_object().ok_or(Error::RecoveryRequired)?;
        let base = [
            "schemaVersion",
            "phase",
            "attemptId",
            "creatorProcess",
            "previousDigest",
            "sourceIdentity",
            "sourceArchive",
            "sourceSchema",
            "targetSchema",
            "inventory",
            "destination",
        ];
        let verified = matches!(value.phase, Phase::Reopened | Phase::Selected);
        if object.len() != base.len() + usize::from(verified)
            || base.iter().any(|key| !object.contains_key(*key))
            || object.contains_key("verifierProcess") != verified
            || value.schema_version != SCHEMA
            || WorkspaceId::parse(&value.attempt_id).is_err()
            || WorkspaceId::parse(&value.creator_process).is_err()
            || !hex(&value.source_identity)
            || !hex(&value.source_archive)
            || value.source_schema > 4
            || value.target_schema != value.source_schema.max(1)
            || (value.phase == Phase::Intent
                && (value.previous_digest.is_some() || value.destination.is_some()))
            || (value.phase != Phase::Intent
                && (!value.previous_digest.as_deref().is_some_and(hex)
                    || !value
                        .destination
                        .as_deref()
                        .is_some_and(|id| WorkspaceId::parse(id).is_ok())))
            || (verified
                && !value.verifier_process.as_deref().is_some_and(|id| {
                    WorkspaceId::parse(id).is_ok() && id != value.creator_process
                }))
        {
            return Err(Error::RecoveryRequired);
        }
        Ok(value)
    }
    pub(super) fn same_binding(&self, previous: &Self) -> bool {
        self.attempt_id == previous.attempt_id
            && self.creator_process == previous.creator_process
            && self.source_identity == previous.source_identity
            && self.source_archive == previous.source_archive
            && self.source_schema == previous.source_schema
            && self.target_schema == previous.target_schema
            && self.inventory == previous.inventory
            && (previous.phase == Phase::Intent || self.destination == previous.destination)
            && (self.phase != Phase::Selected || self.verifier_process == previous.verifier_process)
    }
}

#[cfg(test)]
#[path = "workspace_adoption_record_tests.rs"]
mod tests;
