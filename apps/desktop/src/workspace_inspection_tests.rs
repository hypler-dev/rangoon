use super::*;

#[cfg(feature = "encrypted-workspace")]
use std::{collections::BTreeSet, fs};

#[cfg(feature = "encrypted-workspace")]
use rangoon_workflow::records::{
    SaveIntent, WorkflowRevision, prepare_revision, workflow_id_from_nonce,
};

#[cfg(feature = "encrypted-workspace")]
const SELECTOR_FUZZ_SEED: u64 = 0xE1B2_B2F0_5EED_2026;
#[cfg(feature = "encrypted-workspace")]
const SELECTOR_MUTANT_CASES: usize = 72;
#[cfg(feature = "encrypted-workspace")]
const KEY_ENVELOPE_BYTES: usize = 121;

#[cfg(feature = "encrypted-workspace")]
struct InspectionFixture {
    root: TestRoot,
    vault: FakeVault,
    owner: Owner,
    workspace_id: String,
    capability_id: String,
    capability_initial_revision: String,
    capability_head_revision: String,
    other_capability_revision: String,
    workflow_id: String,
    workflow_initial_revision: String,
    workflow_head_revision: String,
    other_workflow_revision: String,
    vault_writes: usize,
}

#[cfg(feature = "encrypted-workspace")]
#[derive(Clone, Debug, PartialEq, Eq)]
struct InspectionState {
    control: Vec<u8>,
    binding: Vec<u8>,
    database: Vec<u8>,
    root_entries: BTreeSet<String>,
    workspace_entries: BTreeSet<String>,
}

#[cfg(feature = "encrypted-workspace")]
impl InspectionFixture {
    fn new() -> Self {
        let root = TestRoot::new("inspections");
        let vault = FakeVault::default();
        let report = selected_source();
        let (owner, preview) = create_ready(&root, &vault, &report);
        let workspace = Self::trusted_workspace(&root, &vault, &preview.workspace_id);

        let initial = workspace
            .create_capability_v1(
                &report.source.id,
                &report.fragments[0].id,
                "Inspection origin",
            )
            .unwrap()
            .capability;
        let head = workspace
            .revise_capability_v1(
                &initial.id,
                &initial.latest_revision_id,
                "Inspection head",
                "Reviewed source-only capability bytes.\n",
            )
            .unwrap()
            .capability;
        let reviewed = workspace
            .review_capability_v1(&head.id, &head.latest_revision_id)
            .unwrap()
            .capability;

        let other_source = source(b"# Other\r\nSecond source-only fixture.\r\n");
        workspace.save_v1(&other_source).unwrap();
        let other = workspace
            .create_capability_v1(
                &other_source.source.id,
                &other_source.fragments[0].id,
                "Other capability",
            )
            .unwrap()
            .capability;

        let workflow_initial = workflow_revision(0x91, None, "Inspection workflow origin");
        save_workflow(&workspace, &workflow_initial);
        let workflow_head = workflow_revision(
            0x91,
            Some(workflow_initial.id()),
            "Inspection workflow head",
        );
        save_workflow(&workspace, &workflow_head);
        let other_workflow = workflow_revision(0x92, None, "Other workflow");
        save_workflow(&workspace, &other_workflow);

        let vault_writes = vault.writes(&preview.workspace_id);
        Self {
            root,
            vault,
            owner,
            workspace_id: preview.workspace_id,
            capability_id: reviewed.id.clone(),
            capability_initial_revision: initial.latest_revision_id,
            capability_head_revision: reviewed.latest_revision_id,
            other_capability_revision: other.latest_revision_id,
            workflow_id: workflow_head.workflow_id().into(),
            workflow_initial_revision: workflow_initial.id().into(),
            workflow_head_revision: workflow_head.id().into(),
            other_workflow_revision: other_workflow.id().into(),
            vault_writes,
        }
    }

    fn trusted_workspace(
        root: &TestRoot,
        vault: &FakeVault,
        workspace_id: &str,
    ) -> rangoon_store::Workspace {
        let identity = workspace_keys::WorkspaceId::parse(workspace_id).unwrap();
        let slot = vault.open(&identity).unwrap();
        let retained = workspace_keys::read_key(&slot).unwrap();
        workspace_keys::verified_workspace(
            &slot,
            &retained,
            root.encrypted_root().join(workspace_id),
        )
        .unwrap()
    }

    fn workspace(&self) -> rangoon_store::Workspace {
        Self::trusted_workspace(&self.root, &self.vault, &self.workspace_id)
    }

    fn snapshot(&self) -> InspectionState {
        InspectionState {
            control: fs::read(self.root.0.join("encrypted-workspace.lock")).unwrap(),
            binding: fs::read(self.root.binding()).unwrap(),
            database: fs::read(database_path(&self.root, &self.workspace_id)).unwrap(),
            root_entries: entries(&self.root.0),
            workspace_entries: entries(&self.root.encrypted_root().join(&self.workspace_id)),
        }
    }

    fn assert_unchanged(&self, before: &InspectionState) {
        assert_eq!(&self.snapshot(), before);
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

    /// Fixture-only restoration bypasses the vault API so mutation cases prove no Owner writes.
    fn replace_record(&self, bytes: Vec<u8>) {
        self.vault
            .0
            .borrow_mut()
            .slots
            .get_mut(&self.workspace_id)
            .unwrap()
            .record = Some(bytes);
    }

    fn relock_and_unlock(&mut self) {
        assert!(!self.owner.is_unlocked());
        let preview = self.owner.prepare_unlock().unwrap();
        self.owner.confirm(&preview.id, &self.vault).unwrap();
        assert!(self.owner.is_unlocked());
    }
}

#[cfg(feature = "encrypted-workspace")]
fn entries(path: &std::path::Path) -> BTreeSet<String> {
    fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect()
}

#[cfg(feature = "encrypted-workspace")]
fn workflow_revision(seed: u8, parent: Option<&str>, title: &str) -> WorkflowRevision {
    let definition = serde_json::json!({
        "schemaVersion": "rangoon.workflow-definition.v1",
        "title": title,
        "nodes": [],
        "controlEdges": [],
        "dataEdges": []
    });
    prepare_revision(
        &workflow_id_from_nonce(&[seed; 32]),
        parent,
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

#[cfg(not(feature = "encrypted-workspace"))]
#[test]
fn typed_inspections_default_backend_refuses_before_filesystem_or_vault_access() {
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "rangoon-inspections-default-{}",
        std::process::id()
    ));
    assert!(!root.exists());
    match Owner::acquire(root.clone()) {
        Ok(_) => panic!("default backend acquired an owner"),
        Err(error) => assert_eq!(error, Error::BackendUnavailable),
    }
    assert!(!root.exists());
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn source_only_inspections_precede_schema_four_fixture_writes() {
    let root = TestRoot::new("inspections-source-only");
    let vault = FakeVault::default();
    let report = selected_source();
    let (mut owner, preview) = create_ready(&root, &vault, &report);
    let expected = InspectionFixture::trusted_workspace(&root, &vault, &preview.workspace_id);
    let database = database_path(&root, &preview.workspace_id);
    let database_before = fs::read(&database).unwrap();
    let binding_before = fs::read(root.binding()).unwrap();
    let control_before = fs::read(root.0.join("encrypted-workspace.lock")).unwrap();
    let root_before = entries(&root.0);
    let workspace_before = entries(&root.encrypted_root().join(&preview.workspace_id));
    let writes = vault.writes(&preview.workspace_id);

    assert_eq!(owner.list_capabilities(&vault).unwrap(), Vec::new());
    let actual_data = owner.workflow_data(&vault).unwrap();
    assert_eq!(
        serde_json::to_value(&actual_data).unwrap(),
        serde_json::to_value(expected.workflow_data().unwrap()).unwrap()
    );
    assert_eq!(actual_data.records.workflows, 0);
    assert_eq!(actual_data.records.workflow_revisions, 0);
    assert_eq!(owner.list_workflows(&vault).unwrap(), Vec::new());
    let missing_capability = format!("capability:{}", "0".repeat(64));
    let missing_revision = format!("revision:{}", "0".repeat(64));
    let missing_workflow = format!("workflow:{}", "0".repeat(64));
    for route in 0..3 {
        let opens = vault.opens();
        let result = match route {
            0 => owner
                .open_capability(&vault, &missing_capability, Some(&missing_revision))
                .map(|_| ()),
            1 => owner
                .compile_capability(
                    &vault,
                    &missing_capability,
                    &missing_revision,
                    rangoon_compile::Profile::AgentsMdV1,
                )
                .map(|_| ()),
            _ => owner
                .open_workflow(&vault, &missing_workflow, None)
                .map(|_| ()),
        };
        assert_error(result, Error::StoreInvalid);
        assert!(!owner.is_unlocked(), "source-only route {route}");
        assert_eq!(vault.opens(), opens + 1);
        assert_eq!(vault.writes(&preview.workspace_id), writes);
        assert_eq!(fs::read(&database).unwrap(), database_before);
        assert_eq!(fs::read(root.binding()).unwrap(), binding_before);
        assert_eq!(
            fs::read(root.0.join("encrypted-workspace.lock")).unwrap(),
            control_before
        );
        assert_eq!(entries(&root.0), root_before);
        assert_eq!(
            entries(&root.encrypted_root().join(&preview.workspace_id)),
            workspace_before
        );
        let unlock = owner.prepare_unlock().unwrap();
        owner.confirm(&unlock.id, &vault).unwrap();
    }
    assert_eq!(fs::read(&database).unwrap(), database_before);
    assert_eq!(fs::read(root.binding()).unwrap(), binding_before);
    assert_eq!(
        fs::read(root.0.join("encrypted-workspace.lock")).unwrap(),
        control_before
    );
    assert_eq!(entries(&root.0), root_before);
    assert_eq!(
        entries(&root.encrypted_root().join(&preview.workspace_id)),
        workspace_before
    );
    assert_eq!(vault.writes(&preview.workspace_id), writes);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn typed_inspections_refuse_locked_and_recovery_states_before_vault_access() {
    let mut fixture = InspectionFixture::new();
    let before = fixture.snapshot();
    fixture.owner.lock();
    let opens = fixture.vault.opens();
    assert_error(
        fixture.owner.list_capabilities(&fixture.vault).map(|_| ()),
        Error::Locked,
    );
    assert_error(
        fixture
            .owner
            .open_capability(&fixture.vault, &fixture.capability_id, None)
            .map(|_| ()),
        Error::Locked,
    );
    assert_error(
        fixture
            .owner
            .compile_capability(
                &fixture.vault,
                &fixture.capability_id,
                &fixture.capability_head_revision,
                rangoon_compile::Profile::AgentsMdV1,
            )
            .map(|_| ()),
        Error::Locked,
    );
    assert_error(
        fixture.owner.workflow_data(&fixture.vault).map(|_| ()),
        Error::Locked,
    );
    assert_error(
        fixture.owner.list_workflows(&fixture.vault).map(|_| ()),
        Error::Locked,
    );
    assert_error(
        fixture
            .owner
            .open_workflow(&fixture.vault, &fixture.workflow_id, None)
            .map(|_| ()),
        Error::Locked,
    );
    assert_eq!(fixture.vault.opens(), opens);
    fixture.assert_unchanged(&before);

    let mut fixture = InspectionFixture::new();
    let before = fixture.snapshot();
    fixture.owner.recovery = true;
    let opens = fixture.vault.opens();
    assert_error(
        fixture.owner.list_capabilities(&fixture.vault).map(|_| ()),
        Error::RecoveryRequired,
    );
    assert_error(
        fixture
            .owner
            .open_capability(&fixture.vault, &fixture.capability_id, None)
            .map(|_| ()),
        Error::RecoveryRequired,
    );
    assert_error(
        fixture
            .owner
            .compile_capability(
                &fixture.vault,
                &fixture.capability_id,
                &fixture.capability_head_revision,
                rangoon_compile::Profile::AgentsMdV1,
            )
            .map(|_| ()),
        Error::RecoveryRequired,
    );
    assert_error(
        fixture.owner.workflow_data(&fixture.vault).map(|_| ()),
        Error::RecoveryRequired,
    );
    assert_error(
        fixture.owner.list_workflows(&fixture.vault).map(|_| ()),
        Error::RecoveryRequired,
    );
    assert_error(
        fixture
            .owner
            .open_workflow(&fixture.vault, &fixture.workflow_id, None)
            .map(|_| ()),
        Error::RecoveryRequired,
    );
    assert_eq!(fixture.vault.opens(), opens);
    assert!(!fixture.owner.is_unlocked());
    fixture.assert_unchanged(&before);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn typed_inspections_preserve_disposable_legacy_plaintext() {
    let root = TestRoot::new("inspections-legacy");
    let legacy = root.0.join("source-workspace");
    fs::create_dir(&legacy).unwrap();
    let legacy_file = legacy.join("legacy.sqlite3");
    let bytes = b"legacy plaintext fixture\0preserved";
    fs::write(&legacy_file, bytes).unwrap();
    let vault = FakeVault::default();
    let (mut owner, _) = create_ready(&root, &vault, &selected_source());
    assert!(owner.list_capabilities(&vault).unwrap().is_empty());
    assert!(owner.list_workflows(&vault).unwrap().is_empty());
    assert_eq!(fs::read(legacy_file).unwrap(), bytes);
}

#[cfg(feature = "encrypted-workspace")]
fn mutant(prefix: &str, index: usize) -> String {
    let step = SELECTOR_FUZZ_SEED.wrapping_add((index as u64).wrapping_mul(0x9E37_79B9));
    match index % 6 {
        0 => format!("{prefix}{}", "0".repeat(64)),
        1 => format!("wrong:{step:064x}"),
        2 => format!("{prefix}{}", "é".repeat(32)),
        3 => format!("{prefix}{}", "a".repeat(63)),
        4 => format!("{prefix}{}g", "a".repeat(63)),
        _ => format!("{prefix}{}", "a".repeat(8192)),
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn schema_four_typed_inspections_match_complete_store_and_preserve_state() {
    let mut fixture = InspectionFixture::new();
    let before = fixture.snapshot();
    let expected = fixture.workspace();

    let capabilities = fixture.owner.list_capabilities(&fixture.vault).unwrap();
    assert_eq!(capabilities, expected.list_capabilities_v1().unwrap());
    assert_eq!(capabilities.len(), 2);
    let summary = capabilities
        .iter()
        .find(|row| row.id == fixture.capability_id)
        .unwrap();
    assert_eq!(summary.title, "Inspection head");
    assert_eq!(summary.latest_revision_id, fixture.capability_head_revision);
    assert!(summary.reviewed);
    assert_eq!(summary.revision_count, 2);

    let head = fixture
        .owner
        .open_capability(&fixture.vault, &fixture.capability_id, None)
        .unwrap();
    assert_eq!(
        head,
        expected
            .open_capability_v1(&fixture.capability_id, None)
            .unwrap()
    );
    assert_eq!(head.id, fixture.capability_id);
    assert_eq!(head.latest_revision_id, fixture.capability_head_revision);
    assert_eq!(head.revision.id, fixture.capability_head_revision);
    assert_eq!(head.revision.title, "Inspection head");
    assert_eq!(
        head.revision.content,
        "Reviewed source-only capability bytes.\n"
    );
    assert!(head.revision.review.is_some());

    let historical = fixture
        .owner
        .open_capability(
            &fixture.vault,
            &fixture.capability_id,
            Some(&fixture.capability_initial_revision),
        )
        .unwrap();
    assert_eq!(
        historical,
        expected
            .open_capability_v1(
                &fixture.capability_id,
                Some(&fixture.capability_initial_revision),
            )
            .unwrap()
    );
    assert_eq!(historical.revision.id, fixture.capability_initial_revision);
    assert_eq!(
        historical.latest_revision_id,
        fixture.capability_head_revision
    );

    let compiled = fixture
        .owner
        .compile_capability(
            &fixture.vault,
            &fixture.capability_id,
            &fixture.capability_head_revision,
            rangoon_compile::Profile::AgentsMdV1,
        )
        .unwrap();
    let expected_compiled = expected
        .compile_capability(
            &fixture.capability_id,
            &fixture.capability_head_revision,
            rangoon_compile::Profile::AgentsMdV1,
        )
        .unwrap();
    assert_eq!(
        serde_json::to_value(&compiled).unwrap(),
        serde_json::to_value(&expected_compiled).unwrap()
    );
    assert_eq!(
        compiled.observed_current_head,
        fixture.capability_head_revision
    );
    assert_eq!(
        compiled.compilation.profile.id,
        rangoon_compile::Profile::AgentsMdV1
    );
    assert_eq!(
        compiled.compilation.profile.runtime_qualification,
        "untested"
    );
    assert_eq!(
        compiled.compilation.profile.semantic_equivalence,
        "unverified"
    );
    assert_eq!(compiled.compilation.diagnostics.len(), 2);
    assert!(
        compiled
            .compilation
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity == rangoon_compile::Severity::Warning)
    );

    let historical_compiled = fixture
        .owner
        .compile_capability(
            &fixture.vault,
            &fixture.capability_id,
            &fixture.capability_initial_revision,
            rangoon_compile::Profile::AgentsMdV1,
        )
        .unwrap();
    let expected_historical_compiled = expected
        .compile_capability(
            &fixture.capability_id,
            &fixture.capability_initial_revision,
            rangoon_compile::Profile::AgentsMdV1,
        )
        .unwrap();
    assert_eq!(
        serde_json::to_value(&historical_compiled).unwrap(),
        serde_json::to_value(&expected_historical_compiled).unwrap()
    );
    assert_eq!(
        historical_compiled
            .compilation
            .selected_revision
            .revision_id,
        fixture.capability_initial_revision
    );
    assert_eq!(
        historical_compiled.observed_current_head,
        fixture.capability_head_revision
    );
    assert!(
        historical_compiled
            .compilation
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity == rangoon_compile::Severity::Warning)
    );

    let data = fixture.owner.workflow_data(&fixture.vault).unwrap();
    let expected_data = expected.workflow_data().unwrap();
    assert_eq!(
        serde_json::to_value(&data).unwrap(),
        serde_json::to_value(&expected_data).unwrap()
    );
    assert_eq!(data.schema_version, "rangoon.workspace-data.v2");
    assert_eq!(data.records.workflows, 2);
    assert_eq!(data.records.workflow_revisions, 3);
    assert_eq!(data.capabilities.len(), 2);

    let workflows = fixture.owner.list_workflows(&fixture.vault).unwrap();
    assert_eq!(workflows, expected.list_workflows().unwrap());
    let workflow = workflows
        .iter()
        .find(|row| row.id == fixture.workflow_id)
        .unwrap();
    assert_eq!(workflow.label, "Inspection workflow head");
    assert_eq!(workflow.latest_revision_id, fixture.workflow_head_revision);
    assert_eq!(workflow.revision_count, 2);

    let workflow_head = fixture
        .owner
        .open_workflow(&fixture.vault, &fixture.workflow_id, None)
        .unwrap();
    assert_eq!(
        workflow_head,
        expected.open_workflow(&fixture.workflow_id, None).unwrap()
    );
    assert_eq!(workflow_head.schema_version, "rangoon.workflow-detail.v1");
    assert_eq!(workflow_head.revision.id(), fixture.workflow_head_revision);
    assert_eq!(
        workflow_head.head.latest_revision_id,
        fixture.workflow_head_revision
    );

    let workflow_historical = fixture
        .owner
        .open_workflow(
            &fixture.vault,
            &fixture.workflow_id,
            Some(&fixture.workflow_initial_revision),
        )
        .unwrap();
    assert_eq!(
        workflow_historical,
        expected
            .open_workflow(
                &fixture.workflow_id,
                Some(&fixture.workflow_initial_revision),
            )
            .unwrap()
    );
    assert_eq!(
        workflow_historical.revision.id(),
        fixture.workflow_initial_revision
    );
    assert_eq!(
        workflow_historical.head.latest_revision_id,
        fixture.workflow_head_revision
    );

    fixture.assert_unchanged(&before);
    fixture.owner.lock();
    assert_error(
        fixture.owner.list_capabilities(&fixture.vault),
        Error::Locked,
    );
    fixture.assert_unchanged(&before);
    fixture.relock_and_unlock();
    assert_eq!(
        fixture.owner.list_workflows(&fixture.vault).unwrap(),
        expected.list_workflows().unwrap()
    );
    fixture.assert_unchanged(&before);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn refused_selectors_are_closed_autolock_and_preserve_all_fixture_state() {
    let mut fixture = InspectionFixture::new();
    let before = fixture.snapshot();
    let opens = fixture.vault.opens();

    assert_error(
        fixture.owner.open_capability(
            &fixture.vault,
            &fixture.capability_id,
            Some(&fixture.other_capability_revision),
        ),
        Error::StoreInvalid,
    );
    assert!(!fixture.owner.is_unlocked());
    fixture.assert_unchanged(&before);
    fixture.relock_and_unlock();

    assert_error(
        fixture.owner.open_workflow(
            &fixture.vault,
            &fixture.workflow_id,
            Some(&fixture.other_workflow_revision),
        ),
        Error::StoreInvalid,
    );
    assert!(!fixture.owner.is_unlocked());
    fixture.assert_unchanged(&before);
    fixture.relock_and_unlock();

    let mut exercised = BTreeSet::new();
    for index in 0..SELECTOR_MUTANT_CASES {
        let route = index % 3;
        let revision_selector = (index / 18) % 2 == 1;
        let mutation_index = index / 3;
        exercised.insert((route, mutation_index % 6, revision_selector));
        let selection = mutant(
            match route {
                0 | 1 => {
                    if revision_selector {
                        "revision:"
                    } else {
                        "capability:"
                    }
                }
                _ => {
                    if revision_selector {
                        "workflow-revision:"
                    } else {
                        "workflow:"
                    }
                }
            },
            mutation_index,
        );
        let before_opens = fixture.vault.opens();
        let result = match route {
            0 => {
                if !revision_selector {
                    fixture
                        .owner
                        .open_capability(
                            &fixture.vault,
                            &selection,
                            Some(&fixture.capability_head_revision),
                        )
                        .map(|_| ())
                } else {
                    fixture
                        .owner
                        .open_capability(&fixture.vault, &fixture.capability_id, Some(&selection))
                        .map(|_| ())
                }
            }
            1 => {
                if !revision_selector {
                    fixture
                        .owner
                        .compile_capability(
                            &fixture.vault,
                            &selection,
                            &fixture.capability_head_revision,
                            rangoon_compile::Profile::AgentsMdV1,
                        )
                        .map(|_| ())
                } else {
                    fixture
                        .owner
                        .compile_capability(
                            &fixture.vault,
                            &fixture.capability_id,
                            &selection,
                            rangoon_compile::Profile::AgentsMdV1,
                        )
                        .map(|_| ())
                }
            }
            _ => {
                if !revision_selector {
                    fixture
                        .owner
                        .open_workflow(
                            &fixture.vault,
                            &selection,
                            Some(&fixture.workflow_head_revision),
                        )
                        .map(|_| ())
                } else {
                    fixture
                        .owner
                        .open_workflow(&fixture.vault, &fixture.workflow_id, Some(&selection))
                        .map(|_| ())
                }
            }
        };
        assert_error(result, Error::StoreInvalid);
        assert!(!fixture.owner.is_unlocked(), "case {index}");
        assert_eq!(fixture.vault.opens(), before_opens + 1, "case {index}");
        fixture.assert_unchanged(&before);
        fixture.relock_and_unlock();
    }
    assert_eq!(exercised.len(), 3 * 6 * 2);
    assert!(fixture.vault.opens() > opens);

    fixture.owner.lock();
    let before_opens = fixture.vault.opens();
    assert_error(fixture.owner.workflow_data(&fixture.vault), Error::Locked);
    assert_eq!(fixture.vault.opens(), before_opens);
    fixture.owner.recovery = true;
    assert_error(
        fixture.owner.list_workflows(&fixture.vault),
        Error::RecoveryRequired,
    );
    assert_eq!(fixture.vault.opens(), before_opens);
    fixture.assert_unchanged(&before);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn key_envelope_byte_matrix_refuses_all_six_services_without_owner_writes() {
    let mut fixture = InspectionFixture::new();
    let before = fixture.snapshot();
    let original = fixture.record();
    assert_eq!(original.len(), KEY_ENVELOPE_BYTES);

    for index in 0..KEY_ENVELOPE_BYTES {
        let mut mutated = original.clone();
        mutated[index] ^= (index as u8).wrapping_mul(17) | 1;
        fixture.replace_record(mutated);
        let before_opens = fixture.vault.opens();
        let result = match index % 6 {
            0 => fixture.owner.list_capabilities(&fixture.vault).map(|_| ()),
            1 => fixture
                .owner
                .open_capability(&fixture.vault, &fixture.capability_id, None)
                .map(|_| ()),
            2 => fixture
                .owner
                .compile_capability(
                    &fixture.vault,
                    &fixture.capability_id,
                    &fixture.capability_head_revision,
                    rangoon_compile::Profile::AgentsMdV1,
                )
                .map(|_| ()),
            3 => fixture.owner.workflow_data(&fixture.vault).map(|_| ()),
            4 => fixture.owner.list_workflows(&fixture.vault).map(|_| ()),
            _ => fixture
                .owner
                .open_workflow(&fixture.vault, &fixture.workflow_id, None)
                .map(|_| ()),
        };
        assert_error(result, Error::Key(workspace_keys::Error::Changed));
        assert!(!fixture.owner.is_unlocked(), "key byte {index}");
        assert_eq!(fixture.vault.opens(), before_opens + 1, "key byte {index}");
        fixture.assert_unchanged(&before);
        fixture.replace_record(original.clone());
        fixture.relock_and_unlock();
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn corrupted_ciphertext_closes_typed_inspections() {
    let mut fixture = InspectionFixture::new();
    let database = database_path(&fixture.root, &fixture.workspace_id);
    let mut ciphertext = fs::read(&database).unwrap();
    *ciphertext.last_mut().unwrap() ^= 0x01;
    fs::write(&database, &ciphertext).unwrap();
    let after_corruption = fixture.snapshot();
    assert_error(
        fixture.owner.workflow_data(&fixture.vault),
        Error::StoreInvalid,
    );
    assert!(!fixture.owner.is_unlocked());
    fixture.assert_unchanged(&after_corruption);
}

#[cfg(all(feature = "encrypted-workspace", unix))]
#[test]
fn replaced_bound_directory_with_valid_ciphertext_closes_typed_inspections() {
    let mut fixture = InspectionFixture::new();
    let directory = fixture.root.encrypted_root().join(&fixture.workspace_id);
    let valid_database = fs::read(directory.join("workspace.sqlite3")).unwrap();
    let retained = fixture.root.0.join("retained-inspection-directory");
    fs::rename(&directory, &retained).unwrap();
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("workspace.sqlite3"), &valid_database).unwrap();
    let after_replacement = fixture.snapshot();
    let before_opens = fixture.vault.opens();
    assert_error(
        fixture.owner.list_capabilities(&fixture.vault),
        Error::StoreInvalid,
    );
    assert!(!fixture.owner.is_unlocked());
    assert_eq!(fixture.vault.opens(), before_opens);
    fixture.assert_unchanged(&after_replacement);
    assert_eq!(
        fs::read(fixture.root.binding()).unwrap(),
        binding(1, &fixture.workspace_id)
    );
    assert_eq!(
        fs::read(retained.join("workspace.sqlite3")).unwrap(),
        valid_database
    );
}
