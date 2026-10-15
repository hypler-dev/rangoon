use super::*;
use rangoon_desktop::workspace_lifecycle::Gate;

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
use zeroize::Zeroizing;

const WAIT: Duration = Duration::from_secs(10);

#[derive(Default)]
struct FakeStore {
    value: Mutex<Option<Vec<u8>>>,
    reads: AtomicUsize,
}

impl cloud_store::Store for FakeStore {
    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .value
            .lock()
            .map_err(|_| Error::Unavailable)?
            .clone()
            .map(Zeroizing::new))
    }

    fn write(&self, value: &[u8]) -> Result<(), Error> {
        *self.value.lock().map_err(|_| Error::Unavailable)? = Some(value.to_vec());
        Ok(())
    }

    fn delete(&self) -> Result<(), Error> {
        *self.value.lock().map_err(|_| Error::Unavailable)? = None;
        Ok(())
    }
}

fn secret() -> cloud_store::Secret {
    cloud_store::Secret::new(b"synthetic-credential-fixture").unwrap()
}

fn start_drain(gate: &Gate) -> rangoon_desktop::workspace_lifecycle::Drain {
    gate.request_drain().unwrap()
}

fn assert_quiescent(drain: &rangoon_desktop::workspace_lifecycle::Drain, expected: bool) {
    assert_eq!(drain.is_quiescent().unwrap(), expected);
}

fn serialized(completion: CredentialCompletion) -> String {
    serde_json::to_string(&completion.deliver()).unwrap()
}

fn expected_failure(code: &str) -> String {
    format!(
        r#"{{"schemaVersion":"rangoon.cloud-custody.v1","provider":"openai","authority":"none","backend":"{}","outcome":"failed","code":"{code}"}}"#,
        backend()
    )
}

fn expected_cancelled() -> String {
    format!(
        r#"{{"schemaVersion":"rangoon.cloud-custody.v1","provider":"openai","authority":"none","backend":"{}","outcome":"cancelled"}}"#,
        backend()
    )
}

#[test]
fn credential_begin_refuses_closed_gate_without_running_a_store_callback() {
    let gate = Gate::new();
    let custody = Custody::with_gate(gate.clone());
    let (lease, _) = custody.begin(Phase::Reading).unwrap();
    let drain = start_drain(&gate);
    let calls = AtomicUsize::new(0);

    assert_eq!(
        serialized(inspect_with(lease, || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        })),
        expected_failure("unavailable")
    );
    assert!(matches!(
        custody.begin(Phase::Reading),
        Err(Error::Unavailable)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_quiescent(&drain, true);
}

#[test]
fn credential_late_read_is_redacted_without_a_revision() {
    let gate = Gate::new();
    let custody = Custody::with_gate(gate.clone());
    let store = Arc::new(FakeStore::default());
    let expected = secret();
    cloud_store::save(store.as_ref(), &expected, None).unwrap();
    let (lease, _) = custody.begin(Phase::Reading).unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (completion_tx, completion_rx) = mpsc::channel();
    let worker_store = Arc::clone(&store);
    let waiter = tauri::async_runtime::spawn_blocking(move || {
        let completion = inspect_with(lease, || {
            started_tx.send(()).unwrap();
            release_rx.recv_timeout(WAIT).unwrap();
            cloud_store::inspect(worker_store.as_ref())
        });
        completion_tx.send(completion).unwrap();
    });
    started_rx.recv_timeout(WAIT).unwrap();
    let drain = start_drain(&gate);
    release_tx.send(()).unwrap();
    let actual = serialized(completion_rx.recv_timeout(WAIT).unwrap());
    assert!(tauri::async_runtime::block_on(waiter).is_ok());

    assert_eq!(actual, expected_failure("unavailable"));
    assert!(!actual.contains(&expected.revision()));
    assert_eq!(store.reads.load(Ordering::SeqCst), 3);
    assert_quiescent(&drain, true);
}

#[test]
fn credential_completion_retains_gate_until_delivery_or_drop() {
    let gate = Gate::new();
    let custody = Custody::with_gate(gate.clone());
    let (lease, _) = custody.begin(Phase::Reading).unwrap();
    let completion = CredentialCompletion::read(lease, Ok(Outcome::Missing));
    let drain = start_drain(&gate);
    assert_quiescent(&drain, false);
    assert_eq!(serialized(completion), expected_failure("unavailable"));
    assert_quiescent(&drain, true);

    let gate = Gate::new();
    let custody = Custody::with_gate(gate.clone());
    let (lease, _) = custody.begin(Phase::Reading).unwrap();
    let completion = CredentialCompletion::read(lease, Ok(Outcome::Missing));
    let drain = start_drain(&gate);
    assert_quiescent(&drain, false);
    drop(completion);
    assert_quiescent(&drain, true);
}

#[test]
fn credential_closed_generation_refuses_editor_actions_but_allows_cancel_cleanup() {
    let gate = Gate::new();
    let custody = Custody::with_gate(gate.clone());
    let (lease, mut receiver) = custody.begin(Phase::Editing).unwrap();
    let id = lease.id.clone();
    let drain = start_drain(&gate);

    assert!(!lease.is_current());
    assert_eq!(custody.edit_id(), None);
    assert_eq!(custody.submit(&id, secret()), Err(Error::InvalidRequest));
    assert!(!custody.advance(&id, Phase::Prompting));
    assert!(custody.cancel_edit(&id));
    assert!(receiver.try_recv().unwrap().is_none());
    drop(lease);
    assert_quiescent(&drain, true);
}

#[test]
fn credential_stale_mutations_before_prompt_or_delivery_report_changed() {
    for phase in [Phase::Prompting, Phase::Storage] {
        for outcome in [
            Outcome::Saved {
                revision: "a".repeat(64),
            },
            Outcome::Removed,
        ] {
            let gate = Gate::new();
            let custody = Custody::with_gate(gate.clone());
            let (lease, _) = custody.begin(phase).unwrap();
            let drain = start_drain(&gate);

            assert_eq!(
                serialized(CredentialCompletion::mutation(lease, Ok(outcome))),
                expected_failure("changed")
            );
            assert_quiescent(&drain, true);
        }
    }

    let gate = Gate::new();
    let custody = Custody::with_gate(gate.clone());
    let (lease, _) = custody.begin(Phase::Storage).unwrap();
    let drain = start_drain(&gate);
    assert_eq!(
        serialized(CredentialCompletion::mutation(
            lease,
            Err(Error::WriteUncertain),
        )),
        expected_failure("write_uncertain")
    );
    assert_quiescent(&drain, true);
}

#[test]
fn credential_dropped_tauri_waiter_keeps_callback_and_completion_leases() {
    let gate = Gate::new();
    let custody = Custody::with_gate(gate.clone());
    let (lease, _) = custody.begin(Phase::Reading).unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (completion_tx, completion_rx) = mpsc::channel();
    let waiter = tauri::async_runtime::spawn_blocking(move || {
        let completion = inspect_with(lease, || {
            started_tx.send(()).unwrap();
            release_rx.recv_timeout(WAIT).unwrap();
            Ok(Some(secret()))
        });
        completion_tx.send(completion).unwrap();
    });

    started_rx.recv_timeout(WAIT).unwrap();
    drop(waiter);
    let drain = start_drain(&gate);
    assert_quiescent(&drain, false);
    release_tx.send(()).unwrap();
    let completion = completion_rx.recv_timeout(WAIT).unwrap();
    assert_quiescent(&drain, false);
    drop(completion);
    assert_quiescent(&drain, true);
}

#[test]
fn credential_panicking_callback_releases_its_lease() {
    let gate = Gate::new();
    let custody = Custody::with_gate(gate.clone());
    let (lease, _) = custody.begin(Phase::Reading).unwrap();
    let waiter = tauri::async_runtime::spawn_blocking(move || {
        let _ = inspect_with(lease, || -> Result<Option<cloud_store::Secret>, Error> {
            panic!("synthetic credential callback panic")
        });
    });

    assert!(tauri::async_runtime::block_on(waiter).is_err());
    assert_quiescent(&start_drain(&gate), true);
}

#[test]
fn credential_local_cancel_stays_cancelled_and_blocks_until_completion_delivery() {
    let custody = Custody::with_gate(Gate::new());
    let (lease, _) = custody.begin(Phase::Editing).unwrap();
    let id = lease.id.clone();
    assert!(custody.cancel_edit(&id));
    let completion = CredentialCompletion::mutation(lease, Ok(Outcome::Cancelled));

    assert!(matches!(custody.begin(Phase::Reading), Err(Error::Busy)));
    let actual = serialized(completion);
    assert_eq!(actual, expected_cancelled());
    assert!(!actual.contains("revision"));
    assert!(custody.begin(Phase::Reading).is_ok());
}

#[test]
fn credential_current_inspection_preserves_the_stored_revision() {
    let gate = Gate::new();
    let custody = Custody::with_gate(gate.clone());
    let store = FakeStore::default();
    let expected = secret();
    let revision = expected.revision();
    cloud_store::save(&store, &expected, None).unwrap();
    let (lease, _) = custody.begin(Phase::Reading).unwrap();

    assert_eq!(
        serialized(inspect_with(lease, || cloud_store::inspect(&store))),
        format!(
            r#"{{"schemaVersion":"rangoon.cloud-custody.v1","provider":"openai","authority":"none","backend":"{}","outcome":"stored","revision":"{revision}"}}"#,
            backend()
        )
    );
    assert_eq!(store.reads.load(Ordering::SeqCst), 3);
}

#[test]
fn credential_cancelled_removal_before_lookup_stays_cancelled_without_store_access() {
    let gate = Gate::new();
    let custody = Custody::with_gate(gate.clone());
    let (lease, _) = custody.begin(Phase::Reading).unwrap();
    assert!(custody.cancel(None));
    let calls = AtomicUsize::new(0);
    let selected = mutation_read(&lease, || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    })
    .unwrap();
    assert!(selected.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        serialized(CredentialCompletion::mutation(
            lease,
            Ok(Outcome::Cancelled)
        )),
        format!(
            r#"{{"schemaVersion":"rangoon.cloud-custody.v1","provider":"openai","authority":"none","backend":"{}","outcome":"cancelled"}}"#,
            backend()
        )
    );
    assert_quiescent(&start_drain(&gate), true);
}

#[test]
fn credential_workspace_closed_before_mutation_lookup_never_opens_store() {
    let gate = Gate::new();
    let custody = Custody::with_gate(gate.clone());
    let (lease, _) = custody.begin(Phase::Reading).unwrap();
    let drain = start_drain(&gate);
    let calls = AtomicUsize::new(0);
    let result = mutation_read(&lease, || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    assert_eq!(result, Err(Error::Changed));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        serialized(CredentialCompletion::mutation(lease, Err(Error::Changed))),
        expected_failure("changed")
    );
    assert_quiescent(&drain, true);
}
