use super::*;
use std::sync::{Arc, Barrier, mpsc};

fn observe(gate: &Gate) -> GenerationWitness {
    match gate.observe_open() {
        Ok(witness) => witness,
        Err(_) => panic!("expected open generation observation"),
    }
}

fn begin_for(gate: &Gate, witness: &GenerationWitness, lane: Lane) -> Lease {
    match gate.begin_for(witness, lane) {
        Ok(lease) => lease,
        Err(_) => panic!("expected caller-bound admission"),
    }
}

fn drain(gate: &Gate) -> Drain {
    match gate.request_drain() {
        Ok(drain) => drain,
        Err(_) => panic!("expected drain"),
    }
}

#[test]
fn observing_open_generation_is_count_free_and_nonowning() {
    let gate = Gate::new();
    let witness = observe(&gate);
    let drain = drain(&gate);
    assert!(gate.resume(&drain).is_ok());
    assert_eq!(witness.is_current(), Ok(false));
    assert!(matches!(
        gate.begin_for(&witness, Lane::Read),
        Err(Error::Stale)
    ));

    let fresh = observe(&gate);
    let lease = begin_for(&gate, &fresh, Lane::Read);
    assert_eq!(lease.is_current(), Ok(true));
    drop(lease);
}

#[test]
fn queued_old_observation_rejects_after_resume_and_fresh_observations_admit_all_lanes() {
    let gate = Gate::new();
    let old = observe(&gate);
    let closing = drain(&gate);
    assert!(gate.resume(&closing).is_ok());
    for lane in [Lane::Read, Lane::Exclusive, Lane::Model] {
        assert!(matches!(gate.begin_for(&old, lane), Err(Error::Stale)));
        let fresh = observe(&gate);
        let lease = begin_for(&gate, &fresh, lane);
        assert_eq!(lease.is_current(), Ok(true));
        drop(lease);
    }
}

#[test]
fn begin_for_precedence_is_foreign_then_poison_then_closed_then_stale_then_capacity() {
    let foreign = Gate::new();
    let witness = observe(&foreign);
    let poisoned_destination = Gate::new();
    poisoned_destination.poison_for_test();
    assert!(matches!(
        poisoned_destination.begin_for(&witness, Lane::Read),
        Err(Error::Stale)
    ));

    let poisoned = Gate::new();
    let poisoned_witness = observe(&poisoned);
    poisoned.poison_for_test();
    assert!(matches!(
        poisoned.begin_for(&poisoned_witness, Lane::Read),
        Err(Error::Unavailable)
    ));

    let closed = Gate::new();
    let closed_witness = observe(&closed);
    let closing = drain(&closed);
    assert!(matches!(
        closed.begin_for(&closed_witness, Lane::Read),
        Err(Error::Closed)
    ));
    assert!(closed.resume(&closing).is_ok());
    assert!(matches!(
        closed.begin_for(&closed_witness, Lane::Read),
        Err(Error::Stale)
    ));

    let capacity = Gate::new();
    let witness = observe(&capacity);
    let mut readers = Vec::new();
    for _ in 0..64 {
        readers.push(begin_for(&capacity, &witness, Lane::Read));
    }
    assert!(matches!(
        capacity.begin_for(&witness, Lane::Read),
        Err(Error::Busy)
    ));
    drop(readers);
    for lane in [Lane::Exclusive, Lane::Model] {
        let first = begin_for(&capacity, &witness, lane);
        assert!(matches!(
            capacity.begin_for(&witness, lane),
            Err(Error::Busy)
        ));
        drop(first);
    }
}

#[test]
fn stale_and_failed_caller_admission_never_consume_capacity() {
    let gate = Gate::new();
    let old = observe(&gate);
    let closing = drain(&gate);
    assert!(gate.resume(&closing).is_ok());
    assert!(matches!(
        gate.begin_for(&old, Lane::Exclusive),
        Err(Error::Stale)
    ));

    let fresh = observe(&gate);
    let first = begin_for(&gate, &fresh, Lane::Exclusive);
    assert!(matches!(
        gate.begin_for(&fresh, Lane::Exclusive),
        Err(Error::Busy)
    ));
    drop(first);
    let second = begin_for(&gate, &fresh, Lane::Exclusive);
    drop(second);
}

#[test]
fn overflow_closes_begin_for_on_every_lane_without_reopening_observation() {
    let gate = Gate::new();
    assert!(gate.seed_generation(u64::MAX - 1).is_ok());
    let witness = observe(&gate);
    let closing = drain(&gate);
    assert!(matches!(gate.resume(&closing), Err(Error::Exhausted)));
    assert!(matches!(gate.observe_open(), Err(Error::Closed)));
    for lane in [Lane::Read, Lane::Exclusive, Lane::Model] {
        assert!(matches!(gate.begin_for(&witness, lane), Err(Error::Closed)));
    }
}

#[test]
fn begin_for_race_cannot_turn_an_old_request_into_new_generation_admission() {
    let gate = Arc::new(Gate::new());
    let old = observe(&gate);
    let barrier = Arc::new(Barrier::new(2));
    let (result_tx, result_rx) = mpsc::channel();
    let worker_gate = Arc::clone(&gate);
    let worker_barrier = Arc::clone(&barrier);
    let worker = std::thread::spawn(move || {
        worker_barrier.wait();
        let result = worker_gate.begin_for(&old, Lane::Model);
        assert!(result_tx.send((old, result)).is_ok());
    });

    barrier.wait();
    let closing = drain(&gate);
    let (old, result) = result_rx.recv().unwrap();
    match result {
        Ok(lease) => {
            assert!(matches!(gate.resume(&closing), Err(Error::Busy)));
            drop(lease);
            assert!(gate.resume(&closing).is_ok());
        }
        Err(Error::Closed) => assert!(gate.resume(&closing).is_ok()),
        Err(_) => panic!("old caller had an unexpected admission result"),
    }
    assert!(worker.join().is_ok());
    assert!(matches!(
        gate.begin_for(&old, Lane::Model),
        Err(Error::Stale)
    ));

    let fresh = observe(&gate);
    assert!(gate.begin_for(&fresh, Lane::Model).is_ok());
}
