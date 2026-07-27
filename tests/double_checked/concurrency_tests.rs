// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Concurrency tests for atomic gates and shared executor locks.

use std::{
    io,
    sync::{
        Arc,
        Barrier,
        atomic::{
            AtomicBool,
            AtomicUsize,
            Ordering,
        },
        mpsc,
    },
    thread,
    time::Duration,
};

use qubit_dcl::{
    DoubleCheckedLockExecutor,
    ExecutionOutcome,
};
use qubit_lock::{
    ReadWriteLock,
    TryLockError,
};

/// Verifies one executor can run read-only tasks under a shared mode and a
/// write task over different data under the paired exclusive mode.
#[test]
fn test_read_lock_allows_concurrent_read_only_tasks_and_excludes_writer() {
    const READER_COUNT: usize = 2;

    let gate = Arc::new(AtomicBool::new(true));
    let written_value = Arc::new(AtomicUsize::new(0));
    let (entered_sender, entered_receiver) = mpsc::channel();
    let release =
        Arc::new((parking_lot::Mutex::new(false), parking_lot::Condvar::new()));
    let lock = Arc::new(parking_lot::RwLock::new(()));
    let executor = Arc::new(
        DoubleCheckedLockExecutor::builder()
            .when({
                let gate = Arc::clone(&gate);
                move || gate.load(Ordering::Acquire)
            })
            .build(),
    );

    let handles = (0..READER_COUNT)
        .map(|_| {
            let entered_sender = entered_sender.clone();
            let executor = Arc::clone(&executor);
            let lock = Arc::clone(&lock);
            let release = Arc::clone(&release);
            thread::spawn(move || {
                let read_lock = ReadWriteLock::read_lock(lock.as_ref());
                executor.run(&read_lock, || {
                    entered_sender
                        .send(())
                        .expect("reader entry receiver should remain alive");
                    let (released, release_changed) = &*release;
                    let mut released = released.lock();
                    release_changed
                        .wait_while(&mut released, |released| !*released);
                    Ok::<(), io::Error>(())
                })
            })
        })
        .collect::<Vec<_>>();
    drop(entered_sender);

    let readers_overlapped = (0..READER_COUNT).all(|_| {
        entered_receiver
            .recv_timeout(Duration::from_secs(1))
            .is_ok()
    });
    let writer_was_excluded = matches!(
        ReadWriteLock::try_write(lock.as_ref()),
        Err(TryLockError::WouldBlock),
    );
    {
        let (released, release_changed) = &*release;
        *released.lock() = true;
        release_changed.notify_all();
    }

    for handle in handles {
        assert!(matches!(
            handle.join().expect("reader should not panic"),
            ExecutionOutcome::Success(()),
        ));
    }
    assert!(
        readers_overlapped,
        "both read-mode tasks should enter before either is released"
    );
    assert!(writer_was_excluded);

    let write_mode = ReadWriteLock::write_lock(lock.as_ref());
    let write_outcome = executor.run(&write_mode, {
        let gate = Arc::clone(&gate);
        let written_value = Arc::clone(&written_value);
        move || {
            written_value.store(43, Ordering::Release);
            gate.store(false, Ordering::Release);
            Ok::<(), io::Error>(())
        }
    });
    assert!(matches!(write_outcome, ExecutionOutcome::Success(())));
    assert_eq!(written_value.load(Ordering::Acquire), 43);
    assert!(!gate.load(Ordering::Acquire));
}

/// Verifies a task may change the gate while already holding the executor lock,
/// allowing only one competing task to succeed.
#[test]
fn test_task_changes_gate_inside_executor_lock() {
    const THREAD_COUNT: usize = 8;

    let gate = Arc::new(AtomicBool::new(true));
    let task_calls = Arc::new(AtomicUsize::new(0));
    let start = Arc::new(Barrier::new(THREAD_COUNT));
    let lock = Arc::new(parking_lot::Mutex::new(()));
    let executor = Arc::new(
        DoubleCheckedLockExecutor::builder()
            .when({
                let gate = Arc::clone(&gate);
                move || gate.load(Ordering::Acquire)
            })
            .build(),
    );

    let handles = (0..THREAD_COUNT)
        .map(|_| {
            let executor = Arc::clone(&executor);
            let gate = Arc::clone(&gate);
            let task_calls = Arc::clone(&task_calls);
            let start = Arc::clone(&start);
            let lock = Arc::clone(&lock);
            thread::spawn(move || {
                start.wait();
                executor.run(&lock, || {
                    task_calls.fetch_add(1, Ordering::Relaxed);
                    gate.store(false, Ordering::Release);
                    Ok::<(), io::Error>(())
                })
            })
        })
        .collect::<Vec<_>>();

    let success_count = handles
        .into_iter()
        .map(|handle| handle.join().expect("worker should not panic"))
        .filter(|outcome| matches!(outcome, ExecutionOutcome::Success(())))
        .count();

    assert_eq!(success_count, 1);
    assert_eq!(task_calls.load(Ordering::Relaxed), 1);
    assert!(!gate.load(Ordering::Acquire));
}

/// Verifies an external gate transition protected by the same lock is observed
/// by the waiting invocation's second check.
#[test]
fn test_external_gate_change_uses_same_underlying_lock() {
    let lock = Arc::new(parking_lot::Mutex::new(()));
    let gate = Arc::new(AtomicBool::new(true));
    let checks = Arc::new(AtomicUsize::new(0));
    let task_calls = Arc::new(AtomicUsize::new(0));
    let executor = DoubleCheckedLockExecutor::builder()
        .when({
            let gate = Arc::clone(&gate);
            let checks = Arc::clone(&checks);
            move || {
                checks.fetch_add(1, Ordering::Relaxed);
                gate.load(Ordering::Acquire)
            }
        })
        .build();

    let guard = lock.lock();
    let worker_task_calls = Arc::clone(&task_calls);
    let task_lock = Arc::clone(&lock);
    let handle = thread::spawn(move || {
        executor.run(&task_lock, || {
            worker_task_calls.fetch_add(1, Ordering::Relaxed);
            Ok::<(), io::Error>(())
        })
    });
    while checks.load(Ordering::Acquire) == 0 {
        thread::yield_now();
    }
    gate.store(false, Ordering::Release);
    drop(guard);

    let outcome = handle.join().expect("worker should not panic");
    assert!(matches!(outcome, ExecutionOutcome::ConditionNotMet));
    assert_eq!(checks.load(Ordering::Relaxed), 2);
    assert_eq!(task_calls.load(Ordering::Relaxed), 0);
}
