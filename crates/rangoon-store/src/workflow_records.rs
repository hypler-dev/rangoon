//! Internal W2b row validation foundation. No production schema gate or writer.
use super::{Connection, StoreError, composition_records::Records};
use rangoon_domain::{byte_digest, capability::valid_id, capability_v1::RevisionSummary};
use rangoon_workflow::{
    Operation,
    records::{SaveIntent, WorkflowRevision, decode_revision},
};
use std::collections::{BTreeMap, BTreeSet};

const MAX_WORKFLOWS: usize = 128;
const MAX_HISTORY: usize = 32;
const MAX_REVISIONS: usize = 1024;
const MAX_RECORD_BYTES: usize = 160 * 1024;
pub(super) const SCHEMAS: &[(&str, &str)] = &[
    (
        "workflows",
        "CREATE TABLE workflows (workflow_id TEXT PRIMARY KEY NOT NULL, latest_revision_id TEXT NOT NULL)",
    ),
    (
        "workflow_revisions",
        "CREATE TABLE workflow_revisions (revision_id TEXT PRIMARY KEY NOT NULL, workflow_id TEXT NOT NULL, record_json BLOB NOT NULL, saved_at_ms INTEGER NOT NULL)",
    ),
];

#[derive(Clone, Debug)]
pub(super) struct SavedRevision {
    pub record: WorkflowRevision,
    pub saved_at_ms: i64,
}

#[derive(Clone, Debug, Default)]
pub(super) struct WorkflowRows {
    pub owners: BTreeMap<String, String>,
    pub revisions: BTreeMap<String, SavedRevision>,
    histories: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReferenceStatus {
    InvalidReference,
    MissingCapability,
    MissingRevision,
    Resolved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UnionError {
    Invalid,
    Full,
    DependencyMissing,
}

impl WorkflowRows {
    /// The caller must hold a consistent read transaction and provide the fully
    /// validated capability records from that transaction. This only validates
    /// the two additional tables; it does not qualify the complete schema.
    pub fn load(db: &Connection, base: &Records) -> Result<Self, StoreError> {
        for (name, expected) in SCHEMAS {
            let actual: String = db
                .query_row(
                    "SELECT sql FROM sqlite_schema WHERE type='table' AND name=?1",
                    [name],
                    |row| row.get(0),
                )
                .map_err(|_| StoreError::Corrupt)?;
            if actual != *expected {
                return Err(StoreError::Corrupt);
            }
        }
        for (sql, limit) in [
            ("SELECT count(*) FROM workflows", MAX_WORKFLOWS),
            ("SELECT count(*) FROM workflow_revisions", MAX_REVISIONS),
        ] {
            let count: i64 = db.query_row(sql, [], |row| row.get(0))?;
            if count < 0 || count as u64 > limit as u64 {
                return Err(StoreError::Corrupt);
            }
        }
        // SQLite affinities do not ensure storage types. Inspect types and byte
        // lengths before copying attacker-controlled values into Rust buffers.
        for sql in [
            "SELECT EXISTS(SELECT 1 FROM workflows WHERE typeof(workflow_id)!='text' OR length(CAST(workflow_id AS BLOB))!=73 OR typeof(latest_revision_id)!='text' OR length(CAST(latest_revision_id AS BLOB))!=82)",
            "SELECT EXISTS(SELECT 1 FROM workflow_revisions WHERE typeof(revision_id)!='text' OR length(CAST(revision_id AS BLOB))!=82 OR typeof(workflow_id)!='text' OR length(CAST(workflow_id AS BLOB))!=73 OR typeof(record_json)!='blob' OR length(record_json)>163840 OR typeof(saved_at_ms)!='integer' OR saved_at_ms<0 OR saved_at_ms>8640000000000000)",
        ] {
            if db.query_row(sql, [], |row| row.get::<_, bool>(0))? {
                return Err(StoreError::Corrupt);
            }
        }
        let mut result = Self::default();
        let mut owners = db.prepare(
            "SELECT workflow_id,latest_revision_id FROM workflows ORDER BY workflow_id LIMIT 129",
        )?;
        let mut rows = owners.query([])?;
        while let Some(row) = rows.next()? {
            let id: String = row.get(0).map_err(|_| StoreError::Corrupt)?;
            let head: String = row.get(1).map_err(|_| StoreError::Corrupt)?;
            if !valid_id(&id, "workflow:")
                || !valid_id(&head, "workflow-revision:")
                || result.owners.insert(id, head).is_some()
            {
                return Err(StoreError::Corrupt);
            }
        }
        let mut revisions = db.prepare("SELECT revision_id,workflow_id,record_json,saved_at_ms FROM workflow_revisions ORDER BY revision_id LIMIT 1025")?;
        let mut rows = revisions.query([])?;
        while let Some(row) = rows.next()? {
            let id: String = row.get(0).map_err(|_| StoreError::Corrupt)?;
            let owner: String = row.get(1).map_err(|_| StoreError::Corrupt)?;
            let bytes: Vec<u8> = row.get(2).map_err(|_| StoreError::Corrupt)?;
            let saved_at_ms: i64 = row.get(3).map_err(|_| StoreError::Corrupt)?;
            if bytes.len() > MAX_RECORD_BYTES {
                return Err(StoreError::Corrupt);
            }
            let record = decode_revision(&bytes).map_err(|_| StoreError::Corrupt)?;
            if record.id() != id
                || record.workflow_id() != owner
                || record.serialized_bytes().map_err(|_| StoreError::Corrupt)? != bytes
                || result
                    .revisions
                    .insert(
                        id,
                        SavedRevision {
                            record,
                            saved_at_ms,
                        },
                    )
                    .is_some()
            {
                return Err(StoreError::Corrupt);
            }
        }
        result.validate(base)?;
        Ok(result)
    }

    fn validate(&mut self, base: &Records) -> Result<(), StoreError> {
        self.validate_history()?;
        if !self.dependencies_resolve(base) {
            return Err(StoreError::Corrupt);
        }
        Ok(())
    }

    fn validate_history(&mut self) -> Result<(), StoreError> {
        if self.owners.len() > MAX_WORKFLOWS || self.revisions.len() > MAX_REVISIONS {
            return Err(StoreError::Corrupt);
        }
        for (id, saved) in &self.revisions {
            if id != saved.record.id() {
                return Err(StoreError::Corrupt);
            }
        }
        self.histories.clear();
        let mut visited = BTreeSet::new();
        for (owner, head) in &self.owners {
            if !valid_id(owner, "workflow:") || !valid_id(head, "workflow-revision:") {
                return Err(StoreError::Corrupt);
            }
            let mut chain = Vec::new();
            let mut next = Some(head.as_str());
            while let Some(id) = next {
                if chain.len() == MAX_HISTORY || !visited.insert(id.to_owned()) {
                    return Err(StoreError::Corrupt);
                }
                let saved = self.revisions.get(id).ok_or(StoreError::Corrupt)?;
                if saved.record.workflow_id() != owner
                    || !(0..=8_640_000_000_000_000).contains(&saved.saved_at_ms)
                {
                    return Err(StoreError::Corrupt);
                }
                chain.push(id.to_owned());
                next = saved.record.parent_revision_id();
            }
            chain.reverse();
            self.histories.insert(owner.clone(), chain);
        }
        // Every row must appear exactly once in its owner's head-to-root chain.
        // This rejects unowned roots, forks and disconnected history fragments.
        if visited.len() != self.revisions.len() {
            return Err(StoreError::Corrupt);
        }
        Ok(())
    }

    fn dependencies_resolve(&self, base: &Records) -> bool {
        self.revisions.values().all(|saved| {
            saved.record.intent() != SaveIntent::Validated
                || references(&saved.record, base)
                    .iter()
                    .all(|(_, status)| *status == ReferenceStatus::Resolved)
        })
    }

    /// Combine already decoded owner histories against the final, fully
    /// validated capability union. Never append to or retarget a kept owner.
    /// This prepares records only; it performs no SQL or schema mutation.
    pub fn union(&self, incoming: &Self, final_base: &Records) -> Result<(Self, u32), UnionError> {
        let mut result = self.clone();
        let mut incoming = incoming.clone();
        result.validate_history().map_err(|_| UnionError::Invalid)?;
        incoming
            .validate_history()
            .map_err(|_| UnionError::Invalid)?;
        let mut kept = 0;
        for (owner, head) in &incoming.owners {
            let incoming_chain = &incoming.histories[owner];
            if let Some(local_chain) = result.histories.get(owner) {
                if !local_chain.starts_with(incoming_chain)
                    || incoming_chain.iter().any(|id| {
                        let local = &result.revisions[id];
                        let other = &incoming.revisions[id];
                        local.saved_at_ms != other.saved_at_ms || local.record != other.record
                    })
                {
                    return Err(UnionError::Invalid);
                }
                kept += 1;
            } else {
                result.owners.insert(owner.clone(), head.clone());
                for id in incoming_chain {
                    if result
                        .revisions
                        .insert(id.clone(), incoming.revisions[id].clone())
                        .is_some()
                    {
                        return Err(UnionError::Invalid);
                    }
                }
            }
        }
        if result.owners.len() > MAX_WORKFLOWS || result.revisions.len() > MAX_REVISIONS {
            return Err(UnionError::Full);
        }
        result.validate_history().map_err(|_| UnionError::Invalid)?;
        if !result.dependencies_resolve(final_base) {
            return Err(UnionError::DependencyMissing);
        }
        Ok((result, kept))
    }

    /// Exact schema-4 stale-state identity. The caller supplies validated base
    /// records; legacy schema-1/2/3 state serialization remains unchanged.
    pub fn state_id_v4(&self, base: &Records) -> Result<String, StoreError> {
        self.clone().validate(base)?;
        let sources: Vec<_> = base.sources.values().map(|s| &s.metadata).collect();
        let revisions: BTreeMap<_, _> = base
            .revisions
            .iter()
            .map(|(id, r)| (id, (&r.capability_id, RevisionSummary::from(&r.revision))))
            .collect();
        let recipes: BTreeMap<_, _> = base
            .recipes
            .iter()
            .map(|(id, r)| (id, r.created_at_ms))
            .collect();
        let applications: BTreeMap<_, _> = base
            .applications
            .iter()
            .map(|(id, a)| (id, (&a.composition_id, a.created_at_ms)))
            .collect();
        let workflow_revisions: BTreeMap<_, _> = self
            .revisions
            .iter()
            .map(|(id, r)| (id, (r.record.workflow_id(), r.saved_at_ms)))
            .collect();
        let bytes = serde_json::to_vec(&(
            "rangoon.workspace-state.v2",
            4,
            sources,
            &base.owners,
            revisions,
            recipes,
            applications,
            &self.owners,
            workflow_revisions,
        ))
        .map_err(|_| StoreError::Corrupt)?;
        Ok(format!("workspace:{}", byte_digest(&bytes)))
    }

    pub fn history(&self, owner: &str) -> Option<&[String]> {
        self.histories.get(owner).map(Vec::as_slice)
    }
}

pub(super) fn references(record: &WorkflowRevision, base: &Records) -> Vec<(u32, ReferenceStatus)> {
    record
        .inspection()
        .definition()
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            let Operation::Capability {
                capability_id,
                revision_id,
            } = &node.operation
            else {
                return None;
            };
            let status =
                if !valid_id(capability_id, "capability:") || !valid_id(revision_id, "revision:") {
                    ReferenceStatus::InvalidReference
                } else if !base.owners.contains_key(capability_id) {
                    ReferenceStatus::MissingCapability
                } else if base
                    .revisions
                    .get(revision_id)
                    .is_none_or(|revision| revision.capability_id != *capability_id)
                {
                    ReferenceStatus::MissingRevision
                } else {
                    ReferenceStatus::Resolved
                };
            Some((index as u32, status))
        })
        .collect()
}

#[cfg(test)]
#[path = "workflow_records_tests.rs"]
mod tests;
