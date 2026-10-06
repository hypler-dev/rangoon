use super::*;
use composition_records_tests::{TestDir, assert_unchanged, fixture_request, legacy, source};
use rangoon_compile::{Profile, Readiness};
use rangoon_domain::{Authority, capability_v1::RevisionProvenance};

#[test]
fn compilation_never_creates_a_workspace_and_rejects_bad_ids_before_open() {
    let dir = TestDir::new();
    let capability = format!("capability:{}", "a".repeat(64));
    let revision = format!("revision:{}", "b".repeat(64));
    assert!(matches!(
        dir.store()
            .compile_capability(&capability, &revision, Profile::AgentsMdV1),
        Err(StoreError::CapabilityNotFound)
    ));
    for (id, rev) in [
        ("../outside", revision.as_str()),
        (capability.as_str(), "revision:wrong"),
    ] {
        assert!(matches!(
            dir.store().compile_capability(id, rev, Profile::AgentsMdV1),
            Err(StoreError::CapabilityInvalid)
        ));
    }
    assert!(!dir.0.exists());
}

#[test]
fn saved_exact_bytes_and_local_review_control_candidate_without_writes() {
    let dir = TestDir::new();
    let saved = source(&dir);
    let skill = legacy(&dir, &saved);
    let content = "\u{feff}# Keep\r\nPreserve originals 🦀.\r\nNo final newline";
    let skill = dir
        .store()
        .revise_capability_v1(&skill.id, &skill.latest_revision_id, "Exact", content)
        .unwrap()
        .capability;
    let before = fs::read(dir.store().path()).unwrap();
    let before_inventory = dir.store().composition_data().unwrap();
    let unreviewed = dir
        .store()
        .compile_capability(&skill.id, &skill.latest_revision_id, Profile::AgentsMdV1)
        .unwrap();
    assert_eq!(unreviewed.compilation.readiness, Readiness::ReviewRequired);
    assert!(unreviewed.compilation.candidate.is_none());
    assert_eq!(unreviewed.compilation.artifact.content, content);
    assert_eq!(unreviewed.compilation.authority, Authority::None);
    assert_unchanged(&dir, &before);
    assert_eq!(
        dir.store().composition_data().unwrap().state_id,
        before_inventory.state_id
    );

    dir.store()
        .review_capability_v1(&skill.id, &skill.latest_revision_id)
        .unwrap();
    let before = fs::read(dir.store().path()).unwrap();
    let reviewed = dir
        .store()
        .compile_capability(&skill.id, &skill.latest_revision_id, Profile::AgentsMdV1)
        .unwrap();
    assert_eq!(reviewed.compilation.readiness, Readiness::Candidate);
    assert!(reviewed.compilation.candidate.is_some());
    assert_eq!(
        reviewed.compilation.compilation_id,
        unreviewed.compilation.compilation_id
    );
    assert_eq!(
        reviewed.compilation.artifact.content.as_bytes(),
        content.as_bytes()
    );
    assert_eq!(reviewed.observed_current_head, skill.latest_revision_id);
    let claude = dir
        .store()
        .compile_capability(&skill.id, &skill.latest_revision_id, Profile::ClaudeMdV1)
        .unwrap();
    assert_eq!(claude.compilation.artifact.path, "CLAUDE.md");
    assert_eq!(claude.compilation.artifact.content, content);
    assert_ne!(
        claude.compilation.compilation_id,
        reviewed.compilation.compilation_id
    );
    assert_unchanged(&dir, &before);
}

#[test]
fn a_reviewed_historical_revision_keeps_its_bytes_and_identity_after_head_moves() {
    let dir = TestDir::new();
    let saved = source(&dir);
    let skill = legacy(&dir, &saved);
    dir.store()
        .review_capability_v1(&skill.id, &skill.latest_revision_id)
        .unwrap();
    let original = dir
        .store()
        .compile_capability(&skill.id, &skill.latest_revision_id, Profile::AgentsMdV1)
        .unwrap();
    let next = dir
        .store()
        .revise_capability_v1(
            &skill.id,
            &skill.latest_revision_id,
            "Changed",
            "Changed content",
        )
        .unwrap()
        .capability;
    let before = fs::read(dir.store().path()).unwrap();
    let historical = dir
        .store()
        .compile_capability(&skill.id, &skill.latest_revision_id, Profile::AgentsMdV1)
        .unwrap();
    assert_eq!(historical.observed_current_head, next.latest_revision_id);
    assert_eq!(
        historical.compilation.selected_revision.revision_id,
        skill.latest_revision_id
    );
    assert_eq!(
        serde_json::to_vec(&historical.compilation).unwrap(),
        serde_json::to_vec(&original.compilation).unwrap()
    );
    let latest = dir
        .store()
        .compile_capability(&next.id, &next.latest_revision_id, Profile::AgentsMdV1)
        .unwrap();
    assert_eq!(latest.compilation.readiness, Readiness::ReviewRequired);
    assert_eq!(latest.compilation.artifact.content, "Changed content");
    assert_unchanged(&dir, &before);
}

#[test]
fn composition_new_and_append_revisions_use_validated_saved_lineage() {
    let dir = TestDir::new();
    let saved = source(&dir);
    let old = legacy(&dir, &saved);
    let mut request = fixture_request();
    request.targets[0] = rangoon_compose::application::Target::Append {
        capability_id: old.id.clone(),
        expected_revision_id: old.latest_revision_id.clone(),
    };
    let preview = dir.store().preview_composition(&request).unwrap();
    let applied = dir.store().apply_composition(&preview, true).unwrap();
    for skill in &applied.capabilities {
        dir.store()
            .review_capability_v1(&skill.id, &skill.latest_revision_id)
            .unwrap();
    }
    let before = fs::read(dir.store().path()).unwrap();
    for skill in &applied.capabilities {
        let compiled = dir
            .store()
            .compile_capability(&skill.id, &skill.latest_revision_id, Profile::AgentsMdV1)
            .unwrap();
        assert_eq!(compiled.compilation.readiness, Readiness::Candidate);
        assert_eq!(
            compiled.compilation.artifact.content,
            skill.revision.content
        );
        assert!(matches!(
            compiled.compilation.selected_revision.provenance,
            RevisionProvenance::Composition { .. }
        ));
    }
    assert!(matches!(
        dir.store().compile_capability(
            &applied.capabilities[0].id,
            &applied.capabilities[1].latest_revision_id,
            Profile::AgentsMdV1
        ),
        Err(StoreError::CapabilityNotFound)
    ));
    assert_unchanged(&dir, &before);
}

#[test]
fn static_errors_block_reviewed_content_without_silent_rewriting() {
    let dir = TestDir::new();
    let saved = source(&dir);
    let old = legacy(&dir, &saved);
    let content = "Contact team@example.test.\n<!-- Keep this requirement. -->\n";
    let skill = dir
        .store()
        .revise_capability_v1(&old.id, &old.latest_revision_id, "Context", content)
        .unwrap()
        .capability;
    dir.store()
        .review_capability_v1(&skill.id, &skill.latest_revision_id)
        .unwrap();
    let before = fs::read(dir.store().path()).unwrap();
    let result = dir
        .store()
        .compile_capability(&skill.id, &skill.latest_revision_id, Profile::ClaudeMdV1)
        .unwrap();
    assert_eq!(result.compilation.readiness, Readiness::Blocked);
    assert!(result.compilation.candidate.is_none());
    assert_eq!(result.compilation.artifact.content, content);
    assert_eq!(result.compilation.diagnostics.len(), 4);
    assert_unchanged(&dir, &before);
}

#[test]
fn corrupted_content_is_rejected_before_any_compiler_candidate() {
    let dir = TestDir::new();
    let saved = source(&dir);
    let skill = legacy(&dir, &saved);
    dir.store()
        .review_capability_v1(&skill.id, &skill.latest_revision_id)
        .unwrap();
    dir.db()
        .execute(
            "UPDATE revisions SET content=?1 WHERE revision_id=?2",
            params![b"Substituted bytes".as_slice(), skill.latest_revision_id],
        )
        .unwrap();
    let corrupt = fs::read(dir.store().path()).unwrap();
    assert!(matches!(
        dir.store()
            .compile_capability(&skill.id, &skill.latest_revision_id, Profile::AgentsMdV1),
        Err(StoreError::Corrupt)
    ));
    assert_eq!(fs::read(dir.store().path()).unwrap(), corrupt);
}
