#[cfg(feature = "encrypted-workspace")]
use super::*;

#[cfg(feature = "encrypted-workspace")]
use std::{collections::BTreeSet, fs};

#[cfg(feature = "encrypted-workspace")]
use rangoon_workflow::records::{
    SaveIntent, WorkflowRevision, prepare_revision, workflow_id_from_nonce,
};

#[cfg(feature = "encrypted-workspace")]
const SEED: u64 = 0xE1B2_B6DA_5EED_2026;
#[cfg(feature = "encrypted-workspace")]
const DECODER_CASES: usize = 4;
#[cfg(feature = "encrypted-workspace")]
const DELETION_SELECTOR_CASES: usize = 6;

#[cfg(feature = "encrypted-workspace")]
#[derive(Clone, Debug, PartialEq, Eq)]
struct State {
    control: Vec<u8>,
    binding: Vec<u8>,
    database: Vec<u8>,
    root_entries: BTreeSet<String>,
    workspace_entries: BTreeSet<String>,
}

#[cfg(feature = "encrypted-workspace")]
struct Fixture {
    root: TestRoot,
    vault: FakeVault,
    owner: Owner,
    workspace_id: String,
    report: rangoon_domain::AnalysisReport,
    vault_writes: usize,
}

#[cfg(feature = "encrypted-workspace")]
impl Fixture {
    fn source_only(label: &str) -> Self {
        let root = TestRoot::new(label);
        let vault = FakeVault::default();
        let report = selected_source();
        let (owner, preview) = create_ready(&root, &vault, &report);
        let vault_writes = vault.writes(&preview.workspace_id);
        Self {
            root,
            vault,
            owner,
            workspace_id: preview.workspace_id,
            report,
            vault_writes,
        }
    }

    fn workspace(&self) -> rangoon_store::Workspace {
        trusted_workspace(&self.root, &self.vault, &self.workspace_id)
    }

    fn state(&self) -> State {
        State {
            control: fs::read(self.root.0.join("encrypted-workspace.lock")).unwrap(),
            binding: fs::read(self.root.binding()).unwrap(),
            database: fs::read(database_path(&self.root, &self.workspace_id)).unwrap(),
            root_entries: entries(&self.root.0),
            workspace_entries: entries(&self.root.encrypted_root().join(&self.workspace_id)),
        }
    }

    fn assert_unchanged(&self, before: &State) {
        assert_eq!(&self.state(), before);
        assert_eq!(self.vault.writes(&self.workspace_id), self.vault_writes);
    }

    fn record(&self) -> Vec<u8> {
        self.vault
            .0
            .borrow()
            .slots
            .get(&self.workspace_id)
            .unwrap()
            .record
            .clone()
            .unwrap()
    }

    fn replace_record(&self, bytes: Vec<u8>) {
        self.vault
            .0
            .borrow_mut()
            .slots
            .get_mut(&self.workspace_id)
            .unwrap()
            .record = Some(bytes);
    }
}

#[cfg(feature = "encrypted-workspace")]
fn schema_four_fixture(label: &str) -> Fixture {
    let mut fixture = Fixture::source_only(label);
    let workspace = fixture.workspace();
    let capability = workspace
        .create_capability_v1(
            &fixture.report.source.id,
            &fixture.report.fragments[0].id,
            "Selector schema-four capability",
        )
        .unwrap()
        .capability;
    save_workflow(
        &workspace,
        &workflow_revision(&capability.id, &capability.latest_revision_id),
    );
    fixture.vault_writes = fixture.vault.writes(&fixture.workspace_id);
    fixture
}

#[cfg(feature = "encrypted-workspace")]
fn entries(path: &std::path::Path) -> BTreeSet<String> {
    fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect()
}

// Raw workspace access is fixture setup/readback only. Owner tests never receive it.
#[cfg(feature = "encrypted-workspace")]
fn trusted_workspace(
    root: &TestRoot,
    vault: &FakeVault,
    workspace_id: &str,
) -> rangoon_store::Workspace {
    let identity = workspace_keys::WorkspaceId::parse(workspace_id).unwrap();
    let slot = vault.open(&identity).unwrap();
    let retained = workspace_keys::read_key(&slot).unwrap();
    workspace_keys::verified_workspace(&slot, &retained, root.encrypted_root().join(workspace_id))
        .unwrap()
}

#[cfg(feature = "encrypted-workspace")]
fn workflow_revision(capability_id: &str, revision_id: &str) -> WorkflowRevision {
    let definition = serde_json::json!({
        "schemaVersion": "rangoon.workflow-definition.v1",
        "title": "Backup keeps this pinned workflow",
        "nodes": [{
            "id": "capability",
            "title": "Pinned",
            "operation": {"kind": "capability", "capabilityId": capability_id, "revisionId": revision_id},
            "inputs": [],
            "outputs": []
        }],
        "controlEdges": [],
        "dataEdges": []
    });
    prepare_revision(
        &workflow_id_from_nonce(&[0xB6; 32]),
        None,
        SaveIntent::Draft,
        &serde_json::to_vec(&definition).unwrap(),
        br#"{"positions":[]}"#,
    )
    .unwrap()
}

#[cfg(feature = "encrypted-workspace")]
fn save_workflow(workspace: &rangoon_store::Workspace, revision: &WorkflowRevision) {
    let plan = workspace.inspect_workflow_save(revision).unwrap();
    workspace
        .save_workflow(revision, &plan.expected_state_id)
        .unwrap();
}

#[cfg(feature = "encrypted-workspace")]
fn reopen(root: &TestRoot, vault: &FakeVault) -> Owner {
    let mut owner = Owner::acquire(root.0.clone()).unwrap();
    let preview = owner.prepare_unlock().unwrap();
    owner.confirm(&preview.id, vault).unwrap();
    owner
}

#[cfg(feature = "encrypted-workspace")]
fn lcg(mut value: u64, steps: usize) -> u64 {
    for _ in 0..=steps {
        value = value.wrapping_mul(6364136223846793005).wrapping_add(1);
    }
    value
}

#[cfg(feature = "encrypted-workspace")]
fn invalid_selector(
    kind: rangoon_store::WorkflowRecordKind,
    selector: usize,
    shape: usize,
) -> String {
    let prefix = match kind {
        rangoon_store::WorkflowRecordKind::Source => "source:",
        rangoon_store::WorkflowRecordKind::Capability => "capability:",
        rangoon_store::WorkflowRecordKind::Workflow => "workflow:",
    };
    match selector {
        0 => String::new(),
        1 => format!("other:{}", "0".repeat(64)),
        2 => format!("{prefix}{}", "0".repeat(63)),
        _ => {
            let mut bytes = format!("{prefix}{}", "0".repeat(64)).into_bytes();
            let index = prefix.len()
                + (lcg(SEED, shape * DELETION_SELECTOR_CASES + selector) as usize % 64);
            bytes[index] = match selector {
                3 => b'g',
                4 => b'A',
                _ => b'!',
            };
            String::from_utf8(bytes).unwrap()
        }
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn export_and_read_previews_preserve_source_only_bytes_and_plaintext_decode() {
    let mut fixture = Fixture::source_only("data-controls-source-only");
    let expected = fixture.workspace().export_workflow_backup().unwrap();
    let backup = rangoon_store::WorkspaceBackup::decode(&expected).unwrap();
    let before = fixture.state();
    let opens = fixture.vault.opens();

    let archive = fixture
        .owner
        .export_workflow_backup(&fixture.vault)
        .unwrap();
    assert_eq!(archive, expected);
    assert!(archive.starts_with(b"RANGOON-BACKUP-V"));
    let decoded = rangoon_store::WorkspaceBackup::decode(&archive).unwrap();
    assert_eq!(decoded.id(), backup.id());
    assert_eq!(decoded.byte_length(), archive.len());
    let restore = fixture
        .owner
        .prepare_workflow_restore(&fixture.vault, &decoded)
        .unwrap();
    assert_eq!(restore.backup_id(), decoded.id());
    let deletion = fixture
        .owner
        .inspect_workflow_deletion(
            &fixture.vault,
            rangoon_store::WorkflowRecordKind::Source,
            &fixture.report.source.id,
        )
        .unwrap();
    assert_eq!(deletion.id(), fixture.report.source.id);
    fixture.assert_unchanged(&before);
    assert_eq!(fixture.vault.opens(), opens + 3);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn schema_four_restore_is_additive_preserves_workflow_and_noop_bytes() {
    let mut origin = Fixture::source_only("data-controls-workflow-backup");
    let workspace = origin.workspace();
    let capability = workspace
        .create_capability_v1(
            &origin.report.source.id,
            &origin.report.fragments[0].id,
            "Backup capability",
        )
        .unwrap()
        .capability;
    let revision = workflow_revision(&capability.id, &capability.latest_revision_id);
    save_workflow(&workspace, &revision);
    let archive = origin.owner.export_workflow_backup(&origin.vault).unwrap();
    let backup = rangoon_store::WorkspaceBackup::decode(&archive).unwrap();

    let mut destination = Fixture::source_only("data-controls-workflow-destination");
    let unrelated = source(b"# Unrelated destination source\r\nKeep me.\r\n");
    destination.workspace().save_v1(&unrelated).unwrap();
    destination.vault_writes = destination.vault.writes(&destination.workspace_id);
    let before_preview = destination.state();
    let prepared = destination
        .owner
        .prepare_workflow_restore(&destination.vault, &backup)
        .unwrap();
    destination.assert_unchanged(&before_preview);
    let restored = destination
        .owner
        .restore_workflow_backup(&destination.vault, &backup, &prepared)
        .unwrap();
    assert_eq!(restored.records.sources, 2);
    assert_eq!(restored.records.workflows, 1);
    let trusted = destination.workspace();
    assert_eq!(trusted.open(&unrelated.source.id).unwrap(), unrelated);
    let detail = trusted.open_workflow(revision.workflow_id(), None).unwrap();
    assert_eq!(detail.references.len(), 1);
    assert_eq!(detail.references[0].capability_id, capability.id);
    assert_eq!(
        detail.references[0].revision_id,
        capability.latest_revision_id
    );

    let no_op = destination
        .owner
        .prepare_workflow_restore(&destination.vault, &backup)
        .unwrap();
    let before_noop = destination.state();
    destination
        .owner
        .restore_workflow_backup(&destination.vault, &backup, &no_op)
        .unwrap();
    assert_eq!(destination.state(), before_noop);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn dependency_blocked_and_successful_deletion_keep_exact_store_rules() {
    let mut blocked = Fixture::source_only("data-controls-delete-blocked");
    let workspace = blocked.workspace();
    let capability = workspace
        .create_capability_v1(
            &blocked.report.source.id,
            &blocked.report.fragments[0].id,
            "Referenced capability",
        )
        .unwrap()
        .capability;
    let revision = workflow_revision(&capability.id, &capability.latest_revision_id);
    save_workflow(&workspace, &revision);
    blocked.vault_writes = blocked.vault.writes(&blocked.workspace_id);
    let plan = blocked
        .owner
        .inspect_workflow_deletion(
            &blocked.vault,
            rangoon_store::WorkflowRecordKind::Capability,
            &capability.id,
        )
        .unwrap();
    let before = blocked.state();
    assert_error(
        blocked.owner.delete_workflow_record(&blocked.vault, &plan),
        Error::WriteUncertain,
    );
    blocked.assert_unchanged(&before);
    assert!(!blocked.owner.is_unlocked());
    assert_error(blocked.owner.prepare_unlock(), Error::RecoveryRequired);

    let mut allowed = Fixture::source_only("data-controls-delete-success");
    let capability = allowed
        .workspace()
        .create_capability_v1(
            &allowed.report.source.id,
            &allowed.report.fragments[0].id,
            "Unreferenced capability",
        )
        .unwrap()
        .capability;
    let plan = allowed
        .owner
        .inspect_workflow_deletion(
            &allowed.vault,
            rangoon_store::WorkflowRecordKind::Capability,
            &capability.id,
        )
        .unwrap();
    let deleted = allowed
        .owner
        .delete_workflow_record(&allowed.vault, &plan)
        .unwrap();
    assert_eq!(deleted.records.capabilities, 0);
    assert!(
        allowed
            .workspace()
            .open_capability_v1(&capability.id, None)
            .is_err()
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn restore_and_deletion_faults_hide_receipts_then_reacquire_committed_state() {
    let mut origin = Fixture::source_only("data-controls-fault-origin");
    let incoming = source(b"# Incoming backup source\r\nCommit before fault.\r\n");
    origin.workspace().save_v1(&incoming).unwrap();
    let archive = origin.owner.export_workflow_backup(&origin.vault).unwrap();
    let backup = rangoon_store::WorkspaceBackup::decode(&archive).unwrap();

    let mut restore = Fixture::source_only("data-controls-fault-restore");
    let prepared = restore
        .owner
        .prepare_workflow_restore(&restore.vault, &backup)
        .unwrap();
    let before = restore.state();
    let restore_opens = restore.vault.opens();
    let restore_writes = restore.vault.writes(&restore.workspace_id);
    restore.owner.inject_fault(Fault::AfterRestore);
    assert_error(
        restore
            .owner
            .restore_workflow_backup(&restore.vault, &backup, &prepared),
        Error::WriteUncertain,
    );
    assert_ne!(restore.state().database, before.database);
    assert_eq!(restore.vault.opens(), restore_opens + 1);
    assert_eq!(restore.vault.writes(&restore.workspace_id), restore_writes);
    assert!(!restore.owner.is_unlocked());
    assert_error(restore.owner.prepare_unlock(), Error::RecoveryRequired);
    let Fixture {
        root, vault, owner, ..
    } = restore;
    drop(owner);
    let mut reopened = reopen(&root, &vault);
    assert_eq!(reopened.workflow_data(&vault).unwrap().records.sources, 2);
    assert_eq!(
        reopened.open(&vault, &incoming.source.id).unwrap(),
        incoming
    );

    let mut deletion = Fixture::source_only("data-controls-fault-delete");
    let capability = deletion
        .workspace()
        .create_capability_v1(
            &deletion.report.source.id,
            &deletion.report.fragments[0].id,
            "Fault deletion capability",
        )
        .unwrap()
        .capability;
    let plan = deletion
        .owner
        .inspect_workflow_deletion(
            &deletion.vault,
            rangoon_store::WorkflowRecordKind::Capability,
            &capability.id,
        )
        .unwrap();
    let deletion_opens = deletion.vault.opens();
    let deletion_writes = deletion.vault.writes(&deletion.workspace_id);
    deletion.owner.inject_fault(Fault::AfterDeletion);
    assert_error(
        deletion
            .owner
            .delete_workflow_record(&deletion.vault, &plan),
        Error::WriteUncertain,
    );
    assert_eq!(deletion.vault.opens(), deletion_opens + 1);
    assert_eq!(
        deletion.vault.writes(&deletion.workspace_id),
        deletion_writes
    );
    assert!(!deletion.owner.is_unlocked());
    assert_error(deletion.owner.prepare_unlock(), Error::RecoveryRequired);
    let Fixture {
        root, vault, owner, ..
    } = deletion;
    drop(owner);
    let mut reopened = reopen(&root, &vault);
    assert_eq!(
        reopened.workflow_data(&vault).unwrap().records.capabilities,
        0
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn all_five_services_refuse_locked_recovery_and_key_drift_with_exact_vault_counts() {
    for mode in 0..3 {
        for route in 0..5 {
            let mut fixture = Fixture::source_only("data-controls-precall-refusal");
            let bytes = fixture.workspace().export_workflow_backup().unwrap();
            let backup = rangoon_store::WorkspaceBackup::decode(&bytes).unwrap();
            let restore = fixture
                .workspace()
                .prepare_workflow_restore(&backup)
                .unwrap();
            let deletion = fixture
                .workspace()
                .inspect_workflow_deletion(
                    rangoon_store::WorkflowRecordKind::Source,
                    &fixture.report.source.id,
                )
                .unwrap();
            let before = fixture.state();
            let original = fixture.record();
            let opens = fixture.vault.opens();
            match mode {
                0 => fixture.owner.lock(),
                1 => fixture.owner.recovery = true,
                _ => fixture.vault.corrupt_record(&fixture.workspace_id),
            }
            let result = match route {
                0 => fixture
                    .owner
                    .export_workflow_backup(&fixture.vault)
                    .map(|_| ()),
                1 => fixture
                    .owner
                    .prepare_workflow_restore(&fixture.vault, &backup)
                    .map(|_| ()),
                2 => fixture
                    .owner
                    .restore_workflow_backup(&fixture.vault, &backup, &restore)
                    .map(|_| ()),
                3 => fixture
                    .owner
                    .inspect_workflow_deletion(
                        &fixture.vault,
                        rangoon_store::WorkflowRecordKind::Source,
                        &fixture.report.source.id,
                    )
                    .map(|_| ()),
                _ => fixture
                    .owner
                    .delete_workflow_record(&fixture.vault, &deletion)
                    .map(|_| ()),
            };
            let expected = match mode {
                0 => Error::Locked,
                1 => Error::RecoveryRequired,
                _ => Error::Key(workspace_keys::Error::Changed),
            };
            assert_error(result, expected);
            assert_eq!(fixture.vault.opens(), opens + usize::from(mode == 2));
            if mode == 2 {
                assert!(!fixture.owner.is_unlocked());
                fixture.replace_record(original);
            }
            fixture.assert_unchanged(&before);
        }
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn stale_restore_and_deletion_plans_are_uncertain_and_do_not_retry() {
    let mut fixture = Fixture::source_only("data-controls-stale-restore");
    let archive = fixture
        .owner
        .export_workflow_backup(&fixture.vault)
        .unwrap();
    let backup = rangoon_store::WorkspaceBackup::decode(&archive).unwrap();
    let prepared = fixture
        .owner
        .prepare_workflow_restore(&fixture.vault, &backup)
        .unwrap();
    fixture
        .workspace()
        .save_v1(&source(b"# Change after restore preview\n"))
        .unwrap();
    let before_restore = fixture.state();
    assert_error(
        fixture
            .owner
            .restore_workflow_backup(&fixture.vault, &backup, &prepared),
        Error::WriteUncertain,
    );
    assert_eq!(fixture.state(), before_restore);
    assert_error(fixture.owner.prepare_unlock(), Error::RecoveryRequired);

    let mut mismatch = Fixture::source_only("data-controls-mismatched-archive");
    let first = mismatch
        .owner
        .export_workflow_backup(&mismatch.vault)
        .unwrap();
    let first = rangoon_store::WorkspaceBackup::decode(&first).unwrap();
    let prepared = mismatch
        .owner
        .prepare_workflow_restore(&mismatch.vault, &first)
        .unwrap();
    mismatch
        .workspace()
        .save_v1(&source(b"# Different archive identity\n"))
        .unwrap();
    let other = mismatch.workspace().export_workflow_backup().unwrap();
    let other = rangoon_store::WorkspaceBackup::decode(&other).unwrap();
    let before_mismatch = mismatch.state();
    assert_error(
        mismatch
            .owner
            .restore_workflow_backup(&mismatch.vault, &other, &prepared),
        Error::WriteUncertain,
    );
    assert_eq!(mismatch.state(), before_mismatch);
    assert_error(mismatch.owner.prepare_unlock(), Error::RecoveryRequired);

    let mut fixture = Fixture::source_only("data-controls-stale-deletion");
    let capability = fixture
        .workspace()
        .create_capability_v1(
            &fixture.report.source.id,
            &fixture.report.fragments[0].id,
            "Stale deletion capability",
        )
        .unwrap()
        .capability;
    let prepared = fixture
        .owner
        .inspect_workflow_deletion(
            &fixture.vault,
            rangoon_store::WorkflowRecordKind::Capability,
            &capability.id,
        )
        .unwrap();
    fixture
        .workspace()
        .save_v1(&source(b"# Change after deletion preview\n"))
        .unwrap();
    let before_delete = fixture.state();
    assert_error(
        fixture
            .owner
            .delete_workflow_record(&fixture.vault, &prepared),
        Error::WriteUncertain,
    );
    assert_eq!(fixture.state(), before_delete);
    assert_error(fixture.owner.prepare_unlock(), Error::RecoveryRequired);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn fixed_seed_decoder_and_deletion_selector_refusals_are_bounded_and_exact() {
    let fixture = Fixture::source_only("data-controls-seeded-refusals");
    let archive = fixture.workspace().export_workflow_backup().unwrap();
    let before_decode = fixture.state();
    let opens = fixture.vault.opens();
    let mut decoder_coverage = BTreeSet::new();
    for case in 0..DECODER_CASES {
        let selector = (case * 3 + (SEED as usize & 3)) % DECODER_CASES;
        decoder_coverage.insert(selector);
        let mut bytes = archive.clone();
        match selector {
            0 => bytes[0] ^= 1,
            1 => {
                bytes.pop();
            }
            2 => bytes.extend_from_slice(b"x"),
            _ => bytes.truncate(1),
        }
        assert_eq!(
            rangoon_store::WorkspaceBackup::decode(&bytes).unwrap_err(),
            rangoon_store::StoreError::BackupInvalid,
            "seed {SEED:#x}, decoder selector {selector}"
        );
    }
    assert_eq!(decoder_coverage.len(), DECODER_CASES);
    assert_eq!(fixture.state(), before_decode);
    assert_eq!(fixture.vault.opens(), opens);

    let kinds = [
        rangoon_store::WorkflowRecordKind::Source,
        rangoon_store::WorkflowRecordKind::Capability,
        rangoon_store::WorkflowRecordKind::Workflow,
    ];
    let mut coverage = BTreeSet::new();
    for shape in 0..2 {
        for (kind_index, &kind) in kinds.iter().enumerate() {
            for selector in 0..DELETION_SELECTOR_CASES {
                coverage.insert((shape, kind_index, selector));
                let mut refusal = if shape == 0 {
                    Fixture::source_only("data-controls-delete-selector-source")
                } else {
                    schema_four_fixture("data-controls-delete-selector-schema-four")
                };
                let id = invalid_selector(kind, selector, shape);
                let before = refusal.state();
                let opens = refusal.vault.opens();
                let writes = refusal.vault.writes(&refusal.workspace_id);
                assert_error(
                    refusal
                        .owner
                        .inspect_workflow_deletion(&refusal.vault, kind, &id),
                    Error::StoreInvalid,
                );
                assert_eq!(
                    refusal.vault.opens(),
                    opens + 1,
                    "seed {SEED:#x}, shape {shape}, kind {kind_index}, selector {selector}"
                );
                assert_eq!(refusal.vault.writes(&refusal.workspace_id), writes);
                refusal.assert_unchanged(&before);
                assert!(!refusal.owner.is_unlocked());
                let unlock = refusal.owner.prepare_unlock().unwrap();
                refusal.owner.confirm(&unlock.id, &refusal.vault).unwrap();
                assert!(refusal.owner.is_unlocked());
            }
        }
    }
    assert_eq!(coverage.len(), 2 * 3 * DELETION_SELECTOR_CASES);
}
