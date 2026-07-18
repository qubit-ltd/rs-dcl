// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the basic double-checked lock executor.

use std::{
    io,
    panic::{
        AssertUnwindSafe,
        catch_unwind,
    },
    sync::{
        Arc,
        atomic::{
            AtomicUsize,
            Ordering,
        },
    },
};

use qubit_dcl::{
    DoubleCheckedLockExecutor,
    ExecutionOutcome,
    PanicPhase,
};
use qubit_lock::{
    ArcMutex,
    ArcStdMutex,
    Lock,
};

use crate::support::CountingLock;

/// Verifies the fast failure path performs no lock operation.
#[test]
fn test_run_initial_false_skips_lock_and_task() {
    let lock = CountingLock::new(());
    let task_calls = AtomicUsize::new(0);
    let executor = DoubleCheckedLockExecutor::builder(lock.clone())
        .when(|| false)
        .build();

    let outcome = executor.run(|| {
        task_calls.fetch_add(1, Ordering::Relaxed);
        Ok::<(), io::Error>(())
    });

    assert!(matches!(outcome, ExecutionOutcome::ConditionNotMet));
    assert_eq!(lock.calls(), 0);
    assert_eq!(task_calls.load(Ordering::Relaxed), 0);
}

/// Verifies a failed second check prevents task execution after one write-lock
/// call.
#[test]
fn test_run_second_false_skips_task_after_locking() {
    let lock = CountingLock::new(());
    let checks = Arc::new(AtomicUsize::new(0));
    let task_calls = AtomicUsize::new(0);
    let executor = DoubleCheckedLockExecutor::builder(lock.clone())
        .when({
            let checks = Arc::clone(&checks);
            move || checks.fetch_add(1, Ordering::Relaxed) == 0
        })
        .build();

    let outcome = executor.run(|| {
        task_calls.fetch_add(1, Ordering::Relaxed);
        Ok::<(), io::Error>(())
    });

    assert!(matches!(outcome, ExecutionOutcome::ConditionNotMet));
    assert_eq!(checks.load(Ordering::Relaxed), 2);
    assert_eq!(lock.calls(), 1);
    assert_eq!(task_calls.load(Ordering::Relaxed), 0);
}

/// Verifies a successful task runs once under the write lock and preserves its
/// return value.
#[test]
fn test_run_two_true_checks_preserves_success() {
    let lock = CountingLock::new(());
    let checks = Arc::new(AtomicUsize::new(0));
    let task_calls = AtomicUsize::new(0);
    let executor = DoubleCheckedLockExecutor::builder(lock.clone())
        .when({
            let checks = Arc::clone(&checks);
            move || {
                checks.fetch_add(1, Ordering::Relaxed);
                true
            }
        })
        .build();

    let outcome = executor.run(|| {
        task_calls.fetch_add(1, Ordering::Relaxed);
        Ok::<u32, io::Error>(42)
    });

    assert!(matches!(outcome, ExecutionOutcome::Success(42)));
    assert_eq!(checks.load(Ordering::Relaxed), 2);
    assert_eq!(lock.calls(), 1);
    assert_eq!(task_calls.load(Ordering::Relaxed), 1);
}

/// Verifies a task error remains owned and unchanged.
#[test]
fn test_run_preserves_task_error() {
    let executor = DoubleCheckedLockExecutor::builder(ArcMutex::new(()))
        .when(|| true)
        .build();

    let outcome =
        executor.run(|| Err::<(), _>(io::Error::other("task failed")));

    match outcome {
        ExecutionOutcome::TaskFailed(error) => {
            assert_eq!(error.to_string(), "task failed");
        }
        _ => panic!("expected task failure"),
    }
}

/// Verifies panic capture classifies the initial condition check.
#[test]
fn test_run_captures_initial_condition_panic() {
    let executor = DoubleCheckedLockExecutor::builder(ArcMutex::new(()))
        .when(|| panic!("initial check"))
        .catch_panics(true)
        .build();

    let outcome = executor.run(|| Ok::<(), io::Error>(()));

    match outcome {
        ExecutionOutcome::Panicked(panic) => {
            assert_eq!(panic.phase(), PanicPhase::InitialConditionCheck);
            assert_eq!(panic.message(), Some("initial check"));
        }
        _ => panic!("expected captured initial-condition panic"),
    }
}

/// Verifies panic capture classifies the second condition check.
#[test]
fn test_run_captures_second_condition_panic() {
    let checks = Arc::new(AtomicUsize::new(0));
    let executor = DoubleCheckedLockExecutor::builder(ArcMutex::new(()))
        .when({
            let checks = Arc::clone(&checks);
            move || {
                if checks.fetch_add(1, Ordering::Relaxed) == 0 {
                    true
                } else {
                    panic!("second check")
                }
            }
        })
        .catch_panics(true)
        .build();

    let outcome = executor.run(|| Ok::<(), io::Error>(()));

    match outcome {
        ExecutionOutcome::Panicked(panic) => {
            assert_eq!(panic.phase(), PanicPhase::SecondConditionCheck);
            assert_eq!(panic.message(), Some("second check"));
        }
        _ => panic!("expected captured second-condition panic"),
    }
}

/// Verifies panic capture classifies a task panic after the second check.
#[test]
fn test_run_captures_task_panic() {
    let executor = DoubleCheckedLockExecutor::builder(ArcMutex::new(()))
        .when(|| true)
        .catch_panics(true)
        .build();

    let outcome: ExecutionOutcome<(), io::Error> =
        executor.run(|| panic!("task panic"));

    match outcome {
        ExecutionOutcome::Panicked(panic) => {
            assert_eq!(panic.phase(), PanicPhase::Task);
            assert_eq!(panic.message(), Some("task panic"));
        }
        _ => panic!("expected captured task panic"),
    }
}

/// Verifies a captured task unwind still poisons the underlying standard
/// mutex before it is converted into an outcome.
#[test]
fn test_captured_task_panic_preserves_standard_mutex_poisoning() {
    let executor = DoubleCheckedLockExecutor::builder(ArcStdMutex::new(()))
        .when(|| true)
        .catch_panics(true)
        .build();

    let first: ExecutionOutcome<(), io::Error> =
        executor.run(|| panic!("poison standard mutex"));
    assert!(matches!(
        first,
        ExecutionOutcome::Panicked(panic) if panic.phase() == PanicPhase::Task
    ));

    let second = executor.run(|| Ok::<u32, io::Error>(7));
    assert!(matches!(
        second,
        ExecutionOutcome::Panicked(panic)
            if panic.phase() == PanicPhase::LockAcquisition
    ));
}

/// Verifies a captured task unwind does not add poisoning to a parking-lot
/// mutex.
#[test]
fn test_captured_task_panic_preserves_parking_lot_non_poisoning() {
    let executor = DoubleCheckedLockExecutor::builder(ArcMutex::new(()))
        .when(|| true)
        .catch_panics(true)
        .build();

    let first: ExecutionOutcome<(), io::Error> =
        executor.run(|| panic!("parking-lot task panic"));
    assert!(matches!(
        first,
        ExecutionOutcome::Panicked(panic) if panic.phase() == PanicPhase::Task
    ));

    let second = executor.run(|| Ok::<u32, io::Error>(7));
    assert!(matches!(second, ExecutionOutcome::Success(7)));
}

/// Verifies a poisoned standard lock is classified as a lock-acquisition
/// panic.
#[test]
fn test_run_captures_lock_acquisition_panic() {
    let lock = ArcStdMutex::new(());
    let poison_lock = lock.clone();
    let poison_result = catch_unwind(AssertUnwindSafe(|| {
        poison_lock.with_write(|_| panic!("poison lock"));
    }));
    assert!(poison_result.is_err());
    let executor = DoubleCheckedLockExecutor::builder(lock)
        .when(|| true)
        .catch_panics(true)
        .build();

    let outcome = executor.run(|| Ok::<(), io::Error>(()));

    match outcome {
        ExecutionOutcome::Panicked(panic) => {
            assert_eq!(panic.phase(), PanicPhase::LockAcquisition);
        }
        _ => panic!("expected captured lock-acquisition panic"),
    }
}

/// Verifies disabling panic capture resumes unwinding through the caller.
#[test]
fn test_run_propagates_task_panic_when_capture_is_disabled() {
    let executor = DoubleCheckedLockExecutor::builder(ArcMutex::new(()))
        .when(|| true)
        .catch_panics(false)
        .build();

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _: ExecutionOutcome<(), io::Error> =
            executor.run(|| panic!("uncaught task panic"));
    }));

    assert!(panic_result.is_err());
}

/// Verifies a built executor can be cloned when its lock handle is cloneable.
#[test]
fn test_clone_shares_configuration_and_lock() {
    let executor = DoubleCheckedLockExecutor::builder(ArcMutex::new(()))
        .when(|| true)
        .build();
    let cloned = executor.clone();

    let outcome = cloned.run(|| Ok::<u32, io::Error>(7));

    assert!(matches!(outcome, ExecutionOutcome::Success(7)));
}
