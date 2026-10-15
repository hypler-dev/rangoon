use super::*;

use crate::workspace_close::{View, WorkspaceClose};
use crate::{
    composition::CompositionResult,
    data_controls::{DataResult, NativeRecordKind},
    workflows::{BeginRequest, InspectRequest, Target as WorkflowTarget, WorkflowResult},
};
use rangoon_compose::{
    Draft, InputRange, InputReference, Operation as CompositionOperation, OutputRecipe, Piece,
    application::{Request as CompositionRequest, Target as CompositionTarget},
};
use rangoon_desktop::workspace_lifecycle::Lane;
use rangoon_workflow::records::SaveIntent;
use serde_json::value::RawValue;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};

fn view_json(view: View) -> Value {
    serde_json::to_value(view).unwrap()
}

fn attempt(view: &Value) -> (String, String) {
    (
        view["attemptId"].as_str().unwrap().to_owned(),
        view["epoch"].as_str().unwrap().to_owned(),
    )
}

fn acknowledgment(attempt_id: &str, epoch: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": "rangoon.workspace-purge-ack.v1",
        "attemptId": attempt_id,
        "epoch": epoch,
    }))
    .unwrap()
}

fn close_workflow_definition() -> Box<RawValue> {
    RawValue::from_string(
        json!({
            "schemaVersion": "rangoon.workflow-definition.v1",
            "title": "Close-cycle workflow",
            "nodes": [],
            "controlEdges": [],
            "dataEdges": []
        })
        .to_string(),
    )
    .unwrap()
}

fn close_composition_request(source: &AnalysisReport) -> CompositionRequest {
    CompositionRequest {
        schema_version: rangoon_compose::application::REQUEST_SCHEMA.into(),
        draft: Draft {
            schema_version: rangoon_compose::DRAFT_SCHEMA.into(),
            operation: CompositionOperation::Decompose,
            inputs: vec![InputReference::Source {
                source_id: source.source.id.clone(),
                sha256: source.source.sha256.clone(),
            }],
            outputs: vec![OutputRecipe {
                title: "Close-cycle composition".into(),
                pieces: vec![Piece::Copy {
                    range: InputRange {
                        input_index: 0,
                        start_byte: 0,
                        end_byte: source.source.content.len().try_into().unwrap(),
                    },
                }],
            }],
            exclusions: vec![],
            duplications: vec![],
            conflicts: vec![],
        },
        targets: vec![CompositionTarget::New {}],
    }
}

fn close_probe(label: &str) -> (SourceAccess, Gate) {
    let gate = Gate::new();
    let state = Arc::new(ProbeState::default());
    let access = SourceAccess::with_mode(
        gate.clone(),
        Box::new(ProbeBackend::new(
            state,
            report(label, b"# Close fixture\n"),
        )),
        true,
    );
    (access, gate)
}

#[test]
fn legacy_close_is_unavailable_without_cleanup_or_gate_side_effects() {
    let root = TestRoot::new("close-legacy");
    let gate = Gate::new();
    let access = SourceAccess::legacy(gate.clone(), Ok(root.0.clone()));
    let close = WorkspaceClose::new(access);
    let cleanup = Arc::new(AtomicUsize::new(0));

    let view = view_json(close.begin({
        let cleanup = Arc::clone(&cleanup);
        move |_| {
            cleanup.fetch_add(1, Ordering::SeqCst);
            true
        }
    }));

    assert_eq!(view["schemaVersion"], "rangoon.workspace-close-state.v1");
    assert_eq!(view["storage"], "legacy_plaintext");
    assert_eq!(view["state"], "unavailable");
    assert_eq!(view["diagnostic"], "encrypted_close_unavailable");
    assert_eq!(cleanup.load(Ordering::SeqCst), 0);
    assert!(gate.begin(Lane::Read).is_ok());
    assert!(!root.0.join("workspace.sqlite3").exists());
}

#[test]
fn close_uses_one_terminal_cycle_and_never_resumes_admission() {
    let (access, gate) = close_probe("close-cycle.md");
    let close = WorkspaceClose::new(access);
    let cleanup = Arc::new(AtomicUsize::new(0));

    let first = view_json(close.begin({
        let cleanup = Arc::clone(&cleanup);
        move |_| {
            cleanup.fetch_add(1, Ordering::SeqCst);
            true
        }
    }));
    let second = view_json(close.begin({
        let cleanup = Arc::clone(&cleanup);
        move |_| {
            cleanup.fetch_add(1, Ordering::SeqCst);
            true
        }
    }));

    assert_eq!(attempt(&first), attempt(&second));
    assert_eq!(second["state"], "closing");
    assert_eq!(cleanup.load(Ordering::SeqCst), 2);
    assert!(gate.begin(Lane::Read).is_err());
    drop(close);
    assert!(gate.begin(Lane::Read).is_err());
}

#[test]
fn held_lease_defers_key_release_until_the_real_drain_is_quiescent() {
    let (access, _) = close_probe("close-held-lease.md");
    let held = access.begin(Lane::Read).unwrap();
    let close = WorkspaceClose::new(access);
    let cleanup = Arc::new(AtomicUsize::new(0));
    let begin = view_json(close.begin({
        let cleanup = Arc::clone(&cleanup);
        move |_| {
            cleanup.fetch_add(1, Ordering::SeqCst);
            true
        }
    }));
    let (attempt_id, epoch) = attempt(&begin);

    let before_drain = view_json(close.acknowledge(&acknowledgment(&attempt_id, &epoch), |_| true));
    assert_eq!(before_drain["state"], "closing");
    assert_eq!(cleanup.load(Ordering::SeqCst), 1);

    drop(held);
    let complete = view_json(close.inspect({
        let cleanup = Arc::clone(&cleanup);
        move |drained| {
            assert!(drained);
            cleanup.fetch_add(1, Ordering::SeqCst);
            true
        }
    }));
    assert_eq!(complete["state"], "locked");
    assert_eq!(cleanup.load(Ordering::SeqCst), 2);
}

#[test]
fn close_ticket_refuses_release_while_held_and_cannot_target_another_access() {
    let (first, _) = close_probe("close-ticket-first.md");
    let (second, _) = close_probe("close-ticket-second.md");
    let retained = report("close-ticket-retained.md", b"# Retained before close\n");
    select(&second, &retained);
    assert!(second.0.session.snapshot(&retained.source.id).is_some());

    let held = first.begin(Lane::Read).unwrap();
    let ticket = first.request_close().unwrap();
    assert_eq!(ticket.is_quiescent(), Ok(false));
    assert_eq!(ticket.release_key(), Err(()));
    assert!(ticket.purge());
    assert!(second.0.session.snapshot(&retained.source.id).is_some());

    drop(ticket);
    let same_cycle = first.request_close().unwrap();
    drop(held);
    assert_eq!(same_cycle.is_quiescent(), Ok(true));
    assert_eq!(same_cycle.release_key(), Ok(true));
    assert!(second.0.session.snapshot(&retained.source.id).is_some());
}

#[test]
fn close_purges_retained_source_before_the_final_drain_pass() {
    let (access, _) = close_probe("close-immediate-purge.md");
    let retained = report("close-immediate-purge-source.md", b"# Source retained\n");
    select(&access, &retained);
    assert!(access.0.session.snapshot(&retained.source.id).is_some());

    let held = access.begin(Lane::Read).unwrap();
    let close = WorkspaceClose::new(access.clone());
    let begin = view_json(close.begin(|drained| !drained));
    assert_eq!(begin["state"], "closing");
    assert!(access.0.session.snapshot(&retained.source.id).is_none());

    drop(held);
    assert_eq!(
        view_json(close.inspect(|drained| drained))["state"],
        "closing"
    );
}

#[test]
fn acknowledgment_requires_the_exact_bounded_attempt_and_epoch_once() {
    let (access, _) = close_probe("close-ack.md");
    let close = WorkspaceClose::new(access);
    let begin = view_json(close.begin(|_| true));
    let (attempt_id, epoch) = attempt(&begin);
    let malformed = [
        b"{}".as_slice(),
        b"[]",
        b"{\"schemaVersion\":\"rangoon.workspace-purge-ack.v1\",\"attemptId\":\"x\",\"epoch\":\"1\"}",
        b"{\"schemaVersion\":\"rangoon.workspace-purge-ack.v1\",\"attemptId\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"epoch\":\"01\"}",
        b"{\"schemaVersion\":\"rangoon.workspace-purge-ack.v1\",\"attemptId\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"epoch\":\"1\",\"extra\":true}",
    ];
    for raw in malformed {
        let rejected = view_json(close.acknowledge(raw, |_| panic!("invalid ack must not clean")));
        assert_eq!(rejected["diagnostic"], "invalid_workspace_close_request");
        assert_eq!(rejected["state"], "closing");
    }

    let wrong_attempt = acknowledgment(&"0".repeat(64), &epoch);
    let rejected =
        view_json(close.acknowledge(&wrong_attempt, |_| panic!("wrong ack must not clean")));
    assert_eq!(rejected["diagnostic"], "invalid_workspace_close_request");
    let wrong_epoch = acknowledgment(&attempt_id, "2");
    let rejected =
        view_json(close.acknowledge(&wrong_epoch, |_| panic!("stale ack must not clean")));
    assert_eq!(rejected["diagnostic"], "invalid_workspace_close_request");

    let locked = view_json(close.acknowledge(&acknowledgment(&attempt_id, &epoch), |_| true));
    assert_eq!(locked["state"], "locked");
    let replay = view_json(
        close.acknowledge(&acknowledgment(&attempt_id, &epoch), |_| {
            panic!("replay must not clean")
        }),
    );
    assert_eq!(replay["diagnostic"], "invalid_workspace_close_request");
    assert_eq!(replay["state"], "locked");
}

#[test]
fn dropped_tauri_waiter_keeps_the_drain_open_until_its_callback_returns() {
    let (access, _) = close_probe("close-dropped-waiter.md");
    let operation = access.begin_selection().unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (finish_tx, finish_rx) = mpsc::channel();
    let (completed_tx, completed_rx) = mpsc::channel();
    let waiter = tauri::async_runtime::spawn_blocking(move || {
        started_tx.send(()).unwrap();
        finish_rx.recv_timeout(WAIT).unwrap();
        completed_tx
            .send(operation.complete(SelectionResult::Cancelled).deliver())
            .unwrap();
    });
    drop(waiter);
    started_rx.recv_timeout(WAIT).unwrap();

    let close = WorkspaceClose::new(access);
    let begin = view_json(close.begin(|_| true));
    let (attempt_id, epoch) = attempt(&begin);
    let waiting = view_json(
        close.acknowledge(&acknowledgment(&attempt_id, &epoch), |_| {
            panic!("callback lease still holds the drain")
        }),
    );
    assert_eq!(waiting["state"], "closing");

    finish_tx.send(()).unwrap();
    assert_eq!(
        selection_code(completed_rx.recv_timeout(WAIT).unwrap()),
        "analysis_cleared"
    );
    assert_eq!(
        view_json(close.inspect(|drained| drained))["state"],
        "locked"
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn managed_owner_close_locks_without_vault_io_or_plaintext_fallback() {
    let root = TestRoot::new("close-managed-owner");
    let vault = ThreadVault::default();
    let (access, identity) = encrypted_access(&root, &vault);
    let source = report(
        "close-managed-owner.md",
        b"# Encrypted close\nsecret crab\n",
    );
    select(&access, &source);
    assert!(matches!(
        tauri::async_runtime::block_on(access.save_analysis(source.source.id.clone())),
        WorkspaceResult::Saved { .. }
    ));
    let database = root
        .0
        .join("encrypted-workspaces")
        .join(identity)
        .join("workspace.sqlite3");
    let ciphertext = fs::read(&database).unwrap();
    let opens_before_close = vault.0.lock().unwrap().opens;

    let close = WorkspaceClose::new(access.clone());
    let begin = view_json(close.begin(|_| true));
    let (attempt_id, epoch) = attempt(&begin);
    assert_eq!(
        view_json(close.acknowledge(&acknowledgment(&attempt_id, &epoch), |_| true))["state"],
        "locked"
    );
    assert!(matches!(
        access.0.backend.lock().unwrap().list(),
        Err(Error::Managed(
            rangoon_desktop::workspace_session::Error::Locked
        ))
    ));
    assert_eq!(
        workspace_code(tauri::async_runtime::block_on(access.list_snapshots())),
        "workspace_unavailable"
    );
    assert_eq!(fs::read(&database).unwrap(), ciphertext);
    assert_eq!(vault.0.lock().unwrap().opens, opens_before_close);
    assert!(!root.0.join("workspace.sqlite3").exists());
    assert!(
        !ciphertext
            .windows(b"secret crab".len())
            .any(|bytes| bytes == b"secret crab")
    );
}

#[cfg(feature = "encrypted-workspace")]
#[test]
fn existing_owner_recovery_cannot_be_reported_as_a_successful_close() {
    let root = TestRoot::new("close-managed-recovery");
    let vault = ThreadVault::default();
    let (access, _) = encrypted_access(&root, &vault);
    let source = report("close-managed-recovery.md", b"# Recovery\nExact source\n");
    select(&access, &source);
    assert!(matches!(
        tauri::async_runtime::block_on(access.save_analysis(source.source.id.clone())),
        WorkspaceResult::Saved { .. }
    ));
    assert!(matches!(
        tauri::async_runtime::block_on(access.create_capability(
            source.source.id.clone(),
            source.fragments[0].id.clone(),
            " malformed".into(),
        )),
        CapabilityResult::Failed { error } if error.code == "workspace_write_uncertain"
    ));

    let close = WorkspaceClose::new(access);
    let begin = view_json(close.begin(|_| true));
    let (attempt_id, epoch) = attempt(&begin);
    let recovery = view_json(close.acknowledge(&acknowledgment(&attempt_id, &epoch), |_| true));
    assert_eq!(recovery["state"], "recovery_required");
    assert_eq!(recovery["diagnostic"], "workspace_close_incomplete");
}

#[test]
fn poisoned_backend_cleanup_is_recovery_required_and_stays_closed() {
    let (access, _) = close_probe("close-poisoned-backend.md");
    std::thread::scope(|scope| {
        let backend = &access.0.backend;
        assert!(
            scope
                .spawn(move || {
                    let _guard = backend.lock().unwrap();
                    panic!("synthetic close backend poison");
                })
                .join()
                .is_err()
        );
    });

    let close = WorkspaceClose::new(access);
    let begin = view_json(close.begin(|_| true));
    let (attempt_id, epoch) = attempt(&begin);
    let recovery = view_json(close.acknowledge(&acknowledgment(&attempt_id, &epoch), |_| true));
    assert_eq!(recovery["state"], "recovery_required");
    assert_eq!(recovery["diagnostic"], "workspace_close_incomplete");
    assert_eq!(
        view_json(close.inspect(|_| panic!("completed cleanup must not repeat")))["state"],
        "recovery_required"
    );
}

#[test]
fn close_purges_all_retained_families_before_and_after_the_real_drain() {
    let root = TestRoot::new("close-all-retained-families");
    let gate = Gate::new();
    // This is a memory-only close-protocol fixture. Keyed Owner/ciphertext close
    // behavior is covered separately by the managed fixture tests above.
    let access = SourceAccess::with_mode(gate.clone(), Box::new(Legacy(Ok(root.0.clone()))), true);
    let source = report(
        "close-all-retained.md",
        b"# Close all retained\nExact bytes\n",
    );
    select(&access, &source);
    assert!(matches!(
        tauri::async_runtime::block_on(access.save_analysis(source.source.id.clone())),
        WorkspaceResult::Saved { .. }
    ));

    let draft_id = match tauri::async_runtime::block_on(access.begin_workflow_draft(BeginRequest {
        schema_version: "rangoon.workflow-draft.v1".into(),
        target: WorkflowTarget::New {},
    })) {
        WorkflowResult::DraftReady { draft_id, .. } => draft_id,
        _ => panic!("expected routed workflow draft"),
    };
    assert!(matches!(
        tauri::async_runtime::block_on(access.inspect_workflow_save(InspectRequest {
            schema_version: "rangoon.workflow-inspect.v1".into(),
            draft_id,
            intent: SaveIntent::Draft,
            definition: close_workflow_definition(),
            layout: RawValue::from_string(r#"{"positions":[]}"#.into()).unwrap(),
        })),
        WorkflowResult::SaveReady { .. }
    ));
    assert!(matches!(
        tauri::async_runtime::block_on(
            access.preview_composition(close_composition_request(&source))
        ),
        CompositionResult::Ready { .. }
    ));

    let backup = {
        let mut bytes = None;
        assert!(matches!(
            access
                .begin_file()
                .unwrap()
                .export_backup(
                    || Ok(Some(root.0.join("synthetic-close.rangoon-backup"))),
                    |_, exported| {
                        bytes = Some(exported.to_vec());
                        Ok(())
                    },
                )
                .deliver(),
            DataResult::Exported { .. }
        ));
        bytes.unwrap()
    };
    assert!(matches!(
        access
            .begin_file()
            .unwrap()
            .prepare_restore(
                || Ok(Some(root.0.join("synthetic-close.rangoon-backup"))),
                |_| Ok(backup),
            )
            .deliver(),
        DataResult::RestoreReady { .. }
    ));
    assert!(matches!(
        tauri::async_runtime::block_on(
            access.inspect_workspace_deletion(NativeRecordKind::Source, source.source.id.clone(),)
        ),
        DataResult::DeletionReady { .. }
    ));

    assert!(access.0.session.snapshot(&source.source.id).is_some());
    assert!(access.0.workflow.retained_for_close_test());
    assert!(access.0.composition.retained_for_close_test());
    assert!(access.0.backup.retained_for_close_test());
    assert!(access.0.deletion.retained_for_close_test());

    let held = access.begin(Lane::Read).unwrap();
    let close = WorkspaceClose::new(access.clone());
    let begin = view_json(close.begin(|drained| !drained));
    let (attempt_id, epoch) = attempt(&begin);
    assert_eq!(begin["state"], "closing");
    assert!(access.0.session.snapshot(&source.source.id).is_none());
    assert!(!access.0.workflow.retained_for_close_test());
    assert!(!access.0.composition.retained_for_close_test());
    assert!(!access.0.backup.retained_for_close_test());
    assert!(!access.0.deletion.retained_for_close_test());

    assert_eq!(
        view_json(
            close.acknowledge(&acknowledgment(&attempt_id, &epoch), |_| {
                panic!("the final close pass must wait for the held gate lease")
            })
        )["state"],
        "closing"
    );
    drop(held);
    assert_eq!(
        view_json(close.inspect(|drained| drained))["state"],
        "locked"
    );
    assert!(access.0.session.snapshot(&source.source.id).is_none());
    assert!(!access.0.workflow.retained_for_close_test());
    assert!(!access.0.composition.retained_for_close_test());
    assert!(!access.0.backup.retained_for_close_test());
    assert!(!access.0.deletion.retained_for_close_test());
    assert!(gate.begin(Lane::Read).is_err());
}
