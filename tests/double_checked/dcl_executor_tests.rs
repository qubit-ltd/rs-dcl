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

use crate::support::{
    CountingLock,
    NoopLock,
    PanickingReleaseLock,
};
use qubit_dcl::{
    DclExecutor,
    ExecutionOutcome,
    PanicPhase,
};

mod parking_lot {
    pub use std::sync::Mutex;
}

/// Common task signature used to merge generic coverage across executor modes.
type CoverageTask = fn() -> Result<(), io::Error>;

/// Verifies the direct constructor stores a reusable predicate.
#[test]
fn test_new_accepts_predicate() {
    let executor = DclExecutor::new(|| true);

    let outcome =
        executor.run(&std::sync::Mutex::new(()), || Ok::<u32, io::Error>(7));

    assert!(matches!(outcome, ExecutionOutcome::Success(7)));
}

/// Completes successfully for the generic coverage matrix.
fn successful_coverage_task() -> Result<(), io::Error> {
    Ok(())
}

/// Panics for the generic coverage matrix.
fn panicking_coverage_task() -> Result<(), io::Error> {
    panic!("coverage task panic")
}

/// Exercises capturing and non-capturing paths through one `run` closure type.
#[test]
fn test_run_covers_propagating_and_catching_panic_paths() {
    let lock = NoopLock;
    let successful_task = successful_coverage_task as CoverageTask;
    let panicking_task = panicking_coverage_task as CoverageTask;

    let propagate_false = DclExecutor::new(|| false);
    assert!(matches!(
        propagate_false.run(&lock, successful_task),
        ExecutionOutcome::ConditionNotMet
    ));

    let catch_initial_panic =
        DclExecutor::new(|| panic!("coverage initial panic"));
    assert!(matches!(
        catch_initial_panic.run_catching(&lock, successful_task),
        Err(panic) if panic.phase() == PanicPhase::InitialConditionCheck
    ));

    let catch_task = DclExecutor::new(|| true);
    assert!(matches!(
        catch_task.run(&lock, successful_task),
        ExecutionOutcome::Success(())
    ));
    assert!(matches!(
        catch_task.run_catching(&lock, panicking_task),
        Err(panic) if panic.phase() == PanicPhase::Task
    ));

    let propagate_true = DclExecutor::new(|| true);
    assert!(matches!(
        propagate_true.run(&lock, successful_task),
        ExecutionOutcome::Success(())
    ));
}

/// Verifies the fast failure path performs no lock operation.
#[test]
fn test_run_initial_false_skips_lock_and_task() {
    let lock = CountingLock::new();
    let task_calls = AtomicUsize::new(0);
    let executor = DclExecutor::new(|| false);

    let outcome = executor.run(&lock, || {
        task_calls.fetch_add(1, Ordering::Relaxed);
        Ok::<(), io::Error>(())
    });

    assert!(matches!(outcome, ExecutionOutcome::ConditionNotMet));
    assert_eq!(lock.calls(), 0);
    assert_eq!(task_calls.load(Ordering::Relaxed), 0);
}

/// Verifies a captured initial-false fast path stays non-destructive.
#[test]
fn test_run_catching_initial_false_skips_lock_and_task() {
    let lock = CountingLock::new();
    let task_calls = AtomicUsize::new(0);
    let executor = DclExecutor::new(|| false);

    assert!(matches!(
        executor.run_catching(&lock, || {
            task_calls.fetch_add(1, Ordering::Relaxed);
            Ok::<(), io::Error>(())
        }),
        Ok(ExecutionOutcome::ConditionNotMet)
    ));
    assert_eq!(lock.calls(), 0);
    assert_eq!(task_calls.load(Ordering::Relaxed), 0);
}

/// Verifies a failed second check prevents task execution after one lock call.
#[test]
fn test_run_second_false_skips_task_after_locking() {
    let lock = CountingLock::new();
    let checks = Arc::new(AtomicUsize::new(0));
    let task_calls = AtomicUsize::new(0);
    let executor = DclExecutor::new({
        let checks = Arc::clone(&checks);
        move || checks.fetch_add(1, Ordering::Relaxed) == 0
    });

    let outcome = executor.run(&lock, || {
        task_calls.fetch_add(1, Ordering::Relaxed);
        Ok::<(), io::Error>(())
    });

    assert!(matches!(outcome, ExecutionOutcome::ConditionNotMet));
    assert_eq!(checks.load(Ordering::Relaxed), 2);
    assert_eq!(lock.calls(), 1);
    assert_eq!(task_calls.load(Ordering::Relaxed), 0);
}

/// Verifies a successful task runs once under the lock and preserves its
/// return value.
#[test]
fn test_run_two_true_checks_preserves_success() {
    let lock = CountingLock::new();
    let checks = Arc::new(AtomicUsize::new(0));
    let task_calls = AtomicUsize::new(0);
    let executor = DclExecutor::new({
        let checks = Arc::clone(&checks);
        move || {
            checks.fetch_add(1, Ordering::Relaxed);
            true
        }
    });

    let outcome = executor.run(&lock, || {
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
    let executor = DclExecutor::new(|| true);

    let outcome = executor.run(&parking_lot::Mutex::new(()), || {
        Err::<(), _>(io::Error::other("task failed"))
    });

    match outcome {
        ExecutionOutcome::TaskFailed(error) => {
            assert_eq!(error.to_string(), "task failed");
        }
        _ => panic!("expected task failure"),
    }
}

/// Verifies panic-aware execution preserves a successful task outcome.
#[test]
fn test_run_catching_preserves_success() {
    let executor = DclExecutor::new(|| true);

    let outcome = executor
        .run_catching(&parking_lot::Mutex::new(()), || Ok::<u32, io::Error>(7));

    assert!(matches!(outcome, Ok(ExecutionOutcome::Success(7))));
}

/// Verifies panic capture classifies the initial condition check.
#[test]
fn test_run_captures_initial_condition_panic() {
    let executor = DclExecutor::new(|| panic!("initial check"));

    let outcome = executor
        .run_catching(&parking_lot::Mutex::new(()), || Ok::<(), io::Error>(()));

    assert!(matches!(
        outcome,
        Err(panic) if panic.phase() == PanicPhase::InitialConditionCheck
            && panic.message() == Some("initial check")
    ));
}

/// Verifies panic capture classifies the second condition check.
#[test]
fn test_run_captures_second_condition_panic() {
    let checks = Arc::new(AtomicUsize::new(0));
    let executor = DclExecutor::new({
        let checks = Arc::clone(&checks);
        move || {
            if checks.fetch_add(1, Ordering::Relaxed) == 0 {
                true
            } else {
                panic!("second check")
            }
        }
    });

    let outcome = executor
        .run_catching(&parking_lot::Mutex::new(()), || Ok::<(), io::Error>(()));

    assert!(matches!(
        outcome,
        Err(panic) if panic.phase() == PanicPhase::SecondConditionCheck
            && panic.message() == Some("second check")
    ));
}

/// Verifies panic capture classifies a task panic after the second check.
#[test]
fn test_run_captures_task_panic() {
    let executor = DclExecutor::new(|| true);

    let outcome: Result<ExecutionOutcome<(), io::Error>, _> = executor
        .run_catching(&parking_lot::Mutex::new(()), || panic!("task panic"));

    assert!(matches!(
        outcome,
        Err(panic) if panic.phase() == PanicPhase::Task && panic.message() == Some("task panic")
    ));
}

/// Verifies a captured task unwind still poisons the underlying standard
/// mutex before it is converted into an outcome.
#[test]
fn test_captured_task_panic_preserves_standard_mutex_poisoning() {
    let lock = std::sync::Mutex::new(());
    let executor = DclExecutor::new(|| true);

    let first: Result<ExecutionOutcome<(), io::Error>, _> =
        executor.run_catching(&lock, || panic!("poison standard mutex"));
    assert!(matches!(
        first,
        Err(panic) if panic.phase() == PanicPhase::Task
    ));

    let second = executor.run_catching(&lock, || Ok::<u32, io::Error>(7));
    assert!(matches!(
        second,
        Err(panic) if panic.phase() == PanicPhase::LockAcquisition
    ));
}

/// Verifies a captured task unwind does not add poisoning to a parking-lot
/// mutex.
#[cfg(feature = "parking-lot")]
#[test]
fn test_captured_task_panic_preserves_parking_lot_non_poisoning() {
    let lock = ::parking_lot::Mutex::new(());
    let executor = DclExecutor::new(|| true);

    let first: Result<ExecutionOutcome<(), io::Error>, _> =
        executor.run_catching(&lock, || panic!("parking-lot task panic"));
    assert!(matches!(
        first,
        Err(panic) if panic.phase() == PanicPhase::Task
    ));

    let second = executor.run(&lock, || Ok::<u32, io::Error>(7));
    assert!(matches!(second, ExecutionOutcome::Success(7)));
}

/// Verifies a poisoned lock is classified as a lock-acquisition panic when
/// captured.
#[test]
fn test_run_captures_lock_acquisition_panic() {
    let lock = std::sync::Mutex::new(());
    let poison_result = catch_unwind(AssertUnwindSafe(|| {
        let _guard = std::sync::Mutex::lock(&lock).unwrap();
        panic!("poison lock");
    }));
    assert!(poison_result.is_err());
    let executor = DclExecutor::new(|| true);

    let outcome = executor.run_catching(&lock, || Ok::<(), io::Error>(()));

    assert!(matches!(
        outcome,
        Err(panic) if panic.phase() == PanicPhase::LockAcquisition
    ));
}

/// Verifies a guard-drop panic is classified as lock release rather than task
/// execution.
#[test]
fn test_run_captures_lock_release_panic() {
    let executor = DclExecutor::new(|| true);

    let outcome = executor
        .run_catching(&PanickingReleaseLock, || Ok::<u32, io::Error>(7));

    assert!(matches!(
        outcome,
        Err(panic) if panic.phase() == PanicPhase::LockRelease
            && panic.message() == Some("lock release panic")
    ));
}

/// Verifies disabling panic capture resumes unwinding through the caller.
#[test]
fn test_run_propagates_task_panic_without_catching() {
    let executor = DclExecutor::new(|| true);

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _: ExecutionOutcome<(), io::Error> = executor
            .run(&parking_lot::Mutex::new(()), || {
                panic!("uncaught task panic")
            });
    }));

    assert!(panic_result.is_err());
}

/// Verifies a built executor can be cloned independently of lock ownership.
#[test]
fn test_clone_shares_configuration_without_owning_lock() {
    let executor = DclExecutor::new(|| true);
    let cloned = executor.clone();

    let outcome =
        cloned.run(&parking_lot::Mutex::new(()), || Ok::<u32, io::Error>(7));

    assert!(matches!(outcome, ExecutionOutcome::Success(7)));
}
