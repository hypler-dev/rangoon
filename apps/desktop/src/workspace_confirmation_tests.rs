#[cfg(feature = "encrypted-workspace")]
use super::*;

#[cfg(feature = "encrypted-workspace")]
use std::fs;

#[cfg(feature = "encrypted-workspace")]
use rangoon_compose::{
    Draft, InputRange, InputReference, Operation, OutputRecipe, Piece,
    application::{Request, Target},
};

#[cfg(feature = "encrypted-workspace")]
const OPAQUE_MUTATION_SEED: u64 = 0xE1B2_C0F1_1A55_2026;

#[cfg(feature = "encrypted-workspace")]
struct Fixture {
    root: TestRoot,
    vault: FakeVault,
    owner: Owner,
    workspace_id: String,
    report: rangoon_domain::AnalysisReport,
}

#[cfg(feature = "encrypted-workspace")]
impl Fixture {
    fn ready(label: &str) -> Self {
        let root = TestRoot::new(label);
        let vault = FakeVault::default();
        let report = selected_source();
        let (owner, preview) = create_ready(&root, &vault, &report);
        Self {
            root,
            vault,
            owner,
            workspace_id: preview.workspace_id,
            report,
        }
    }

    fn request(&self) -> Request {
        let end = self.report.source.content.len() as u64;
        Request {
            schema_version: rangoon_compose::application::REQUEST_SCHEMA.into(),
            draft: Draft {
                schema_version: rangoon_compose::DRAFT_SCHEMA.into(),
                operation: Operation::Decompose,
                inputs: vec![InputReference::Source {
                    source_id: self.report.source.id.clone(),
                    sha256: self.report.source.sha256.clone(),
                }],
                outputs: vec![OutputRecipe {
                    title: "Exact retained confirmation bytes 🦀".into(),
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

    fn database(&self) -> Vec<u8> {
        fs::read(database_path(&self.root, &self.workspace_id)).unwrap()
    }

    fn key_record(&self) -> Vec<u8> {
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

    fn assert_unchanged(&self, database: &[u8], key_record: &[u8], reads: usize, writes: usize) {
        assert_eq!(self.database(), database);
        assert_eq!(self.key_record(), key_record);
        assert_eq!(self.vault.reads(&self.workspace_id), reads);
        assert_eq!(self.vault.writes(&self.workspace_id), writes);
    }
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn retained_confirmation_commits_exact_store_preview_with_no_authority() {
    let mut fixture = Fixture::ready("confirmation-success");
    let request = fixture.request();
    let confirmation = fixture
        .owner
        .prepare_composition_confirmation(&fixture.vault, &request)
        .unwrap();

    assert_eq!(confirmation.workspace_id, fixture.workspace_id);
    assert_eq!(fixture.owner.composition_generation, Some(2));
    assert_eq!(confirmation.generation, "2");
    assert!(confirmation.inspection.preview.saveable);
    let receipt = fixture
        .owner
        .confirm_composition(&fixture.vault, &confirmation.id, true)
        .unwrap();
    assert_eq!(receipt.authority, rangoon_domain::Authority::None);
    assert_eq!(receipt.capabilities.len(), 1);
    assert_eq!(
        receipt.capabilities[0].revision.content,
        fixture.report.source.content
    );
    let database = fixture.database();
    let key_record = fixture.key_record();
    let reads = fixture.vault.reads(&fixture.workspace_id);
    let writes = fixture.vault.writes(&fixture.workspace_id);
    let opens = fixture.vault.opens();
    assert_error(
        fixture
            .owner
            .confirm_composition(&fixture.vault, &confirmation.id, true)
            .map(|_| ()),
        Error::Stale,
    );
    assert_eq!(fixture.vault.opens(), opens);
    fixture.assert_unchanged(&database, &key_record, reads, writes);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn display_copy_tamper_cannot_change_the_retained_composition() {
    let mut fixture = Fixture::ready("confirmation-copy-isolation");
    let request = fixture.request();
    let mut confirmation = fixture
        .owner
        .prepare_composition_confirmation(&fixture.vault, &request)
        .unwrap();
    let id = confirmation.id.clone();
    confirmation.workspace_id = "0".repeat(64);
    confirmation.generation = "99999999999999999999".into();
    confirmation.inspection.expected_state_id = "workspace:forged-display-copy".into();
    confirmation.inspection.preview.core.outputs[0].content = "forged renderer text".into();
    confirmation.inspection.preview.core.outputs[0].sha256 = "0".repeat(64);
    confirmation.inspection.preview.saveable = false;

    let receipt = fixture
        .owner
        .confirm_composition(&fixture.vault, &id, true)
        .unwrap();
    assert_eq!(receipt.authority, rangoon_domain::Authority::None);
    assert_eq!(
        receipt.capabilities[0].revision.content,
        fixture.report.source.content
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn opaque_unknown_ids_with_false_ack_leave_the_one_valid_record_usable() {
    let mut fixture = Fixture::ready("confirmation-opaque-stale");
    let request = fixture.request();
    let confirmation = fixture
        .owner
        .prepare_composition_confirmation(&fixture.vault, &request)
        .unwrap();
    let database = fixture.database();
    let key_record = fixture.key_record();
    let reads = fixture.vault.reads(&fixture.workspace_id);
    let writes = fixture.vault.writes(&fixture.workspace_id);
    let opens = fixture.vault.opens();
    let mut ids = vec![String::new(), format!("{}x", confirmation.id)];
    ids.push(confirmation.id[..confirmation.id.len() - 1].into());
    ids.push(confirmation.id.replacen(':', ";", 1));
    let mut bytes = confirmation.id.as_bytes().to_vec();
    let index = (OPAQUE_MUTATION_SEED as usize) % bytes.len();
    bytes[index] = b'!';
    let exact_mutation = String::from_utf8(bytes).unwrap();
    assert_ne!(exact_mutation, confirmation.id);
    ids.push(exact_mutation);

    for id in ids {
        assert_error(
            fixture
                .owner
                .confirm_composition(&fixture.vault, &id, false)
                .map(|_| ()),
            Error::Stale,
        );
        assert_eq!(fixture.vault.opens(), opens);
        fixture.assert_unchanged(&database, &key_record, reads, writes);
    }
    fixture
        .owner
        .confirm_composition(&fixture.vault, &confirmation.id, true)
        .unwrap();
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn matching_decline_consumes_confirmation_without_vault_or_store_access() {
    let mut fixture = Fixture::ready("confirmation-decline");
    let request = fixture.request();
    let confirmation = fixture
        .owner
        .prepare_composition_confirmation(&fixture.vault, &request)
        .unwrap();
    let database = fixture.database();
    let key_record = fixture.key_record();
    let reads = fixture.vault.reads(&fixture.workspace_id);
    let writes = fixture.vault.writes(&fixture.workspace_id);
    let opens = fixture.vault.opens();

    assert_error(
        fixture
            .owner
            .confirm_composition(&fixture.vault, &confirmation.id, false)
            .map(|_| ()),
        Error::ConsentRequired,
    );
    assert_eq!(fixture.vault.opens(), opens);
    fixture.assert_unchanged(&database, &key_record, reads, writes);
    assert_error(
        fixture
            .owner
            .confirm_composition(&fixture.vault, &confirmation.id, true)
            .map(|_| ()),
        Error::Stale,
    );
    assert_eq!(fixture.vault.opens(), opens);
    fixture.assert_unchanged(&database, &key_record, reads, writes);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn replacement_and_failed_preparation_invalidate_older_confirmations() {
    let mut fixture = Fixture::ready("confirmation-replacement");
    let request = fixture.request();
    let first = fixture
        .owner
        .prepare_composition_confirmation(&fixture.vault, &request)
        .unwrap();
    let second = fixture
        .owner
        .prepare_composition_confirmation(&fixture.vault, &request)
        .unwrap();
    let opens = fixture.vault.opens();
    assert_error(
        fixture
            .owner
            .confirm_composition(&fixture.vault, &first.id, true)
            .map(|_| ()),
        Error::Stale,
    );
    assert_eq!(fixture.vault.opens(), opens);
    fixture
        .owner
        .confirm_composition(&fixture.vault, &second.id, true)
        .unwrap();

    // The committed output identity cannot be created again. Prepare a new
    // candidate before testing invalidation by a failed subsequent preview.
    let mut request = request;
    request.draft.outputs[0].title = "Fresh pending output after commit".into();
    let current = fixture
        .owner
        .prepare_composition_confirmation(&fixture.vault, &request)
        .unwrap();
    let mut malformed = request.clone();
    let InputReference::Source { sha256, .. } = &mut malformed.draft.inputs[0] else {
        unreachable!();
    };
    *sha256 = "0".repeat(64);
    let opens = fixture.vault.opens();
    assert_error(
        fixture
            .owner
            .prepare_composition_confirmation(&fixture.vault, &malformed)
            .map(|_| ()),
        Error::StoreInvalid,
    );
    assert_eq!(fixture.vault.opens(), opens + 1);
    let opens = fixture.vault.opens();
    assert_error(
        fixture
            .owner
            .confirm_composition(&fixture.vault, &current.id, true)
            .map(|_| ()),
        Error::Stale,
    );
    assert_eq!(fixture.vault.opens(), opens);

    let mut pre_custody = Fixture::ready("confirmation-pre-custody-failure");
    let request = pre_custody.request();
    let retained = pre_custody
        .owner
        .prepare_composition_confirmation(&pre_custody.vault, &request)
        .unwrap();
    pre_custody.owner.recovery = true;
    let opens = pre_custody.vault.opens();
    assert_error(
        pre_custody
            .owner
            .prepare_composition_confirmation(&pre_custody.vault, &request)
            .map(|_| ()),
        Error::RecoveryRequired,
    );
    assert_eq!(pre_custody.vault.opens(), opens);
    assert_error(
        pre_custody
            .owner
            .confirm_composition(&pre_custody.vault, &retained.id, true)
            .map(|_| ()),
        Error::Stale,
    );
    assert_eq!(pre_custody.vault.opens(), opens);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn explicit_and_automatic_locks_purge_confirmation_then_reacquire_rejects_replay() {
    let mut fixture = Fixture::ready("confirmation-lock-replay");
    let request = fixture.request();
    let explicit = fixture
        .owner
        .prepare_composition_confirmation(&fixture.vault, &request)
        .unwrap();
    fixture.owner.lock();
    let opens = fixture.vault.opens();
    assert_error(
        fixture
            .owner
            .confirm_composition(&fixture.vault, &explicit.id, false)
            .map(|_| ()),
        Error::Stale,
    );
    assert_eq!(fixture.vault.opens(), opens);

    let unlock = fixture.owner.prepare_unlock().unwrap();
    fixture.owner.confirm(&unlock.id, &fixture.vault).unwrap();
    let opens = fixture.vault.opens();
    assert_error(
        fixture
            .owner
            .confirm_composition(&fixture.vault, &explicit.id, false)
            .map(|_| ()),
        Error::Stale,
    );
    assert_eq!(fixture.vault.opens(), opens);
    let automatic = fixture
        .owner
        .prepare_composition_confirmation(&fixture.vault, &request)
        .unwrap();
    fixture.vault.unavailable();
    assert_error(
        fixture
            .owner
            .preview_composition(&fixture.vault, &request)
            .map(|_| ()),
        Error::Key(workspace_keys::Error::Unavailable),
    );
    assert!(!fixture.owner.is_unlocked());
    let opens = fixture.vault.opens();
    assert_error(
        fixture
            .owner
            .confirm_composition(&fixture.vault, &automatic.id, true)
            .map(|_| ()),
        Error::Stale,
    );
    assert_eq!(fixture.vault.opens(), opens);

    let Fixture {
        root,
        vault,
        workspace_id: _,
        owner,
        report: _,
    } = fixture;
    drop(owner);
    vault.0.borrow_mut().unavailable = false;
    let mut reopened = Owner::acquire(root.0.clone()).unwrap();
    let unlock = reopened.prepare_unlock().unwrap();
    reopened.confirm(&unlock.id, &vault).unwrap();
    assert_error(
        reopened
            .confirm_composition(&vault, &automatic.id, false)
            .map(|_| ()),
        Error::Stale,
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn state_drift_and_pre_call_custody_refusal_do_not_apply_a_retained_preview() {
    let mut fixture = Fixture::ready("confirmation-state-drift");
    let request = fixture.request();
    let confirmation = fixture
        .owner
        .prepare_composition_confirmation(&fixture.vault, &request)
        .unwrap();
    fixture
        .owner
        .save(&fixture.vault, &source(b"# State changed\n"))
        .unwrap();
    let database = fixture.database();
    assert_error(
        fixture
            .owner
            .confirm_composition(&fixture.vault, &confirmation.id, true)
            .map(|_| ()),
        Error::WriteUncertain,
    );
    assert!(!fixture.owner.is_unlocked());
    assert_eq!(fixture.database(), database);
    assert_error(fixture.owner.prepare_unlock(), Error::RecoveryRequired);

    let mut custody = Fixture::ready("confirmation-custody-refusal");
    let custody_request = custody.request();
    let handle = custody
        .owner
        .prepare_composition_confirmation(&custody.vault, &custody_request)
        .unwrap();
    let database = custody.database();
    let key_record = custody.key_record();
    let reads = custody.vault.reads(&custody.workspace_id);
    let writes = custody.vault.writes(&custody.workspace_id);
    let opens = custody.vault.opens();
    custody.vault.unavailable();
    assert_error(
        custody
            .owner
            .confirm_composition(&custody.vault, &handle.id, true)
            .map(|_| ()),
        Error::Key(workspace_keys::Error::Unavailable),
    );
    assert_eq!(custody.vault.opens(), opens + 1);
    custody.assert_unchanged(&database, &key_record, reads, writes);
    let opens = custody.vault.opens();
    assert_error(
        custody
            .owner
            .confirm_composition(&custody.vault, &handle.id, false)
            .map(|_| ()),
        Error::Stale,
    );
    assert_eq!(custody.vault.opens(), opens);
    custody.assert_unchanged(&database, &key_record, reads, writes);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn private_retained_generation_or_identity_mismatch_consumes_without_store_access() {
    let mut generation = Fixture::ready("confirmation-generation-mismatch");
    let request = generation.request();
    let confirmation = generation
        .owner
        .prepare_composition_confirmation(&generation.vault, &request)
        .unwrap();
    let database = generation.database();
    let key_record = generation.key_record();
    let reads = generation.vault.reads(&generation.workspace_id);
    let writes = generation.vault.writes(&generation.workspace_id);
    let opens = generation.vault.opens();
    generation.owner.composition_generation = Some(77);
    assert_error(
        generation
            .owner
            .confirm_composition(&generation.vault, &confirmation.id, true)
            .map(|_| ()),
        Error::Stale,
    );
    assert_eq!(generation.vault.opens(), opens);
    generation.assert_unchanged(&database, &key_record, reads, writes);
    generation.owner.composition_generation = Some(confirmation.generation.parse().unwrap());
    assert_error(
        generation
            .owner
            .confirm_composition(&generation.vault, &confirmation.id, true)
            .map(|_| ()),
        Error::Stale,
    );
    assert_eq!(generation.vault.opens(), opens);
    generation.assert_unchanged(&database, &key_record, reads, writes);

    let mut identity = Fixture::ready("confirmation-identity-mismatch");
    let request = identity.request();
    let confirmation = identity
        .owner
        .prepare_composition_confirmation(&identity.vault, &request)
        .unwrap();
    let database = identity.database();
    let key_record = identity.key_record();
    let reads = identity.vault.reads(&identity.workspace_id);
    let writes = identity.vault.writes(&identity.workspace_id);
    let opens = identity.vault.opens();
    identity
        .owner
        .retained_composition
        .as_mut()
        .unwrap()
        .workspace_id = "0".repeat(64);
    assert_error(
        identity
            .owner
            .confirm_composition(&identity.vault, &confirmation.id, true)
            .map(|_| ()),
        Error::Stale,
    );
    assert_eq!(identity.vault.opens(), opens);
    identity.assert_unchanged(&database, &key_record, reads, writes);
    if let Some(retained) = identity.owner.retained_composition.as_mut() {
        retained.workspace_id = identity.workspace_id.clone();
    }
    assert_error(
        identity
            .owner
            .confirm_composition(&identity.vault, &confirmation.id, true)
            .map(|_| ()),
        Error::Stale,
    );
    assert_eq!(identity.vault.opens(), opens);
    identity.assert_unchanged(&database, &key_record, reads, writes);
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn post_apply_uncertainty_locks_owner_and_never_returns_a_receipt() {
    let mut fixture = Fixture::ready("confirmation-post-apply-fault");
    let request = fixture.request();
    let confirmation = fixture
        .owner
        .prepare_composition_confirmation(&fixture.vault, &request)
        .unwrap();
    let database = fixture.database();
    fixture.owner.inject_fault(Fault::AfterCompositionApply);
    assert_error(
        fixture
            .owner
            .confirm_composition(&fixture.vault, &confirmation.id, true)
            .map(|_| ()),
        Error::WriteUncertain,
    );
    assert!(!fixture.owner.is_unlocked());
    assert_ne!(fixture.database(), database);
    assert_error(fixture.owner.prepare_unlock(), Error::RecoveryRequired);
    assert_error(
        fixture
            .owner
            .confirm_composition(&fixture.vault, &confirmation.id, false)
            .map(|_| ()),
        Error::Stale,
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn generation_overflow_permanently_refuses_new_issuance_but_lock_releases_session() {
    let mut fixture = Fixture::ready("confirmation-overflow");
    let request = fixture.request();
    fixture.owner.composition_generation = Some(u64::MAX);
    fixture.owner.retained_composition = None;
    let opens = fixture.vault.opens();
    assert_error(
        fixture
            .owner
            .prepare_composition_confirmation(&fixture.vault, &request)
            .map(|_| ()),
        Error::Unavailable,
    );
    assert_eq!(fixture.owner.composition_generation, None);
    assert_eq!(fixture.vault.opens(), opens);
    fixture.owner.lock();
    assert!(!fixture.owner.is_unlocked());
    assert_error(
        fixture
            .owner
            .prepare_composition_confirmation(&fixture.vault, &request)
            .map(|_| ()),
        Error::Unavailable,
    );
    assert_eq!(fixture.vault.opens(), opens);
    let unlock = fixture.owner.prepare_unlock().unwrap();
    fixture.owner.confirm(&unlock.id, &fixture.vault).unwrap();
    assert!(fixture.owner.is_unlocked());
    assert_eq!(fixture.owner.composition_generation, None);
    let opens = fixture.vault.opens();
    assert_error(
        fixture
            .owner
            .prepare_composition_confirmation(&fixture.vault, &request)
            .map(|_| ()),
        Error::Unavailable,
    );
    assert_eq!(fixture.vault.opens(), opens);
}
