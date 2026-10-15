#[cfg(feature = "encrypted-workspace")]
use super::*;

#[cfg(feature = "encrypted-workspace")]
use std::{collections::BTreeSet, fs};

#[cfg(feature = "encrypted-workspace")]
use rangoon_compose::{
    Draft, InputRange, InputReference, Operation, OutputRecipe, Piece,
    application::{Request, Target},
};
#[cfg(feature = "encrypted-workspace")]
use rangoon_domain::{
    Authority,
    capability_v1::{Origin, RevisionProvenance},
};
#[cfg(feature = "encrypted-workspace")]
use rangoon_workflow::records::{
    SaveIntent, WorkflowRevision, prepare_revision, workflow_id_from_nonce,
};

#[cfg(feature = "encrypted-workspace")]
const TAMPER_SEED: u64 = 0xE1B2_B5C0_5EED_2026;
#[cfg(feature = "encrypted-workspace")]
const TAMPER_KINDS: usize = 9;

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
    target_id: Option<String>,
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
            target_id: None,
            workflow_id: None,
            workflow_head: None,
            vault_writes,
        }
    }

    fn complete() -> Self {
        let mut fixture = Self::source_only("composition-complete");
        let workspace = fixture.workspace();
        let target = workspace
            .create_capability_v1(
                &fixture.report.source.id,
                &fixture.report.fragments[0].id,
                "Pinned Unicode source 🦀",
            )
            .unwrap()
            .capability;
        let target = workspace
            .review_capability_v1(&target.id, &target.latest_revision_id)
            .unwrap()
            .capability;
        let extra = source(b"# Extra source\r\nHistory must remain.\r\n");
        workspace.save_v1(&extra).unwrap();
        let workflow = workflow_revision(
            0xB5,
            "Pinned workflow reference",
            &target.id,
            &target.latest_revision_id,
        );
        save_workflow(&workspace, &workflow);
        fixture.target_id = Some(target.id.clone());
        fixture.workflow_id = Some(workflow.workflow_id().into());
        fixture.workflow_head = Some(workflow.id().into());
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
fn source_request(report: &rangoon_domain::AnalysisReport) -> Request {
    let end = report.source.content.len() as u64;
    Request {
        schema_version: rangoon_compose::application::REQUEST_SCHEMA.into(),
        draft: Draft {
            schema_version: rangoon_compose::DRAFT_SCHEMA.into(),
            operation: Operation::Decompose,
            inputs: vec![InputReference::Source {
                source_id: report.source.id.clone(),
                sha256: report.source.sha256.clone(),
            }],
            outputs: vec![OutputRecipe {
                title: "Exact source bytes 🦀".into(),
                pieces: vec![Piece::Copy {
                    range: InputRange {
                        input_index: 0,
                        start_byte: 0,
                        end_byte: end,
                    },
                }],
            }],
            exclusions: vec![],
            duplications: vec![],
            conflicts: vec![],
        },
        targets: vec![Target::New {}],
    }
}

#[cfg(feature = "encrypted-workspace")]
fn split_request(detail: &rangoon_domain::capability_v1::CapabilityDetail) -> Request {
    Request {
        schema_version: rangoon_compose::application::REQUEST_SCHEMA.into(),
        draft: Draft {
            schema_version: rangoon_compose::DRAFT_SCHEMA.into(),
            operation: Operation::Split,
            inputs: vec![InputReference::Revision {
                capability_id: detail.id.clone(),
                revision_id: detail.latest_revision_id.clone(),
                sha256: detail.revision.sha256.clone(),
            }],
            outputs: vec![
                OutputRecipe {
                    title: "Append exact Unicode 🦀".into(),
                    pieces: vec![Piece::Copy {
                        range: InputRange {
                            input_index: 0,
                            start_byte: 0,
                            end_byte: detail.revision.content.len() as u64,
                        },
                    }],
                },
                OutputRecipe {
                    title: "New exact derived Δ".into(),
                    pieces: vec![Piece::Authored {
                        content: "New derived bytes ✨\r\n".into(),
                        reason: "Fixture-owned exact output".into(),
                    }],
                },
            ],
            exclusions: vec![],
            duplications: vec![],
            conflicts: vec![],
        },
        targets: vec![
            Target::Append {
                capability_id: detail.id.clone(),
                expected_revision_id: detail.latest_revision_id.clone(),
            },
            Target::New {},
        ],
    }
}

#[cfg(feature = "encrypted-workspace")]
fn merge_request(
    target: &rangoon_domain::capability_v1::CapabilityDetail,
    derived: &rangoon_domain::capability_v1::CapabilityDetail,
) -> Request {
    Request {
        schema_version: rangoon_compose::application::REQUEST_SCHEMA.into(),
        draft: Draft {
            schema_version: rangoon_compose::DRAFT_SCHEMA.into(),
            operation: Operation::Merge,
            inputs: vec![
                InputReference::Revision {
                    capability_id: target.id.clone(),
                    revision_id: target.latest_revision_id.clone(),
                    sha256: target.revision.sha256.clone(),
                },
                InputReference::Revision {
                    capability_id: derived.id.clone(),
                    revision_id: derived.latest_revision_id.clone(),
                    sha256: derived.revision.sha256.clone(),
                },
            ],
            outputs: vec![OutputRecipe {
                title: "Merged preserved bytes".into(),
                pieces: vec![
                    Piece::Copy {
                        range: InputRange {
                            input_index: 0,
                            start_byte: 0,
                            end_byte: target.revision.content.len() as u64,
                        },
                    },
                    Piece::Copy {
                        range: InputRange {
                            input_index: 1,
                            start_byte: 0,
                            end_byte: derived.revision.content.len() as u64,
                        },
                    },
                ],
            }],
            exclusions: vec![],
            duplications: vec![],
            conflicts: vec![],
        },
        targets: vec![Target::Append {
            capability_id: target.id.clone(),
            expected_revision_id: target.latest_revision_id.clone(),
        }],
    }
}

#[cfg(feature = "encrypted-workspace")]
fn workflow_revision(
    seed: u8,
    title: &str,
    capability_id: &str,
    revision_id: &str,
) -> WorkflowRevision {
    let definition = serde_json::json!({
        "schemaVersion": "rangoon.workflow-definition.v1",
        "title": title,
        "nodes": [{
            "id": "capability",
            "title": "Pinned",
            "operation": {
                "kind": "capability",
                "capabilityId": capability_id,
                "revisionId": revision_id
            },
            "inputs": [],
            "outputs": []
        }],
        "controlEdges": [],
        "dataEdges": []
    });
    prepare_revision(
        &workflow_id_from_nonce(&[seed; 32]),
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
fn assert_recovery(owner: &mut Owner) {
    assert!(!owner.is_unlocked());
    owner.lock();
    assert_error(owner.prepare_unlock(), Error::RecoveryRequired);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn source_only_preview_is_read_preserving_and_apply_keeps_exact_unicode_bytes() {
    let mut fixture = Fixture::source_only("composition-source-only");
    let request = source_request(&fixture.report);
    let expected = fixture.workspace().preview_composition(&request).unwrap();
    let before = fixture.state();
    let preview = fixture
        .owner
        .preview_composition(&fixture.vault, &request)
        .unwrap();
    assert_eq!(
        serde_json::to_value(&preview).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    fixture.assert_unchanged(&before);

    let receipt = fixture
        .owner
        .apply_composition(&fixture.vault, &preview, true)
        .unwrap();
    assert_eq!(receipt.authority, Authority::None);
    assert_eq!(receipt.capabilities.len(), 1);
    let composed = &receipt.capabilities[0];
    assert_eq!(composed.revision.content, fixture.report.source.content);
    assert!(matches!(&composed.origin, Origin::Composition { .. }));
    assert!(matches!(
        &composed.revision.provenance,
        RevisionProvenance::Composition { .. }
    ));
    assert!(composed.revision.review.is_none());
    assert_eq!(
        fixture.workspace().open(&fixture.report.source.id).unwrap(),
        fixture.report
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn schema_four_split_merge_new_append_preserve_history_reviews_sources_and_workflow_refs() {
    let mut fixture = Fixture::complete();
    let workspace = fixture.workspace();
    let source_before = workspace
        .list()
        .unwrap()
        .into_iter()
        .map(|snapshot| workspace.open(&snapshot.source_id).unwrap())
        .collect::<Vec<_>>();
    let target_before = workspace
        .open_capability_v1(fixture.target_id.as_ref().unwrap(), None)
        .unwrap();
    let workflow_before = workspace
        .open_workflow(fixture.workflow_id.as_ref().unwrap(), None)
        .unwrap();
    let request = split_request(&target_before);
    let before_preview = fixture.state();
    let preview = fixture
        .owner
        .preview_composition(&fixture.vault, &request)
        .unwrap();
    fixture.assert_unchanged(&before_preview);
    let split = fixture
        .owner
        .apply_composition(&fixture.vault, &preview, true)
        .unwrap();
    assert_eq!(split.authority, Authority::None);
    assert_eq!(split.capabilities.len(), 2);
    let appended = split
        .capabilities
        .iter()
        .find(|detail| detail.id == target_before.id)
        .unwrap();
    let derived = split
        .capabilities
        .iter()
        .find(|detail| detail.id != target_before.id)
        .unwrap();
    assert_eq!(appended.origin, target_before.origin);
    assert_eq!(appended.history[0], target_before.history[0]);
    assert!(appended.history[0].review.is_some());
    assert_eq!(appended.revision.content, target_before.revision.content);
    assert!(matches!(&derived.origin, Origin::Composition { .. }));
    assert_eq!(derived.revision.content, "New derived bytes ✨\r\n");
    for detail in &split.capabilities {
        assert_eq!(detail.authority, Authority::None);
        assert!(detail.revision.review.is_none());
        assert!(matches!(
            &detail.revision.provenance,
            RevisionProvenance::Composition { .. }
        ));
    }

    let merged_target = fixture
        .workspace()
        .open_capability_v1(&appended.id, None)
        .unwrap();
    let merged_derived = fixture
        .workspace()
        .open_capability_v1(&derived.id, None)
        .unwrap();
    let merge_request = merge_request(&merged_target, &merged_derived);
    let merge_preview = fixture
        .owner
        .preview_composition(&fixture.vault, &merge_request)
        .unwrap();
    let merged_receipt = fixture
        .owner
        .apply_composition(&fixture.vault, &merge_preview, true)
        .unwrap();
    let merged = &merged_receipt.capabilities[0];
    assert_eq!(merged.id, target_before.id);
    assert_eq!(
        merged.revision.content,
        format!("{}{}", appended.revision.content, derived.revision.content)
    );
    assert!(merged.revision.review.is_none());
    assert!(matches!(
        &merged.revision.provenance,
        RevisionProvenance::Composition { .. }
    ));
    let trusted = fixture.workspace();
    let source_after = trusted
        .list()
        .unwrap()
        .into_iter()
        .map(|snapshot| trusted.open(&snapshot.source_id).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(source_after, source_before);
    assert_eq!(
        trusted
            .open_capability_v1(&target_before.id, Some(&target_before.latest_revision_id))
            .unwrap()
            .revision,
        target_before.revision
    );
    assert_eq!(
        trusted
            .open_workflow(fixture.workflow_id.as_ref().unwrap(), None)
            .unwrap(),
        workflow_before
    );
    assert_eq!(
        trusted
            .open_workflow(
                fixture.workflow_id.as_ref().unwrap(),
                fixture.workflow_head.as_deref(),
            )
            .unwrap(),
        workflow_before
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn locked_recovery_and_key_drift_refuse_preview_and_apply_before_store_access() {
    for route in 0..2 {
        let mut fixture = Fixture::source_only("composition-locked");
        let request = source_request(&fixture.report);
        let prepared = fixture.workspace().preview_composition(&request).unwrap();
        let before = fixture.state();
        fixture.owner.lock();
        let opens = fixture.vault.opens();
        let result = match route {
            0 => fixture
                .owner
                .preview_composition(&fixture.vault, &request)
                .map(|_| ()),
            _ => fixture
                .owner
                .apply_composition(&fixture.vault, &prepared, true)
                .map(|_| ()),
        };
        assert_error(result, Error::Locked);
        assert_eq!(fixture.vault.opens(), opens);
        fixture.assert_unchanged(&before);
    }
    for route in 0..2 {
        let mut fixture = Fixture::source_only("composition-recovery");
        let request = source_request(&fixture.report);
        let prepared = fixture.workspace().preview_composition(&request).unwrap();
        let before = fixture.state();
        fixture.owner.recovery = true;
        let opens = fixture.vault.opens();
        let result = match route {
            0 => fixture
                .owner
                .preview_composition(&fixture.vault, &request)
                .map(|_| ()),
            _ => fixture
                .owner
                .apply_composition(&fixture.vault, &prepared, true)
                .map(|_| ()),
        };
        assert_error(result, Error::RecoveryRequired);
        assert_eq!(fixture.vault.opens(), opens);
        fixture.assert_unchanged(&before);
    }
    for route in 0..2 {
        let mut fixture = Fixture::source_only("composition-key-drift");
        let request = source_request(&fixture.report);
        let prepared = fixture.workspace().preview_composition(&request).unwrap();
        let before = fixture.state();
        let original = fixture.record();
        fixture.vault.corrupt_record(&fixture.workspace_id);
        let opens = fixture.vault.opens();
        let result = match route {
            0 => fixture
                .owner
                .preview_composition(&fixture.vault, &request)
                .map(|_| ()),
            _ => fixture
                .owner
                .apply_composition(&fixture.vault, &prepared, true)
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
#[test]
fn preview_rejects_wrong_inputs_ranges_and_append_heads_without_writes() {
    for case in 0..4 {
        let mut fixture = if case == 3 {
            Fixture::complete()
        } else {
            Fixture::source_only("composition-preview-refusal")
        };
        let mut request = if case == 3 {
            let target = fixture
                .workspace()
                .open_capability_v1(fixture.target_id.as_ref().unwrap(), None)
                .unwrap();
            split_request(&target)
        } else {
            source_request(&fixture.report)
        };
        let expected = match case {
            0 => {
                let InputReference::Source { source_id, .. } = &mut request.draft.inputs[0] else {
                    unreachable!()
                };
                *source_id = format!("source:{}", "0".repeat(64));
                rangoon_store::StoreError::NotFound
            }
            1 => {
                let InputReference::Source { sha256, .. } = &mut request.draft.inputs[0] else {
                    unreachable!()
                };
                *sha256 = "0".repeat(64);
                rangoon_store::StoreError::Corrupt
            }
            2 => {
                let Piece::Copy { range } = &mut request.draft.outputs[0].pieces[0] else {
                    unreachable!()
                };
                range.end_byte += 1;
                rangoon_store::StoreError::CompositionInvalid
            }
            _ => {
                let Target::Append {
                    expected_revision_id,
                    ..
                } = &mut request.targets[0]
                else {
                    unreachable!()
                };
                *expected_revision_id = format!("revision:{}", "f".repeat(64));
                rangoon_store::StoreError::CapabilityConflict
            }
        };
        match fixture.workspace().preview_composition(&request) {
            Ok(_) => panic!("case {case} accepted a malformed request"),
            Err(error) => assert_eq!(error, expected),
        }
        let before = fixture.state();
        let opens = fixture.vault.opens();
        assert_error(
            fixture
                .owner
                .preview_composition(&fixture.vault, &request)
                .map(|_| ()),
            Error::StoreInvalid,
        );
        assert_eq!(fixture.vault.opens(), opens + 1, "case {case}");
        assert!(!fixture.owner.is_unlocked());
        fixture.assert_unchanged(&before);
        assert!(fixture.owner.prepare_unlock().is_ok());
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn composition_refusals_are_uncertain_locked_and_byte_preserving() {
    for case in 0..6 {
        let mut fixture = Fixture::source_only("composition-refusal");
        let request = source_request(&fixture.report);
        let mut prepared = fixture
            .owner
            .preview_composition(&fixture.vault, &request)
            .unwrap();
        let mut before = fixture.state();
        match case {
            0 => {
                fixture
                    .workspace()
                    .save_v1(&source(b"# Stale source\nChanged state.\n"))
                    .unwrap();
                before = fixture.state();
            }
            1 => {
                fixture
                    .owner
                    .apply_composition(&fixture.vault, &prepared, true)
                    .unwrap();
                before = fixture.state();
            }
            2 => {}
            3 => prepared.preview.core.outputs[0].content.push_str("forged"),
            4 => {
                let mut incomplete = request.clone();
                incomplete.draft.outputs[0].title.clear();
                prepared = fixture
                    .owner
                    .preview_composition(&fixture.vault, &incomplete)
                    .unwrap();
                assert!(!prepared.preview.saveable);
                before = fixture.state();
            }
            _ => {
                prepared.preview.applied_outputs[0].revision_id =
                    format!("revision:{}", "f".repeat(64))
            }
        }
        let opens = fixture.vault.opens();
        let acknowledged = case != 2;
        assert_error(
            fixture
                .owner
                .apply_composition(&fixture.vault, &prepared, acknowledged)
                .map(|_| ()),
            Error::WriteUncertain,
        );
        assert_eq!(fixture.vault.opens(), opens + 1, "case {case}");
        fixture.assert_unchanged(&before);
        assert_recovery(&mut fixture.owner);
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn current_valid_state_substitution_is_rejected_for_source_only_and_schema_four() {
    for shape in 0..2 {
        let mut fixture = if shape == 0 {
            Fixture::source_only("composition-current-state-source")
        } else {
            Fixture::complete()
        };
        let request = source_request(&fixture.report);
        let mut old = fixture
            .owner
            .preview_composition(&fixture.vault, &request)
            .unwrap();
        fixture
            .workspace()
            .save_v1(&source(b"# Current valid state\nUnrelated change.\n"))
            .unwrap();
        let fresh = fixture.workspace().preview_composition(&request).unwrap();
        assert_ne!(old.expected_state_id, fresh.expected_state_id);
        assert_eq!(old.preview, fresh.preview);
        old.expected_state_id = fresh.expected_state_id;
        let before = fixture.state();
        let opens = fixture.vault.opens();
        assert_error(
            fixture
                .owner
                .apply_composition(&fixture.vault, &old, true)
                .map(|_| ()),
            Error::WriteUncertain,
        );
        assert_eq!(fixture.vault.opens(), opens + 1, "shape {shape}");
        fixture.assert_unchanged(&before);
        assert_recovery(&mut fixture.owner);
    }
}

#[cfg(feature = "encrypted-workspace")]
fn mutate_preview(prepared: &mut rangoon_store::CompositionPreview, kind: usize, case: usize) {
    let seed = TAMPER_SEED.wrapping_add((case as u64).wrapping_mul(0x9E37_79B9));
    match kind {
        0 => prepared.expected_state_id = format!("workspace:{seed:064x}"),
        1 => prepared.preview.schema_version = format!("preview:{seed:016x}"),
        2 => prepared.preview.core.schema_version = format!("core:{seed:016x}"),
        3 => prepared.preview.core.outputs[0].content = format!("content 🦀 {seed:016x}"),
        4 => prepared.preview.core.outputs[0].sha256 = format!("digest:{seed:016x}"),
        5 => prepared.preview.core.composition_id = format!("composition:{seed:064x}"),
        6 => prepared.preview.application_id = format!("application:{seed:064x}"),
        7 => prepared.preview.applied_outputs[0].capability_id = format!("capability:{seed:064x}"),
        _ => prepared.preview.saveable = false,
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn fixed_seed_public_preview_tamper_matrix_refuses_without_retry_or_key_write() {
    let mut covered = BTreeSet::new();
    for case in 0..(TAMPER_KINDS * 2) {
        let source_only = case < TAMPER_KINDS;
        let mut fixture = if source_only {
            Fixture::source_only("composition-tamper-source")
        } else {
            Fixture::complete()
        };
        let request = source_request(&fixture.report);
        let mut prepared = fixture
            .owner
            .preview_composition(&fixture.vault, &request)
            .unwrap();
        let kind = case % TAMPER_KINDS;
        mutate_preview(&mut prepared, kind, case);
        covered.insert((source_only, kind));
        let before = fixture.state();
        let opens = fixture.vault.opens();
        assert_error(
            fixture
                .owner
                .apply_composition(&fixture.vault, &prepared, true)
                .map(|_| ()),
            Error::WriteUncertain,
        );
        assert_eq!(fixture.vault.opens(), opens + 1, "case {case}");
        fixture.assert_unchanged(&before);
        assert_recovery(&mut fixture.owner);
    }
    assert_eq!(covered.len(), TAMPER_KINDS * 2);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn post_commit_fault_returns_no_receipt_then_reacquire_unlock_reads_committed_output() {
    let mut fixture = Fixture::source_only("composition-post-commit-fault");
    let request = source_request(&fixture.report);
    let prepared = fixture
        .owner
        .preview_composition(&fixture.vault, &request)
        .unwrap();
    let capability_id = prepared.preview.applied_outputs[0].capability_id.clone();
    let before = fixture.state();
    let writes = fixture.vault.writes(&fixture.workspace_id);
    let opens = fixture.vault.opens();
    fixture.owner.inject_fault(Fault::AfterCompositionApply);
    assert_error(
        fixture
            .owner
            .apply_composition(&fixture.vault, &prepared, true)
            .map(|_| ()),
        Error::WriteUncertain,
    );
    assert_recovery(&mut fixture.owner);
    assert_ne!(fixture.state().database, before.database);
    assert_eq!(fixture.vault.opens(), opens + 1);
    assert_eq!(fixture.vault.writes(&fixture.workspace_id), writes);

    let Fixture {
        root,
        vault,
        workspace_id,
        owner,
        ..
    } = fixture;
    drop(owner);
    let mut reopened = Owner::acquire(root.0.clone()).unwrap();
    let unlock = reopened.prepare_unlock().unwrap();
    reopened.confirm(&unlock.id, &vault).unwrap();
    let committed = reopened
        .open_capability(&vault, &capability_id, None)
        .unwrap();
    let trusted = trusted_workspace(&root, &vault, &workspace_id)
        .open_capability_v1(&capability_id, None)
        .unwrap();
    assert_eq!(committed, trusted);
    assert_eq!(
        committed.revision.content,
        "\u{feff}# Rules\r\nKeep exact 🦀 bytes.\r\n"
    );
}
