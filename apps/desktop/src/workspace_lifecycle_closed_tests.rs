use super::*;
use std::sync::{Arc, Barrier, mpsc};

fn begin_closed(gate: &Gate, drain: &Drain) -> ClosedLease {
    match gate.begin_closed(drain) {
        Ok(lease) => lease,
        Err(_) => panic!("expected closed-control custody"),
    }
}

fn assert_closed_current(lease: &ClosedLease, expected: bool) {
    match lease.is_current() {
        Ok(actual) => assert_eq!(actual, expected),
        Err(_) => panic!("expected closed-control state"),
    }
}

#[test]
fn closed_control_keeps_regular_lanes_closed() {
    let gate = Gate::new();
    let drain = request_drain(&gate);
    let control = begin_closed(&gate, &drain);

    for lane in [Lane::Read, Lane::Exclusive, Lane::Model] {
        assert!(matches!(gate.begin(lane), Err(Error::Closed)));
    }
    assert_closed_current(&control, true);
    assert!(matches!(gate.resume(&drain), Err(Error::Busy)));
    drop(control);
    assert_quiescent(&drain, true);
}

#[test]
fn closed_control_refuses_cross_gate_and_stale_drain_tokens() {
    let gate = Gate::new();
    let drain = request_drain(&gate);
    let other = Gate::new();
    let other_drain = request_drain(&other);

    assert!(matches!(gate.begin_closed(&other_drain), Err(Error::Stale)));
    let renewed = gate.renew_drain(&drain).unwrap();
    assert!(matches!(gate.begin_closed(&drain), Err(Error::Stale)));
    assert!(matches!(gate.begin_closed(&other_drain), Err(Error::Stale)));
    let control = begin_closed(&gate, &renewed);
    drop(control);
    assert!(gate.resume(&renewed).is_ok());
    assert!(matches!(gate.begin_closed(&renewed), Err(Error::Stale)));
}

#[test]
fn only_one_closed_control_can_hold_actual_custody() {
    let gate = Gate::new();
    let drain = request_drain(&gate);
    let control = begin_closed(&gate, &drain);

    assert!(matches!(gate.begin_closed(&drain), Err(Error::Busy)));
    assert_quiescent(&drain, false);
    drop(control);
    let next = begin_closed(&gate, &drain);
    drop(next);
    assert_quiescent(&drain, true);
}

#[test]
fn ordinary_callback_custody_blocks_closed_control_until_actual_drop() {
    let gate = Gate::new();
    let callback = begin(&gate, Lane::Model);
    let drain = request_drain(&gate);

    assert!(matches!(gate.begin_closed(&drain), Err(Error::Busy)));
    assert_quiescent(&drain, false);
    drop(callback);
    let control = begin_closed(&gate, &drain);
    drop(control);
    assert_quiescent(&drain, true);
}

#[test]
fn renewal_invalidates_old_control_without_releasing_its_actual_custody() {
    let gate = Gate::new();
    let drain = request_drain(&gate);
    let control = begin_closed(&gate, &drain);
    let renewed = gate.renew_drain(&drain).unwrap();

    assert_closed_current(&control, false);
    assert!(matches!(gate.begin_closed(&renewed), Err(Error::Busy)));
    assert!(matches!(gate.resume(&renewed), Err(Error::Busy)));
    assert_quiescent(&renewed, false);
    drop(control);
    assert_quiescent(&renewed, true);
    assert!(gate.resume(&renewed).is_ok());
}

#[test]
fn dropped_waiter_does_not_release_closed_callback_arc() {
    let gate = Gate::new();
    let drain = request_drain(&gate);
    let waiter = Arc::new(begin_closed(&gate, &drain));
    let callback = Arc::clone(&waiter);
    drop(waiter);

    assert_quiescent(&drain, false);
    assert!(matches!(gate.resume(&drain), Err(Error::Busy)));
    drop(callback);
    assert_quiescent(&drain, true);
}

#[test]
fn retained_recovery_plan_arc_keeps_renewed_drain_busy_until_drop() {
    let gate = Gate::new();
    let drain = request_drain(&gate);
    let prompt = Arc::new(begin_closed(&gate, &drain));
    let retained_recovery = Arc::clone(&prompt);
    drop(prompt);
    let renewed = gate.renew_drain(&drain).unwrap();

    assert!(matches!(gate.resume(&renewed), Err(Error::Busy)));
    assert!(matches!(gate.begin_closed(&renewed), Err(Error::Busy)));
    drop(retained_recovery);
    assert_quiescent(&renewed, true);
    assert!(gate.resume(&renewed).is_ok());
}

#[test]
fn successful_resume_stales_old_drain_and_closed_witnesses() {
    let gate = Gate::new();
    let drain = request_drain(&gate);
    let control = begin_closed(&gate, &drain);
    drop(control);

    assert!(gate.resume(&drain).is_ok());
    assert!(matches!(drain.is_quiescent(), Err(Error::Stale)));
    assert!(matches!(gate.resume(&drain), Err(Error::Stale)));
    let next = request_drain(&gate);
    let next_control = begin_closed(&gate, &next);
    assert_closed_current(&next_control, true);
}

#[test]
fn dropped_drain_never_reopens_closed_control_or_regular_admission() {
    let gate = Gate::new();
    let drain = request_drain(&gate);
    let control = begin_closed(&gate, &drain);
    drop(drain);

    for lane in [Lane::Read, Lane::Exclusive, Lane::Model] {
        assert!(matches!(gate.begin(lane), Err(Error::Closed)));
    }
    assert_closed_current(&control, true);
    drop(control);
    assert!(matches!(gate.begin(Lane::Read), Err(Error::Closed)));
}

#[test]
fn coordinated_closed_control_and_resume_race_preserves_actual_custody() {
    let gate = Arc::new(Gate::new());
    let drain = Arc::new(request_drain(&gate));
    let barrier = Arc::new(Barrier::new(2));
    let (attempt_tx, attempt_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker_gate = Arc::clone(&gate);
    let worker_drain = Arc::clone(&drain);
    let worker_barrier = Arc::clone(&barrier);
    let worker = std::thread::spawn(move || {
        worker_barrier.wait();
        match worker_gate.begin_closed(&worker_drain) {
            Ok(lease) => {
                assert!(attempt_tx.send(Ok(())).is_ok());
                assert!(release_rx.recv().is_ok());
                drop(lease);
            }
            Err(error) => {
                assert!(attempt_tx.send(Err(error)).is_ok());
                assert!(release_rx.recv().is_ok());
            }
        }
    });

    barrier.wait();
    let resume = gate.resume(&drain);
    let attempt = attempt_rx.recv().unwrap();
    match (attempt, resume) {
        (Ok(()), Err(Error::Busy)) => {
            for lane in [Lane::Read, Lane::Exclusive, Lane::Model] {
                assert!(matches!(gate.begin(lane), Err(Error::Closed)));
            }
            assert!(release_tx.send(()).is_ok());
            assert!(worker.join().is_ok());
            assert_quiescent(&drain, true);
            assert!(gate.resume(&drain).is_ok());
        }
        (Err(Error::Stale), Ok(())) => {
            assert!(release_tx.send(()).is_ok());
            assert!(worker.join().is_ok());
            assert!(gate.begin(Lane::Read).is_ok());
        }
        (attempt, resume) => panic!("unexpected closed/resume race: {attempt:?} {resume:?}"),
    }
}

#[test]
fn threaded_retained_control_stays_counted_after_waiter_drop_and_renewal() {
    let gate = Arc::new(Gate::new());
    let drain = request_drain(&gate);
    let waiter = Arc::new(begin_closed(&gate, &drain));
    let retained_callback = Arc::clone(&waiter);
    drop(waiter);
    let barrier = Arc::new(Barrier::new(2));
    let (held_tx, held_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (current_tx, current_rx) = mpsc::channel();
    let worker_barrier = Arc::clone(&barrier);
    let worker = std::thread::spawn(move || {
        worker_barrier.wait();
        assert!(held_tx.send(()).is_ok());
        assert!(release_rx.recv().is_ok());
        assert!(current_tx.send(retained_callback.is_current()).is_ok());
    });

    barrier.wait();
    assert!(held_rx.recv().is_ok());
    let renewed_once = gate.renew_drain(&drain).unwrap();
    let renewed = gate.renew_drain(&renewed_once).unwrap();
    assert!(matches!(drain.is_quiescent(), Err(Error::Stale)));
    assert!(matches!(renewed_once.is_quiescent(), Err(Error::Stale)));
    assert!(matches!(gate.resume(&drain), Err(Error::Stale)));
    assert!(matches!(gate.resume(&renewed_once), Err(Error::Stale)));
    assert!(matches!(gate.begin_closed(&drain), Err(Error::Stale)));
    assert!(matches!(
        gate.begin_closed(&renewed_once),
        Err(Error::Stale)
    ));
    assert!(matches!(gate.resume(&renewed), Err(Error::Busy)));
    assert!(matches!(gate.begin_closed(&renewed), Err(Error::Busy)));
    assert_eq!(current_rx.try_recv(), Err(mpsc::TryRecvError::Empty));
    assert!(release_tx.send(()).is_ok());
    assert_eq!(current_rx.recv().unwrap(), Ok(false));
    assert!(worker.join().is_ok());
    assert_quiescent(&renewed, true);
    assert!(gate.resume(&renewed).is_ok());
}

#[test]
fn renew_rejects_cross_gate_replayed_open_poisoned_and_exhausted_tokens() {
    let source = Gate::new();
    let source_drain = request_drain(&source);
    let poisoned_destination = Gate::new();
    let poisoned_drain = request_drain(&poisoned_destination);
    poisoned_destination.poison_for_test();
    assert!(matches!(
        poisoned_destination.renew_drain(&source_drain),
        Err(Error::Stale)
    ));
    assert!(matches!(
        poisoned_destination.renew_drain(&poisoned_drain),
        Err(Error::Unavailable)
    ));

    let gate = Gate::new();
    let first = request_drain(&gate);
    let second = gate.renew_drain(&first).unwrap();
    let third = gate.renew_drain(&second).unwrap();
    assert!(matches!(gate.renew_drain(&first), Err(Error::Stale)));
    assert!(matches!(gate.renew_drain(&second), Err(Error::Stale)));
    assert!(matches!(first.is_quiescent(), Err(Error::Stale)));
    assert!(matches!(second.is_quiescent(), Err(Error::Stale)));
    assert_quiescent(&third, true);
    assert!(gate.resume(&third).is_ok());
    assert!(matches!(gate.renew_drain(&third), Err(Error::Stale)));

    let exhausted = Gate::new();
    assert!(exhausted.seed_generation(u64::MAX - 1).is_ok());
    let drain = request_drain(&exhausted);
    assert!(matches!(
        exhausted.renew_drain(&drain),
        Err(Error::Exhausted)
    ));
    assert!(matches!(
        exhausted.begin_closed(&drain),
        Err(Error::Exhausted)
    ));
}

#[test]
fn renewal_retains_ordinary_lease_count_until_actual_callback_drop() {
    let gate = Gate::new();
    let callback = begin(&gate, Lane::Read);
    let drain = request_drain(&gate);
    let renewed = gate.renew_drain(&drain).unwrap();

    assert_current(&callback, false);
    assert!(matches!(drain.is_quiescent(), Err(Error::Stale)));
    assert_quiescent(&renewed, false);
    assert!(matches!(gate.begin_closed(&renewed), Err(Error::Busy)));
    assert!(matches!(gate.resume(&renewed), Err(Error::Busy)));
    drop(callback);
    assert_quiescent(&renewed, true);
    let control = begin_closed(&gate, &renewed);
    drop(control);
    assert!(gate.resume(&renewed).is_ok());
}

#[test]
fn closed_control_respects_poison_exhaustion_and_error_precedence() {
    let source = Gate::new();
    let source_drain = request_drain(&source);
    let poisoned = Gate::new();
    let poisoned_drain = request_drain(&poisoned);
    poisoned.poison_for_test();
    assert!(matches!(
        poisoned.begin_closed(&source_drain),
        Err(Error::Stale)
    ));
    assert!(matches!(
        poisoned.begin_closed(&poisoned_drain),
        Err(Error::Unavailable)
    ));

    let exhausted = Gate::new();
    assert!(exhausted.seed_generation(u64::MAX - 1).is_ok());
    let drain = request_drain(&exhausted);
    let control = begin_closed(&exhausted, &drain);
    assert!(matches!(
        exhausted.renew_drain(&drain),
        Err(Error::Exhausted)
    ));
    assert_closed_current(&control, false);
    assert!(matches!(drain.is_quiescent(), Err(Error::Exhausted)));
}
