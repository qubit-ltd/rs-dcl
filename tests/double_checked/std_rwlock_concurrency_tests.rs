// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Concurrency tests for the standard library read-write lock backend.

use std::io;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use qubit_dcl::DclExecutor;
use qubit_dcl::ExecutionOutcome;
use qubit_lock::ReadWriteLock;
use qubit_lock::TryLockError;

/// Verifies standard-library read modes overlap, exclude writers, and preserve
/// the executor's gate behavior after an exclusive task changes the gate.
#[test]
fn test_std_read_modes_overlap_and_exclude_writer() {
    const READER_COUNT: usize = 2;
    const WAIT_TIMEOUT: Duration = Duration::from_secs(5);

    let gate = Arc::new(AtomicBool::new(true));
    let task_calls = Arc::new(AtomicUsize::new(0));
    let written_value = Arc::new(AtomicUsize::new(0));
    let (entered_sender, entered_receiver) = mpsc::channel();
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let lock = Arc::new(RwLock::new(()));
    let executor = Arc::new(DclExecutor::new({
        let gate = Arc::clone(&gate);
        move || gate.load(Ordering::Acquire)
    }));

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
                    let released = released.lock().expect("release mutex should not be poisoned");
                    let (released, wait_result) = release_changed
                        .wait_timeout_while(released, WAIT_TIMEOUT, |released| !*released)
                        .expect("release mutex should not be poisoned");
                    if wait_result.timed_out() && !*released {
                        return Err(io::Error::new(io::ErrorKind::TimedOut, "reader release timed out"));
                    }
                    Ok::<(), io::Error>(())
                })
            })
        })
        .collect::<Vec<_>>();
    drop(entered_sender);

    let readers_overlapped = (0..READER_COUNT).all(|_| entered_receiver.recv_timeout(WAIT_TIMEOUT).is_ok());
    let writer_was_excluded = matches!(ReadWriteLock::try_write(lock.as_ref()), Err(TryLockError::WouldBlock),);
    {
        let (released, release_changed) = &*release;
        *released.lock().expect("release mutex should not be poisoned") = true;
        release_changed.notify_all();
    }

    let reader_outcomes = handles
        .into_iter()
        .map(|handle| handle.join().expect("reader should not panic"))
        .collect::<Vec<_>>();
    assert!(
        readers_overlapped,
        "both read-mode tasks should enter before either is released"
    );
    assert!(writer_was_excluded);
    assert!(
        reader_outcomes
            .iter()
            .all(|outcome| matches!(outcome, ExecutionOutcome::Success(())))
    );

    let write_mode = ReadWriteLock::write_lock(lock.as_ref());
    let write_outcome = executor.run(&write_mode, {
        let gate = Arc::clone(&gate);
        let task_calls = Arc::clone(&task_calls);
        let written_value = Arc::clone(&written_value);
        move || {
            task_calls.fetch_add(1, Ordering::Relaxed);
            written_value.store(43, Ordering::Release);
            gate.store(false, Ordering::Release);
            Ok::<(), io::Error>(())
        }
    });
    assert!(matches!(write_outcome, ExecutionOutcome::Success(())));
    assert_eq!(written_value.load(Ordering::Acquire), 43);
    assert!(!gate.load(Ordering::Acquire));

    let rejected_outcome = executor.run(&write_mode, || {
        task_calls.fetch_add(1, Ordering::Relaxed);
        Ok::<(), io::Error>(())
    });
    assert!(matches!(rejected_outcome, ExecutionOutcome::ConditionNotMet));
    assert_eq!(task_calls.load(Ordering::Relaxed), 1);
}
