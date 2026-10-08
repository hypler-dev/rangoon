use super::*;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Barrier, mpsc},
};

fn begin(gate: &Gate, lane: Lane) -> Lease {
    match gate.begin(lane) {
        Ok(lease) => lease,
        Err(_) => panic!("expected synthetic lane admission"),
    }
}

fn request_drain(gate: &Gate) -> Drain {
    match gate.request_drain() {
        Ok(drain) => drain,
        Err(_) => panic!("expected synthetic drain admission"),
    }
}

fn assert_current(lease: &Lease, expected: bool) {
    match lease.is_current() {
        Ok(actual) => assert_eq!(actual, expected),
        Err(_) => panic!("expected synthetic lease state"),
    }
}

fn assert_quiescent(drain: &Drain, expected: bool) {
    match drain.is_quiescent() {
        Ok(actual) => assert_eq!(actual, expected),
        Err(_) => panic!("expected synthetic drain state"),
    }
}

#[test]
fn readers_cap_at_sixty_four_and_coexist_with_other_lanes() {
    let gate = Gate::new();
    let mut readers = Vec::with_capacity(64);
    for _ in 0..64 {
        readers.push(begin(&gate, Lane::Read));
    }
    assert!(matches!(gate.begin(Lane::Read), Err(Error::Busy)));

    let exclusive = begin(&gate, Lane::Exclusive);
    let model = begin(&gate, Lane::Model);
    assert!(matches!(gate.begin(Lane::Exclusive), Err(Error::Busy)));
    assert!(matches!(gate.begin(Lane::Model), Err(Error::Busy)));

    assert_current(&exclusive, true);
    assert_current(&model, true);
    drop(readers);
    drop(exclusive);
    drop(model);
    assert!(gate.begin(Lane::Read).is_ok());
}

#[test]
fn dropping_one_reader_recovers_exactly_one_capacity_slot() {
    let gate = Gate::new();
    let mut readers = Vec::with_capacity(64);
    for _ in 0..64 {
        readers.push(begin(&gate, Lane::Read));
    }
    assert!(matches!(gate.begin(Lane::Read), Err(Error::Busy)));

    let reader = match readers.pop() {
        Some(reader) => reader,
        None => panic!("expected synthetic reader"),
    };
    drop(reader);
    assert!(gate.begin(Lane::Read).is_ok());
}

#[test]
fn drain_closes_all_lanes_invalidates_leases_and_waits_for_every_owner() {
    let gate = Gate::new();
    let read = begin(&gate, Lane::Read);
    let exclusive = begin(&gate, Lane::Exclusive);
    let model = begin(&gate, Lane::Model);

    let drain = request_drain(&gate);
    for lane in [Lane::Read, Lane::Exclusive, Lane::Model] {
        assert!(matches!(gate.begin(lane), Err(Error::Closed)));
    }
    assert_current(&read, false);
    assert_current(&exclusive, false);
    assert_current(&model, false);
    assert_quiescent(&drain, false);

    drop(read);
    assert_quiescent(&drain, false);
    drop(exclusive);
    assert_quiescent(&drain, false);
    drop(model);
    assert_quiescent(&drain, true);
}

#[test]
fn callback_arc_custody_keeps_drain_nonquiescent_after_waiter_drop() {
    let gate = Gate::new();
    let waiter = Arc::new(begin(&gate, Lane::Model));
    let callback_owner = Arc::clone(&waiter);
    drop(waiter);

    let drain = request_drain(&gate);
    assert_quiescent(&drain, false);
    drop(callback_owner);
    assert_quiescent(&drain, true);
}

#[test]
fn resume_refuses_active_wrong_and_reused_drains() {
    let gate = Gate::new();
    let read = begin(&gate, Lane::Read);
    let drain = request_drain(&gate);
    assert!(matches!(gate.resume(&drain), Err(Error::Busy)));
    drop(read);
    assert!(gate.resume(&drain).is_ok());
    assert!(matches!(gate.resume(&drain), Err(Error::Stale)));

    let other_gate = Gate::new();
    let other_drain = request_drain(&other_gate);
    assert!(matches!(gate.resume(&other_drain), Err(Error::Stale)));
}

#[test]
fn old_drain_is_stale_after_resume_and_a_new_drain() {
    let gate = Gate::new();
    let first = request_drain(&gate);
    assert!(gate.resume(&first).is_ok());
    let second = request_drain(&gate);

    assert!(matches!(first.is_quiescent(), Err(Error::Stale)));
    assert!(matches!(gate.resume(&first), Err(Error::Stale)));
    assert_quiescent(&second, true);
}

#[test]
fn second_drain_cannot_replace_an_active_drain() {
    let gate = Gate::new();
    let lease = begin(&gate, Lane::Model);
    let drain = request_drain(&gate);

    assert!(matches!(gate.request_drain(), Err(Error::Closed)));
    assert_quiescent(&drain, false);
    drop(lease);
    assert_quiescent(&drain, true);
    assert!(gate.resume(&drain).is_ok());
}

#[test]
fn dropping_a_drain_does_not_reopen_admission() {
    let gate = Gate::new();
    let drain = request_drain(&gate);
    drop(drain);
    for lane in [Lane::Read, Lane::Exclusive, Lane::Model] {
        assert!(matches!(gate.begin(lane), Err(Error::Closed)));
    }
}

#[test]
fn generation_overflow_closes_permanently_before_reopen() {
    let gate = Gate::new();
    assert!(gate.seed_generation(u64::MAX - 1).is_ok());
    let drain = request_drain(&gate);

    assert!(matches!(gate.resume(&drain), Err(Error::Exhausted)));
    assert!(matches!(gate.begin(Lane::Read), Err(Error::Closed)));
    assert!(matches!(gate.request_drain(), Err(Error::Exhausted)));
    assert!(matches!(gate.resume(&drain), Err(Error::Exhausted)));
    assert!(matches!(drain.is_quiescent(), Err(Error::Exhausted)));
}

#[test]
fn drain_overflow_invalidates_but_never_releases_an_active_lease() {
    let gate = Gate::new();
    assert!(gate.seed_generation(u64::MAX).is_ok());
    let lease = begin(&gate, Lane::Read);

    assert!(matches!(gate.request_drain(), Err(Error::Exhausted)));
    assert_current(&lease, false);
    assert!(matches!(gate.begin(Lane::Read), Err(Error::Closed)));

    drop(lease);
    assert!(matches!(gate.begin(Lane::Read), Err(Error::Closed)));
}

#[test]
fn wrong_gate_drain_is_stale_before_an_exhausted_destination_is_inspected() {
    let source = Gate::new();
    let source_drain = request_drain(&source);
    let exhausted_destination = Gate::new();
    assert!(exhausted_destination.seed_generation(u64::MAX).is_ok());
    assert!(matches!(
        exhausted_destination.request_drain(),
        Err(Error::Exhausted)
    ));

    assert!(matches!(
        exhausted_destination.resume(&source_drain),
        Err(Error::Stale)
    ));
}

#[test]
fn poisoned_gate_fails_closed_as_unavailable() {
    let gate = Gate::new();
    gate.poison_for_test();
    assert!(matches!(gate.begin(Lane::Read), Err(Error::Unavailable)));
    assert!(matches!(gate.request_drain(), Err(Error::Unavailable)));
}

#[test]
fn poison_stays_unavailable_with_retained_lease_and_drain_after_drop() {
    let gate = Gate::new();
    let lease = begin(&gate, Lane::Exclusive);
    let drain = request_drain(&gate);
    gate.poison_for_test();

    assert!(matches!(lease.is_current(), Err(Error::Unavailable)));
    assert!(matches!(drain.is_quiescent(), Err(Error::Unavailable)));
    assert!(matches!(gate.resume(&drain), Err(Error::Unavailable)));
    drop(lease);
    assert!(matches!(drain.is_quiescent(), Err(Error::Unavailable)));
    assert!(matches!(
        gate.begin(Lane::Exclusive),
        Err(Error::Unavailable)
    ));
}

#[test]
fn seed_generation_refuses_active_and_draining_states() {
    let gate = Gate::new();
    let lease = begin(&gate, Lane::Read);
    assert!(matches!(gate.seed_generation(9), Err(Error::Busy)));
    drop(lease);

    let drain = request_drain(&gate);
    assert!(matches!(gate.seed_generation(9), Err(Error::Busy)));
    assert_quiescent(&drain, true);
}

#[test]
fn coordinated_cross_thread_drain_closes_new_admission_until_callback_releases() {
    let gate = Arc::new(Gate::new());
    let barrier = Arc::new(Barrier::new(2));
    let (ready_tx, ready_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker_gate = Arc::clone(&gate);
    let worker_barrier = Arc::clone(&barrier);
    let worker = std::thread::spawn(move || {
        worker_barrier.wait();
        let callback_lease = begin(&worker_gate, Lane::Read);
        assert!(ready_tx.send(()).is_ok());
        assert!(release_rx.recv().is_ok());
        drop(callback_lease);
    });

    barrier.wait();
    assert!(ready_rx.recv().is_ok());
    let drain = request_drain(&gate);
    assert!(matches!(gate.begin(Lane::Exclusive), Err(Error::Closed)));
    assert_quiescent(&drain, false);

    assert!(release_tx.send(()).is_ok());
    assert!(worker.join().is_ok());
    assert_quiescent(&drain, true);
    assert!(gate.resume(&drain).is_ok());
    assert!(gate.begin(Lane::Exclusive).is_ok());
}

#[test]
fn unwinding_callback_releases_its_lane() {
    let gate = Gate::new();
    let unwind = catch_unwind(AssertUnwindSafe(|| {
        let lease = begin(&gate, Lane::Exclusive);
        assert_current(&lease, true);
        panic!("synthetic callback unwind");
    }));
    assert!(unwind.is_err());
    assert!(gate.begin(Lane::Exclusive).is_ok());
}
