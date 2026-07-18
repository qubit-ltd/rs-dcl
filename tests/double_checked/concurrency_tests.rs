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
    },
    thread,
};

use qubit_dcl::{
    DoubleCheckedLockExecutor,
    ExecutionOutcome,
};
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
