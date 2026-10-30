//! Complete V4 logical recovery: V3 base plus exact instruction changes/journal.
//! Plaintext in-memory encoding is not an authenticated export format.
use super::*;
use composition_records::Records;
use instruction_history::{HistoryRows, InstructionChange, InstructionEvent};
use rangoon_domain::{byte_digest, capability::valid_id};

const MAGIC: &[u8] = b"RANGOON-BACKUP-V4\n";
const MAX_MANIFEST: usize = 128 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Descriptor {
    id: String,
    byte_length: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema_version: String,
    base_length: u32,
    #[serde(deserialize_with = "changes")]
    changes: Vec<Descriptor>,
    #[serde(deserialize_with = "events")]
    events: Vec<Descriptor>,
}
fn changes<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Descriptor>, D::Error> {
    composition_backup::bounded::<_, _, 128>(d)
}
fn events<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Descriptor>, D::Error> {
    composition_backup::bounded::<_, _, 512>(d)
}
impl Manifest {
    fn bytes(&self) -> Result<Vec<u8>, StoreError> {
        let bytes = serde_json::to_vec(self).map_err(|_| StoreError::BackupInvalid)?;
        if bytes.len() > MAX_MANIFEST {
            return Err(StoreError::Full);
        }
        Ok(bytes)
    }
    fn payload_length(&self) -> Result<usize, StoreError> {
        if self.schema_version != "rangoon.backup.v4"
            || self.base_length == 0
            || self.base_length as usize > MAX_BACKUP_BYTES
        {
            return Err(StoreError::BackupInvalid);
        }
        let mut total = self.base_length as usize;
        let mut history_total = 0usize;
        for (records, prefix, count, bound) in [
            (
                &self.changes,
                "instruction-change:",
                128,
                instruction_history::MAX_RECORD_BYTES,
            ),
            (&self.events, "instruction-event:", 512, 2048),
        ] {
            if records.len() > count {
                return Err(StoreError::BackupInvalid);
            }
            let mut previous: Option<&str> = None;
            for item in records {
                if !valid_id(&item.id, prefix)
                    || previous.is_some_and(|id| id >= item.id.as_str())
                    || item.byte_length == 0
                    || item.byte_length as usize > bound
                {
                    return Err(StoreError::BackupInvalid);
                }
                history_total = history_total
                    .checked_add(item.byte_length as usize)
                    .ok_or(StoreError::Full)?;
                if history_total > instruction_history::MAX_INSTRUCTION_HISTORY_BYTES {
                    return Err(StoreError::Full);
                }
                total = total
                    .checked_add(item.byte_length as usize)
                    .ok_or(StoreError::Full)?;
                if total > MAX_BACKUP_BYTES {
                    return Err(StoreError::Full);
                }
                previous = Some(&item.id);
            }
        }
        Ok(total)
    }
}

pub(super) fn canonical_database(base: &Records) -> Result<Connection, StoreError> {
    if base.version != 5 {
        return Err(StoreError::UnsupportedSchema);
    }
    base.history.validate(base)?;
    let mut projected = base.clone();
    projected.version = 4;
    projected.history = HistoryRows::default();
    let mut db = workflow_backup::canonical_database(&projected)?;
    let tx = db.transaction()?;
    for (_, sql) in instruction_history::SCHEMAS {
        tx.execute_batch(sql)?;
    }
    tx.pragma_update(None, "user_version", 5)?;
    base.history.insert_all(&tx)?;
    let checked = Records::load_complete(&tx)?;
    workflow_mutations::data(&tx, &checked)?;
    tx.commit()?;
    Ok(db)
}
pub(super) fn encode(base: &Records) -> Result<Vec<u8>, StoreError> {
    canonical_database(base)?;
    let mut projected = base.clone();
    projected.version = 4;
    projected.history = HistoryRows::default();
    let base_bytes = zeroize::Zeroizing::new(workflow_backup::encode(&projected)?);
    let changes = base
        .history
        .changes
        .values()
        .map(|c| {
            Ok(Descriptor {
                id: c.id().into(),
                byte_length: c.bytes()?.len() as u32,
            })
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    let events = base
        .history
        .events
        .values()
        .map(|e| {
            Ok(Descriptor {
                id: e.id().into(),
                byte_length: e.bytes()?.len() as u32,
            })
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    let manifest = Manifest {
        schema_version: "rangoon.backup.v4".into(),
        base_length: base_bytes.len() as u32,
        changes,
        events,
    };
    let metadata = manifest.bytes()?;
    let length = MAGIC
        .len()
        .checked_add(4)
        .and_then(|n| n.checked_add(metadata.len()))
        .and_then(|n| n.checked_add(manifest.payload_length().ok()?))
        .and_then(|n| n.checked_add(64))
        .filter(|n| *n <= MAX_BACKUP_BYTES)
        .ok_or(StoreError::Full)?;
    let mut bytes = Vec::with_capacity(length);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&(metadata.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&metadata);
    bytes.extend_from_slice(&base_bytes);
    for change in base.history.changes.values() {
        bytes.extend_from_slice(&change.bytes()?);
    }
    for event in base.history.events.values() {
        bytes.extend_from_slice(&event.bytes()?);
    }
    bytes.extend_from_slice(byte_digest(&bytes).as_bytes());
    if bytes.len() != length {
        return Err(StoreError::Corrupt);
    }
    Ok(bytes)
}
pub(super) fn decode(bytes: &[u8]) -> Result<Records, StoreError> {
    if bytes.len() > MAX_BACKUP_BYTES
        || bytes.len() < MAGIC.len() + 4 + 64
        || !bytes.starts_with(MAGIC)
    {
        return Err(StoreError::BackupInvalid);
    }
    let end = bytes.len() - 64;
    if byte_digest(&bytes[..end]).as_bytes() != &bytes[end..] {
        return Err(StoreError::BackupInvalid);
    }
    let start = MAGIC.len() + 4;
    let size = u32::from_be_bytes(
        bytes[MAGIC.len()..start]
            .try_into()
            .map_err(|_| StoreError::BackupInvalid)?,
    ) as usize;
    let payload = start
        .checked_add(size)
        .filter(|n| *n <= end && size <= MAX_MANIFEST)
        .ok_or(StoreError::BackupInvalid)?;
    let manifest: Manifest =
        serde_json::from_slice(&bytes[start..payload]).map_err(|_| StoreError::BackupInvalid)?;
    if manifest.bytes()? != bytes[start..payload]
        || payload.checked_add(manifest.payload_length()?) != Some(end)
    {
        return Err(StoreError::BackupInvalid);
    }
    let base_end = payload
        .checked_add(manifest.base_length as usize)
        .filter(|n| *n <= end)
        .ok_or(StoreError::BackupInvalid)?;
    // No recursive archive embedding or legacy base substitution.
    if !bytes[payload..base_end].starts_with(b"RANGOON-BACKUP-V3\n") {
        return Err(StoreError::BackupInvalid);
    }
    let mut records = WorkspaceBackup::decode(&bytes[payload..base_end])?.records;
    if records.version != 4 {
        return Err(StoreError::BackupInvalid);
    }
    let mut cursor = base_end;
    for (items, change) in [(&manifest.changes, true), (&manifest.events, false)] {
        for item in items {
            let next = cursor
                .checked_add(item.byte_length as usize)
                .filter(|n| *n <= end)
                .ok_or(StoreError::BackupInvalid)?;
            let data = &bytes[cursor..next];
            cursor = next;
            if change {
                let value =
                    InstructionChange::decode(data).map_err(|_| StoreError::BackupInvalid)?;
                if value.id() != item.id
                    || records
                        .history
                        .changes
                        .insert(item.id.clone(), value)
                        .is_some()
                {
                    return Err(StoreError::BackupInvalid);
                }
            } else {
                let value =
                    InstructionEvent::decode(data).map_err(|_| StoreError::BackupInvalid)?;
                if value.id() != item.id
                    || records
                        .history
                        .events
                        .insert(item.id.clone(), value)
                        .is_some()
                {
                    return Err(StoreError::BackupInvalid);
                }
            }
        }
    }
    records.version = 5;
    if cursor != end {
        return Err(StoreError::BackupInvalid);
    }
    canonical_database(&records).map_err(|_| StoreError::BackupInvalid)?;
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn combined_history_descriptor_budget_rejects_before_payload_decode() {
        let changes = (0..8)
            .map(|i| Descriptor {
                id: format!("instruction-change:{i:064x}"),
                byte_length: (2 * 1024 * 1024) as u32,
            })
            .collect();
        let events = vec![Descriptor {
            id: format!("instruction-event:{:064x}", 0),
            byte_length: 1,
        }];
        let manifest = Manifest {
            schema_version: "rangoon.backup.v4".into(),
            base_length: 1,
            changes,
            events,
        };
        assert_eq!(manifest.payload_length(), Err(StoreError::Full));
        // No base or change/event payload exists: refusal is descriptor-only.
    }
}
