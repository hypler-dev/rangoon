#[cfg(feature = "encrypted-workspace")]
use super::*;

#[cfg(feature = "encrypted-workspace")]
use std::{collections::BTreeSet, fs};

#[cfg(feature = "encrypted-workspace")]
use rangoon_domain::capability_v1::{Origin, RevisionProvenance};
#[cfg(feature = "encrypted-workspace")]
use rangoon_workflow::records::{
    SaveIntent, WorkflowRevision, prepare_revision, workflow_id_from_nonce,
};

#[cfg(feature = "encrypted-workspace")]
const MUTATION_SEED: u64 = 0xE1B2_B3CA_5EED_2026;
#[cfg(feature = "encrypted-workspace")]
const MUTATION_CASES: usize = 36;

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
    source_id: String,
    fragment_id: String,
    second_source_id: String,
    second_fragment_id: String,
    second_capability_id: String,
    target_id: String,
    target_head: String,
    workflow_id: String,
    workflow_head: String,
    vault_writes: usize,
}

#[cfg(feature = "encrypted-workspace")]
impl Fixture {
    fn new() -> Self {
        let root = TestRoot::new("capability-mutations");
        let vault = FakeVault::default();
        let report = selected_source();
        let (owner, preview) = create_ready(&root, &vault, &report);
        let workspace = Self::trusted_workspace(&root, &vault, &preview.workspace_id);
        let seeded = workspace
            .create_capability_v1(
                &report.source.id,
                &report.fragments[0].id,
                "Seed capability 🦀",
            )
            .unwrap()
            .capability;
        let second = source(b"# Second source\r\nPreserve this complete state.\r\n");
        workspace.save_v1(&second).unwrap();
        let second_capability = workspace
            .create_capability_v1(
                &second.source.id,
                &second.fragments[0].id,
                "Second capability",
            )
            .unwrap()
            .capability;
        let workflow = workflow_revision(
            0xB3,
            None,
            "Workflow keeps its capability reference",
            Some((&seeded.id, &seeded.latest_revision_id)),
        );
        save_workflow(&workspace, &workflow);
        let vault_writes = vault.writes(&preview.workspace_id);
        Self {
            root,
            vault,
            owner,
            workspace_id: preview.workspace_id,
            source_id: report.source.id,
            fragment_id: report.fragments[0].id.clone(),
            second_source_id: second.source.id,
            second_fragment_id: second.fragments[0].id.clone(),
            second_capability_id: second_capability.id,
            target_id: seeded.id,
            target_head: seeded.latest_revision_id,
            workflow_id: workflow.workflow_id().into(),
            workflow_head: workflow.id().into(),
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

    /// This test helper bypasses Vault so refusal cases prove Owner made no writes.
    fn replace_record(&self, bytes: Vec<u8>) {
        self.vault
            .0
            .borrow_mut()
            .slots
            .get_mut(&self.workspace_id)
            .unwrap()
            .record = Some(bytes);
    }

    fn reopen(&mut self) {
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
fn workflow_revision(
    seed: u8,
    parent: Option<&str>,
    title: &str,
    capability: Option<(&str, &str)>,
) -> WorkflowRevision {
    let mut nodes = vec![serde_json::json!({
        "id": "start",
        "title": "Input",
        "operation": {"kind": "input"},
        "inputs": [],
        "outputs": []
    })];
    if let Some((capability_id, revision_id)) = capability {
        nodes.push(serde_json::json!({
            "id": "capability",
            "title": "Stored capability",
            "operation": {
                "kind": "capability",
                "capabilityId": capability_id,
                "revisionId": revision_id
            },
            "inputs": [],
            "outputs": []
        }));
    }
    let definition = serde_json::json!({
        "schemaVersion": "rangoon.workflow-definition.v1",
        "title": title,
        "nodes": nodes,
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

#[cfg(feature = "encrypted-workspace")]
fn assert_recovery(owner: &mut Owner) {
    assert!(!owner.is_unlocked());
    owner.lock();
    assert_error(owner.prepare_unlock(), Error::RecoveryRequired);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn managed_capability_mutations_preserve_exact_records_and_complete_state() {
    let mut fixture = Fixture::new();
    let workflow_before = fixture
        .workspace()
        .open_workflow(&fixture.workflow_id, None)
        .unwrap();
    let source_before = fixture.workspace().open(&fixture.source_id).unwrap();
    let second_before = fixture.workspace().open(&fixture.second_source_id).unwrap();

    let opens = fixture.vault.opens();
    let created = fixture
        .owner
        .create_capability(
            &fixture.vault,
            &fixture.source_id,
            &fixture.fragment_id,
            "Managed origin 🦀",
        )
        .unwrap();
    assert_eq!(fixture.vault.opens(), opens + 1);
    assert!(!created.already_applied);
    assert_eq!(created.capability.revision.title, "Managed origin 🦀");
    assert_eq!(
        created.capability.revision.content,
        "\u{feff}# Rules\r\nKeep exact 🦀 bytes.\r\n"
    );
    assert!(matches!(
        created.capability.origin,
        Origin::Source {
            ref source_id,
            ref fragment_id,
            ..
        } if source_id == &fixture.source_id && fragment_id == &fixture.fragment_id
    ));
    assert_eq!(created.capability.history.len(), 1);
    assert!(matches!(
        created.capability.revision.provenance,
        RevisionProvenance::Ordinary {}
    ));
    assert_eq!(
        serde_json::to_value(&created).unwrap(),
        serde_json::to_value(
            fixture
                .workspace()
                .open_capability_v1(&created.capability.id, None)
                .map(|capability| rangoon_store::CapabilityReceiptV1 {
                    capability,
                    already_applied: false,
                })
                .unwrap()
        )
        .unwrap()
    );

    let repeated = fixture
        .owner
        .create_capability(
            &fixture.vault,
            &fixture.source_id,
            &fixture.fragment_id,
            "Managed origin 🦀",
        )
        .unwrap();
    assert!(repeated.already_applied);
    assert_eq!(repeated.capability, created.capability);

    let initial = created.capability.latest_revision_id.clone();
    let revised = fixture
        .owner
        .revise_capability(
            &fixture.vault,
            &created.capability.id,
            &initial,
            "Managed revision Δ",
            "Exact revised content 🦀\n",
        )
        .unwrap();
    assert!(!revised.already_applied);
    assert_eq!(revised.capability.id, created.capability.id);
    assert_eq!(
        revised.capability.revision.parent_revision_id.as_deref(),
        Some(initial.as_str())
    );
    assert_eq!(revised.capability.revision.title, "Managed revision Δ");
    assert_eq!(
        revised.capability.revision.content,
        "Exact revised content 🦀\n"
    );
    assert_eq!(revised.capability.history.len(), 2);
    assert_eq!(revised.capability.history[0].id, initial);
    assert_eq!(
        revised.capability.history[1].id,
        revised.capability.latest_revision_id
    );
    assert!(revised.capability.revision.created_at_ms > 0);

    let retried = fixture
        .owner
        .revise_capability(
            &fixture.vault,
            &revised.capability.id,
            &initial,
            "Managed revision Δ",
            "Exact revised content 🦀\n",
        )
        .unwrap();
    assert!(retried.already_applied);
    assert_eq!(retried.capability, revised.capability);

    let reviewed = fixture
        .owner
        .review_capability(
            &fixture.vault,
            &revised.capability.id,
            &revised.capability.latest_revision_id,
        )
        .unwrap();
    assert!(!reviewed.already_applied);
    let review = reviewed.capability.revision.review.as_ref().unwrap();
    assert_eq!(
        review.reviewer,
        rangoon_domain::capability::Reviewer::LocalOperator
    );
    assert!(review.reviewed_at_ms > 0);
    let reviewed_again = fixture
        .owner
        .review_capability(
            &fixture.vault,
            &reviewed.capability.id,
            &reviewed.capability.latest_revision_id,
        )
        .unwrap();
    assert!(reviewed_again.already_applied);
    assert_eq!(reviewed_again.capability, reviewed.capability);

    let trusted = fixture.workspace();
    assert_eq!(
        reviewed.capability,
        trusted
            .open_capability_v1(&reviewed.capability.id, None)
            .unwrap()
    );
    assert_eq!(trusted.open(&fixture.source_id).unwrap(), source_before);
    assert_eq!(
        trusted.open(&fixture.second_source_id).unwrap(),
        second_before
    );
    assert_eq!(
        trusted
            .open_capability_v1(&fixture.second_capability_id, None)
            .unwrap()
            .revision
            .title,
        "Second capability"
    );
    assert_eq!(
        trusted.open_workflow(&fixture.workflow_id, None).unwrap(),
        workflow_before
    );
    assert_eq!(
        trusted
            .open_workflow(&fixture.workflow_id, Some(&fixture.workflow_head))
            .unwrap(),
        workflow_before
    );
    assert_eq!(
        trusted
            .open_capability_v1(&fixture.target_id, Some(&fixture.target_head))
            .unwrap()
            .latest_revision_id,
        fixture.target_head
    );

    fixture.owner.lock();
    fixture.reopen();
    assert_eq!(
        fixture
            .owner
            .open_capability(&fixture.vault, &reviewed.capability.id, None)
            .unwrap(),
        reviewed.capability
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn managed_capability_mutations_refuse_locked_and_recovery_before_vault_access() {
    let mut fixture = Fixture::new();
    let before = fixture.state();
    fixture.owner.lock();
    let opens = fixture.vault.opens();
    assert_error(
        fixture
            .owner
            .create_capability(
                &fixture.vault,
                &fixture.source_id,
                &fixture.fragment_id,
                "Locked create",
            )
            .map(|_| ()),
        Error::Locked,
    );
    assert_error(
        fixture
            .owner
            .revise_capability(
                &fixture.vault,
                &fixture.target_id,
                &fixture.target_head,
                "Locked revise",
                "Locked content",
            )
            .map(|_| ()),
        Error::Locked,
    );
    assert_error(
        fixture
            .owner
            .review_capability(&fixture.vault, &fixture.target_id, &fixture.target_head)
            .map(|_| ()),
        Error::Locked,
    );
    assert_eq!(fixture.vault.opens(), opens);
    fixture.assert_unchanged(&before);

    let mut fixture = Fixture::new();
    let before = fixture.state();
    fixture.owner.recovery = true;
    let opens = fixture.vault.opens();
    for route in 0..3 {
        let result = match route {
            0 => fixture
                .owner
                .create_capability(
                    &fixture.vault,
                    &fixture.source_id,
                    &fixture.fragment_id,
                    "Recovery create",
                )
                .map(|_| ()),
            1 => fixture
                .owner
                .revise_capability(
                    &fixture.vault,
                    &fixture.target_id,
                    &fixture.target_head,
                    "Recovery revise",
                    "Recovery content",
                )
                .map(|_| ()),
            _ => fixture
                .owner
                .review_capability(&fixture.vault, &fixture.target_id, &fixture.target_head)
                .map(|_| ()),
        };
        assert_error(result, Error::RecoveryRequired);
    }
    assert_eq!(fixture.vault.opens(), opens);
    fixture.assert_unchanged(&before);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn key_drift_refuses_all_mutations_before_store_attempt_without_writes() {
    for route in 0..3 {
        let mut fixture = Fixture::new();
        let before = fixture.state();
        let original = fixture.record();
        fixture.vault.corrupt_record(&fixture.workspace_id);
        let opens = fixture.vault.opens();
        let result = match route {
            0 => fixture
                .owner
                .create_capability(
                    &fixture.vault,
                    &fixture.source_id,
                    &fixture.fragment_id,
                    "Drift create",
                )
                .map(|_| ()),
            1 => fixture
                .owner
                .revise_capability(
                    &fixture.vault,
                    &fixture.target_id,
                    &fixture.target_head,
                    "Drift revise",
                    "Drift content",
                )
                .map(|_| ()),
            _ => fixture
                .owner
                .review_capability(&fixture.vault, &fixture.target_id, &fixture.target_head)
                .map(|_| ()),
        };
        assert_error(result, Error::Key(workspace_keys::Error::Changed));
        assert!(!fixture.owner.is_unlocked(), "route {route}");
        assert_eq!(fixture.vault.opens(), opens + 1, "route {route}");
        fixture.replace_record(original);
        fixture.assert_unchanged(&before);
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn store_refusals_are_uncertain_fenced_and_do_not_change_database() {
    for case in 0..6 {
        let mut fixture = Fixture::new();
        let before = fixture.state();
        let opens = fixture.vault.opens();
        let missing_source = format!("source:{}", "0".repeat(64));
        let missing_capability = format!("capability:{}", "1".repeat(64));
        let stale_revision = format!("revision:{}", "2".repeat(64));
        let result = match case {
            0 => fixture
                .owner
                .create_capability(
                    &fixture.vault,
                    &missing_source,
                    &fixture.fragment_id,
                    "Missing source",
                )
                .map(|_| ()),
            1 => fixture
                .owner
                .create_capability(
                    &fixture.vault,
                    &fixture.source_id,
                    &fixture.second_fragment_id,
                    "Wrong source owner",
                )
                .map(|_| ()),
            2 => fixture
                .owner
                .revise_capability(
                    &fixture.vault,
                    &missing_capability,
                    &stale_revision,
                    "Missing capability",
                    "No database change",
                )
                .map(|_| ()),
            3 => fixture
                .owner
                .revise_capability(
                    &fixture.vault,
                    &fixture.target_id,
                    &stale_revision,
                    "Stale revision",
                    "No database change",
                )
                .map(|_| ()),
            4 => fixture
                .owner
                .review_capability(&fixture.vault, &missing_capability, &stale_revision)
                .map(|_| ()),
            _ => fixture
                .owner
                .review_capability(&fixture.vault, &fixture.target_id, &stale_revision)
                .map(|_| ()),
        };
        assert_error(result, Error::WriteUncertain);
        assert_eq!(fixture.vault.opens(), opens + 1, "case {case}");
        fixture.assert_unchanged(&before);
        assert_recovery(&mut fixture.owner);
    }
}

#[cfg(feature = "encrypted-workspace")]
fn unicode_mutant(index: usize) -> String {
    let value = MUTATION_SEED.wrapping_add((index as u64).wrapping_mul(0x9E37_79B9));
    match index % 6 {
        0 => String::new(),
        1 => format!("  spaced-{value:016x}"),
        2 => format!("line\u{2028}{value:016x}"),
        3 => format!("bad\0{value:016x}"),
        4 => format!("é{}", "a".repeat(300)),
        _ => format!("🦀-{value:016x}-{}", "b".repeat(300)),
    }
}

#[cfg(feature = "encrypted-workspace")]
fn invalid_content_mutant(index: usize) -> String {
    let value = MUTATION_SEED.wrapping_add((index as u64).wrapping_mul(0x9E37_79B9));
    match index % 6 {
        0 => String::new(),
        1 => " \t\r\n".into(),
        2 => format!("bad\0{value:016x}"),
        3 => format!("é{value:016x}{}", "a".repeat(256 * 1024)),
        4 => "\u{2028}\u{2003}".into(),
        _ => format!("🦀-{value:016x}{}", "b".repeat(256 * 1024)),
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn fixed_unicode_mutation_matrix_refuses_without_retry_or_key_write() {
    let mut exercised = BTreeSet::new();
    for index in 0..MUTATION_CASES {
        let mut fixture = Fixture::new();
        let before = fixture.state();
        let opens = fixture.vault.opens();
        let value = unicode_mutant(index);
        let route = (index / 6) % 3;
        let alternate_field = index >= 18;
        exercised.insert((route, index % 6, alternate_field));
        let result = match route {
            0 => fixture
                .owner
                .create_capability(
                    &fixture.vault,
                    if alternate_field {
                        &value
                    } else {
                        &fixture.source_id
                    },
                    &fixture.fragment_id,
                    if alternate_field {
                        "Valid title"
                    } else {
                        &value
                    },
                )
                .map(|_| ()),
            1 => fixture
                .owner
                .revise_capability(
                    &fixture.vault,
                    &fixture.target_id,
                    &fixture.target_head,
                    if alternate_field {
                        "Valid title"
                    } else {
                        &value
                    },
                    &if alternate_field {
                        invalid_content_mutant(index)
                    } else {
                        "Valid content".into()
                    },
                )
                .map(|_| ()),
            _ => fixture
                .owner
                .review_capability(
                    &fixture.vault,
                    if alternate_field {
                        &fixture.target_id
                    } else {
                        &value
                    },
                    if alternate_field {
                        &value
                    } else {
                        &fixture.target_head
                    },
                )
                .map(|_| ()),
        };
        assert_error(result, Error::WriteUncertain);
        assert_eq!(fixture.vault.opens(), opens + 1, "case {index}");
        fixture.assert_unchanged(&before);
        assert_recovery(&mut fixture.owner);
    }
    assert_eq!(exercised.len(), 3 * 6 * 2);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn post_commit_fault_returns_no_receipt_and_reopen_proves_no_rollback_claim() {
    for route in 0..3 {
        let mut fixture = Fixture::new();
        let before = fixture.state();
        let vault_writes = fixture.vault.writes(&fixture.workspace_id);
        fixture.owner.inject_fault(Fault::AfterCapabilityMutation);
        let title = format!("Committed then uncertain {route}");
        let result = match route {
            0 => fixture
                .owner
                .create_capability(
                    &fixture.vault,
                    &fixture.source_id,
                    &fixture.fragment_id,
                    &title,
                )
                .map(|receipt| receipt.capability.id),
            1 => fixture
                .owner
                .revise_capability(
                    &fixture.vault,
                    &fixture.target_id,
                    &fixture.target_head,
                    &title,
                    "Committed revision bytes",
                )
                .map(|receipt| receipt.capability.id),
            _ => fixture
                .owner
                .review_capability(&fixture.vault, &fixture.target_id, &fixture.target_head)
                .map(|receipt| receipt.capability.id),
        };
        assert_error(result, Error::WriteUncertain);
        assert_recovery(&mut fixture.owner);
        assert_ne!(fixture.state().database, before.database, "route {route}");
        assert_eq!(fixture.vault.writes(&fixture.workspace_id), vault_writes);

        let capability_id = if route == 0 {
            rangoon_domain::capability::capability_id(
                &fixture.source_id,
                &fixture.fragment_id,
                &title,
            )
        } else {
            fixture.target_id.clone()
        };
        drop(fixture.owner);
        let mut reopened = Owner::acquire(fixture.root.0.clone()).unwrap();
        let preview = reopened.prepare_unlock().unwrap();
        reopened.confirm(&preview.id, &fixture.vault).unwrap();
        let trusted =
            Fixture::trusted_workspace(&fixture.root, &fixture.vault, &fixture.workspace_id);
        let committed = reopened
            .open_capability(&fixture.vault, &capability_id, None)
            .unwrap();
        assert_eq!(committed.id, capability_id);
        match route {
            0 => assert_eq!(committed.revision.title, title),
            1 => {
                assert_eq!(committed.revision.title, title);
                assert_eq!(committed.revision.content, "Committed revision bytes");
            }
            _ => assert!(committed.revision.review.is_some()),
        }
        assert_eq!(
            committed,
            trusted.open_capability_v1(&capability_id, None).unwrap()
        );
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn managed_mutations_leave_disposable_legacy_plaintext_unchanged() {
    let root = TestRoot::new("capability-mutations-legacy");
    let legacy = root.0.join("source-workspace");
    fs::create_dir(&legacy).unwrap();
    let legacy_file = legacy.join("legacy.sqlite3");
    let bytes = b"legacy plaintext fixture\0preserved";
    fs::write(&legacy_file, bytes).unwrap();
    let vault = FakeVault::default();
    let report = selected_source();
    let (mut owner, preview) = create_ready(&root, &vault, &report);
    let created = owner
        .create_capability(
            &vault,
            &report.source.id,
            &report.fragments[0].id,
            "Legacy isolation",
        )
        .unwrap();
    assert!(!created.already_applied);
    assert_eq!(
        created.capability.revision.content,
        "\u{feff}# Rules\r\nKeep exact 🦀 bytes.\r\n"
    );
    let revised = owner
        .revise_capability(
            &vault,
            &created.capability.id,
            &created.capability.latest_revision_id,
            "Legacy revision",
            "Legacy source-only revision 🦀\n",
        )
        .unwrap();
    assert!(!revised.already_applied);
    assert_eq!(revised.capability.history.len(), 2);
    let reviewed = owner
        .review_capability(
            &vault,
            &revised.capability.id,
            &revised.capability.latest_revision_id,
        )
        .unwrap();
    assert!(!reviewed.already_applied);
    assert!(reviewed.capability.revision.review.is_some());
    assert_eq!(fs::read(legacy_file).unwrap(), bytes);
    assert_eq!(vault.writes(&preview.workspace_id), 1);
}
