//! Disposable fixtures for read-only revision-level provenance validation.
use super::*;
use composition_records::Records;
use rangoon_compose::{
    Draft, InputRange, InputReference, Operation, OutputRecipe, Piece, ResolvedInput,
    application::{self, Request, Target, TargetHead},
};
use rangoon_domain::{
    Authority, byte_digest,
    capability::{self, CapabilityDetail as LegacyDetail},
    capability_v1::{CapabilityDetail, Origin, RevisionProvenance},
};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);
pub(super) struct TestDir(pub(super) PathBuf);
impl TestDir {
    pub(super) fn new() -> Self {
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rangoon-composition-records-{}-{}-{}", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        )))
    }
    pub(super) fn store(&self) -> Workspace {
        Workspace::new(self.0.clone())
    }
    pub(super) fn db(&self) -> Connection {
        Connection::open(self.store().path()).unwrap()
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn source(dir: &TestDir) -> AnalysisReport {
    let source = analyze(
        "AGENTS.md",
        b"\xef\xbb\xbf# Rules\r\nKeep originals.\r\n# Tests\r\nCheck edits.\r\n",
    )
    .unwrap();
    dir.store().save(&source).unwrap();
    source
}
pub(super) fn legacy(dir: &TestDir, source: &AnalysisReport) -> LegacyDetail {
    dir.store()
        .create_capability(&source.source.id, &source.fragments[0].id, "Legacy")
        .unwrap()
        .capability
}
pub(super) fn fixture_request() -> Request {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/composition/decompose-v0.json"
    ))
    .unwrap();
    Request {
        schema_version: application::REQUEST_SCHEMA.into(),
        draft: serde_json::from_value(value["draft"].clone()).unwrap(),
        targets: vec![Target::New {}, Target::New {}],
    }
}
pub(super) fn resolved_source(source: &AnalysisReport) -> Vec<ResolvedInput> {
    vec![ResolvedInput::Source {
        source_id: source.source.id.clone(),
        sha256: source.source.sha256.clone(),
        content: source.source.content.clone(),
    }]
}
pub(super) fn schema3(db: &Connection) {
    let version: i64 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    if version == 1 {
        for (_, sql) in capabilities::SCHEMAS {
            db.execute_batch(sql).unwrap();
        }
    }
    for (_, sql) in composition_records::SCHEMAS {
        db.execute_batch(sql).unwrap();
    }
    db.pragma_update(None, "user_version", 3).unwrap();
}

/// Test-only composition fixture construction. Production restore validates archives;
/// a production composition-application commit API is still pending.
pub(super) fn retain_outputs(
    db: &Connection,
    request: &Request,
    inputs: &[ResolvedInput],
    heads: &[TargetHead],
    live: &[usize],
) -> application::ApplicationPreview {
    let preview = application::preview(request, inputs, heads).unwrap();
    assert!(preview.saveable);
    db.execute(
        "INSERT OR IGNORE INTO compositions VALUES (?1,?2,?3,1)",
        params![
            preview.core.composition_id,
            rangoon_compose::canonical_draft_bytes(&request.draft).unwrap(),
            rangoon_compose::TRANSFORMATION_VERSION
        ],
    )
    .unwrap();
    db.execute(
        "INSERT INTO composition_applications VALUES (?1,?2,?3,'local_operator',1)",
        params![
            preview.application_id,
            preview.core.composition_id,
            serde_json::to_vec(&request.targets).unwrap()
        ],
    )
    .unwrap();
    for &index in live {
        let applied = &preview.applied_outputs[index];
        let output = &preview.core.outputs[index];
        db.execute(
            "INSERT INTO revisions VALUES (?1,?2,?3,?4,?5,?6,1)",
            params![
                applied.revision_id,
                applied.capability_id,
                applied.parent_revision_id,
                output.title,
                output.content.as_bytes(),
                output.sha256
            ],
        )
        .unwrap();
        db.execute(
            "INSERT INTO revision_derivations VALUES (?1,?2,?3)",
            params![applied.revision_id, preview.application_id, index as u32],
        )
        .unwrap();
        match applied.kind {
            application::AppliedKind::New => {
                db.execute(
                    "INSERT INTO derived_capabilities VALUES (?1,?2,?3,?4)",
                    params![
                        applied.capability_id,
                        preview.core.composition_id,
                        index as u32,
                        applied.revision_id
                    ],
                )
                .unwrap();
            }
            application::AppliedKind::Append => {
                let n = db
                    .execute(
                        "UPDATE capabilities SET latest_revision_id=?1 WHERE capability_id=?2",
                        params![applied.revision_id, applied.capability_id],
                    )
                    .unwrap();
                let m=db.execute("UPDATE derived_capabilities SET latest_revision_id=?1 WHERE capability_id=?2",params![applied.revision_id,applied.capability_id]).unwrap();
                assert_eq!(n + m, 1);
            }
        }
    }
    preview
}
pub(super) fn assert_unchanged(dir: &TestDir, before: &[u8]) {
    assert_eq!(fs::read(dir.store().path()).unwrap(), before);
}

#[test]
fn versioned_reads_project_legacy_history_without_migration_or_writes() {
    let dir = TestDir::new();
    assert!(dir.store().list_capabilities_v1().unwrap().is_empty());
    assert!(dir.store().preview_composition(&fixture_request()).is_err());
    assert!(!dir.0.exists());
    let saved = source(&dir);
    let one = fs::read(dir.store().path()).unwrap();
    assert!(dir.store().list_capabilities_v1().unwrap().is_empty());
    assert_unchanged(&dir, &one);
    let first = legacy(&dir, &saved);
    let reviewed = dir
        .store()
        .review_capability(&first.id, &first.latest_revision_id)
        .unwrap()
        .capability;
    let changed = dir
        .store()
        .revise_capability(
            &first.id,
            &first.latest_revision_id,
            "Edited",
            "Local edit\r\n",
        )
        .unwrap()
        .capability;
    let before = fs::read(dir.store().path()).unwrap();
    let old = dir
        .store()
        .open_capability_v1(&first.id, Some(&first.revision.id))
        .unwrap();
    let mut expected = CapabilityDetail::from(reviewed);
    expected.latest_revision_id = changed.latest_revision_id.clone();
    expected.history = CapabilityDetail::from(changed.clone()).history;
    assert_eq!(old, expected);
    assert_eq!(
        dir.store().open_capability_v1(&first.id, None).unwrap(),
        CapabilityDetail::from(changed)
    );
    assert_eq!(
        dir.store().list_capabilities_v1().unwrap()[0].revision_count,
        2
    );
    assert_eq!(
        dir.db()
            .pragma_query_value::<i64, _>(None, "user_version", |r| r.get(0))
            .unwrap(),
        2
    );
    assert_unchanged(&dir, &before);
}

#[test]
fn preview_resolves_actual_saved_inputs_and_current_heads_without_writing() {
    let dir = TestDir::new();
    let saved = source(&dir);
    let original = legacy(&dir, &saved);
    let mut request = fixture_request();
    request.targets[0] = Target::Append {
        capability_id: original.id.clone(),
        expected_revision_id: original.revision.id.clone(),
    };
    let before = fs::read(dir.store().path()).unwrap();
    let preview = dir.store().preview_composition(&request).unwrap();
    assert!(preview.preview.saveable);
    assert_eq!(
        preview.preview.core.composition_id,
        "composition:0b2728072f1c198fe6594bee3838e2a30856d4c1d0bdd3e3724fa72c39e3435f"
    );
    assert_eq!(
        preview.preview.applied_outputs[0].capability_id,
        original.id
    );
    assert_eq!(
        preview.preview.core.outputs[0].content,
        "\u{feff}# Rules\r\nKeep originals.\r\n"
    );
    assert_eq!(preview.preview.authority, Authority::None);
    assert_unchanged(&dir, &before);
    dir.store()
        .review_capability(&original.id, &original.revision.id)
        .unwrap();
    assert_ne!(
        preview.expected_state_id,
        dir.store()
            .preview_composition(&request)
            .unwrap()
            .expected_state_id
    );
    dir.store()
        .revise_capability(&original.id, &original.revision.id, "Changed", "Changed\n")
        .unwrap();
    assert!(matches!(
        dir.store().preview_composition(&request),
        Err(StoreError::CapabilityConflict)
    ));
    request.targets[0] = Target::New {};
    if let InputReference::Source { sha256, .. } = &mut request.draft.inputs[0] {
        *sha256 = "0".repeat(64);
    }
    assert!(matches!(
        dir.store().preview_composition(&request),
        Err(StoreError::Corrupt)
    ));
}

#[test]
fn schema3_keeps_birth_and_revision_derivation_separate_and_blocks_legacy_writes() {
    let dir = TestDir::new();
    let saved = source(&dir);
    let original = legacy(&dir, &saved);
    let db = dir.db();
    schema3(&db);
    let mut request = fixture_request();
    request.targets[0] = Target::Append {
        capability_id: original.id.clone(),
        expected_revision_id: original.revision.id.clone(),
    };
    let preview = retain_outputs(
        &db,
        &request,
        &resolved_source(&saved),
        &[TargetHead {
            capability_id: original.id.clone(),
            revision_id: original.revision.id.clone(),
        }],
        &[0, 1],
    );
    let applied = &preview.applied_outputs[0];
    let ordinary_id = capability::revision_id(
        &original.id,
        Some(&applied.revision_id),
        "Later edit",
        "Unreviewed successor\n",
    );
    db.execute(
        "INSERT INTO revisions VALUES (?1,?2,?3,'Later edit',?4,?5,2)",
        params![
            ordinary_id,
            original.id,
            applied.revision_id,
            b"Unreviewed successor\n",
            byte_digest(b"Unreviewed successor\n")
        ],
    )
    .unwrap();
    db.execute(
        "UPDATE capabilities SET latest_revision_id=?1 WHERE capability_id=?2",
        params![ordinary_id, original.id],
    )
    .unwrap();
    let before = fs::read(dir.store().path()).unwrap();
    let detail = dir.store().open_capability_v1(&original.id, None).unwrap();
    assert!(matches!(detail.origin, Origin::Source { .. }));
    assert_eq!(detail.history.len(), 3);
    assert!(matches!(
        detail.history[1].provenance,
        RevisionProvenance::Composition { .. }
    ));
    assert!(matches!(
        detail.revision.provenance,
        RevisionProvenance::Ordinary {}
    ));
    assert!(detail.revision.review.is_none());
    let new = dir
        .store()
        .open_capability_v1(&preview.applied_outputs[1].capability_id, None)
        .unwrap();
    assert!(matches!(
        new.origin,
        Origin::Composition {
            output_index: 1,
            ..
        }
    ));
    assert_eq!(new.authority, Authority::None);
    assert_eq!(dir.store().list_capabilities_v1().unwrap().len(), 2);
    assert!(matches!(
        dir.store().save(&saved),
        Err(StoreError::UnsupportedSchema)
    ));
    assert!(matches!(
        dir.store().list_capabilities(),
        Err(StoreError::UnsupportedSchema)
    ));
    assert!(matches!(
        dir.store()
            .create_capability(&saved.source.id, &saved.fragments[0].id, "Other"),
        Err(StoreError::UnsupportedSchema)
    ));
    assert!(matches!(
        dir.store()
            .revise_capability(&original.id, &ordinary_id, "No", "No"),
        Err(StoreError::UnsupportedSchema)
    ));
    assert!(matches!(
        dir.store().review_capability(&original.id, &ordinary_id),
        Err(StoreError::UnsupportedSchema)
    ));
    assert!(matches!(
        dir.store().open_capability(&original.id, None),
        Err(StoreError::UnsupportedSchema)
    ));
    assert!(matches!(
        dir.store().export_backup(),
        Err(StoreError::UnsupportedSchema)
    ));
    assert!(matches!(
        dir.store().data(),
        Err(StoreError::UnsupportedSchema)
    ));
    assert_unchanged(&dir, &before);
}

#[test]
fn surviving_outputs_do_not_recreate_deleted_destinations() {
    let dir = TestDir::new();
    let saved = source(&dir);
    let db = dir.db();
    schema3(&db);
    let mut request = fixture_request();
    let ghost_cap = format!("capability:{}", "a".repeat(64));
    let ghost_rev = format!("revision:{}", "b".repeat(64));
    request.targets[1] = Target::Append {
        capability_id: ghost_cap.clone(),
        expected_revision_id: ghost_rev.clone(),
    };
    let preview = retain_outputs(
        &db,
        &request,
        &resolved_source(&saved),
        &[TargetHead {
            capability_id: ghost_cap,
            revision_id: ghost_rev,
        }],
        &[0],
    );
    let before = fs::read(dir.store().path()).unwrap();
    let rows = dir.store().list_capabilities_v1().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, preview.applied_outputs[0].capability_id);
    assert!(
        dir.store()
            .open_capability_v1(&preview.applied_outputs[1].capability_id, None)
            .is_err()
    );
    // Reusing an occupied deterministic new target must request an explicit append.
    let mut fresh = fixture_request();
    fresh.targets[1] = Target::New {};
    assert!(matches!(
        dir.store().preview_composition(&fresh),
        Err(StoreError::CapabilityConflict)
    ));
    assert_unchanged(&dir, &before);
    db.execute("DELETE FROM snapshots", []).unwrap();
    assert!(dir.store().list_capabilities_v1().is_err());
}

#[test]
fn malformed_canonical_records_and_provenance_fail_closed_without_repair() {
    let mutations = [
        "UPDATE composition_applications SET acknowledged_by='administrator'",
        "UPDATE composition_applications SET targets_json=CAST('[ {\"kind\":\"new\"},{\"kind\":\"new\"}]' AS BLOB)",
        "UPDATE compositions SET draft_json=CAST(CAST(draft_json AS TEXT)||' ' AS BLOB)",
        "UPDATE compositions SET transformation_version='99.0.0'",
        "UPDATE compositions SET created_at_ms=-1",
        "DELETE FROM revision_derivations",
        "UPDATE revision_derivations SET output_index=15 WHERE output_index=0",
        "UPDATE revisions SET content=CAST('forged' AS BLOB)",
        "UPDATE revisions SET parent_revision_id=revision_id",
        "UPDATE derived_capabilities SET latest_revision_id='revision:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff'",
        "DELETE FROM derived_capabilities",
        "INSERT INTO reviews VALUES ('revision:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff','local_operator',1)",
        "UPDATE snapshots SET content=zeroblob(262145)",
        "UPDATE composition_applications SET targets_json=zeroblob(16385)",
    ];
    for mutation in mutations {
        let dir = TestDir::new();
        let saved = source(&dir);
        let db = dir.db();
        schema3(&db);
        retain_outputs(
            &db,
            &fixture_request(),
            &resolved_source(&saved),
            &[],
            &[0, 1],
        );
        db.execute_batch(mutation).unwrap();
        let before = fs::read(dir.store().path()).unwrap();
        assert!(
            dir.store().list_capabilities_v1().is_err(),
            "accepted {mutation}"
        );
        assert_unchanged(&dir, &before);
    }
}

#[test]
fn recomputed_output_hash_and_revision_id_cannot_hide_materialization_mismatch() {
    let dir = TestDir::new();
    let saved = source(&dir);
    let db = dir.db();
    schema3(&db);
    let preview = retain_outputs(&db, &fixture_request(), &resolved_source(&saved), &[], &[0]);
    let output = &preview.applied_outputs[0];
    let fake_id = capability::revision_id(&output.capability_id, None, "Rules", "Rewritten bytes");
    db.execute(
        "UPDATE revisions SET revision_id=?1,content=?2,sha256=?3",
        params![fake_id, b"Rewritten bytes", byte_digest(b"Rewritten bytes")],
    )
    .unwrap();
    db.execute(
        "UPDATE derived_capabilities SET latest_revision_id=?1",
        [&fake_id],
    )
    .unwrap();
    db.execute("UPDATE revision_derivations SET revision_id=?1", [&fake_id])
        .unwrap();
    assert!(matches!(
        dir.store().list_capabilities_v1(),
        Err(StoreError::Corrupt)
    ));
}

#[test]
fn disconnected_history_and_owner_aliases_are_rejected() {
    for alias in [false, true] {
        let dir = TestDir::new();
        let saved = source(&dir);
        let db = dir.db();
        schema3(&db);
        let preview = retain_outputs(&db, &fixture_request(), &resolved_source(&saved), &[], &[0]);
        let cap = &preview.applied_outputs[0].capability_id;
        if alias {
            db.execute(
                "INSERT INTO capabilities VALUES (?1,?2,?3,?4)",
                params![
                    cap,
                    saved.source.id,
                    saved.fragments[0].id,
                    preview.applied_outputs[0].revision_id
                ],
            )
            .unwrap();
        } else {
            let id = capability::revision_id(cap, None, "Orphan", "Disconnected");
            db.execute(
                "INSERT INTO revisions VALUES (?1,?2,NULL,'Orphan',?3,?4,1)",
                params![id, cap, b"Disconnected", byte_digest(b"Disconnected")],
            )
            .unwrap();
        }
        assert!(matches!(
            dir.store().list_capabilities_v1(),
            Err(StoreError::Corrupt)
        ));
    }
}

pub(super) fn split_request(
    detail: &CapabilityDetail,
    index: usize,
) -> (Request, Vec<ResolvedInput>) {
    let revision = &detail.revision;
    let draft = Draft {
        schema_version: rangoon_compose::DRAFT_SCHEMA.into(),
        operation: Operation::Split,
        inputs: vec![InputReference::Revision {
            capability_id: detail.id.clone(),
            revision_id: revision.id.clone(),
            sha256: revision.sha256.clone(),
        }],
        outputs: vec![
            OutputRecipe {
                title: format!("Stage {index}"),
                pieces: vec![Piece::Copy {
                    range: InputRange {
                        input_index: 0,
                        start_byte: 0,
                        end_byte: revision.content.len() as u64,
                    },
                }],
            },
            OutputRecipe {
                title: "Additional context".into(),
                pieces: vec![Piece::Authored {
                    content: "Context".into(),
                    reason: "Explicit supplemental context".into(),
                }],
            },
        ],
        exclusions: vec![],
        duplications: vec![],
        conflicts: vec![],
    };
    (
        Request {
            schema_version: application::REQUEST_SCHEMA.into(),
            draft,
            targets: vec![Target::New {}, Target::New {}],
        },
        vec![ResolvedInput::Revision {
            capability_id: detail.id.clone(),
            revision_id: revision.id.clone(),
            sha256: revision.sha256.clone(),
            content: revision.content.clone(),
        }],
    )
}

#[test]
fn provenance_depth_bound_survives_cached_shorter_paths() {
    let dir = TestDir::new();
    let saved = source(&dir);
    let original = legacy(&dir, &saved);
    let db = dir.db();
    schema3(&db);
    let mut detail = CapabilityDetail::from(original);
    for index in 0..64 {
        let (request, inputs) = split_request(&detail, index);
        let preview = retain_outputs(&db, &request, &inputs, &[], &[0]);
        detail = Records::load(&db)
            .unwrap()
            .detail(&preview.applied_outputs[0].capability_id, None)
            .unwrap();
    }
    // Each retained composition adds a recipe and a revision dependency edge.
    // Exact depth 128 is accepted; the next pair must fail regardless of ID order.
    let (request, inputs) = split_request(&detail, 64);
    retain_outputs(&db, &request, &inputs, &[], &[0]);
    assert!(matches!(Records::load(&db), Err(StoreError::Corrupt)));
}

#[test]
fn derived_skill_append_and_ordinary_edit_preserve_original_birth() {
    let dir = TestDir::new();
    let saved = source(&dir);
    let db = dir.db();
    schema3(&db);
    let initial = retain_outputs(&db, &fixture_request(), &resolved_source(&saved), &[], &[0]);
    let cap = &initial.applied_outputs[0].capability_id;
    let first = Records::load(&db).unwrap().detail(cap, None).unwrap();
    let (mut request, inputs) = split_request(&first, 1);
    request.targets[0] = Target::Append {
        capability_id: cap.clone(),
        expected_revision_id: first.revision.id.clone(),
    };
    let composed = retain_outputs(
        &db,
        &request,
        &inputs,
        &[TargetHead {
            capability_id: cap.clone(),
            revision_id: first.revision.id.clone(),
        }],
        &[0],
    );
    let parent = &composed.applied_outputs[0].revision_id;
    let ordinary = capability::revision_id(cap, Some(parent), "Edited", "Manual successor");
    db.execute(
        "INSERT INTO revisions VALUES (?1,?2,?3,'Edited',?4,?5,3)",
        params![
            ordinary,
            cap,
            parent,
            b"Manual successor",
            byte_digest(b"Manual successor")
        ],
    )
    .unwrap();
    db.execute(
        "UPDATE derived_capabilities SET latest_revision_id=?1 WHERE capability_id=?2",
        params![ordinary, cap],
    )
    .unwrap();
    let detail = dir.store().open_capability_v1(cap, None).unwrap();
    assert_eq!(detail.origin, first.origin);
    assert_eq!(detail.history.len(), 3);
    assert!(
        matches!(&detail.history[1].provenance, RevisionProvenance::Composition {composition_id, ..} if composition_id == &composed.core.composition_id)
    );
    assert!(matches!(
        detail.revision.provenance,
        RevisionProvenance::Ordinary {}
    ));
    assert_eq!(detail.revision.parent_revision_id.as_ref(), Some(parent));
    assert!(
        detail
            .history
            .iter()
            .all(|revision| revision.review.is_none())
    );
}

#[test]
fn cross_skill_dependencies_follow_revisions_instead_of_rejecting_capability_cycles() {
    let dir = TestDir::new();
    let saved = source(&dir);
    let a = legacy(&dir, &saved);
    let b = dir
        .store()
        .create_capability(&saved.source.id, &saved.fragments[0].id, "Second")
        .unwrap()
        .capability;
    let db = dir.db();
    schema3(&db);
    let (mut first_request, first_inputs) = split_request(&CapabilityDetail::from(b.clone()), 1);
    first_request.targets[0] = Target::Append {
        capability_id: a.id.clone(),
        expected_revision_id: a.revision.id.clone(),
    };
    retain_outputs(
        &db,
        &first_request,
        &first_inputs,
        &[TargetHead {
            capability_id: a.id.clone(),
            revision_id: a.revision.id.clone(),
        }],
        &[0],
    );
    let a_new = Records::load(&db).unwrap().detail(&a.id, None).unwrap();
    let (mut second_request, second_inputs) = split_request(&a_new, 2);
    second_request.targets[0] = Target::Append {
        capability_id: b.id.clone(),
        expected_revision_id: b.revision.id.clone(),
    };
    retain_outputs(
        &db,
        &second_request,
        &second_inputs,
        &[TargetHead {
            capability_id: b.id.clone(),
            revision_id: b.revision.id.clone(),
        }],
        &[0],
    );
    let records = Records::load(&db).unwrap();
    assert_eq!(records.detail(&a.id, None).unwrap().history.len(), 2);
    assert_eq!(records.detail(&b.id, None).unwrap().history.len(), 2);
    // Both skills depend on each other at the capability level, but their
    // immutable revision dependencies are ordered and acyclic.
    assert_eq!(dir.store().list_capabilities_v1().unwrap().len(), 2);
}
