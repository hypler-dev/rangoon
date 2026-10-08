use super::*;
use rangoon_desktop::workspace_lifecycle::Drain;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

const WAIT: Duration = Duration::from_secs(10);

struct CompletionDrop(mpsc::Sender<()>);

impl Drop for CompletionDrop {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

fn dispatch<T: Send + 'static>(
    gate: &Gate,
    action: impl FnOnce() -> T + Send + 'static,
) -> tauri::async_runtime::JoinHandle<T> {
    match dispatch_read(gate, action) {
        Ok(handle) => handle,
        Err(_) => panic!("expected synthetic read admission"),
    }
}

fn begin_exclusive(gate: &Gate) -> Lease {
    match gate.begin(Lane::Exclusive) {
        Ok(lease) => lease,
        Err(_) => panic!("expected synthetic exclusive admission"),
    }
}

fn assert_quiescent(drain: &Drain, expected: bool) {
    match drain.is_quiescent() {
        Ok(actual) => assert_eq!(actual, expected),
        Err(_) => panic!("expected synthetic drain state"),
    }
}

#[test]
fn dropped_read_waiter_keeps_callback_lease_until_callback_result_drops() {
    let (gate, _) = admission_state();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (completion_tx, completion_rx) = mpsc::channel();

    let waiter = dispatch(&gate, move || {
        assert!(started_tx.send(()).is_ok());
        assert!(release_rx.recv_timeout(WAIT).is_ok());
        CompletionDrop(completion_tx)
    });

    assert!(started_rx.recv_timeout(WAIT).is_ok());
    drop(waiter);

    let drain = match gate.request_drain() {
        Ok(drain) => drain,
        Err(_) => panic!("expected synthetic drain admission"),
    };
    assert_quiescent(&drain, false);
    assert!(release_tx.send(()).is_ok());
    assert!(completion_rx.recv_timeout(WAIT).is_ok());
    assert_quiescent(&drain, true);
}

#[test]
fn read_refusals_do_not_schedule_actions_when_capacity_or_drain_is_closed() {
    let gate = Gate::new();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Arc::new(Mutex::new(release_rx));
    let mut callbacks = Vec::with_capacity(64);
    for _ in 0..64 {
        let receiver = Arc::clone(&release_rx);
        callbacks.push(dispatch(&gate, move || {
            let result = match receiver.lock() {
                Ok(receiver) => receiver.recv_timeout(WAIT),
                Err(_) => panic!("synthetic release channel poisoned"),
            };
            assert!(result.is_ok());
        }));
    }

    let capacity_refusal_calls = Arc::new(AtomicUsize::new(0));
    let capacity_action_calls = Arc::clone(&capacity_refusal_calls);
    assert!(matches!(
        dispatch_read(&gate, move || {
            capacity_action_calls.fetch_add(1, Ordering::SeqCst);
        }),
        Err(AdmissionError::Busy)
    ));
    assert_eq!(capacity_refusal_calls.load(Ordering::SeqCst), 0);

    for _ in 0..64 {
        assert!(release_tx.send(()).is_ok());
    }
    for callback in callbacks {
        assert!(tauri::async_runtime::block_on(callback).is_ok());
    }

    let drain = match gate.request_drain() {
        Ok(drain) => drain,
        Err(_) => panic!("expected synthetic drain admission"),
    };
    let closed_refusal_calls = Arc::new(AtomicUsize::new(0));
    let closed_action_calls = Arc::clone(&closed_refusal_calls);
    assert!(matches!(
        dispatch_read(&gate, move || {
            closed_action_calls.fetch_add(1, Ordering::SeqCst);
        }),
        Err(AdmissionError::Unavailable)
    ));
    assert_eq!(closed_refusal_calls.load(Ordering::SeqCst), 0);
    assert_quiescent(&drain, true);
}

#[test]
fn panicking_read_callback_returns_join_error_and_releases_its_lane() {
    let gate = Gate::new();
    let callback = dispatch(&gate, || panic!("synthetic native callback panic"));
    assert!(tauri::async_runtime::block_on(callback).is_err());

    let drain = match gate.request_drain() {
        Ok(drain) => drain,
        Err(_) => panic!("expected drain after callback panic"),
    };
    assert_quiescent(&drain, true);
}

#[test]
fn factory_shares_model_exclusive_and_dispatched_read_custody() {
    let (gate, model) = admission_state();
    let waiter = Arc::new(match model.begin() {
        Ok(lease) => lease,
        Err(_) => panic!("expected synthetic model admission"),
    });
    let callback_owner = Arc::clone(&waiter);
    drop(waiter);
    let exclusive = begin_exclusive(&gate);

    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let read = dispatch(&gate, move || {
        assert!(started_tx.send(()).is_ok());
        assert!(release_rx.recv_timeout(WAIT).is_ok());
    });
    assert!(started_rx.recv_timeout(WAIT).is_ok());

    let drain = match gate.request_drain() {
        Ok(drain) => drain,
        Err(_) => panic!("expected synthetic drain admission"),
    };
    assert_quiescent(&drain, false);
    drop(exclusive);
    assert!(release_tx.send(()).is_ok());
    assert!(tauri::async_runtime::block_on(read).is_ok());
    assert_quiescent(&drain, false);
    drop(callback_owner);
    assert_quiescent(&drain, true);
    assert!(gate.resume(&drain).is_ok());
    assert!(model.begin().is_ok());
}
