// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the lifecycle-aware DCL executor.

use std::{
    error::Error,
    fmt,
    io,
    panic::{
        AssertUnwindSafe,
        catch_unwind,
    },
    sync::{
        Arc,
        Barrier,
        Mutex,
        atomic::{
            AtomicBool,
            AtomicUsize,
            Ordering,
        },
    },
    thread,
};

use parking_lot::Mutex as ParkingLotMutex;
use qubit_dcl::{
    FinalizationOutcome,
    LifecycleDoubleCheckedLockExecutor,
    LifecycleOutcome,
    PanicPhase,
    RollbackCause,
};

use crate::support::{
    PanicOnDrop,
    PanickingReleaseLock,
};

/// Rollback error whose destructor panics to exercise panic-priority handling.
#[derive(Debug)]
struct PanicOnDropRollbackError;

impl fmt::Display for PanicOnDropRollbackError {
    /// Formats the error for the standard error trait.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("secondary rollback error")
    }
}

impl Error for PanicOnDropRollbackError {}

impl Drop for PanicOnDropRollbackError {
    /// Panics to simulate an error whose destructor is not unwind-safe.
    fn drop(&mut self) {
        panic!("secondary rollback error drop panic");
    }
}

/// Verifies a failed initial check bypasses every lifecycle callback and lock.
#[test]
fn test_run_initial_false_does_not_prepare_or_finalize() {
    let prepare_calls = Arc::new(AtomicUsize::new(0));
    let commit_calls = Arc::new(AtomicUsize::new(0));
    let rollback_calls = Arc::new(AtomicUsize::new(0));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| false)
        .prepare({
            let prepare_calls = Arc::clone(&prepare_calls);
            move || {
                prepare_calls.fetch_add(1, Ordering::Relaxed);
                Ok::<(), io::Error>(())
            }
        })
        .commit({
            let commit_calls = Arc::clone(&commit_calls);
            move |_| {
                commit_calls.fetch_add(1, Ordering::Relaxed);
                Ok::<(), io::Error>(())
            }
        })
        .rollback({
            let rollback_calls = Arc::clone(&rollback_calls);
            move |_, _| {
                rollback_calls.fetch_add(1, Ordering::Relaxed);
                Ok::<(), io::Error>(())
            }
        })
        .build();

    let outcome =
        executor.run(&parking_lot::Mutex::new(()), || Ok::<(), io::Error>(()));

    assert!(matches!(outcome, LifecycleOutcome::InitialConditionNotMet));
    assert_eq!(prepare_calls.load(Ordering::Relaxed), 0);
    assert_eq!(commit_calls.load(Ordering::Relaxed), 0);
    assert_eq!(rollback_calls.load(Ordering::Relaxed), 0);
}

/// Verifies a prepare error preserves its error and never calls rollback.
#[test]
fn test_run_prepare_error_does_not_rollback_without_token() {
    let rollback_calls = Arc::new(AtomicUsize::new(0));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare(|| Err::<u32, _>(io::Error::other("prepare failed")))
        .no_commit()
        .rollback({
            let rollback_calls = Arc::clone(&rollback_calls);
            move |_, _| {
                rollback_calls.fetch_add(1, Ordering::Relaxed);
                Ok::<(), io::Error>(())
            }
        })
        .build();

    let outcome =
        executor.run(&parking_lot::Mutex::new(()), || Ok::<(), io::Error>(()));

    match outcome {
        LifecycleOutcome::PrepareFailed(error) => {
            assert_eq!(error.to_string(), "prepare failed");
        }
        _ => panic!("expected prepare failure"),
    }
    assert_eq!(rollback_calls.load(Ordering::Relaxed), 0);
}

/// Verifies a captured initial predicate panic does not start preparation.
#[test]
fn test_run_captures_initial_predicate_panic_before_prepare() {
    let prepare_calls = Arc::new(AtomicUsize::new(0));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| panic!("initial predicate panic"))
        .catch_panics(true)
        .prepare({
            let prepare_calls = Arc::clone(&prepare_calls);
            move || {
                prepare_calls.fetch_add(1, Ordering::Relaxed);
                Ok::<(), io::Error>(())
            }
        })
        .no_commit()
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();

    let outcome =
        executor.run(&parking_lot::Mutex::new(()), || Ok::<(), io::Error>(()));

    assert!(matches!(
        outcome,
        LifecycleOutcome::InitialConditionCheckPanicked(panic)
            if panic.phase() == PanicPhase::InitialConditionCheck
    ));
    assert_eq!(prepare_calls.load(Ordering::Relaxed), 0);
}

/// Verifies a captured prepare panic reports no execution and cannot roll back
/// because no token was produced.
#[test]
fn test_run_captures_prepare_panic_without_rollback() {
    let rollback_calls = Arc::new(AtomicUsize::new(0));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(true)
        .prepare(|| -> Result<(), io::Error> { panic!("prepare panic") })
        .no_commit()
        .rollback({
            let rollback_calls = Arc::clone(&rollback_calls);
            move |_, _| {
                rollback_calls.fetch_add(1, Ordering::Relaxed);
                Ok::<(), io::Error>(())
            }
        })
        .build();

    let outcome =
        executor.run(&parking_lot::Mutex::new(()), || Ok::<(), io::Error>(()));

    assert!(matches!(
        outcome,
        LifecycleOutcome::PreparePanicked(panic)
            if panic.phase() == PanicPhase::Prepare
    ));
    assert_eq!(rollback_calls.load(Ordering::Relaxed), 0);
}

/// Verifies the capturing state machine retains the outer-false fast exit.
#[test]
fn test_run_catching_initial_false_does_not_prepare() {
    let prepare_calls = Arc::new(AtomicUsize::new(0));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| false)
        .catch_panics(true)
        .prepare({
            let prepare_calls = Arc::clone(&prepare_calls);
            move || {
                prepare_calls.fetch_add(1, Ordering::Relaxed);
                Ok::<(), io::Error>(())
            }
        })
        .no_commit()
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();

    let outcome =
        executor.run(&parking_lot::Mutex::new(()), || Ok::<(), io::Error>(()));

    assert!(matches!(outcome, LifecycleOutcome::InitialConditionNotMet));
    assert_eq!(prepare_calls.load(Ordering::Relaxed), 0);
}

/// Verifies the capturing state machine retains a returned prepare error.
#[test]
fn test_run_catching_prepare_error_preserves_lifecycle_error() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(true)
        .prepare(|| Err::<(), _>(io::Error::other("prepare failed")))
        .no_commit()
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();

    let outcome =
        executor.run(&parking_lot::Mutex::new(()), || Ok::<(), io::Error>(()));

    assert!(matches!(
        outcome,
        LifecycleOutcome::PrepareFailed(error)
            if error.to_string() == "prepare failed"
    ));
}

/// Verifies a failed second check rolls back its token after the lock is
/// released.
#[test]
fn test_run_second_false_rolls_back_after_unlock() {
    let lock = Arc::new(ParkingLotMutex::new(()));
    let checks = Arc::new(AtomicUsize::new(0));
    let rolled_back_token = Arc::new(AtomicUsize::new(0));
    let saw_condition_cause = Arc::new(AtomicBool::new(false));
    let rollback_obtained_lock = Arc::new(AtomicBool::new(false));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when({
            let checks = Arc::clone(&checks);
            move || checks.fetch_add(1, Ordering::Relaxed) == 0
        })
        .prepare(|| Ok::<usize, io::Error>(17))
        .no_commit()
        .rollback({
            let lock = lock.clone();
            let rolled_back_token = Arc::clone(&rolled_back_token);
            let saw_condition_cause = Arc::clone(&saw_condition_cause);
            let rollback_obtained_lock = Arc::clone(&rollback_obtained_lock);
            move |token, cause| {
                rolled_back_token.store(token, Ordering::Relaxed);
                saw_condition_cause.store(
                    matches!(cause, RollbackCause::ConditionNotMet),
                    Ordering::Relaxed,
                );
                rollback_obtained_lock
                    .store(lock.try_lock().is_some(), Ordering::Relaxed);
                Ok::<(), io::Error>(())
            }
        })
        .build();

    let outcome = executor.run(&lock, || Ok::<(), io::Error>(()));

    assert!(matches!(
        outcome,
        LifecycleOutcome::SecondConditionNotMet {
            rollback: FinalizationOutcome::Succeeded,
        }
    ));
    assert_eq!(rolled_back_token.load(Ordering::Relaxed), 17);
    assert!(saw_condition_cause.load(Ordering::Relaxed));
    assert!(rollback_obtained_lock.load(Ordering::Relaxed));
}

/// Verifies a captured second predicate panic releases the lock and supplies
/// its precise phase to rollback.
#[test]
fn test_run_captures_second_predicate_panic_then_rolls_back() {
    let checks = Arc::new(AtomicUsize::new(0));
    let rollback_phase = Arc::new(Mutex::new(None));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when({
            let checks = Arc::clone(&checks);
            move || {
                if checks.fetch_add(1, Ordering::Relaxed) == 0 {
                    true
                } else {
                    panic!("second predicate panic")
                }
            }
        })
        .catch_panics(true)
        .prepare(|| Ok::<(), io::Error>(()))
        .no_commit()
        .rollback({
            let rollback_phase = Arc::clone(&rollback_phase);
            move |_, cause| {
                let RollbackCause::Panicked(panic) = cause else {
                    panic!("expected panic rollback cause");
                };
                *rollback_phase
                    .lock()
                    .expect("rollback phase mutex should not be poisoned") =
                    Some(panic.phase());
                Ok::<(), io::Error>(())
            }
        })
        .build();

    let outcome =
        executor.run(&parking_lot::Mutex::new(()), || Ok::<(), io::Error>(()));

    assert!(matches!(
        outcome,
        LifecycleOutcome::ExecutionPanicked {
            panic,
            rollback: FinalizationOutcome::Succeeded,
        }
            if panic.phase() == PanicPhase::SecondConditionCheck
    ));
    assert_eq!(
        *rollback_phase
            .lock()
            .expect("rollback phase mutex should not be poisoned"),
        Some(PanicPhase::SecondConditionCheck)
    );
}

/// Verifies a poisoned lock is classified during acquisition and the prepared
/// token is rolled back after the failed locked phase.
#[test]
fn test_run_captures_poisoned_lock_acquisition_then_rolls_back() {
    let lock = Arc::new(Mutex::new(()));
    let poison_lock = lock.clone();
    let poison_result = catch_unwind(AssertUnwindSafe(|| {
        let _guard = poison_lock
            .lock()
            .expect("standard mutex should start unpoisoned");
        panic!("poison lifecycle lock");
    }));
    assert!(poison_result.is_err());
    let rollback_phase = Arc::new(Mutex::new(None));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(true)
        .prepare(|| Ok::<(), io::Error>(()))
        .no_commit()
        .rollback({
            let rollback_phase = Arc::clone(&rollback_phase);
            move |_, cause| {
                let RollbackCause::Panicked(panic) = cause else {
                    panic!("expected panic rollback cause");
                };
                *rollback_phase
                    .lock()
                    .expect("rollback phase mutex should not be poisoned") =
                    Some(panic.phase());
                Ok::<(), io::Error>(())
            }
        })
        .build();

    let outcome = executor.run(&lock, || Ok::<(), io::Error>(()));

    assert!(matches!(
        outcome,
        LifecycleOutcome::ExecutionPanicked {
            panic,
            rollback: FinalizationOutcome::Succeeded,
        }
            if panic.phase() == PanicPhase::LockAcquisition
    ));
    assert_eq!(
        *rollback_phase
            .lock()
            .expect("rollback phase mutex should not be poisoned"),
        Some(PanicPhase::LockAcquisition)
    );
}

/// Verifies a captured guard-drop panic is supplied to rollback with its lock
/// release phase.
#[test]
fn test_run_captures_lock_release_panic_then_rolls_back() {
    let rollback_phase = Arc::new(Mutex::new(None));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(true)
        .prepare(|| Ok::<(), io::Error>(()))
        .no_commit()
        .rollback({
            let rollback_phase = Arc::clone(&rollback_phase);
            move |_, cause| {
                let RollbackCause::Panicked(panic) = cause else {
                    panic!("expected panic rollback cause");
                };
                *rollback_phase
                    .lock()
                    .expect("rollback phase mutex should not be poisoned") =
                    Some(panic.phase());
                Ok::<(), io::Error>(())
            }
        })
        .build();

    let outcome =
        executor.run(&PanickingReleaseLock, || Ok::<u32, io::Error>(7));

    assert!(matches!(
        outcome,
        LifecycleOutcome::ExecutionPanicked {
            panic,
            rollback: FinalizationOutcome::Succeeded,
        } if panic.phase() == PanicPhase::LockRelease
    ));
    assert_eq!(
        rollback_phase
            .lock()
            .expect("rollback phase mutex should not be poisoned")
            .as_ref(),
        Some(&PanicPhase::LockRelease)
    );
}

/// Verifies task success commits the token after releasing the executor lock.
#[test]
fn test_run_success_commits_after_unlock() {
    let lock = Arc::new(ParkingLotMutex::new(()));
    let committed_token = Arc::new(AtomicUsize::new(0));
    let commit_obtained_lock = Arc::new(AtomicBool::new(false));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<usize, io::Error>(23))
        .commit({
            let lock = lock.clone();
            let committed_token = Arc::clone(&committed_token);
            let commit_obtained_lock = Arc::clone(&commit_obtained_lock);
            move |token| {
                committed_token.store(token, Ordering::Relaxed);
                commit_obtained_lock
                    .store(lock.try_lock().is_some(), Ordering::Relaxed);
                Ok::<(), io::Error>(())
            }
        })
        .no_rollback()
        .build();

    let outcome = executor.run(&lock, || Ok::<u32, io::Error>(42));

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskSucceeded {
            value: 42,
            commit: FinalizationOutcome::Succeeded,
        }
    ));
    assert_eq!(committed_token.load(Ordering::Relaxed), 23);
    assert!(commit_obtained_lock.load(Ordering::Relaxed));
}

/// Verifies task failure supplies a borrowed view of the original error and
/// still returns that owned error in the report.
#[test]
fn test_run_task_error_rolls_back_with_original_error_view() {
    let rollback_message = Arc::new(Mutex::new(None));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<(), io::Error>(()))
        .no_commit()
        .rollback({
            let rollback_message = Arc::clone(&rollback_message);
            move |_, cause| {
                let RollbackCause::TaskFailed(error) = cause else {
                    panic!("expected task failure cause");
                };
                let error = error
                    .downcast_ref::<io::Error>()
                    .expect("rollback should see the original io::Error");
                *rollback_message
                    .lock()
                    .expect("rollback message mutex should not be poisoned") =
                    Some(error.to_string());
                Ok::<(), io::Error>(())
            }
        })
        .build();

    let outcome = executor.run(&parking_lot::Mutex::new(()), || {
        Err::<(), _>(io::Error::other("task failed"))
    });

    match outcome {
        LifecycleOutcome::TaskFailed {
            error,
            rollback: FinalizationOutcome::Succeeded,
        } => {
            assert_eq!(error.to_string(), "task failed");
        }
        _ => panic!("expected task failure"),
    }
    assert_eq!(
        rollback_message
            .lock()
            .expect("rollback message mutex should not be poisoned")
            .as_deref(),
        Some("task failed")
    );
}

/// Verifies task changes to a per-invocation token are visible to commit.
#[test]
fn test_run_with_token_commits_task_updates() {
    let committed_token = Arc::new(Mutex::new(None));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<Vec<&'static str>, io::Error>(vec!["prepare"]))
        .commit({
            let committed_token = Arc::clone(&committed_token);
            move |token| {
                *committed_token
                    .lock()
                    .expect("committed token mutex should not be poisoned") =
                    Some(token);
                Ok::<(), io::Error>(())
            }
        })
        .no_rollback()
        .build();

    let outcome =
        executor.run_with_token(&parking_lot::Mutex::new(()), |token| {
            token.push("task");
            Ok::<usize, io::Error>(token.len())
        });

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskSucceeded {
            value: 2,
            commit: FinalizationOutcome::Succeeded,
        }
    ));
    assert_eq!(
        committed_token
            .lock()
            .expect("committed token mutex should not be poisoned")
            .as_deref(),
        Some(["prepare", "task"].as_slice())
    );
}

/// Verifies commit failure does not overwrite task success.
#[test]
fn test_run_commit_failure_preserves_success() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<(), io::Error>(()))
        .commit(|_| Err::<(), _>(io::Error::other("commit failed")))
        .no_rollback()
        .build();

    let outcome =
        executor.run(&parking_lot::Mutex::new(()), || Ok::<u32, io::Error>(42));

    match outcome {
        LifecycleOutcome::TaskSucceeded {
            value: 42,
            commit: FinalizationOutcome::Failed(error),
        } => {
            assert_eq!(error.to_string(), "commit failed");
        }
        _ => panic!("expected commit failure"),
    }
}

/// Verifies a captured commit panic remains beside the task success.
#[test]
fn test_run_captures_commit_panic_without_overwriting_success() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(true)
        .prepare(|| Ok::<(), io::Error>(()))
        .commit(|_| -> Result<(), io::Error> { panic!("commit panic") })
        .no_rollback()
        .build();

    let outcome =
        executor.run(&parking_lot::Mutex::new(()), || Ok::<u32, io::Error>(42));

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskSucceeded {
            value: 42,
            commit: FinalizationOutcome::Panicked(panic),
        }
            if panic.phase() == PanicPhase::Commit
    ));
}

/// Verifies token destruction without a commit callback is captured as a
/// commit-phase panic while preserving task success.
#[test]
fn test_run_captures_token_drop_panic_without_commit() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(true)
        .prepare(|| Ok::<PanicOnDrop, io::Error>(PanicOnDrop))
        .no_commit()
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();

    let outcome =
        executor.run(&parking_lot::Mutex::new(()), || Ok::<u32, io::Error>(42));

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskSucceeded {
            value: 42,
            commit: FinalizationOutcome::Panicked(panic),
        }
            if panic.phase() == PanicPhase::Commit
                && panic.message() == Some("secondary token drop panic")
    ));
}

/// Verifies captured callback execution preserves simultaneous task and
/// rollback errors.
#[test]
fn test_run_catching_task_and_rollback_errors_preserves_both() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(true)
        .prepare(|| Ok::<(), io::Error>(()))
        .no_commit()
        .rollback(|_, _| Err::<(), _>(io::Error::other("rollback failed")))
        .build();

    let outcome = executor.run(&parking_lot::Mutex::new(()), || {
        Err::<(), _>(io::Error::other("task failed"))
    });

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskFailed {
            error,
            rollback: FinalizationOutcome::Failed(rollback_error),
        } if error.to_string() == "task failed"
            && rollback_error.to_string() == "rollback failed"
    ));
}

/// Verifies a cloned lifecycle executor shares callbacks while retaining an
/// independent invocation token.
#[test]
fn test_clone_shares_lifecycle_callbacks() {
    let commit_calls = Arc::new(AtomicUsize::new(0));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(true)
        .prepare(|| Ok::<u32, io::Error>(17))
        .commit({
            let commit_calls = Arc::clone(&commit_calls);
            move |token| {
                assert_eq!(token, 17);
                commit_calls.fetch_add(1, Ordering::Relaxed);
                Ok::<(), io::Error>(())
            }
        })
        .no_rollback()
        .build();
    let cloned = executor.clone();

    let outcome =
        cloned.run(&parking_lot::Mutex::new(()), || Ok::<u32, io::Error>(42));

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskSucceeded {
            value: 42,
            commit: FinalizationOutcome::Succeeded,
        }
    ));
    assert_eq!(commit_calls.load(Ordering::Relaxed), 1);
}

/// Verifies rollback failure does not overwrite the original task error.
#[test]
fn test_run_rollback_failure_preserves_task_error() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<(), io::Error>(()))
        .no_commit()
        .rollback(|_, _| Err::<(), _>(io::Error::other("rollback failed")))
        .build();

    let outcome = executor.run(&parking_lot::Mutex::new(()), || {
        Err::<(), _>(io::Error::other("task failed"))
    });

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskFailed {
            rollback: FinalizationOutcome::Failed(_),
            ..
        }
    ));
}

/// Verifies captured task panic is rolled back with its original phase.
#[test]
fn test_run_captured_task_panic_rolls_back() {
    let rollback_phase = Arc::new(Mutex::new(None));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(true)
        .prepare(|| Ok::<(), io::Error>(()))
        .no_commit()
        .rollback({
            let rollback_phase = Arc::clone(&rollback_phase);
            move |_, cause| {
                let RollbackCause::Panicked(panic) = cause else {
                    panic!("expected panic rollback cause");
                };
                *rollback_phase
                    .lock()
                    .expect("rollback phase mutex should not be poisoned") =
                    Some(panic.phase());
                Ok::<(), io::Error>(())
            }
        })
        .build();

    let outcome: LifecycleOutcome<(), io::Error, io::Error> =
        executor.run(&parking_lot::Mutex::new(()), || panic!("task panic"));

    assert!(matches!(
        outcome,
        LifecycleOutcome::ExecutionPanicked {
            panic,
            rollback: FinalizationOutcome::Succeeded,
        } if panic.phase() == PanicPhase::Task
    ));
    assert_eq!(
        *rollback_phase
            .lock()
            .expect("rollback phase mutex should not be poisoned"),
        Some(PanicPhase::Task)
    );
}

/// Verifies a rollback panic is retained independently from the task panic.
#[test]
fn test_run_captured_task_and_rollback_panics_preserves_both() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(true)
        .prepare(|| Ok::<(), io::Error>(()))
        .no_commit()
        .rollback(|_, _| panic!("rollback panic"))
        .build();

    let outcome: LifecycleOutcome<(), io::Error, io::Error> =
        executor.run(&parking_lot::Mutex::new(()), || panic!("task panic"));

    assert!(matches!(
        outcome,
        LifecycleOutcome::ExecutionPanicked {
            panic,
            rollback: FinalizationOutcome::Panicked(rollback_panic),
        } if panic.phase() == PanicPhase::Task
            && rollback_panic.phase() == PanicPhase::Rollback
    ));
}

/// Verifies token destruction without a rollback callback is captured as a
/// rollback-phase panic without replacing the task error.
#[test]
fn test_run_captures_token_drop_panic_without_rollback() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(true)
        .prepare(|| Ok::<PanicOnDrop, io::Error>(PanicOnDrop))
        .commit(|_| Ok::<(), io::Error>(()))
        .no_rollback()
        .build();

    let outcome = executor.run(&parking_lot::Mutex::new(()), || {
        Err::<(), _>(io::Error::other("task failed"))
    });

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskFailed {
            error,
            rollback: FinalizationOutcome::Panicked(panic),
        } if error.to_string() == "task failed"
            && panic.phase() == PanicPhase::Rollback
                && panic.message() == Some("secondary token drop panic")
    ));
}

/// Verifies disabled capture rolls back a locked-phase panic before resuming
/// the original payload.
#[test]
fn test_run_uncaptured_task_panic_rolls_back_then_resumes_original() {
    let rollback_calls = Arc::new(AtomicUsize::new(0));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(false)
        .prepare(|| Ok::<(), io::Error>(()))
        .no_commit()
        .rollback({
            let rollback_calls = Arc::clone(&rollback_calls);
            move |_, _| {
                rollback_calls.fetch_add(1, Ordering::Relaxed);
                Ok::<(), io::Error>(())
            }
        })
        .build();

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _: LifecycleOutcome<(), io::Error, io::Error> = executor
            .run(&parking_lot::Mutex::new(()), || {
                panic!("original task panic")
            });
    }));

    let payload = panic_result.expect_err("task panic should resume unwinding");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"original task panic"));
    assert_eq!(rollback_calls.load(Ordering::Relaxed), 1);
}

/// Verifies a rollback error cannot replace the original locked-phase panic
/// when panic capture is disabled.
#[test]
fn test_run_uncaptured_task_panic_outranks_rollback_error() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(false)
        .prepare(|| Ok::<(), io::Error>(()))
        .no_commit()
        .rollback(|_, _| {
            Err::<(), _>(io::Error::other("secondary rollback error"))
        })
        .build();

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _: LifecycleOutcome<(), io::Error, io::Error> = executor
            .run(&parking_lot::Mutex::new(()), || {
                panic!("original task panic")
            });
    }));

    let payload = panic_result.expect_err("task panic should resume unwinding");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"original task panic"));
}

/// Verifies discarding a rollback error whose destructor panics cannot abort
/// the process while the original task panic is resumed.
#[test]
fn test_run_uncaptured_task_panic_outranks_rollback_error_drop_panic() {
    let test_binary = std::env::current_exe()
        .expect("integration test binary path should be available");
    let output = std::process::Command::new(test_binary)
        .arg("--exact")
        .arg("double_checked::lifecycle_double_checked_lock_executor_tests::test_uncaptured_task_panic_with_panicking_rollback_error_drop_child")
        .arg("--ignored")
        .output()
        .expect("child integration test should start");

    assert!(
        output.status.success(),
        "child process should preserve the original task panic without aborting\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Reproduces the rollback-error destructor panic in a child process because
/// an abort cannot be observed safely in the parent test process.
#[test]
#[ignore]
fn test_uncaptured_task_panic_with_panicking_rollback_error_drop_child() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(false)
        .prepare(|| Ok::<(), PanicOnDropRollbackError>(()))
        .no_commit()
        .rollback(|_, _| Err::<(), _>(PanicOnDropRollbackError))
        .build();

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _: LifecycleOutcome<(), io::Error, PanicOnDropRollbackError> =
            executor.run(&parking_lot::Mutex::new(()), || {
                panic!("original task panic")
            });
    }));

    let payload = panic_result.expect_err("task panic should resume unwinding");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"original task panic"));
}

/// Verifies a secondary rollback panic cannot replace the original locked
/// phase panic when panic capture is disabled.
#[test]
fn test_run_uncaptured_task_panic_outranks_rollback_panic() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(false)
        .prepare(|| Ok::<(), io::Error>(()))
        .no_commit()
        .rollback(|_, _| -> Result<(), io::Error> {
            panic!("secondary rollback panic")
        })
        .build();

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _: LifecycleOutcome<(), io::Error, io::Error> = executor
            .run(&parking_lot::Mutex::new(()), || {
                panic!("original task panic")
            });
    }));

    let payload = panic_result.expect_err("task panic should resume unwinding");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"original task panic"));
}

/// Verifies an uncaptured initial predicate panic propagates before prepare.
#[test]
fn test_run_uncaptured_initial_predicate_panic_propagates() {
    let prepare_calls = Arc::new(AtomicUsize::new(0));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| panic!("initial predicate panic"))
        .prepare({
            let prepare_calls = Arc::clone(&prepare_calls);
            move || {
                prepare_calls.fetch_add(1, Ordering::Relaxed);
                Ok::<(), io::Error>(())
            }
        })
        .no_commit()
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _report = executor
            .run(&parking_lot::Mutex::new(()), || Ok::<(), io::Error>(()));
    }));

    let payload =
        panic_result.expect_err("initial predicate panic should propagate");
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"initial predicate panic")
    );
    assert_eq!(prepare_calls.load(Ordering::Relaxed), 0);
}

/// Verifies an uncaptured prepare panic propagates without rollback because no
/// token exists.
#[test]
fn test_run_uncaptured_prepare_panic_propagates_without_rollback() {
    let rollback_calls = Arc::new(AtomicUsize::new(0));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare(|| -> Result<(), io::Error> { panic!("prepare panic") })
        .no_commit()
        .rollback({
            let rollback_calls = Arc::clone(&rollback_calls);
            move |_, _| {
                rollback_calls.fetch_add(1, Ordering::Relaxed);
                Ok::<(), io::Error>(())
            }
        })
        .build();

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _report = executor
            .run(&parking_lot::Mutex::new(()), || Ok::<(), io::Error>(()));
    }));

    let payload = panic_result.expect_err("prepare panic should propagate");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"prepare panic"));
    assert_eq!(rollback_calls.load(Ordering::Relaxed), 0);
}

/// Verifies an uncaptured commit panic propagates only after task success and
/// lock release.
#[test]
fn test_run_uncaptured_commit_panic_propagates() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<(), io::Error>(()))
        .commit(|_| -> Result<(), io::Error> { panic!("commit panic") })
        .no_rollback()
        .build();

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _report = executor
            .run(&parking_lot::Mutex::new(()), || Ok::<(), io::Error>(()));
    }));

    let payload = panic_result.expect_err("commit panic should propagate");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"commit panic"));
}

/// Verifies an uncaptured ordinary rollback panic propagates when no earlier
/// locked-phase panic exists.
#[test]
fn test_run_uncaptured_ordinary_rollback_panic_propagates() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<(), io::Error>(()))
        .no_commit()
        .rollback(|_, _| -> Result<(), io::Error> { panic!("rollback panic") })
        .build();

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _report = executor.run(&parking_lot::Mutex::new(()), || {
            Err::<(), _>(io::Error::other("task failed"))
        });
    }));

    let payload = panic_result.expect_err("rollback panic should propagate");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"rollback panic"));
}

/// Verifies a no-rollback lifecycle still resumes the original locked-phase
/// panic after consuming its token.
#[test]
fn test_run_uncaptured_task_panic_without_rollback_resumes_original() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<(), io::Error>(()))
        .commit(|_| Ok::<(), io::Error>(()))
        .no_rollback()
        .build();

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _: LifecycleOutcome<(), io::Error, io::Error> = executor
            .run(&parking_lot::Mutex::new(()), || {
                panic!("original task panic")
            });
    }));

    let payload = panic_result.expect_err("task panic should resume unwinding");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"original task panic"));
}

/// Verifies a token destructor panic cannot replace the original locked-phase
/// panic when no rollback callback is configured.
#[test]
fn test_run_uncaptured_task_panic_outranks_token_drop_panic_without_rollback() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(false)
        .prepare(|| Ok::<PanicOnDrop, io::Error>(PanicOnDrop))
        .commit(|_| Ok::<(), io::Error>(()))
        .no_rollback()
        .build();

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _: LifecycleOutcome<(), io::Error, io::Error> = executor
            .run(&parking_lot::Mutex::new(()), || {
                panic!("original task panic")
            });
    }));

    let payload = panic_result.expect_err("task panic should resume unwinding");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"original task panic"));
}

/// Verifies concurrent prepare calls retain distinct tokens for commit and
/// rollback.
#[test]
fn test_run_concurrent_prepare_tokens_do_not_cross_calls() {
    let lock = Arc::new(ParkingLotMutex::new(()));
    let gate = Arc::new(AtomicBool::new(true));
    let prepare_barrier = Arc::new(Barrier::new(2));
    let next_token = Arc::new(AtomicUsize::new(0));
    let committed = Arc::new(Mutex::new(Vec::new()));
    let rolled_back = Arc::new(Mutex::new(Vec::new()));
    let executor = Arc::new(
        LifecycleDoubleCheckedLockExecutor::builder()
            .when({
                let gate = Arc::clone(&gate);
                move || gate.load(Ordering::Acquire)
            })
            .prepare({
                let prepare_barrier = Arc::clone(&prepare_barrier);
                let next_token = Arc::clone(&next_token);
                move || {
                    let token = next_token.fetch_add(1, Ordering::Relaxed);
                    prepare_barrier.wait();
                    Ok::<usize, io::Error>(token)
                }
            })
            .commit({
                let committed = Arc::clone(&committed);
                move |token| {
                    committed
                        .lock()
                        .expect("commit token mutex should not be poisoned")
                        .push(token);
                    Ok::<(), io::Error>(())
                }
            })
            .rollback({
                let rolled_back = Arc::clone(&rolled_back);
                move |token, _| {
                    rolled_back
                        .lock()
                        .expect("rollback token mutex should not be poisoned")
                        .push(token);
                    Ok::<(), io::Error>(())
                }
            })
            .build(),
    );

    let handles = (0..2)
        .map(|_| {
            let executor = Arc::clone(&executor);
            let gate = Arc::clone(&gate);
            let lock = Arc::clone(&lock);
            thread::spawn(move || {
                executor.run(&lock, || {
                    gate.store(false, Ordering::Release);
                    Ok::<(), io::Error>(())
                })
            })
        })
        .collect::<Vec<_>>();
    for handle in handles {
        let _report = handle.join().expect("worker should not panic");
    }

    let committed = committed
        .lock()
        .expect("commit token mutex should not be poisoned");
    let rolled_back = rolled_back
        .lock()
        .expect("rollback token mutex should not be poisoned");
    assert_eq!(committed.len(), 1);
    assert_eq!(rolled_back.len(), 1);
    assert_ne!(committed[0], rolled_back[0]);
    assert_eq!(next_token.load(Ordering::Relaxed), 2);
}
