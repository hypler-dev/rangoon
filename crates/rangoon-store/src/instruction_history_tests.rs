use super::*;
use crate::composition_records_tests::TestDir;
use rangoon_compose::{Draft, InputRange, Operation, OutputRecipe, Piece, ResolvedInput};

fn target(present: bool) -> InstructionTarget {
    InstructionTarget {
        workspace_identity: "a".repeat(64),
        target_identity: "b".repeat(64),
        parent_identity: "c".repeat(64),
        file_name: "AGENTS.md".into(),
        file_identity: present.then(|| "d".repeat(64)),
    }
}
fn change(before: Option<&[u8]>, after: Option<&[u8]>) -> InstructionChange {
    InstructionChange::prepare(
        target(before.is_some()),
        before,
        after,
        vec![],
        None,
        None,
        10,
    )
    .unwrap()
}
#[test]
fn exact_bytes_absent_empty_binary_and_redacted_debug() {
    let bytes = b"\xef\xbb\xbf# Plan\r\n\xc3\xa9\r\n\xff";
    let c = change(Some(bytes), Some(b""));
    assert_eq!(c.before(), Some(bytes.as_slice()));
    assert_eq!(c.after(), Some(b"".as_slice()));
    assert_eq!(c.before_sha256(), Some(byte_digest(bytes).as_str()));
    assert_eq!(c.after_sha256(), Some(byte_digest(b"").as_str()));
    let absent = change(None, Some(b""));
    assert_ne!(absent.id(), c.id());
    assert_ne!(change(Some(b""), None).id(), absent.id());
    assert_eq!(InstructionChange::decode(&c.bytes().unwrap()).unwrap(), c);
    assert_eq!(format!("{c:?}"), "InstructionChange(<redacted>)");
    assert!(c.analyze_after().is_ok()); // empty supported Markdown remains exact
    assert!(change(None, Some(bytes)).analyze_after().is_err());
}
#[test]
fn changes_reject_noop_oversize_bad_identity_and_path() {
    assert!(InstructionChange::prepare(target(false), None, None, vec![], None, None, 0).is_err());
    assert!(
        InstructionChange::prepare(target(true), Some(b"x"), Some(b"x"), vec![], None, None, 0)
            .is_err()
    );
    assert!(
        InstructionChange::prepare(
            target(false),
            None,
            Some(&vec![0; MAX_INSTRUCTION_BYTES + 1]),
            vec![],
            None,
            None,
            0
        )
        .is_err()
    );
    assert!(
        InstructionChange::prepare(target(true), None, Some(b"x"), vec![], None, None, 0).is_err()
    );
    for name in ["../AGENTS.md", "a/b", "a\\b", "C:x", "", ".", "..", "x\0md"] {
        let mut t = target(false);
        t.file_name = name.into();
        assert!(InstructionChange::prepare(t, None, Some(b"x"), vec![], None, None, 0).is_err());
    }
    let mut t = target(false);
    t.parent_identity = "D".repeat(64);
    assert!(InstructionChange::prepare(t, None, Some(b"x"), vec![], None, None, 0).is_err());
}
#[test]
fn decoder_rejects_changed_digest_unknown_fields_version_and_noncanonical() {
    let bytes = change(None, Some(b"secret")).bytes().unwrap();
    for field in ["schemaVersion", "afterSha256", "parentChangeId", "inputs"] {
        let mut wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        wire[field] = "invalid".into();
        assert!(InstructionChange::decode(&serde_json::to_vec(&wire).unwrap()).is_err());
    }
    let mut wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    wire["authority"] = "allow".into();
    assert!(InstructionChange::decode(&serde_json::to_vec(&wire).unwrap()).is_err());
    let mut extra = bytes.to_vec();
    extra.push(b' ');
    assert!(InstructionChange::decode(&extra).is_err());
    assert!(InstructionChange::decode(&vec![b' '; MAX_RECORD_BYTES + 1]).is_err());
}
#[test]
fn events_require_explicit_attempt_and_exact_readback() {
    let c = change(None, Some(b"new"));
    let mut records = Records::empty();
    records.version = 5;
    records.history.changes.insert(c.id().into(), c.clone());
    let attempted =
        InstructionEvent::prepare(c.id().into(), None, InstructionPhase::Attempted, None, 11)
            .unwrap();
    let verified = InstructionEvent::prepare(
        c.id().into(),
        Some(attempted.id().into()),
        InstructionPhase::Verified,
        Some((Some("e".repeat(64)), Some(b"new"))),
        12,
    )
    .unwrap();
    records
        .history
        .events
        .insert(attempted.id().into(), attempted.clone());
    records
        .history
        .events
        .insert(verified.id().into(), verified.clone());
    assert!(records.history.validate(&records).is_ok());
    let wrong = InstructionEvent::prepare(
        c.id().into(),
        Some(attempted.id().into()),
        InstructionPhase::Verified,
        Some((Some("e".repeat(64)), Some(b"wrong"))),
        12,
    )
    .unwrap();
    records.history.events.remove(verified.id());
    records.history.events.insert(wrong.id().into(), wrong);
    assert!(records.history.validate(&records).is_err());
    assert!(
        InstructionEvent::prepare(c.id().into(), None, InstructionPhase::Verified, None, 12)
            .is_err()
    );
    assert!(
        InstructionEvent::prepare(
            c.id().into(),
            None,
            InstructionPhase::Attempted,
            Some((None, None)),
            12
        )
        .is_err()
    );
}
#[test]
fn archive_parent_requires_exact_content_scope_and_retention() {
    let first = change(None, Some(b"one"));
    let mut records = Records::empty();
    records.version = 5;
    records
        .history
        .changes
        .insert(first.id().into(), first.clone());
    let child = InstructionChange::prepare(
        target(true),
        Some(b"one"),
        Some(b"two"),
        vec![],
        None,
        Some(first.id().into()),
        11,
    )
    .unwrap();
    records
        .history
        .changes
        .insert(child.id().into(), child.clone());
    assert!(records.history.validate(&records).is_ok());
    assert_eq!(deletion(&records, first.id()), Err(StoreError::RecordInUse));
    assert!(deletion(&records, child.id()).is_ok());
    let bad = InstructionChange::prepare(
        target(true),
        Some(b"other"),
        Some(b"three"),
        vec![],
        None,
        Some(first.id().into()),
        12,
    )
    .unwrap();
    records.history.changes.insert(bad.id().into(), bad);
    assert!(records.history.validate(&records).is_err());
}
#[test]
fn parser_ranges_split_merge_and_reassembly_preserve_original_bytes() {
    let text = "\u{feff}# First\r\nUse careful review.\r\n## Second\r\nKeep provenance.\r\n";
    let c = change(None, Some(text.as_bytes()));
    let report = c.analyze_after().unwrap();
    assert_eq!(report.source.content.as_bytes(), text.as_bytes());
    assert!(report.fragments.len() >= 2);
    let inputs = vec![ResolvedInput::Source {
        source_id: report.source.id.clone(),
        sha256: report.source.sha256.clone(),
        content: text.into(),
    }];
    let reference = InputReference::Source {
        source_id: report.source.id.clone(),
        sha256: report.source.sha256.clone(),
    };
    let pieces: Vec<_> = report
        .fragments
        .iter()
        .map(|f| Piece::Copy {
            range: InputRange {
                input_index: 0,
                start_byte: f.span.start_byte,
                end_byte: f.span.end_byte,
            },
        })
        .collect();
    let mut draft = Draft {
        schema_version: rangoon_compose::DRAFT_SCHEMA.into(),
        operation: Operation::Decompose,
        inputs: vec![reference],
        outputs: vec![OutputRecipe {
            title: "Reassembled".into(),
            pieces,
        }],
        exclusions: vec![],
        duplications: vec![],
        conflicts: vec![],
    };
    let preview = rangoon_compose::preview(&draft, &inputs).unwrap();
    assert_eq!(preview.outputs[0].content.as_bytes(), text.as_bytes());
    draft.operation = Operation::Split;
    let cap = format!("capability:{}", "1".repeat(64));
    let rev = format!("revision:{}", "2".repeat(64));
    draft.inputs = vec![InputReference::Revision {
        capability_id: cap.clone(),
        revision_id: rev.clone(),
        sha256: report.source.sha256.clone(),
    }];
    let inputs = vec![ResolvedInput::Revision {
        capability_id: cap,
        revision_id: rev,
        sha256: report.source.sha256.clone(),
        content: text.into(),
    }];
    draft.outputs = report
        .fragments
        .iter()
        .enumerate()
        .map(|(i, f)| OutputRecipe {
            title: format!("Part {i}"),
            pieces: vec![Piece::Copy {
                range: InputRange {
                    input_index: 0,
                    start_byte: f.span.start_byte,
                    end_byte: f.span.end_byte,
                },
            }],
        })
        .collect();
    let split = rangoon_compose::preview(&draft, &inputs).unwrap();
    let joined: String = split.outputs.iter().map(|o| o.content.as_str()).collect();
    assert_eq!(joined, text);
    let parts: Vec<_> = split
        .outputs
        .iter()
        .enumerate()
        .map(|(i, o)| analyze(&format!("part-{i}.md"), o.content.as_bytes()).unwrap())
        .collect();
    let merged = Draft {
        schema_version: rangoon_compose::DRAFT_SCHEMA.into(),
        operation: Operation::Merge,
        inputs: parts
            .iter()
            .enumerate()
            .map(|(i, p)| InputReference::Revision {
                capability_id: format!("capability:{}", byte_digest(&[i as u8])),
                revision_id: format!("revision:{}", byte_digest(&[i as u8])),
                sha256: p.source.sha256.clone(),
            })
            .collect(),
        outputs: vec![OutputRecipe {
            title: "Merged".into(),
            pieces: parts
                .iter()
                .enumerate()
                .map(|(i, p)| Piece::Copy {
                    range: InputRange {
                        input_index: i as u32,
                        start_byte: 0,
                        end_byte: p.source.byte_length,
                    },
                })
                .collect(),
        }],
        exclusions: vec![],
        duplications: vec![],
        conflicts: vec![],
    };
    let resolved: Vec<_> = parts
        .iter()
        .enumerate()
        .map(|(i, p)| ResolvedInput::Revision {
            capability_id: format!("capability:{}", byte_digest(&[i as u8])),
            revision_id: format!("revision:{}", byte_digest(&[i as u8])),
            sha256: p.source.sha256.clone(),
            content: p.source.content.clone(),
        })
        .collect();
    assert_eq!(
        rangoon_compose::preview(&merged, &resolved)
            .unwrap()
            .outputs[0]
            .content,
        text
    );

    let mut invalid = draft.clone();
    if let Piece::Copy { range } = &mut invalid.outputs[0].pieces[0] {
        range.start_byte = 1;
    }
    assert!(rangoon_compose::preview(&invalid, &inputs).is_err());
}
#[test]
fn plaintext_workspaces_refuse_all_history_access_without_creating_files() {
    let root = TestDir::new();
    let store = root.store();
    let c = change(None, Some(b"new"));
    assert_eq!(
        store.save_instruction_change(&c, &format!("workspace:{}", "a".repeat(64))),
        Err(StoreError::EncryptionUnavailable)
    );
    assert!(matches!(
        store.instruction_changes(),
        Err(StoreError::EncryptionUnavailable)
    ));
    assert!(matches!(
        store.snapshot_encrypted_recovery(),
        Err(StoreError::EncryptionUnavailable)
    ));
    assert!(!root.0.exists());
}
#[cfg(feature = "encrypted-sqlite")]
fn keyed(root: &TestDir, byte: u8) -> Workspace {
    Workspace::encrypted(root.0.clone(), WorkspaceKey::from_bytes([byte; 32])).unwrap()
}
#[cfg(feature = "encrypted-sqlite")]
fn seed(root: &TestDir, byte: u8) -> Workspace {
    let store = keyed(root, byte);
    store.initialize_empty_encrypted().unwrap();
    store
}
#[test]
#[cfg(feature = "encrypted-sqlite")]
fn encrypted_persistence_restore_eligibility_drift_and_reopen() {
    let root = TestDir::new();
    let store = seed(&root, 0x31);
    let c = change(Some(b"original\r\n"), Some(b"changed"));
    let state = store.workflow_data().unwrap().state_id;
    let archived = store.save_instruction_change(&c, &state).unwrap();
    assert_eq!(
        store.save_instruction_change(&c, &state),
        Err(StoreError::WorkspaceChanged)
    );
    let attempted =
        InstructionEvent::prepare(c.id().into(), None, InstructionPhase::Attempted, None, 11)
            .unwrap();
    let pending = store
        .append_instruction_event(&attempted, &archived)
        .unwrap();
    assert!(
        store
            .inspect_instruction_restore(c.id(), &target(true), Some(b"changed"))
            .is_err()
    );
    let verified = InstructionEvent::prepare(
        c.id().into(),
        Some(attempted.id().into()),
        InstructionPhase::Verified,
        Some((Some("e".repeat(64)), Some(b"changed"))),
        12,
    )
    .unwrap();
    let final_state = store.append_instruction_event(&verified, &pending).unwrap();
    let mut current = target(true);
    current.file_identity = Some("e".repeat(64));
    assert_eq!(
        store
            .inspect_instruction_restore(c.id(), &current, Some(b"changed"))
            .unwrap(),
        c
    );
    assert!(matches!(
        store.inspect_instruction_restore(c.id(), &current, Some(b"drift")),
        Err(StoreError::WorkspaceChanged)
    ));
    current.parent_identity = "f".repeat(64);
    assert!(matches!(
        store.inspect_instruction_restore(c.id(), &current, Some(b"changed")),
        Err(StoreError::WorkspaceChanged)
    ));
    drop(store);
    let reopened = keyed(&root, 0x31);
    assert_eq!(reopened.workflow_data().unwrap().state_id, final_state);
    assert_eq!(reopened.instruction_changes().unwrap(), vec![c]);
    assert_eq!(
        reopened.workflow_data().unwrap().records.instruction_events,
        2
    );
    let disk = fs::read(reopened.path()).unwrap();
    assert!(!disk.windows(8).any(|w| w == b"original"));
    assert!(!disk.starts_with(b"SQLite format 3"));
}
#[test]
#[cfg(feature = "encrypted-sqlite")]
fn protected_history_only_leaves_as_encrypted_complete_recovery_copy() {
    let a = TestDir::new();
    let b = TestDir::new();
    let c = TestDir::new();
    let store = seed(&a, 0x32);
    let change = change(None, Some(b"private instruction history"));
    store
        .save_instruction_change(&change, &store.workflow_data().unwrap().state_id)
        .unwrap();
    assert_eq!(
        store.export_workflow_backup(),
        Err(StoreError::UnsupportedSchema)
    );
    let captured = store.snapshot_encrypted_recovery().unwrap();
    let internal = workflow_backup::encode(&captured.records).unwrap();
    assert!(internal.starts_with(b"RANGOON-BACKUP-V4\n"));
    assert!(matches!(
        WorkspaceBackup::decode(&internal),
        Err(StoreError::BackupInvalid)
    ));
    let recovery = keyed(&b, 0x33);
    let ciphertext = recovery.create_encrypted_recovery_copy(&captured).unwrap();
    assert!(!ciphertext.windows(7).any(|w| w == b"private"));
    let readback = recovery
        .inspect_encrypted_recovery_copy(ciphertext.len() as u64)
        .unwrap();
    assert!(captured.same_recovery_records(&readback).unwrap());
    let fresh = seed(&c, 0x34);
    let plan = fresh.prepare_empty_workflow_restore(&readback, 5).unwrap();
    fresh
        .restore_empty_workflow_backup(&readback, &plan)
        .unwrap();
    drop(fresh);
    let reopened = keyed(&c, 0x34);
    assert_eq!(reopened.instruction_changes().unwrap(), vec![change]);
    assert!(
        captured
            .same_recovery_records(&reopened.snapshot_encrypted_recovery().unwrap())
            .unwrap()
    );
    assert!(matches!(
        keyed(&b, 0x77).inspect_encrypted_recovery_copy(ciphertext.len() as u64),
        Err(StoreError::EncryptedStoreInvalid)
    ));
    let before = fs::read(recovery.path()).unwrap();
    assert_eq!(before, ciphertext);
    let mut altered = before.clone();
    altered[4200] ^= 1;
    fs::write(recovery.path(), &altered).unwrap();
    assert!(
        recovery
            .inspect_encrypted_recovery_copy(altered.len() as u64)
            .is_err()
    );
    assert_eq!(fs::read(recovery.path()).unwrap(), altered);
}

#[test]
fn unknown_and_failed_events_never_satisfy_restore_evidence_and_forks_refuse() {
    let c = change(None, Some(b"new"));
    let attempt =
        InstructionEvent::prepare(c.id().into(), None, InstructionPhase::Attempted, None, 11)
            .unwrap();
    let unknown = InstructionEvent::prepare(
        c.id().into(),
        Some(attempt.id().into()),
        InstructionPhase::Unknown,
        None,
        12,
    )
    .unwrap();
    let mut records = Records::empty();
    records.version = 5;
    records.history.changes.insert(c.id().into(), c.clone());
    records
        .history
        .events
        .insert(attempt.id().into(), attempt.clone());
    records.history.events.insert(unknown.id().into(), unknown);
    assert!(records.history.validate(&records).is_ok());
    assert!(
        !records
            .history
            .events
            .values()
            .any(|e| e.phase() == InstructionPhase::Verified)
    );
    let fail = InstructionEvent::prepare(
        c.id().into(),
        Some(attempt.id().into()),
        InstructionPhase::Failed,
        None,
        13,
    )
    .unwrap();
    records.history.events.insert(fail.id().into(), fail);
    assert!(records.history.validate(&records).is_err());
}
#[test]
fn malformed_bytes_and_rechecksummed_internal_archives_do_not_panic_or_validate() {
    let c = change(None, Some(b"synthetic"));
    let bytes = c.bytes().unwrap();
    for end in 0..bytes.len() {
        assert!(InstructionChange::decode(&bytes[..end]).is_err());
    }
    let mut records = Records::empty();
    records.version = 5;
    records.history.changes.insert(c.id().into(), c);
    let archive = workflow_backup::encode(&records).unwrap();
    for index in (0..archive.len() - 64).step_by(71) {
        let mut altered = archive.clone();
        altered[index] ^= 0x80;
        let end = altered.len() - 64;
        let digest = byte_digest(&altered[..end]);
        altered[end..].copy_from_slice(digest.as_bytes());
        assert!(
            instruction_history_backup::decode(&altered).is_err(),
            "accepted altered byte {index}"
        );
    }
    let mut trailing = archive.clone();
    trailing.push(0);
    assert!(instruction_history_backup::decode(&trailing).is_err());
    assert!(instruction_history_backup::decode(&vec![0; MAX_BACKUP_BYTES + 1]).is_err());
}
#[test]
fn history_count_and_storage_budget_fail_without_eviction() {
    let mut records = Records::empty();
    records.version = 5;
    for i in 0..MAX_INSTRUCTION_CHANGES {
        let c = InstructionChange::prepare(
            target(false),
            None,
            Some(format!("change-{i}").as_bytes()),
            vec![],
            None,
            None,
            i as i64,
        )
        .unwrap();
        records.history.changes.insert(c.id().into(), c);
    }
    assert!(records.history.validate(&records).is_ok());
    let extra = change(None, Some(b"extra"));
    records.history.changes.insert(extra.id().into(), extra);
    assert_eq!(records.history.validate(&records), Err(StoreError::Full));
    assert_eq!(records.history.changes.len(), MAX_INSTRUCTION_CHANGES + 1);
    records.history.changes.clear();
    for i in 0..32 {
        let mut data = vec![255; MAX_INSTRUCTION_BYTES];
        data[0] = i;
        let c = change(None, Some(&data));
        records.history.changes.insert(c.id().into(), c);
    }
    assert_eq!(records.history.validate(&records), Err(StoreError::Full));
}
#[test]
#[cfg(feature = "encrypted-sqlite")]
fn nonempty_restore_merges_history_events_and_preserves_later_authoring() {
    use rangoon_workflow::records::{SaveIntent, prepare_revision, workflow_id_from_nonce};
    let a = TestDir::new();
    let b = TestDir::new();
    let sa = seed(&a, 0x41);
    let sb = seed(&b, 0x42);
    let ca = change(None, Some(b"one"));
    let cb = change(None, Some(b"two"));
    sa.save_instruction_change(&ca, &sa.workflow_data().unwrap().state_id)
        .unwrap();
    sb.save_instruction_change(&cb, &sb.workflow_data().unwrap().state_id)
        .unwrap();
    let incoming = sa.snapshot_encrypted_recovery().unwrap();
    let plan = sb.prepare_workflow_restore(&incoming).unwrap();
    sb.restore_workflow_backup(&incoming, &plan).unwrap();
    assert_eq!(sb.instruction_changes().unwrap().len(), 2);
    let report = analyze("SKILL.md", b"# Skill\nKeep exact history.\n").unwrap();
    sb.save_v1(&report).unwrap();
    let cap = sb
        .create_capability_v1(&report.source.id, &report.fragments[0].id, "Saved")
        .unwrap();
    let cap = sb
        .revise_capability_v1(
            &cap.capability.id,
            &cap.capability.latest_revision_id,
            "Revised",
            "# Revised\nNew content.\n",
        )
        .unwrap();
    sb.review_capability_v1(&cap.capability.id, &cap.capability.latest_revision_id)
        .unwrap();
    assert!(
        sb.compile_capability(
            &cap.capability.id,
            &cap.capability.latest_revision_id,
            rangoon_compile::Profile::AgentsMdV1
        )
        .is_ok()
    );
    let definition=br#"{"schemaVersion":"rangoon.workflow-definition.v1","title":"Draft","nodes":[],"controlEdges":[],"dataEdges":[]}"#;
    let workflow = prepare_revision(
        &workflow_id_from_nonce(&[6; 32]),
        None,
        SaveIntent::Draft,
        definition,
        br#"{"positions":[]}"#,
    )
    .unwrap();
    let save = sb.inspect_workflow_save(&workflow).unwrap();
    sb.save_workflow(&workflow, &save.expected_state_id)
        .unwrap();
    assert_eq!(sb.instruction_changes().unwrap().len(), 2);
    let db = sb.connect_complete_read_only(false).unwrap().unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        5
    );
}
#[test]
#[cfg(feature = "encrypted-sqlite")]
fn lineage_dependency_deletion_and_explicit_archive_removal() {
    let root = TestDir::new();
    let store = seed(&root, 0x44);
    let report = analyze("AGENTS.md", b"# Original\nKeep lineage.\n").unwrap();
    store.save_v1(&report).unwrap();
    let c = InstructionChange::prepare(
        target(false),
        None,
        Some(b"new"),
        vec![InputReference::Source {
            source_id: report.source.id.clone(),
            sha256: report.source.sha256.clone(),
        }],
        None,
        None,
        10,
    )
    .unwrap();
    store
        .save_instruction_change(&c, &store.workflow_data().unwrap().state_id)
        .unwrap();
    assert!(matches!(
        store.inspect_workflow_deletion(WorkflowRecordKind::Source, &report.source.id),
        Err(StoreError::RecordInUse)
    ));
    let retained = store.prepare_instruction_history_deletion(c.id()).unwrap();
    assert_eq!(retained.change_id(), c.id());
    assert_eq!(retained.removed_event_count(), 0);
    let other = change(None, Some(b"other"));
    store
        .save_instruction_change(&other, &store.workflow_data().unwrap().state_id)
        .unwrap();
    assert_eq!(
        store.delete_instruction_history(&retained),
        Err(StoreError::WorkspaceChanged)
    );
    let retained = store.prepare_instruction_history_deletion(c.id()).unwrap();
    store.delete_instruction_history(&retained).unwrap();
    assert_eq!(store.instruction_changes().unwrap(), vec![other]);
    let plan = store
        .inspect_workflow_deletion(WorkflowRecordKind::Source, &report.source.id)
        .unwrap();
    store.delete_workflow_record(&plan).unwrap();
}
#[test]
#[cfg(feature = "encrypted-sqlite")]
fn wrong_key_does_not_mutate_workspace_or_expose_history() {
    let root = TestDir::new();
    let store = seed(&root, 0x45);
    let c = change(None, Some(b"private"));
    store
        .save_instruction_change(&c, &store.workflow_data().unwrap().state_id)
        .unwrap();
    let before = fs::read(store.path()).unwrap();
    let wrong = keyed(&root, 0x46);
    assert!(wrong.instruction_changes().is_err());
    assert!(
        wrong
            .save_instruction_change(&c, &store.workflow_data().unwrap().state_id)
            .is_err()
    );
    assert_eq!(fs::read(store.path()).unwrap(), before);
}

#[test]
#[ignore = "launched only by the isolated history parent test"]
#[cfg(feature = "encrypted-sqlite")]
fn instruction_history_child() {
    let root = TestDir(PathBuf::from(
        std::env::var_os("RANGOON_RECOVERY_TEST_WORKSPACE").unwrap(),
    ));
    // Parent owns this directory and cleanup. TestDir's Drop is avoided in the
    // child so a successful distinct-process read cannot delete parent evidence.
    let root = std::mem::ManuallyDrop::new(root);
    let store = keyed(&root, 0x5c);
    let c = change(None, Some(b"child history"));
    match std::env::var("RANGOON_RECOVERY_TEST_DELETE_ID")
        .unwrap()
        .as_str()
    {
        "reopen" => {
            assert_eq!(store.instruction_changes().unwrap(), vec![c]);
            assert_eq!(store.workflow_data().unwrap().records.instruction_events, 1);
        }
        "change" => {
            store
                .save_instruction_change(&c, &store.workflow_data().unwrap().state_id)
                .unwrap();
        }
        "event" => {
            let attempted = InstructionEvent::prepare(
                c.id().into(),
                None,
                InstructionPhase::Attempted,
                None,
                11,
            )
            .unwrap();
            store
                .append_instruction_event(&attempted, &store.workflow_data().unwrap().state_id)
                .unwrap();
        }
        _ => panic!("unknown controlled history child case"),
    }
}
#[test]
#[cfg(feature = "encrypted-sqlite")]
fn process_restart_and_killed_transactions_retain_exact_prior_state() {
    use crate::composition_recovery_tests::fault_child_for;
    for phase in [
        "after_instruction_history_schema",
        "after_instruction_history_change",
        "after_instruction_history_event",
    ] {
        let root = TestDir::new();
        let store = seed(&root, 0x5c);
        let c = change(None, Some(b"child history"));
        let action = if phase.ends_with("event") {
            store
                .save_instruction_change(&c, &store.workflow_data().unwrap().state_id)
                .unwrap();
            "event"
        } else {
            "change"
        };
        let before = store.workflow_data().unwrap().state_id;
        fault_child_for(
            &root,
            &root.0.join("unused"),
            phase,
            "crash",
            action,
            "instruction_history::tests::instruction_history_child",
        );
        // Open the keyed writable connection for ordinary SQLite rollback-journal
        // recovery after the owned child is killed. No target file was touched.
        drop(store.connect_complete(false).unwrap().unwrap());
        assert_eq!(store.workflow_data().unwrap().state_id, before);
        assert_eq!(store.workflow_data().unwrap().records.instruction_events, 0);
    }
    let root = TestDir::new();
    let store = seed(&root, 0x5c);
    let c = change(None, Some(b"child history"));
    store
        .save_instruction_change(&c, &store.workflow_data().unwrap().state_id)
        .unwrap();
    let attempted =
        InstructionEvent::prepare(c.id().into(), None, InstructionPhase::Attempted, None, 11)
            .unwrap();
    store
        .append_instruction_event(&attempted, &store.workflow_data().unwrap().state_id)
        .unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "instruction_history::tests::instruction_history_child",
        ])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("RANGOON_RECOVERY_TEST_WORKSPACE", &root.0)
        .env("RANGOON_RECOVERY_TEST_DELETE_ID", "reopen")
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
#[cfg(feature = "encrypted-sqlite")]
fn maximum_binary_original_and_proposal_fit_after_schema_one_upgrade() {
    let root = TestDir::new();
    let store = seed(&root, 0x49);
    let before = vec![255; MAX_INSTRUCTION_BYTES];
    let mut after = before.clone();
    after[0] = 254;
    let c = change(Some(&before), Some(&after));
    assert!(c.bytes().unwrap().len() <= MAX_RECORD_BYTES);
    store
        .save_instruction_change(&c, &store.workflow_data().unwrap().state_id)
        .unwrap();
    assert_eq!(store.instruction_changes().unwrap(), vec![c]);
}

#[test]
#[cfg(feature = "encrypted-sqlite")]
fn quota_refuses_before_mutation_and_explicit_delete_removes_only_selected_journal() {
    let mut records = Records::empty();
    records.version = 5;
    for i in 0..MAX_INSTRUCTION_CHANGES {
        let c = change(None, Some(format!("bounded-{i}").as_bytes()));
        records.history.changes.insert(c.id().into(), c);
    }
    let root = TestDir::new();
    let store = keyed(&root, 0x4a);
    let backup =
        WorkspaceBackup::decode_complete(&workflow_backup::encode(&records).unwrap()).unwrap();
    store.create_encrypted_recovery_copy(&backup).unwrap();
    let state = store.workflow_data().unwrap().state_id;
    let before = fs::read(store.path()).unwrap();
    assert_eq!(
        store.save_instruction_change(&change(None, Some(b"overflow")), &state),
        Err(StoreError::Full)
    );
    assert_eq!(fs::read(store.path()).unwrap(), before);
    let selected = store.instruction_changes().unwrap()[0].clone();
    let attempt = InstructionEvent::prepare(
        selected.id().into(),
        None,
        InstructionPhase::Attempted,
        None,
        11,
    )
    .unwrap();
    let state = store.append_instruction_event(&attempt, &state).unwrap();
    let unknown = InstructionEvent::prepare(
        selected.id().into(),
        Some(attempt.id().into()),
        InstructionPhase::Unknown,
        None,
        12,
    )
    .unwrap();
    store.append_instruction_event(&unknown, &state).unwrap();
    assert_eq!(store.instruction_events(selected.id()).unwrap().len(), 2);
    let delete = store
        .prepare_instruction_history_deletion(selected.id())
        .unwrap();
    assert_eq!(delete.removed_event_count(), 2);
    store.delete_instruction_history(&delete).unwrap();
    let data = store.workflow_data().unwrap();
    assert_eq!(data.records.instruction_changes, 127);
    assert_eq!(data.records.instruction_events, 0);
    assert!(matches!(
        store.instruction_events(selected.id()),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn combined_database_budget_refuses_before_decoding_any_blob() {
    let db = Connection::open_in_memory().unwrap();
    for (_, sql) in SCHEMAS {
        db.execute_batch(sql).unwrap();
    }
    for i in 0..8 {
        db.execute(
            "INSERT INTO instruction_changes VALUES (?1,zeroblob(?2))",
            params![format!("instruction-change:{i:064x}"), 2 * 1024 * 1024],
        )
        .unwrap();
    }
    db.execute(
        "INSERT INTO instruction_events VALUES (?1,zeroblob(1))",
        [format!("instruction-event:{:064x}", 0)],
    )
    .unwrap();
    // Blobs are deliberately undecodable. Full, rather than a decoding error,
    // proves the shared-budget rejection happens before record reconstruction.
    assert!(matches!(
        HistoryRows::load(&db, &Records::empty()),
        Err(StoreError::Full)
    ));
}

#[test]
#[cfg(feature = "encrypted-sqlite")]
fn nonempty_recovery_retains_applied_composition_recipe_and_exact_lineage() {
    let source = TestDir::new();
    let target_root = TestDir::new();
    let source_store = seed(&source, 0x4b);
    let report = analyze(
        "AGENTS.md",
        b"\xef\xbb\xbf# Rules\r\nKeep originals.\r\n# Tests\r\nCheck edits.\r\n",
    )
    .unwrap();
    source_store.save_v1(&report).unwrap();
    let mut request = crate::composition_records_tests::fixture_request();
    request.draft.inputs = vec![InputReference::Source {
        source_id: report.source.id.clone(),
        sha256: report.source.sha256.clone(),
    }];
    request.draft.outputs = report
        .fragments
        .iter()
        .enumerate()
        .map(|(i, fragment)| OutputRecipe {
            title: format!("Archived part {i}"),
            pieces: vec![Piece::Copy {
                range: InputRange {
                    input_index: 0,
                    start_byte: fragment.span.start_byte,
                    end_byte: fragment.span.end_byte,
                },
            }],
        })
        .collect();
    let recipe_id = rangoon_compose::composition_id(&request.draft).unwrap();
    let preview = source_store.preview_composition(&request).unwrap();
    assert!(preview.preview.saveable);
    source_store.apply_composition(&preview, true).unwrap();
    let change = InstructionChange::prepare(
        target(false),
        None,
        Some(b"derived"),
        request.draft.inputs.clone(),
        Some(recipe_id.clone()),
        None,
        10,
    )
    .unwrap();
    source_store
        .save_instruction_change(&change, &source_store.workflow_data().unwrap().state_id)
        .unwrap();
    let incoming = source_store.snapshot_encrypted_recovery().unwrap();
    let destination = seed(&target_root, 0x4c);
    let other = super::tests::change(None, Some(b"existing"));
    destination
        .save_instruction_change(&other, &destination.workflow_data().unwrap().state_id)
        .unwrap();
    let plan = destination.prepare_workflow_restore(&incoming).unwrap();
    destination
        .restore_workflow_backup(&incoming, &plan)
        .unwrap();
    drop(destination);
    let reopened = keyed(&target_root, 0x4c);
    let snapshot = reopened.snapshot_encrypted_recovery().unwrap();
    assert!(snapshot.records.recipes.contains_key(&recipe_id));
    assert_eq!(snapshot.records.applications.len(), 1);
    let recovered = reopened
        .instruction_changes()
        .unwrap()
        .into_iter()
        .find(|c| c.id() == change.id())
        .unwrap();
    assert_eq!(recovered, change);
    assert_eq!(recovered.inputs(), request.draft.inputs);
    let mut forged = incoming.records.clone();
    forged.recipes.get_mut(&recipe_id).unwrap().draft.outputs[0].title = "altered".into();
    assert!(composition_backup::canonical_database(&forged).is_err());
}

#[test]
#[cfg(feature = "encrypted-sqlite")]
fn incompatible_nonempty_recovery_refuses_missing_pinned_revision_without_mutation() {
    let source = TestDir::new();
    let prior = TestDir::new();
    let source_store = seed(&source, 0x4d);
    let report = analyze("AGENTS.md", b"# Scope\nInitial.\n").unwrap();
    source_store.save_v1(&report).unwrap();
    let root = source_store
        .create_capability_v1(&report.source.id, &report.fragments[0].id, "Scoped")
        .unwrap()
        .capability;
    let ancestor = source_store.snapshot_encrypted_recovery().unwrap();
    let destination = keyed(&prior, 0x4e);
    destination
        .create_encrypted_recovery_copy(&ancestor)
        .unwrap();
    let revised = source_store
        .revise_capability_v1(
            &root.id,
            &root.latest_revision_id,
            "Scoped",
            "# Scope\nChanged.\n",
        )
        .unwrap()
        .capability;
    let change = InstructionChange::prepare(
        target(false),
        None,
        Some(b"new"),
        vec![InputReference::Revision {
            capability_id: revised.id,
            revision_id: revised.revision.id,
            sha256: revised.revision.sha256,
        }],
        None,
        None,
        10,
    )
    .unwrap();
    source_store
        .save_instruction_change(&change, &source_store.workflow_data().unwrap().state_id)
        .unwrap();
    let incoming = source_store.snapshot_encrypted_recovery().unwrap();
    let before = fs::read(destination.path()).unwrap();
    let state = destination.workflow_data().unwrap().state_id;
    assert!(matches!(
        destination.prepare_workflow_restore(&incoming),
        Err(StoreError::RecordInUse)
    ));
    assert_eq!(destination.workflow_data().unwrap().state_id, state);
    assert_eq!(fs::read(destination.path()).unwrap(), before);
    assert!(destination.instruction_changes().unwrap().is_empty());
}

#[test]
fn old_inventory_wire_is_exact_and_schema_five_negotiates_v3_even_empty() {
    for version in 1..=4 {
        let mut records = Records::empty();
        records.version = version;
        let data = workflow_views::data(&records, 0, 0).unwrap();
        let expected = format!(
            r#"{{"schemaVersion":"rangoon.workspace-data.v2","stateId":"{}","records":{{"sources":0,"capabilities":0,"revisions":0,"reviews":0,"recipes":0,"applications":0,"derivations":0,"workflows":0,"workflowRevisions":0}},"databaseBytes":0,"reusableBytes":0,"sources":[],"capabilities":[],"workflows":[]}}"#,
            records.state_id().unwrap()
        );
        assert_eq!(serde_json::to_string(&data).unwrap(), expected);
    }
    let mut records = Records::empty();
    records.version = 5;
    let empty = workflow_views::data(&records, 0, 0).unwrap();
    assert_eq!(empty.schema_version, "rangoon.workspace-data.v3");
    let c = change(None, Some(b"new"));
    records.history.changes.insert(c.id().into(), c);
    let json = serde_json::to_value(workflow_views::data(&records, 0, 0).unwrap()).unwrap();
    assert_eq!(json["schemaVersion"], "rangoon.workspace-data.v3");
    assert_eq!(json["records"]["instructionChanges"], 1);
}
#[test]
#[cfg(feature = "encrypted-sqlite")]
fn history_restore_plan_version_is_explicit_and_stale_preview_refuses() {
    let a = TestDir::new();
    let b = TestDir::new();
    let sa = seed(&a, 0x60);
    let sb = seed(&b, 0x61);
    let old = sa.snapshot_encrypted_recovery().unwrap();
    assert_eq!(
        serde_json::to_value(sb.prepare_workflow_restore(&old).unwrap()).unwrap()["schemaVersion"],
        "rangoon.restore-plan.v2"
    );
    let c = change(None, Some(b"new"));
    sa.save_instruction_change(&c, &sa.workflow_data().unwrap().state_id)
        .unwrap();
    let incoming = sa.snapshot_encrypted_recovery().unwrap();
    let retained = sb.prepare_workflow_restore(&incoming).unwrap();
    let json = serde_json::to_value(&retained).unwrap();
    assert_eq!(json["schemaVersion"], "rangoon.restore-plan.v3");
    assert_eq!(json["add"]["instructionChanges"], 1);
    sb.save_instruction_change(
        &change(None, Some(b"local")),
        &sb.workflow_data().unwrap().state_id,
    )
    .unwrap();
    let before = fs::read(sb.path()).unwrap();
    assert!(matches!(
        sb.restore_workflow_backup(&incoming, &retained),
        Err(StoreError::WorkspaceChanged)
    ));
    assert_eq!(fs::read(sb.path()).unwrap(), before);
    assert_eq!(
        serde_json::to_value(sb.prepare_workflow_restore(&old).unwrap()).unwrap()["schemaVersion"],
        "rangoon.restore-plan.v3"
    );
    let remove = sb
        .prepare_instruction_history_deletion(sb.instruction_changes().unwrap()[0].id())
        .unwrap();
    sb.delete_instruction_history(&remove).unwrap();
    assert_eq!(
        sb.workflow_data().unwrap().schema_version,
        "rangoon.workspace-data.v3"
    );
    assert_eq!(
        serde_json::to_value(sb.prepare_workflow_restore(&old).unwrap()).unwrap()["schemaVersion"],
        "rangoon.restore-plan.v3"
    );
}

#[test]
#[cfg(feature = "encrypted-sqlite")]
fn generic_empty_history_recovery_preserves_schema_five_for_fresh_targets() {
    let source = TestDir::new();
    let sa = seed(&source, 0x62);
    let c = change(None, Some(b"temporary"));
    sa.save_instruction_change(&c, &sa.workflow_data().unwrap().state_id)
        .unwrap();
    let delete = sa.prepare_instruction_history_deletion(c.id()).unwrap();
    sa.delete_instruction_history(&delete).unwrap();
    let archive = sa.snapshot_encrypted_recovery().unwrap();
    assert_eq!(archive.records.version, 5);
    for initialized in [false, true] {
        let target_root = TestDir::new();
        let target = keyed(&target_root, 0x63);
        if initialized {
            target.initialize_empty_encrypted().unwrap();
        }
        let preview = target.prepare_workflow_restore(&archive).unwrap();
        let json = serde_json::to_value(&preview).unwrap();
        assert_eq!(json["schemaVersion"], "rangoon.restore-plan.v3");
        assert_eq!(json["targetWorkspaceSchema"], 5);
        let data = target.restore_workflow_backup(&archive, &preview).unwrap();
        assert_eq!(data.schema_version, "rangoon.workspace-data.v3");
        assert_eq!(data.records, WorkflowRecoveryCounts::default());
        drop(target);
        let reopened = keyed(&target_root, 0x63);
        assert_eq!(
            reopened
                .snapshot_encrypted_recovery()
                .unwrap()
                .records
                .version,
            5
        );
        let report = analyze("AGENTS.md", b"# After recovery\nKeep version.\n").unwrap();
        reopened.save_v1(&report).unwrap();
        assert_eq!(
            reopened.workflow_data().unwrap().schema_version,
            "rangoon.workspace-data.v3"
        );
        assert_eq!(
            reopened
                .snapshot_encrypted_recovery()
                .unwrap()
                .records
                .version,
            5
        );
        assert_eq!(
            reopened.export_workflow_backup(),
            Err(StoreError::UnsupportedSchema)
        );
    }
}
