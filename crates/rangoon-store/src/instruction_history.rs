//! Exact instruction restore points. Persistence is encrypted-only; no file writes.
use super::*;
use composition_records::Records;
use rangoon_compose::InputReference;
use rangoon_domain::{AnalysisReport, byte_digest, capability::valid_id};
use std::collections::{BTreeMap, BTreeSet};
use zeroize::Zeroize;

pub const MAX_INSTRUCTION_CHANGES: usize = 128;
pub const MAX_INSTRUCTION_EVENTS: usize = 512;
pub const MAX_INSTRUCTION_BYTES: usize = 256 * 1024;
pub const MAX_INSTRUCTION_HISTORY_BYTES: usize = 16 * 1024 * 1024;
pub(super) const MAX_RECORD_BYTES: usize = 2 * MAX_INSTRUCTION_BYTES * 4 + 16 * 1024;
pub(super) const SCHEMAS: &[(&str, &str)] = &[
    (
        "instruction_changes",
        "CREATE TABLE instruction_changes (change_id TEXT PRIMARY KEY NOT NULL, record_json BLOB NOT NULL)",
    ),
    (
        "instruction_events",
        "CREATE TABLE instruction_events (event_id TEXT PRIMARY KEY NOT NULL, record_json BLOB NOT NULL)",
    ),
];

/// Native-selected identities, never evidence that filesystem custody was acquired.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstructionTarget {
    pub workspace_identity: String,
    pub target_identity: String,
    pub parent_identity: String,
    pub file_name: String,
    pub file_identity: Option<String>,
}
impl InstructionTarget {
    fn valid(&self) -> bool {
        [
            &self.workspace_identity,
            &self.target_identity,
            &self.parent_identity,
        ]
        .iter()
        .all(|id| valid_id(id, ""))
            && self
                .file_identity
                .as_ref()
                .is_none_or(|id| valid_id(id, ""))
            && !self.file_name.is_empty()
            && self.file_name.len() <= 255
            && !matches!(self.file_name.as_str(), "." | "..")
            && !self
                .file_name
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '\u{2028}' | '\u{2029}'))
    }
    fn same_location(&self, other: &Self) -> bool {
        self.workspace_identity == other.workspace_identity
            && self.target_identity == other.target_identity
            && self.parent_identity == other.parent_identity
            && self.file_name == other.file_name
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ChangeWire {
    schema_version: String,
    target: InstructionTarget,
    before: Option<Vec<u8>>,
    after: Option<Vec<u8>>,
    before_sha256: Option<String>,
    after_sha256: Option<String>,
    inputs: Vec<InputReference>,
    composition_id: Option<String>,
    parent_change_id: Option<String>,
    created_at_ms: i64,
}
impl Drop for ChangeWire {
    fn drop(&mut self) {
        if let Some(bytes) = &mut self.before {
            bytes.zeroize();
        }
        if let Some(bytes) = &mut self.after {
            bytes.zeroize();
        }
    }
}

/// A native-host value with private validated bytes. Never renderer JSON input.
#[derive(Clone, PartialEq, Eq)]
pub struct InstructionChange {
    id: String,
    wire: ChangeWire,
}
impl std::fmt::Debug for InstructionChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InstructionChange(<redacted>)")
    }
}
impl InstructionChange {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        target: InstructionTarget,
        before: Option<&[u8]>,
        after: Option<&[u8]>,
        inputs: Vec<InputReference>,
        composition_id: Option<String>,
        parent_change_id: Option<String>,
        created_at_ms: i64,
    ) -> Result<Self, StoreError> {
        // Check lengths before copying native-captured originals.
        if before.is_some_and(|b| b.len() > MAX_INSTRUCTION_BYTES)
            || after.is_some_and(|b| b.len() > MAX_INSTRUCTION_BYTES)
        {
            return Err(StoreError::InstructionHistoryInvalid);
        }
        let wire = ChangeWire {
            schema_version: "rangoon.instruction-change.v1".into(),
            target,
            before: before.map(<[u8]>::to_vec),
            after: after.map(<[u8]>::to_vec),
            before_sha256: before.map(byte_digest),
            after_sha256: after.map(byte_digest),
            inputs,
            composition_id,
            parent_change_id,
            created_at_ms,
        };
        Self::from_wire(wire)
    }
    fn from_wire(wire: ChangeWire) -> Result<Self, StoreError> {
        if wire.schema_version != "rangoon.instruction-change.v1"
            || !wire.target.valid()
            || wire.before == wire.after
            || wire.target.file_identity.is_some() != wire.before.is_some()
            || wire
                .before
                .as_ref()
                .is_some_and(|b| b.len() > MAX_INSTRUCTION_BYTES)
            || wire
                .after
                .as_ref()
                .is_some_and(|b| b.len() > MAX_INSTRUCTION_BYTES)
            || wire.before.as_deref().map(byte_digest) != wire.before_sha256
            || wire.after.as_deref().map(byte_digest) != wire.after_sha256
            || wire.inputs.len() > 16
            || !valid_time(wire.created_at_ms)
            || wire
                .composition_id
                .as_ref()
                .is_some_and(|id| !valid_id(id, "composition:"))
            || wire
                .parent_change_id
                .as_ref()
                .is_some_and(|id| !valid_id(id, "instruction-change:"))
        {
            return Err(StoreError::InstructionHistoryInvalid);
        }
        let mut unique = BTreeSet::new();
        for input in &wire.inputs {
            let (id, digest) = match input {
                InputReference::Source { source_id, sha256 } if valid_id(source_id, "source:") => {
                    (source_id, sha256)
                }
                InputReference::Revision {
                    capability_id,
                    revision_id,
                    sha256,
                } if valid_id(capability_id, "capability:")
                    && valid_id(revision_id, "revision:") =>
                {
                    (revision_id, sha256)
                }
                _ => return Err(StoreError::InstructionHistoryInvalid),
            };
            if !valid_id(digest, "") || !unique.insert(id) {
                return Err(StoreError::InstructionHistoryInvalid);
            }
        }
        let bytes = zeroize::Zeroizing::new(
            serde_json::to_vec(&wire).map_err(|_| StoreError::InstructionHistoryInvalid)?,
        );
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(StoreError::InstructionHistoryInvalid);
        }
        Ok(Self {
            id: format!("instruction-change:{}", byte_digest(&bytes)),
            wire,
        })
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn target(&self) -> &InstructionTarget {
        &self.wire.target
    }
    pub fn parent_change_id(&self) -> Option<&str> {
        self.wire.parent_change_id.as_deref()
    }
    pub fn inputs(&self) -> &[InputReference] {
        &self.wire.inputs
    }
    pub fn composition_id(&self) -> Option<&str> {
        self.wire.composition_id.as_deref()
    }
    pub fn created_at_ms(&self) -> i64 {
        self.wire.created_at_ms
    }
    /// Trusted host access to exact originals. No renderer command is registered.
    pub fn before(&self) -> Option<&[u8]> {
        self.wire.before.as_deref()
    }
    pub fn after(&self) -> Option<&[u8]> {
        self.wire.after.as_deref()
    }
    pub fn before_sha256(&self) -> Option<&str> {
        self.wire.before_sha256.as_deref()
    }
    pub fn after_sha256(&self) -> Option<&str> {
        self.wire.after_sha256.as_deref()
    }
    /// Reuses the deterministic analyzer; unsupported/binary bytes remain archived.
    /// Its fragments and categories are advisory, never restoration authority.
    pub fn analyze_after(&self) -> Result<AnalysisReport, StoreError> {
        analyze(
            &self.wire.target.file_name,
            self.after().ok_or(StoreError::InstructionHistoryInvalid)?,
        )
        .map_err(|_| StoreError::InstructionHistoryInvalid)
    }
    pub(super) fn bytes(&self) -> Result<zeroize::Zeroizing<Vec<u8>>, StoreError> {
        Ok(zeroize::Zeroizing::new(
            serde_json::to_vec(&self.wire).map_err(|_| StoreError::Corrupt)?,
        ))
    }
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, StoreError> {
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(StoreError::InstructionHistoryInvalid);
        }
        let wire =
            serde_json::from_slice(bytes).map_err(|_| StoreError::InstructionHistoryInvalid)?;
        let value = Self::from_wire(wire)?;
        if value.bytes()?.as_slice() != bytes {
            return Err(StoreError::InstructionHistoryInvalid);
        }
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionPhase {
    Attempted,
    Verified,
    Failed,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Observation {
    file_identity: Option<String>,
    sha256: Option<String>,
    byte_length: Option<u32>,
}
impl Observation {
    fn capture(file_identity: Option<String>, bytes: Option<&[u8]>) -> Result<Self, StoreError> {
        if file_identity.is_some() != bytes.is_some()
            || file_identity.as_ref().is_some_and(|id| !valid_id(id, ""))
            || bytes.is_some_and(|b| b.len() > MAX_INSTRUCTION_BYTES)
        {
            return Err(StoreError::InstructionHistoryInvalid);
        }
        Ok(Self {
            file_identity,
            sha256: bytes.map(byte_digest),
            byte_length: bytes.map(|b| b.len() as u32),
        })
    }
    fn valid(&self) -> bool {
        match (&self.file_identity, &self.sha256, self.byte_length) {
            (None, None, None) => true,
            (Some(id), Some(sha), Some(len)) => {
                valid_id(id, "") && valid_id(sha, "") && len as usize <= MAX_INSTRUCTION_BYTES
            }
            _ => false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EventWire {
    schema_version: String,
    change_id: String,
    parent_event_id: Option<String>,
    phase: InstructionPhase,
    observation: Option<Observation>,
    observed_at_ms: i64,
}
#[derive(Clone, PartialEq, Eq)]
pub struct InstructionEvent {
    id: String,
    wire: EventWire,
}
impl std::fmt::Debug for InstructionEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InstructionEvent(<redacted>)")
    }
}
impl InstructionEvent {
    /// Trusted native caller supplies readback bytes. This value is not an
    /// authorization receipt or proof that the caller acquired exclusive custody.
    pub fn prepare(
        change_id: String,
        parent_event_id: Option<String>,
        phase: InstructionPhase,
        observation: Option<(Option<String>, Option<&[u8]>)>,
        observed_at_ms: i64,
    ) -> Result<Self, StoreError> {
        Self::from_wire(EventWire {
            schema_version: "rangoon.instruction-event.v1".into(),
            change_id,
            parent_event_id,
            phase,
            observation: observation
                .map(|(id, bytes)| Observation::capture(id, bytes))
                .transpose()?,
            observed_at_ms,
        })
    }
    fn from_wire(wire: EventWire) -> Result<Self, StoreError> {
        if wire.schema_version != "rangoon.instruction-event.v1"
            || !valid_id(&wire.change_id, "instruction-change:")
            || wire
                .parent_event_id
                .as_ref()
                .is_some_and(|id| !valid_id(id, "instruction-event:"))
            || !valid_time(wire.observed_at_ms)
            || wire.observation.as_ref().is_some_and(|o| !o.valid())
            || (wire.phase == InstructionPhase::Verified && wire.observation.is_none())
            || (wire.phase == InstructionPhase::Attempted && wire.observation.is_some())
        {
            return Err(StoreError::InstructionHistoryInvalid);
        }
        let bytes = serde_json::to_vec(&wire).map_err(|_| StoreError::InstructionHistoryInvalid)?;
        Ok(Self {
            id: format!("instruction-event:{}", byte_digest(&bytes)),
            wire,
        })
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn phase(&self) -> InstructionPhase {
        self.wire.phase
    }
    pub fn change_id(&self) -> &str {
        &self.wire.change_id
    }
    pub fn parent_event_id(&self) -> Option<&str> {
        self.wire.parent_event_id.as_deref()
    }
    pub fn observed_at_ms(&self) -> i64 {
        self.wire.observed_at_ms
    }
    pub(super) fn bytes(&self) -> Result<Vec<u8>, StoreError> {
        serde_json::to_vec(&self.wire).map_err(|_| StoreError::Corrupt)
    }
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, StoreError> {
        if bytes.len() > 2048 {
            return Err(StoreError::InstructionHistoryInvalid);
        }
        let value = Self::from_wire(
            serde_json::from_slice(bytes).map_err(|_| StoreError::InstructionHistoryInvalid)?,
        )?;
        if value.bytes()? != bytes {
            return Err(StoreError::InstructionHistoryInvalid);
        }
        Ok(value)
    }
}
fn valid_time(t: i64) -> bool {
    (0..=8_640_000_000_000_000).contains(&t)
}

#[derive(Clone, Debug, Default)]
pub(super) struct HistoryRows {
    pub changes: BTreeMap<String, InstructionChange>,
    pub events: BTreeMap<String, InstructionEvent>,
}
impl HistoryRows {
    pub fn load(db: &Connection, base: &Records) -> Result<Self, StoreError> {
        // Enforce the shared budget before copying or decoding any row. Per-table
        // checks alone could exceed the combined bound during reconstruction.
        let total: i64 = db.query_row("SELECT (SELECT coalesce(sum(length(record_json)),0) FROM instruction_changes) + (SELECT coalesce(sum(length(record_json)),0) FROM instruction_events)", [], |row| row.get(0))?;
        if total < 0 || total as u64 > MAX_INSTRUCTION_HISTORY_BYTES as u64 {
            return Err(StoreError::Full);
        }
        let mut result = Self::default();
        for (table, column, max_count, max_bytes) in [
            (
                "instruction_changes",
                "change_id",
                MAX_INSTRUCTION_CHANGES,
                MAX_RECORD_BYTES,
            ),
            (
                "instruction_events",
                "event_id",
                MAX_INSTRUCTION_EVENTS,
                2048,
            ),
        ] {
            let invalid: bool = db.query_row(
                &format!(
                    "SELECT count(*) > ?1 OR coalesce(sum(length(record_json)),0) > ?2 FROM {table}"
                ),
                params![max_count as i64, MAX_INSTRUCTION_HISTORY_BYTES as i64],
                |row| row.get(0),
            )?;
            let malformed: bool = db.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE typeof({column}) != 'text' OR length(CAST({column} AS BLOB)) > 96 OR typeof(record_json) != 'blob' OR length(record_json) > ?1)"), [max_bytes as i64], |row| row.get(0))?;
            if invalid || malformed {
                return Err(StoreError::Corrupt);
            }
            let mut q = db.prepare(&format!(
                "SELECT {column},record_json FROM {table} ORDER BY {column} LIMIT {}",
                max_count + 1
            ))?;
            let mut rows = q.query([])?;
            while let Some(row) = rows.next()? {
                let id: String = row.get(0)?;
                let bytes: zeroize::Zeroizing<Vec<u8>> = zeroize::Zeroizing::new(row.get(1)?);
                if table == "instruction_changes" {
                    let value =
                        InstructionChange::decode(&bytes).map_err(|_| StoreError::Corrupt)?;
                    if value.id != id || result.changes.insert(id, value).is_some() {
                        return Err(StoreError::Corrupt);
                    }
                } else {
                    let value =
                        InstructionEvent::decode(&bytes).map_err(|_| StoreError::Corrupt)?;
                    if value.id != id || result.events.insert(id, value).is_some() {
                        return Err(StoreError::Corrupt);
                    }
                }
            }
        }
        result.validate(base)?;
        Ok(result)
    }
    pub fn validate(&self, base: &Records) -> Result<(), StoreError> {
        if self.changes.len() > MAX_INSTRUCTION_CHANGES
            || self.events.len() > MAX_INSTRUCTION_EVENTS
        {
            return Err(StoreError::Full);
        }
        let mut total = 0usize;
        let mut children = BTreeSet::new();
        for (id, change) in &self.changes {
            if id != change.id() {
                return Err(StoreError::Corrupt);
            }
            total = total
                .checked_add(change.bytes()?.len())
                .ok_or(StoreError::Full)?;
            if let Some(parent_id) = &change.wire.parent_change_id {
                let parent = self
                    .changes
                    .get(parent_id)
                    .ok_or(StoreError::InstructionHistoryInvalid)?;
                if !parent.wire.target.same_location(&change.wire.target)
                    || parent.after() != change.before()
                    || parent.wire.created_at_ms > change.wire.created_at_ms
                {
                    return Err(StoreError::InstructionHistoryInvalid);
                }
                let mut seen = BTreeSet::new();
                let mut cursor = Some(id.as_str());
                while let Some(id) = cursor {
                    if !seen.insert(id) {
                        return Err(StoreError::InstructionHistoryInvalid);
                    }
                    cursor = self
                        .changes
                        .get(id)
                        .ok_or(StoreError::InstructionHistoryInvalid)?
                        .wire
                        .parent_change_id
                        .as_deref();
                }
            }
            for input in &change.wire.inputs {
                let valid = match input {
                    InputReference::Source { source_id, sha256 } => base
                        .sources
                        .get(source_id)
                        .is_some_and(|s| &s.metadata.sha256 == sha256),
                    InputReference::Revision {
                        capability_id,
                        revision_id,
                        sha256,
                    } => base.revisions.get(revision_id).is_some_and(|r| {
                        &r.capability_id == capability_id && &r.revision.sha256 == sha256
                    }),
                };
                if !valid {
                    return Err(StoreError::RecordInUse);
                }
            }
            if change
                .wire
                .composition_id
                .as_ref()
                .is_some_and(|id| !base.recipes.contains_key(id))
            {
                return Err(StoreError::RecordInUse);
            }
        }
        for (id, event) in &self.events {
            if id != event.id() {
                return Err(StoreError::Corrupt);
            }
            total = total
                .checked_add(event.bytes()?.len())
                .ok_or(StoreError::Full)?;
            let change = self
                .changes
                .get(&event.wire.change_id)
                .ok_or(StoreError::InstructionHistoryInvalid)?;
            if event.wire.observed_at_ms < change.wire.created_at_ms {
                return Err(StoreError::InstructionHistoryInvalid);
            }
            match &event.wire.parent_event_id {
                None => {
                    if event.wire.phase != InstructionPhase::Attempted
                        || !children.insert((event.wire.change_id.as_str(), None))
                    {
                        return Err(StoreError::InstructionHistoryInvalid);
                    }
                }
                Some(parent_id) => {
                    let parent = self
                        .events
                        .get(parent_id)
                        .ok_or(StoreError::InstructionHistoryInvalid)?;
                    if parent.wire.change_id != event.wire.change_id
                        || parent.wire.observed_at_ms > event.wire.observed_at_ms
                        || parent.wire.phase != InstructionPhase::Attempted
                        || event.wire.phase == InstructionPhase::Attempted
                        || !children.insert((event.wire.change_id.as_str(), Some(parent_id)))
                    {
                        return Err(StoreError::InstructionHistoryInvalid);
                    }
                }
            }
            if event.wire.phase == InstructionPhase::Verified {
                let observation = event
                    .wire
                    .observation
                    .as_ref()
                    .ok_or(StoreError::InstructionHistoryInvalid)?;
                if observation.sha256.as_deref() != change.after_sha256()
                    || observation.byte_length.map(|n| n as usize)
                        != change.after().map(<[u8]>::len)
                {
                    return Err(StoreError::InstructionHistoryInvalid);
                }
            }
        }
        if total > MAX_INSTRUCTION_HISTORY_BYTES {
            return Err(StoreError::Full);
        }
        Ok(())
    }
    pub fn insert_all(&self, db: &Connection) -> Result<(), StoreError> {
        for (id, change) in &self.changes {
            db.execute(
                "INSERT INTO instruction_changes VALUES (?1,?2)",
                params![id, change.bytes()?.as_slice()],
            )?;
        }
        for (id, event) in &self.events {
            db.execute(
                "INSERT INTO instruction_events VALUES (?1,?2)",
                params![id, event.bytes()?],
            )?;
        }
        Ok(())
    }
    pub fn union(&self, other: &Self, base: &Records) -> Result<Self, StoreError> {
        let mut result = self.clone();
        for (id, value) in &other.changes {
            if result.changes.get(id).is_some_and(|v| v != value) {
                return Err(StoreError::InstructionHistoryInvalid);
            }
            result.changes.insert(id.clone(), value.clone());
        }
        for (id, value) in &other.events {
            if result.events.get(id).is_some_and(|v| v != value) {
                return Err(StoreError::InstructionHistoryInvalid);
            }
            result.events.insert(id.clone(), value.clone());
        }
        result.validate(base)?;
        Ok(result)
    }
    pub fn state_id(&self, base: &Records) -> Result<String, StoreError> {
        self.validate(base)?;
        let bytes = serde_json::to_vec(&(
            "rangoon.workspace-state.v3",
            base.workflows.state_id_v4(base)?,
            self.changes.keys().collect::<Vec<_>>(),
            self.events.keys().collect::<Vec<_>>(),
        ))
        .map_err(|_| StoreError::Corrupt)?;
        Ok(format!("workspace:{}", byte_digest(&bytes)))
    }
}

/// Exact-state deletion preview. Removing history is logical deletion, not erasure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstructionHistoryDeletion {
    id: String,
    expected_state_id: String,
    events: usize,
}
impl InstructionHistoryDeletion {
    pub fn change_id(&self) -> &str {
        &self.id
    }
    pub fn removed_event_count(&self) -> usize {
        self.events
    }
    pub fn expected_state_id(&self) -> &str {
        &self.expected_state_id
    }
}

impl Workspace {
    fn require_history_key(&self) -> Result<(), StoreError> {
        let key = self.key.as_ref().ok_or(StoreError::EncryptionUnavailable)?;
        encryption::verify_backend(key)
    }
    /// Encrypted-only opt-in. Does not change any external instruction file.
    pub fn save_instruction_change(
        &self,
        change: &InstructionChange,
        expected_state_id: &str,
    ) -> Result<String, StoreError> {
        self.require_history_key()?;
        let checked = InstructionChange::decode(&change.bytes()?)?;
        let mut db = self
            .connect_complete_read_only(false)?
            .ok_or(StoreError::WorkspaceChanged)?;
        let tx = db.transaction()?;
        let current = workflow_mutations::read_records(&tx)?;
        let result = prepare_change(&current, &checked, expected_state_id)?;
        tx.commit()?;
        drop(db);
        let mut db = self
            .connect_complete(false)?
            .ok_or(StoreError::WorkspaceChanged)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = workflow_mutations::read_records(&tx)?;
        let fresh = prepare_change(&current, &checked, expected_state_id)?;
        if fresh.state_id()? != result.state_id()? {
            return Err(StoreError::WorkspaceChanged);
        }
        workflow_mutations::ensure_schema(&tx, 5)?;
        tx.set_limit(
            rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH,
            (rangoon_compose::MAX_DRAFT_BYTES + 4096) as i32,
        )?;
        #[cfg(test)]
        composition_recovery_tests::fault_checkpoint(&tx, "after_instruction_history_schema")?;
        if !current.history.changes.contains_key(checked.id()) {
            tx.execute(
                "INSERT INTO instruction_changes VALUES (?1,?2)",
                params![checked.id(), checked.bytes()?.as_slice()],
            )?;
        }
        #[cfg(test)]
        composition_recovery_tests::fault_checkpoint(&tx, "after_instruction_history_change")?;
        let actual = workflow_mutations::read_records(&tx)?;
        if actual.state_id()? != fresh.state_id()? {
            return Err(StoreError::Corrupt);
        }
        workflow_mutations::data(&tx, &actual)?;
        tx.commit()?;
        actual.state_id()
    }
    pub fn append_instruction_event(
        &self,
        event: &InstructionEvent,
        expected_state_id: &str,
    ) -> Result<String, StoreError> {
        self.require_history_key()?;
        let event = InstructionEvent::decode(&event.bytes()?)?;
        let mut db = self
            .connect_complete(false)?
            .ok_or(StoreError::WorkspaceChanged)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = workflow_mutations::read_records(&tx)?;
        if current.version != 5 || current.state_id()? != expected_state_id {
            return Err(StoreError::WorkspaceChanged);
        }
        let mut result = current.clone();
        if result
            .history
            .events
            .insert(event.id().into(), event.clone())
            .is_some()
        {
            return Err(StoreError::InstructionHistoryInvalid);
        }
        result.history.validate(&result)?;
        composition_backup::canonical_database(&result)?;
        tx.execute(
            "INSERT INTO instruction_events VALUES (?1,?2)",
            params![event.id(), event.bytes()?],
        )?;
        #[cfg(test)]
        composition_recovery_tests::fault_checkpoint(&tx, "after_instruction_history_event")?;
        let actual = workflow_mutations::read_records(&tx)?;
        if actual.state_id()? != result.state_id()? {
            return Err(StoreError::Corrupt);
        }
        tx.commit()?;
        actual.state_id()
    }
    pub fn instruction_changes(&self) -> Result<Vec<InstructionChange>, StoreError> {
        self.require_history_key()?;
        let Some(mut db) = self.connect_complete_read_only(false)? else {
            return Ok(Vec::new());
        };
        let tx = db.transaction()?;
        let records = workflow_mutations::read_records(&tx)?;
        let values = records.history.changes.into_values().collect();
        tx.commit()?;
        Ok(values)
    }
    pub fn instruction_events(&self, change_id: &str) -> Result<Vec<InstructionEvent>, StoreError> {
        self.require_history_key()?;
        if !valid_id(change_id, "instruction-change:") {
            return Err(StoreError::InstructionHistoryInvalid);
        }
        let mut db = self
            .connect_complete_read_only(false)?
            .ok_or(StoreError::NotFound)?;
        let tx = db.transaction()?;
        let records = workflow_mutations::read_records(&tx)?;
        if !records.history.changes.contains_key(change_id) {
            return Err(StoreError::NotFound);
        }
        let events = records
            .history
            .events
            .into_values()
            .filter(|e| e.change_id() == change_id)
            .collect();
        tx.commit()?;
        Ok(events)
    }
    pub fn inspect_instruction_restore(
        &self,
        id: &str,
        target: &InstructionTarget,
        current_bytes: Option<&[u8]>,
    ) -> Result<InstructionChange, StoreError> {
        self.require_history_key()?;
        if !valid_id(id, "instruction-change:") || !target.valid() {
            return Err(StoreError::InstructionHistoryInvalid);
        }
        let mut db = self
            .connect_complete_read_only(false)?
            .ok_or(StoreError::NotFound)?;
        let tx = db.transaction()?;
        let records = workflow_mutations::read_records(&tx)?;
        let change = records
            .history
            .changes
            .get(id)
            .ok_or(StoreError::NotFound)?;
        let verified = records
            .history
            .events
            .values()
            .find(|e| e.wire.change_id == id && e.wire.phase == InstructionPhase::Verified)
            .ok_or(StoreError::InstructionHistoryInvalid)?;
        let observed = Observation::capture(target.file_identity.clone(), current_bytes)?;
        if !target.same_location(change.target())
            || Some(&observed) != verified.wire.observation.as_ref()
            || current_bytes != change.after()
        {
            return Err(StoreError::WorkspaceChanged);
        }
        // Eligibility only. Native custody, single-use preview and file replacement
        // are deliberately not implemented by this read-only inspection.
        let value = change.clone();
        tx.commit()?;
        Ok(value)
    }
    pub fn prepare_instruction_history_deletion(
        &self,
        id: &str,
    ) -> Result<InstructionHistoryDeletion, StoreError> {
        self.require_history_key()?;
        let mut db = self
            .connect_complete_read_only(false)?
            .ok_or(StoreError::NotFound)?;
        let tx = db.transaction()?;
        let records = workflow_mutations::read_records(&tx)?;
        let plan = deletion(&records, id)?;
        tx.commit()?;
        Ok(plan)
    }
    pub fn delete_instruction_history(
        &self,
        retained: &InstructionHistoryDeletion,
    ) -> Result<(), StoreError> {
        self.require_history_key()?;
        let mut db = self
            .connect_complete(false)?
            .ok_or(StoreError::WorkspaceChanged)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let records = workflow_mutations::read_records(&tx)?;
        if &deletion(&records, &retained.id)? != retained {
            return Err(StoreError::WorkspaceChanged);
        }
        for (id, event) in &records.history.events {
            if event.wire.change_id == retained.id {
                tx.execute("DELETE FROM instruction_events WHERE event_id=?1", [id])?;
            }
        }
        tx.execute(
            "DELETE FROM instruction_changes WHERE change_id=?1",
            [&retained.id],
        )?;
        workflow_mutations::read_records(&tx)?;
        tx.commit()?;
        Ok(())
    }
}
fn deletion(records: &Records, id: &str) -> Result<InstructionHistoryDeletion, StoreError> {
    if !valid_id(id, "instruction-change:") {
        return Err(StoreError::InstructionHistoryInvalid);
    }
    if !records.history.changes.contains_key(id) {
        return Err(StoreError::NotFound);
    }
    if records
        .history
        .changes
        .values()
        .any(|c| c.wire.parent_change_id.as_deref() == Some(id))
    {
        return Err(StoreError::RecordInUse);
    }
    Ok(InstructionHistoryDeletion {
        id: id.into(),
        expected_state_id: records.state_id()?,
        events: records
            .history
            .events
            .values()
            .filter(|e| e.wire.change_id == id)
            .count(),
    })
}
fn prepare_change(
    current: &Records,
    change: &InstructionChange,
    expected: &str,
) -> Result<Records, StoreError> {
    if !valid_id(expected, "workspace:") || current.state_id()? != expected {
        return Err(StoreError::WorkspaceChanged);
    }
    let mut result = current.clone();
    result.version = 5;
    result
        .history
        .changes
        .insert(change.id().into(), change.clone());
    result.history.validate(&result)?;
    Records::load_complete(&composition_backup::canonical_database(&result)?)
}

#[cfg(test)]
#[path = "instruction_history_tests.rs"]
mod tests;
