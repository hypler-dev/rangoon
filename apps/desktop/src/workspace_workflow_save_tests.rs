#[cfg(feature = "encrypted-workspace")]
use super::*;

#[cfg(feature = "encrypted-workspace")]
use std::{collections::BTreeSet, fs};

#[cfg(feature = "encrypted-workspace")]
use rangoon_workflow::records::{
    SaveIntent, WorkflowRevision, prepare_revision, workflow_id_from_nonce,
};

#[cfg(feature = "encrypted-workspace")]
const EXPECTED_STATE_SEED: u64 = 0xE1B2_B4A4_5EED_2026;
#[cfg(feature = "encrypted-workspace")]
const EXPECTED_STATE_KINDS: usize = 7;

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
    second_report: Option<rangoon_domain::AnalysisReport>,
    capability_id: Option<String>,
    capability_head: Option<String>,
    workflow_id: Option<String>,
    workflow_head: Option<String>,
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
            second_report: None,
            capability_id: None,
            capability_head: None,
            workflow_id: None,
            workflow_head: None,
            vault_writes,
        }
    }

    fn complete() -> Self {
        let mut fixture = Self::source_only("workflow-authoring");
        let workspace = fixture.workspace();
        let capability = workspace
            .create_capability_v1(
                &fixture.report.source.id,
                &fixture.report.fragments[0].id,
                "Workflow capability 🦀",
            )
            .unwrap()
            .capability;
        let second = source(b"# Second source\r\nComplete state stays exact.\r\n");
        workspace.save_v1(&second).unwrap();
        let root = workflow_revision(
            0xB4,
            None,
            SaveIntent::Draft,
            "Existing workflow history",
            Some((&capability.id, &capability.latest_revision_id)),
        );
        save_direct(&workspace, &root);
        fixture.second_report = Some(second);
        fixture.capability_id = Some(capability.id.clone());
        fixture.capability_head = Some(capability.latest_revision_id.clone());
        fixture.workflow_id = Some(root.workflow_id().into());
        fixture.workflow_head = Some(root.id().into());
        fixture.vault_writes = fixture.vault.writes(&fixture.workspace_id);
        fixture
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
fn entries(path: &std::path::Path) -> BTreeSet<String> {
    fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect()
}

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
fn workflow_revision(
    seed: u8,
    parent: Option<&str>,
    intent: SaveIntent,
    title: &str,
    capability: Option<(&str, &str)>,
) -> WorkflowRevision {
    let layout: &[u8] = if capability.is_some() {
        br#"{"positions":[{"nodeIndex":0,"x":-120,"y":40},{"nodeIndex":1,"x":0,"y":-80},{"nodeIndex":2,"x":240,"y":120}]}"#
    } else {
        br#"{"positions":[]}"#
    };
    let definition = match capability {
        None => serde_json::json!({
            "schemaVersion": "rangoon.workflow-definition.v1",
            "title": title,
            "nodes": [],
            "controlEdges": [],
            "dataEdges": []
        }),
        Some((capability_id, revision_id)) => serde_json::json!({
            "schemaVersion": "rangoon.workflow-definition.v1",
            "title": title,
            "nodes": [
                {"id":"start","title":"Start","operation":{"kind":"input"},"inputs":[],"outputs":[{"name":"text","dataType":"text"}]},
                {"id":"work","title":"Work","operation":{"kind":"capability","capabilityId":capability_id,"revisionId":revision_id},"inputs":[{"name":"text","dataType":"text"}],"outputs":[{"name":"text","dataType":"text"}]},
                {"id":"finish","title":"Finish","operation":{"kind":"output"},"inputs":[{"name":"text","dataType":"text"}],"outputs":[]}
            ],
            "controlEdges": [
                {"fromNode":"start","outlet":"next","toNode":"work"},
                {"fromNode":"work","outlet":"next","toNode":"finish"}
            ],
            "dataEdges": [
                {"fromNode":"start","fromPort":"text","toNode":"work","toPort":"text"},
                {"fromNode":"work","fromPort":"text","toNode":"finish","toPort":"text"}
            ]
        }),
    };
    prepare_revision(
        &workflow_id_from_nonce(&[seed; 32]),
        parent,
        intent,
        &serde_json::to_vec(&definition).unwrap(),
        layout,
    )
    .unwrap()
}

#[cfg(feature = "encrypted-workspace")]
fn save_direct(workspace: &rangoon_store::Workspace, candidate: &WorkflowRevision) {
    let plan = workspace.inspect_workflow_save(candidate).unwrap();
    workspace
        .save_workflow(candidate, &plan.expected_state_id)
        .unwrap();
}

#[cfg(feature = "encrypted-workspace")]
fn assert_recovery(owner: &mut Owner) {
    assert!(!owner.is_unlocked());
    owner.lock();
    assert_error(owner.prepare_unlock(), Error::RecoveryRequired);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn source_only_inspection_and_save_preserve_source_bytes() {
    let mut fixture = Fixture::source_only("workflow-authoring-source-only");
    let candidate = workflow_revision(1, None, SaveIntent::Draft, "Draft 🦀", None);
    let before = fixture.state();
    let expected = fixture
        .workspace()
        .inspect_workflow_save(&candidate)
        .unwrap();
    let plan = fixture
        .owner
        .inspect_workflow_save(&fixture.vault, &candidate)
        .unwrap();
    assert_eq!(plan, expected);
    fixture.assert_unchanged(&before);

    let saved = fixture
        .owner
        .save_workflow(&fixture.vault, &candidate, &plan.expected_state_id)
        .unwrap();
    let trusted = fixture.workspace();
    assert_eq!(
        saved.workflow,
        trusted
            .open_workflow(candidate.workflow_id(), None)
            .unwrap()
    );
    assert_eq!(
        trusted.open(&fixture.report.source.id).unwrap(),
        fixture.report
    );
    assert_eq!(
        saved.workflow.revision.serialized_bytes().unwrap(),
        candidate.serialized_bytes().unwrap()
    );
    assert_eq!(
        saved.workflow.revision.layout().serialized_bytes().unwrap(),
        candidate.layout().serialized_bytes().unwrap()
    );
    assert!(!saved.already_saved);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn schema_four_save_preserves_complete_records_history_layout_and_references() {
    let mut fixture = Fixture::complete();
    let workspace = fixture.workspace();
    let source_before = workspace.open(&fixture.report.source.id).unwrap();
    let second_before = workspace
        .open(&fixture.second_report.as_ref().unwrap().source.id)
        .unwrap();
    let capability_before = workspace
        .open_capability_v1(fixture.capability_id.as_ref().unwrap(), None)
        .unwrap();
    let history_before = workspace
        .open_workflow(fixture.workflow_id.as_ref().unwrap(), None)
        .unwrap();
    let candidate = workflow_revision(
        0xB4,
        fixture.workflow_head.as_deref(),
        SaveIntent::Draft,
        "Child preserves complete state",
        Some((
            fixture.capability_id.as_ref().unwrap(),
            fixture.capability_head.as_ref().unwrap(),
        )),
    );
    let expected = workspace.inspect_workflow_save(&candidate).unwrap();
    let before = fixture.state();
    let plan = fixture
        .owner
        .inspect_workflow_save(&fixture.vault, &candidate)
        .unwrap();
    assert_eq!(plan, expected);
    assert_eq!(
        plan.expected_head_id.as_deref(),
        fixture.workflow_head.as_deref()
    );
    fixture.assert_unchanged(&before);
    let saved = fixture
        .owner
        .save_workflow(&fixture.vault, &candidate, &plan.expected_state_id)
        .unwrap();
    let trusted = fixture.workspace();
    assert_eq!(
        saved.workflow,
        trusted
            .open_workflow(candidate.workflow_id(), None)
            .unwrap()
    );
    assert_eq!(
        trusted.open(&fixture.report.source.id).unwrap(),
        source_before
    );
    assert_eq!(
        trusted.open(&second_before.source.id).unwrap(),
        second_before
    );
    assert_eq!(
        trusted
            .open_capability_v1(fixture.capability_id.as_ref().unwrap(), None)
            .unwrap(),
        capability_before
    );
    let historical = trusted
        .open_workflow(
            fixture.workflow_id.as_ref().unwrap(),
            Some(fixture.workflow_head.as_ref().unwrap()),
        )
        .unwrap();
    assert_eq!(historical.revision, history_before.revision);
    assert_eq!(historical.head, saved.workflow.head);
    assert_eq!(historical.history, saved.workflow.history);
    assert_eq!(historical.references, history_before.references);
    assert_eq!(saved.workflow.history.len(), 2);
    assert_eq!(
        saved.workflow.history[0].id,
        fixture.workflow_head.as_deref().unwrap()
    );
    assert_eq!(
        saved.workflow.revision.serialized_bytes().unwrap(),
        candidate.serialized_bytes().unwrap()
    );
    assert_eq!(
        saved.workflow.revision.layout().serialized_bytes().unwrap(),
        candidate.layout().serialized_bytes().unwrap()
    );
    assert_eq!(saved.workflow.references.len(), 1);
    assert_eq!(
        saved.workflow.references[0].capability_id,
        fixture.capability_id.unwrap()
    );
    assert_eq!(
        saved.workflow.references[0].revision_id,
        fixture.capability_head.unwrap()
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn drafts_keep_unresolved_references_while_validated_requires_resolved_structures() {
    let mut fixture = Fixture::complete();
    let absent_capability = format!("capability:{}", "a".repeat(64));
    let absent_revision = format!("revision:{}", "b".repeat(64));
    let draft = workflow_revision(
        3,
        None,
        SaveIntent::Draft,
        "Unresolved draft",
        Some((&absent_capability, &absent_revision)),
    );
    let plan = fixture
        .owner
        .inspect_workflow_save(&fixture.vault, &draft)
        .unwrap();
    assert_eq!(plan.unresolved_references, 1);
    let saved = fixture
        .owner
        .save_workflow(&fixture.vault, &draft, &plan.expected_state_id)
        .unwrap();
    assert_eq!(saved.workflow.head.unresolved_references, 1);
    assert_eq!(saved.workflow.revision.intent(), SaveIntent::Draft);

    let validated = workflow_revision(
        4,
        None,
        SaveIntent::Validated,
        "Resolved validated workflow",
        Some((
            fixture.capability_id.as_ref().unwrap(),
            fixture.capability_head.as_ref().unwrap(),
        )),
    );
    let plan = fixture
        .owner
        .inspect_workflow_save(&fixture.vault, &validated)
        .unwrap();
    assert_eq!(plan.unresolved_references, 0);
    let saved = fixture
        .owner
        .save_workflow(&fixture.vault, &validated, &plan.expected_state_id)
        .unwrap();
    assert!(saved.workflow.head.structurally_valid);
    assert_eq!(saved.workflow.revision.intent(), SaveIntent::Validated);
    let report = serde_json::to_value(saved.workflow.revision.inspection().report()).unwrap();
    assert_eq!(report["authority"], "none");
    assert_eq!(report["executionStatus"], "unavailable");

    let rejected = workflow_revision(
        5,
        None,
        SaveIntent::Validated,
        "Rejected unresolved validated workflow",
        Some((&absent_capability, &absent_revision)),
    );
    let before = fixture.state();
    assert_error(
        fixture
            .owner
            .inspect_workflow_save(&fixture.vault, &rejected),
        Error::StoreInvalid,
    );
    fixture.assert_unchanged(&before);
    assert!(!fixture.owner.is_unlocked());

    let mut save_fixture = Fixture::source_only("workflow-authoring-unresolved-save");
    let expected_state_id = save_fixture.workspace().workflow_data().unwrap().state_id;
    let before = save_fixture.state();
    assert_error(
        save_fixture
            .owner
            .save_workflow(&save_fixture.vault, &rejected, &expected_state_id),
        Error::WriteUncertain,
    );
    save_fixture.assert_unchanged(&before);
    assert_recovery(&mut save_fixture.owner);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn stale_plans_exact_retry_and_historical_conflict_keep_store_rules() {
    let mut retry_fixture = Fixture::source_only("workflow-authoring-retry");
    let root = workflow_revision(6, None, SaveIntent::Draft, "Retry root", None);
    let plan = retry_fixture
        .owner
        .inspect_workflow_save(&retry_fixture.vault, &root)
        .unwrap();
    let first = retry_fixture
        .owner
        .save_workflow(&retry_fixture.vault, &root, &plan.expected_state_id)
        .unwrap();
    let before_retry = retry_fixture.state();
    let retry = retry_fixture
        .owner
        .save_workflow(&retry_fixture.vault, &root, &plan.expected_state_id)
        .unwrap();
    assert!(retry.already_saved);
    assert_eq!(retry.workflow, first.workflow);
    assert_eq!(retry.state_id, first.state_id);
    retry_fixture.assert_unchanged(&before_retry);
    let child = workflow_revision(6, Some(root.id()), SaveIntent::Draft, "Retry child", None);
    let child_plan = retry_fixture
        .owner
        .inspect_workflow_save(&retry_fixture.vault, &child)
        .unwrap();
    let child_saved = retry_fixture
        .owner
        .save_workflow(&retry_fixture.vault, &child, &child_plan.expected_state_id)
        .unwrap();
    assert_eq!(
        retry_fixture
            .workspace()
            .inspect_workflow_save(&root)
            .unwrap_err(),
        rangoon_store::StoreError::WorkflowConflict
    );
    let before = retry_fixture.state();
    assert_error(
        retry_fixture
            .owner
            .save_workflow(&retry_fixture.vault, &root, &child_saved.state_id),
        Error::WriteUncertain,
    );
    retry_fixture.assert_unchanged(&before);
    assert_recovery(&mut retry_fixture.owner);

    let mut stale_fixture = Fixture::source_only("workflow-authoring-stale");
    let candidate = workflow_revision(7, None, SaveIntent::Draft, "Stale candidate", None);
    let stale = stale_fixture
        .owner
        .inspect_workflow_save(&stale_fixture.vault, &candidate)
        .unwrap();
    let intervening = source(b"# Intervening source\nChanges state.\n");
    stale_fixture.workspace().save_v1(&intervening).unwrap();
    let before = stale_fixture.state();
    assert_error(
        stale_fixture.owner.save_workflow(
            &stale_fixture.vault,
            &candidate,
            &stale.expected_state_id,
        ),
        Error::WriteUncertain,
    );
    stale_fixture.assert_unchanged(&before);
    assert_recovery(&mut stale_fixture.owner);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn locked_recovery_and_key_drift_refuse_both_authoring_paths() {
    for route in 0..2 {
        let mut fixture = Fixture::source_only("workflow-authoring-locked");
        let candidate = workflow_revision(8, None, SaveIntent::Draft, "Locked", None);
        let plan = fixture
            .workspace()
            .inspect_workflow_save(&candidate)
            .unwrap();
        let before = fixture.state();
        fixture.owner.lock();
        let opens = fixture.vault.opens();
        let result = match route {
            0 => fixture
                .owner
                .inspect_workflow_save(&fixture.vault, &candidate)
                .map(|_| ()),
            _ => fixture
                .owner
                .save_workflow(&fixture.vault, &candidate, &plan.expected_state_id)
                .map(|_| ()),
        };
        assert_error(result, Error::Locked);
        assert_eq!(fixture.vault.opens(), opens);
        fixture.assert_unchanged(&before);
    }
    for route in 0..2 {
        let mut fixture = Fixture::source_only("workflow-authoring-recovery");
        let candidate = workflow_revision(9, None, SaveIntent::Draft, "Recovery", None);
        let plan = fixture
            .workspace()
            .inspect_workflow_save(&candidate)
            .unwrap();
        let before = fixture.state();
        fixture.owner.recovery = true;
        let opens = fixture.vault.opens();
        let result = match route {
            0 => fixture
                .owner
                .inspect_workflow_save(&fixture.vault, &candidate)
                .map(|_| ()),
            _ => fixture
                .owner
                .save_workflow(&fixture.vault, &candidate, &plan.expected_state_id)
                .map(|_| ()),
        };
        assert_error(result, Error::RecoveryRequired);
        assert_eq!(fixture.vault.opens(), opens);
        fixture.assert_unchanged(&before);
    }
    for route in 0..2 {
        let mut fixture = Fixture::source_only("workflow-authoring-key-drift");
        let candidate = workflow_revision(10, None, SaveIntent::Draft, "Key drift", None);
        let plan = fixture
            .workspace()
            .inspect_workflow_save(&candidate)
            .unwrap();
        let before = fixture.state();
        let original = fixture.record();
        fixture.vault.corrupt_record(&fixture.workspace_id);
        let opens = fixture.vault.opens();
        let result = match route {
            0 => fixture
                .owner
                .inspect_workflow_save(&fixture.vault, &candidate)
                .map(|_| ()),
            _ => fixture
                .owner
                .save_workflow(&fixture.vault, &candidate, &plan.expected_state_id)
                .map(|_| ()),
        };
        assert_error(result, Error::Key(workspace_keys::Error::Changed));
        assert_eq!(fixture.vault.opens(), opens + 1);
        assert!(!fixture.owner.is_unlocked());
        fixture.replace_record(original);
        fixture.assert_unchanged(&before);
    }
}

#[cfg(feature = "encrypted-workspace")]
fn expected_state_mutant(kind: usize, index: usize) -> String {
    let seed = EXPECTED_STATE_SEED.wrapping_add(index as u64 * 0x9E37_79B9);
    match kind {
        0 => String::new(),
        1 => " \t\r\n".into(),
        2 => format!("workspace:\0{seed:016x}"),
        3 => format!("workflow:{seed:064x}"),
        4 => format!("workspace:{seed:065x}"),
        5 => format!("workspace:{}", "g".repeat(64)),
        _ => format!("workspace:{seed:064x}"),
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn fixed_expected_state_matrix_fences_store_refusals_without_retry() {
    let mut covered = BTreeSet::new();
    for index in 0..(EXPECTED_STATE_KINDS * 2) {
        let source_only = index < EXPECTED_STATE_KINDS;
        let mut fixture = if source_only {
            Fixture::source_only("workflow-authoring-expected-state-source")
        } else {
            Fixture::complete()
        };
        let kind = index % EXPECTED_STATE_KINDS;
        let candidate = workflow_revision(
            32 + index as u8,
            None,
            SaveIntent::Draft,
            "Expected state refusal",
            None,
        );
        let before = fixture.state();
        let opens = fixture.vault.opens();
        let value = expected_state_mutant(kind, index);
        covered.insert((source_only, kind));
        assert_error(
            fixture
                .owner
                .save_workflow(&fixture.vault, &candidate, &value),
            Error::WriteUncertain,
        );
        assert_eq!(fixture.vault.opens(), opens + 1, "case {index}");
        fixture.assert_unchanged(&before);
        assert_recovery(&mut fixture.owner);
    }
    assert_eq!(covered.len(), EXPECTED_STATE_KINDS * 2);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn post_commit_fault_returns_no_receipt_then_reacquire_reads_committed_revision() {
    let mut fixture = Fixture::complete();
    let candidate = workflow_revision(
        0xB4,
        fixture.workflow_head.as_deref(),
        SaveIntent::Draft,
        "Committed then uncertain",
        Some((
            fixture.capability_id.as_ref().unwrap(),
            fixture.capability_head.as_ref().unwrap(),
        )),
    );
    let plan = fixture
        .owner
        .inspect_workflow_save(&fixture.vault, &candidate)
        .unwrap();
    let before = fixture.state();
    let writes = fixture.vault.writes(&fixture.workspace_id);
    let opens = fixture.vault.opens();
    fixture.owner.inject_fault(Fault::AfterWorkflowSave);
    assert_error(
        fixture
            .owner
            .save_workflow(&fixture.vault, &candidate, &plan.expected_state_id),
        Error::WriteUncertain,
    );
    assert_recovery(&mut fixture.owner);
    assert_ne!(fixture.state().database, before.database);
    assert_eq!(fixture.vault.opens(), opens + 1);
    assert_eq!(fixture.vault.writes(&fixture.workspace_id), writes);

    let workflow_id = candidate.workflow_id().to_owned();
    let Fixture {
        root,
        vault,
        workspace_id,
        owner,
        ..
    } = fixture;
    drop(owner);
    let mut reopened = Owner::acquire(root.0.clone()).unwrap();
    let preview = reopened.prepare_unlock().unwrap();
    reopened.confirm(&preview.id, &vault).unwrap();
    let reopened_detail = reopened.open_workflow(&vault, &workflow_id, None).unwrap();
    let trusted = trusted_workspace(&root, &vault, &workspace_id)
        .open_workflow(&workflow_id, None)
        .unwrap();
    assert_eq!(reopened_detail, trusted);
    assert_eq!(
        reopened_detail.revision.serialized_bytes().unwrap(),
        candidate.serialized_bytes().unwrap()
    );
}
