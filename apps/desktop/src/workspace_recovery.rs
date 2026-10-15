//! Bounded write-ahead observations, not authenticated audit or delivery receipts.
use super::*;

const FILES: [&str; 2] = [
    "encrypted-workspace.operation.pending",
    "encrypted-workspace.operation.terminal",
];
const PREFIX: &[u8] = b"rangoon.workspace-operation.v1\0";
const STATE_BYTES: usize = 128;
const BYTES: usize = PREFIX.len() + 2 + 8 + 128 + STATE_BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    CreateSource,
    CreateEmpty,
    SaveSource,
    CreateCapability,
    ReviseCapability,
    ReviewCapability,
    SaveWorkflow,
    Compose,
    Restore,
    Delete,
}
impl Operation {
    fn code(self) -> u8 {
        self as u8
    }
    fn decode(code: u8) -> Result<Self, Error> {
        Ok(match code {
            0 => Self::CreateSource,
            1 => Self::CreateEmpty,
            2 => Self::SaveSource,
            3 => Self::CreateCapability,
            4 => Self::ReviseCapability,
            5 => Self::ReviewCapability,
            6 => Self::SaveWorkflow,
            7 => Self::Compose,
            8 => Self::Restore,
            9 => Self::Delete,
            _ => return Err(Error::RecoveryRequired),
        })
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Pending,
    Observed,
    Reconciled,
}
#[derive(Clone, PartialEq, Eq)]
struct Record {
    phase: Phase,
    operation: Operation,
    sequence: u64,
    workspace: String,
    attempt: String,
    state: String,
}
impl Record {
    fn bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(BYTES);
        bytes.extend_from_slice(PREFIX);
        bytes.push(self.phase as u8);
        bytes.push(self.operation.code());
        bytes.extend_from_slice(&self.sequence.to_be_bytes());
        bytes.extend_from_slice(self.workspace.as_bytes());
        bytes.extend_from_slice(self.attempt.as_bytes());
        bytes.extend_from_slice(self.state.as_bytes());
        bytes.resize(BYTES, 0);
        bytes
    }
    fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != BYTES || !bytes.starts_with(PREFIX) {
            return Err(Error::RecoveryRequired);
        }
        let start = PREFIX.len();
        let phase = match bytes[start] {
            0 => Phase::Pending,
            1 => Phase::Observed,
            2 => Phase::Reconciled,
            _ => return Err(Error::RecoveryRequired),
        };
        let operation = Operation::decode(bytes[start + 1])?;
        let sequence = u64::from_be_bytes(bytes[start + 2..start + 10].try_into().unwrap());
        let workspace = std::str::from_utf8(&bytes[start + 10..start + 74])
            .map_err(|_| Error::RecoveryRequired)?
            .to_owned();
        let attempt = std::str::from_utf8(&bytes[start + 74..start + 138])
            .map_err(|_| Error::RecoveryRequired)?
            .to_owned();
        WorkspaceId::parse(&workspace).map_err(|_| Error::RecoveryRequired)?;
        WorkspaceId::parse(&attempt).map_err(|_| Error::RecoveryRequired)?;
        let padded = &bytes[start + 138..];
        let length = padded
            .iter()
            .position(|b| *b == 0)
            .ok_or(Error::RecoveryRequired)?;
        if sequence == 0
            || attempt == workspace
            || padded[length..].iter().any(|b| *b != 0)
            || !padded[..length].iter().all(u8::is_ascii_graphic)
            || (phase == Phase::Pending) != (length == 0)
        {
            return Err(Error::RecoveryRequired);
        }
        let state = std::str::from_utf8(&padded[..length])
            .map_err(|_| Error::RecoveryRequired)?
            .to_owned();
        Ok(Self {
            phase,
            operation,
            sequence,
            workspace,
            attempt,
            state,
        })
    }
}

/// Actual authenticated inventory for explicit reconciliation. A display copy
/// cannot change the native retained plan. Outcome and result delivery are unknown.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryPreview {
    pub id: String,
    pub workspace_id: String,
    pub attempt_id: String,
    pub operation: Operation,
    pub metadata_damage: bool,
    pub outcome: &'static str,
    pub inventory: WorkflowWorkspaceData,
}
pub(super) struct Retained {
    token: String,
    record: Checkpoint,
    binding: Vec<u8>,
    state: String,
    opened: Opened,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Checkpoint {
    slots: [Option<Vec<u8>>; 2],
    record: Record,
    damaged: bool,
}
impl Owner {
    pub(super) fn read_operation(&self) -> Result<Option<Checkpoint>, Error> {
        self.check_control()?;
        let mut slots: [Option<Vec<u8>>; 2] = [None, None];
        for (index, name) in FILES.iter().enumerate() {
            let path = self.root.join(name);
            let before = match fs::symlink_metadata(&path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => return Err(Error::RecoveryRequired),
                Ok(value) => value,
            };
            regular(&before).map_err(|_| Error::RecoveryRequired)?;
            if before.len() > BYTES as u64 {
                return Err(Error::RecoveryRequired);
            }
            let file = options()
                .read(true)
                .open(&path)
                .map_err(|_| Error::RecoveryRequired)?;
            verify_file(&file, &path).map_err(|_| Error::RecoveryRequired)?;
            if !same_file(
                &before,
                &file.metadata().map_err(|_| Error::RecoveryRequired)?,
            ) {
                return Err(Error::RecoveryRequired);
            }
            let mut bytes = Vec::with_capacity(BYTES + 1);
            (&file)
                .take((BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|_| Error::RecoveryRequired)?;
            verify_file(&file, &path).map_err(|_| Error::RecoveryRequired)?;
            let after = file.metadata().map_err(|_| Error::RecoveryRequired)?;
            if bytes.len() > BYTES
                || after.len() != before.len()
                || after.len() != bytes.len() as u64
            {
                return Err(Error::RecoveryRequired);
            }
            slots[index] = Some(bytes);
        }
        self.check_control()?;
        if slots.iter().all(Option::is_none) {
            return Ok(None);
        }
        let pending = slots[0].as_deref().and_then(|b| Record::decode(b).ok());
        if pending.as_ref().is_some_and(|r| r.phase != Phase::Pending) {
            return Err(Error::RecoveryRequired);
        }
        let terminal = slots[1].as_deref().and_then(|b| Record::decode(b).ok());
        if terminal.as_ref().is_some_and(|r| r.phase == Phase::Pending) {
            return Err(Error::RecoveryRequired);
        }
        let (record, damaged) = match (pending, terminal) {
            (Some(p), Some(t)) => {
                if p.workspace != t.workspace {
                    return Err(Error::RecoveryRequired);
                }
                if p.sequence == t.sequence && p.attempt == t.attempt && p.operation == t.operation
                {
                    (t, false)
                } else if t.sequence.checked_add(1) == Some(p.sequence) && p.attempt != t.attempt {
                    (p, false)
                } else {
                    return Err(Error::RecoveryRequired);
                }
            }
            (Some(p), None) => (p, slots[1].is_some()),
            (None, Some(t)) => {
                if !slots[0].as_ref().is_some_and(|b| b.len() == BYTES) {
                    return Err(Error::RecoveryRequired);
                }
                (t, true)
            }
            _ => return Err(Error::RecoveryRequired),
        };
        Ok(Some(Checkpoint {
            slots,
            record,
            damaged,
        }))
    }
    pub(super) fn terminal_operation(&self, binding: &Binding) -> Result<Checkpoint, Error> {
        let observed = self.read_operation()?.ok_or(Error::RecoveryRequired)?;
        if observed.damaged
            || observed.record.phase == Phase::Pending
            || observed.record.workspace != binding.identity.as_hex()
        {
            return Err(Error::RecoveryRequired);
        }
        Ok(observed)
    }
    fn write_operation(&self, record: &Record, previous: Option<&Checkpoint>) -> Result<(), Error> {
        self.check_control()?;
        if self.read_operation()?.as_ref() != previous {
            return Err(Error::RecoveryRequired);
        }
        let index = usize::from(record.phase != Phase::Pending);
        let path = self.root.join(FILES[index]);
        let mut opts = options();
        opts.read(true).write(true);
        if previous.is_none_or(|p| p.slots[index].is_none()) {
            opts.create_new(true);
        }
        let mut file = opts.open(&path).map_err(|_| Error::WriteUncertain)?;
        verify_file(&file, &path).map_err(|_| Error::WriteUncertain)?;
        // Opening an existing slot must not consume a changed observation.
        if previous.is_some_and(|p| p.slots[index].is_some())
            && self.read_operation()?.as_ref() != previous
        {
            return Err(Error::WriteUncertain);
        }
        #[cfg(all(test, feature = "encrypted-workspace"))]
        self.fail_at(if record.phase == Phase::Pending {
            Fault::BeforeIntentWrite
        } else {
            Fault::BeforeCompletionWrite
        })?;
        file.seek(SeekFrom::Start(0))
            .map_err(|_| Error::WriteUncertain)?;
        file.write_all(&record.bytes())
            .map_err(|_| Error::WriteUncertain)?;
        #[cfg(all(test, feature = "encrypted-workspace"))]
        self.fail_at(if record.phase == Phase::Pending {
            Fault::BeforeIntentSync
        } else {
            Fault::BeforeCompletionSync
        })?;
        file.sync_all().map_err(|_| Error::WriteUncertain)?;
        sync_directory(&self.root).map_err(|_| Error::WriteUncertain)?;
        #[cfg(all(test, feature = "encrypted-workspace"))]
        self.fail_at(if record.phase == Phase::Pending {
            Fault::BeforeIntentReadback
        } else {
            Fault::BeforeCompletionReadback
        })?;
        verify_file(&file, &path).map_err(|_| Error::WriteUncertain)?;
        let observed = self.read_operation()?.ok_or(Error::WriteUncertain)?;
        if observed.slots[index].as_deref() != Some(record.bytes().as_slice())
            || (record.phase != Phase::Pending && observed.record != *record)
        {
            return Err(Error::WriteUncertain);
        }
        Ok(())
    }
    pub(super) fn begin_operation(
        &self,
        binding: &Binding,
        operation: Operation,
    ) -> Result<(), Error> {
        let previous = self.read_operation()?;
        let sequence = if let Some(bytes) = &previous {
            let old = &bytes.record;
            if old.phase == Phase::Pending || old.workspace != binding.identity.as_hex() {
                return Err(Error::RecoveryRequired);
            }
            old.sequence.checked_add(1).ok_or(Error::RecoveryRequired)?
        } else {
            1
        };
        let attempt = WorkspaceId::generate()
            .map_err(|_| Error::Unavailable)?
            .as_hex();
        if attempt == binding.identity.as_hex() {
            return Err(Error::Unavailable);
        }
        self.write_operation(
            &Record {
                phase: Phase::Pending,
                operation,
                sequence,
                workspace: binding.identity.as_hex(),
                attempt,
                state: String::new(),
            },
            previous.as_ref(),
        )
    }
    pub(super) fn begin_mutation(&mut self, operation: Operation) -> Result<(), Error> {
        let result = self.begin_operation(
            &self.opened.as_ref().ok_or(Error::Locked)?.binding,
            operation,
        );
        if result.is_err() {
            self.lock();
            self.recovery = true;
        }
        result
    }
    pub(super) fn observe_operation(&self, opened: &Opened) -> Result<(), Error> {
        let previous = self.read_operation()?.ok_or(Error::RecoveryRequired)?;
        let mut record = previous.record.clone();
        if record.phase != Phase::Pending || record.workspace != opened.binding.identity.as_hex() {
            return Err(Error::RecoveryRequired);
        }
        self.check_control()?;
        self.unchanged_directories(&opened.binding, &opened.directories)?;
        let before = self.database(&opened.binding)?;
        let inventory = opened
            .workspace
            .workflow_data()
            .map_err(|_| Error::StoreInvalid)?;
        self.unchanged_database(&opened.binding, &before)?;
        self.unchanged_directories(&opened.binding, &opened.directories)?;
        if self.read_binding()? != opened.binding.bytes()
            || inventory.state_id.is_empty()
            || inventory.state_id.len() >= STATE_BYTES
            || !inventory.state_id.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(Error::RecoveryRequired);
        }
        record.phase = Phase::Observed;
        record.state = inventory.state_id;
        self.write_operation(&record, Some(&previous))
    }
    pub(super) fn finish_mutation<T>(
        &mut self,
        result: Result<T, rangoon_store::StoreError>,
        previous: Metadata,
    ) -> Result<T, Error> {
        let checked = self.finish_read(result, previous).and_then(|value| {
            self.observe_operation(self.opened.as_ref().ok_or(Error::Locked)?)?;
            Ok(value)
        });
        if checked.is_err() {
            self.lock();
            self.recovery = true;
            return Err(Error::WriteUncertain);
        }
        checked
    }
    /// Explicit trusted-host inspection only. The caller must obtain consent
    /// before vault access. No key creation, store mutation or unlock occurs.
    pub fn inspect_recovery(&mut self, vault: &impl Vault) -> Result<RecoveryPreview, Error> {
        if self.opened.is_some() || self.pending.is_some() || self.reconciliation.is_some() {
            return Err(Error::Busy);
        }
        self.check_control()?;
        let observed = self.read_operation()?.ok_or(Error::RecoveryRequired)?;
        let record = &observed.record;
        let binding_bytes = self.read_binding()?;
        let binding = Binding::decode(&binding_bytes)?;
        if record.workspace != binding.identity.as_hex()
            || (!binding.ready
                && !matches!(
                    record.operation,
                    Operation::CreateSource | Operation::CreateEmpty
                ))
        {
            return Err(Error::RecoveryRequired);
        }
        let directories = self.directories(&binding)?;
        let opened = self.unlock(binding, directories, vault)?;
        let inventory = opened
            .workspace
            .workflow_data()
            .map_err(|_| Error::StoreInvalid)?;
        self.unchanged_directories(&opened.binding, &opened.directories)?;
        if self.read_operation()?.as_ref() != Some(&observed)
            || self.read_binding()? != binding_bytes
        {
            return Err(Error::Stale);
        }
        if observed.damaged && record.phase != Phase::Pending && inventory.state_id != record.state
        {
            return Err(Error::RecoveryRequired);
        }
        let token = WorkspaceId::generate()
            .map_err(|_| Error::Unavailable)?
            .as_hex();
        if token == record.workspace || token == record.attempt {
            return Err(Error::Unavailable);
        }
        let preview = RecoveryPreview {
            id: token.clone(),
            workspace_id: record.workspace.clone(),
            attempt_id: record.attempt.clone(),
            operation: record.operation,
            metadata_damage: observed.damaged,
            outcome: "unknown",
            inventory,
        };
        self.reconciliation = Some(Retained {
            token,
            record: observed,
            binding: binding_bytes,
            state: preview.inventory.state_id.clone(),
            opened,
        });
        Ok(preview)
    }
    /// Consume the exact private inspection, then reconcile metadata only.
    /// This is neither a replay nor an approval. A separate unlock is required.
    pub fn confirm_recovery(&mut self, token: &str, vault: &impl Vault) -> Result<(), Error> {
        if !self
            .reconciliation
            .as_ref()
            .is_some_and(|r| r.token == token)
        {
            return Err(Error::Stale);
        }
        let mut retained = self.reconciliation.take().ok_or(Error::Stale)?;
        let mut attempted = false;
        let result = (|| {
            self.check_control()?;
            if self.read_operation()?.as_ref() != Some(&retained.record)
                || self.read_binding()? != retained.binding
            {
                return Err(Error::Stale);
            }
            let opened = &mut retained.opened;
            self.unchanged_directories(&opened.binding, &opened.directories)?;
            let previous = self.database(&opened.binding)?;
            let slot = self.slot(vault, &opened.binding.identity)?;
            let workspace = workspace_keys::verified_workspace(
                &slot,
                &opened.retained,
                self.directory(&opened.binding),
            )
            .map_err(Error::Key)?;
            let inventory = workspace.workflow_data().map_err(|_| Error::StoreInvalid)?;
            self.check_control()?;
            self.unchanged_directories(&opened.binding, &opened.directories)?;
            self.unchanged_database(&opened.binding, &previous)?;
            if inventory.state_id != retained.state
                || self.read_operation()?.as_ref() != Some(&retained.record)
                || self.read_binding()? != retained.binding
            {
                return Err(Error::Stale);
            }
            let mut record = retained.record.record.clone();
            if !opened.binding.ready {
                attempted = true;
                let path = self.root.join(BINDING);
                let mut file = options()
                    .read(true)
                    .write(true)
                    .open(&path)
                    .map_err(|_| Error::WriteUncertain)?;
                verify_file(&file, &path)?;
                #[cfg(all(test, feature = "encrypted-workspace"))]
                self.fail_at(Fault::BeforeReconcileReadyWrite)?;
                opened.binding.ready = true;
                file.seek(SeekFrom::Start(0))
                    .map_err(|_| Error::WriteUncertain)?;
                file.write_all(&opened.binding.bytes())
                    .map_err(|_| Error::WriteUncertain)?;
                #[cfg(all(test, feature = "encrypted-workspace"))]
                self.fail_at(Fault::BeforeReconcileReadySync)?;
                file.sync_all().map_err(|_| Error::WriteUncertain)?;
                sync_directory(&self.root)?;
                verify_file(&file, &path)?;
                #[cfg(all(test, feature = "encrypted-workspace"))]
                self.fail_at(Fault::BeforeReconcileReadyReadback)?;
                if self.read_binding()? != opened.binding.bytes() {
                    return Err(Error::WriteUncertain);
                }
            }
            if retained.record.damaged && record.phase != Phase::Pending {
                attempted = true;
                let mut pending = record.clone();
                pending.phase = Phase::Pending;
                pending.state.clear();
                self.write_operation(&pending, Some(&retained.record))?;
                retained.record = self.read_operation()?.ok_or(Error::RecoveryRequired)?;
            }
            attempted = true;
            record.phase = Phase::Reconciled;
            record.state = retained.state.clone();
            self.write_operation(&record, Some(&retained.record))
        })();
        self.lock();
        self.recovery = result.is_err();
        match result {
            Err(_) if attempted => Err(Error::WriteUncertain),
            other => other,
        }
    }
}
