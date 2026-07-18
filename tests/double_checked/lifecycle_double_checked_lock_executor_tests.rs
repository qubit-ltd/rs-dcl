// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the lifecycle-aware DCL executor.

use std::{
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

use qubit_dcl::{
    ExecutionOutcome,
    LifecycleDoubleCheckedLockExecutor,
    PanicPhase,
    PreparationOutcome,
    RollbackCause,
};
use qubit_lock::{
    ArcMutex,
    ArcStdMutex,
    Lock,
};

/// Verifies a failed initial check bypasses every lifecycle callback and lock.
#[test]
fn test_run_initial_false_does_not_prepare_or_finalize() {
    let prepare_calls = Arc::new(AtomicUsize::new(0));
    let commit_calls = Arc::new(AtomicUsize::new(0));
    let rollback_calls = Arc::new(AtomicUsize::new(0));
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
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

    let report = executor.run(|| Ok::<(), io::Error>(()));

    assert!(matches!(
        report.execution(),
        ExecutionOutcome::ConditionNotMet
    ));
    assert!(matches!(
        report.preparation(),
        PreparationOutcome::NotStarted
    ));
    assert_eq!(prepare_calls.load(Ordering::Relaxed), 0);
    assert_eq!(commit_calls.load(Ordering::Relaxed), 0);
    assert_eq!(rollback_calls.load(Ordering::Relaxed), 0);
}

/// Verifies a prepare error reports `NotExecuted` and never calls rollback.
#[test]
fn test_run_prepare_error_does_not_rollback_without_token() {
    let rollback_calls = Arc::new(AtomicUsize::new(0));
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
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

    let report = executor.run(|| Ok::<(), io::Error>(()));

    assert!(matches!(report.execution(), ExecutionOutcome::NotExecuted));
    match report.preparation() {
        PreparationOutcome::PrepareFailed(error) => {
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
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
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

    let report = executor.run(|| Ok::<(), io::Error>(()));

    assert!(matches!(
        report.execution(),
        ExecutionOutcome::Panicked(panic)
            if panic.phase() == PanicPhase::InitialConditionCheck
    ));
    assert!(matches!(
        report.preparation(),
        PreparationOutcome::NotStarted
    ));
    assert_eq!(prepare_calls.load(Ordering::Relaxed), 0);
}

/// Verifies a captured prepare panic reports no execution and cannot roll back
/// because no token was produced.
#[test]
fn test_run_captures_prepare_panic_without_rollback() {
    let rollback_calls = Arc::new(AtomicUsize::new(0));
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
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

    let report = executor.run(|| Ok::<(), io::Error>(()));

    assert!(matches!(report.execution(), ExecutionOutcome::NotExecuted));
    assert!(matches!(
        report.preparation(),
        PreparationOutcome::PreparePanicked(panic)
            if panic.phase() == PanicPhase::Prepare
    ));
    assert_eq!(rollback_calls.load(Ordering::Relaxed), 0);
}

/// Verifies the capturing state machine retains the outer-false fast exit.
#[test]
fn test_run_catching_initial_false_does_not_prepare() {
    let prepare_calls = Arc::new(AtomicUsize::new(0));
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
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

    let report = executor.run(|| Ok::<(), io::Error>(()));

    assert!(matches!(
        report.execution(),
        ExecutionOutcome::ConditionNotMet
    ));
    assert!(matches!(
        report.preparation(),
        PreparationOutcome::NotStarted
    ));
    assert_eq!(prepare_calls.load(Ordering::Relaxed), 0);
}

/// Verifies the capturing state machine retains a returned prepare error.
#[test]
fn test_run_catching_prepare_error_preserves_lifecycle_error() {
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
            .when(|| true)
            .catch_panics(true)
            .prepare(|| Err::<(), _>(io::Error::other("prepare failed")))
            .no_commit()
            .rollback(|_, _| Ok::<(), io::Error>(()))
            .build();

    let report = executor.run(|| Ok::<(), io::Error>(()));

    assert!(matches!(report.execution(), ExecutionOutcome::NotExecuted));
    assert!(matches!(
        report.preparation(),
        PreparationOutcome::PrepareFailed(error)
            if error.to_string() == "prepare failed"
    ));
}

/// Verifies a failed second check rolls back its token after the lock is
/// released.
#[test]
fn test_run_second_false_rolls_back_after_unlock() {
    let lock = ArcMutex::new(());
    let checks = Arc::new(AtomicUsize::new(0));
    let rolled_back_token = Arc::new(AtomicUsize::new(0));
    let saw_condition_cause = Arc::new(AtomicBool::new(false));
    let rollback_obtained_lock = Arc::new(AtomicBool::new(false));
    let executor = LifecycleDoubleCheckedLockExecutor::builder(lock.clone())
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
                rollback_obtained_lock.store(
                    lock.try_with_write(|_| ()).is_ok(),
                    Ordering::Relaxed,
                );
                Ok::<(), io::Error>(())
            }
        })
        .build();

    let report = executor.run(|| Ok::<(), io::Error>(()));

    assert!(matches!(
        report.execution(),
        ExecutionOutcome::ConditionNotMet
    ));
    assert!(matches!(
        report.preparation(),
        PreparationOutcome::RolledBack
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
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
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
                    *rollback_phase.lock().expect(
                        "rollback phase mutex should not be poisoned",
                    ) = Some(panic.phase());
                    Ok::<(), io::Error>(())
                }
            })
            .build();

    let report = executor.run(|| Ok::<(), io::Error>(()));

    assert!(matches!(
        report.execution(),
        ExecutionOutcome::Panicked(panic)
            if panic.phase() == PanicPhase::SecondConditionCheck
    ));
    assert!(matches!(
        report.preparation(),
        PreparationOutcome::RolledBack
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
    let lock = ArcStdMutex::new(());
    let poison_lock = lock.clone();
    let poison_result = catch_unwind(AssertUnwindSafe(|| {
        poison_lock.with_write(|_| panic!("poison lifecycle lock"));
    }));
    assert!(poison_result.is_err());
    let rollback_phase = Arc::new(Mutex::new(None));
    let executor = LifecycleDoubleCheckedLockExecutor::builder(lock)
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

    let report = executor.run(|| Ok::<(), io::Error>(()));

    assert!(matches!(
        report.execution(),
        ExecutionOutcome::Panicked(panic)
            if panic.phase() == PanicPhase::LockAcquisition
    ));
    assert!(matches!(
        report.preparation(),
        PreparationOutcome::RolledBack
    ));
    assert_eq!(
        *rollback_phase
            .lock()
            .expect("rollback phase mutex should not be poisoned"),
        Some(PanicPhase::LockAcquisition)
    );
}

/// Verifies task success commits the token after releasing the executor lock.
#[test]
fn test_run_success_commits_after_unlock() {
    let lock = ArcMutex::new(());
    let committed_token = Arc::new(AtomicUsize::new(0));
    let commit_obtained_lock = Arc::new(AtomicBool::new(false));
    let executor = LifecycleDoubleCheckedLockExecutor::builder(lock.clone())
        .when(|| true)
        .prepare(|| Ok::<usize, io::Error>(23))
        .commit({
            let lock = lock.clone();
            let committed_token = Arc::clone(&committed_token);
            let commit_obtained_lock = Arc::clone(&commit_obtained_lock);
            move |token| {
                committed_token.store(token, Ordering::Relaxed);
                commit_obtained_lock.store(
                    lock.try_with_write(|_| ()).is_ok(),
                    Ordering::Relaxed,
                );
                Ok::<(), io::Error>(())
            }
        })
        .no_rollback()
        .build();

    let report = executor.run(|| Ok::<u32, io::Error>(42));

    assert!(matches!(report.execution(), ExecutionOutcome::Success(42)));
    assert!(matches!(
        report.preparation(),
        PreparationOutcome::Committed
    ));
    assert_eq!(committed_token.load(Ordering::Relaxed), 23);
    assert!(commit_obtained_lock.load(Ordering::Relaxed));
}

/// Verifies task failure supplies a borrowed view of the original error and
/// still returns that owned error in the report.
#[test]
fn test_run_task_error_rolls_back_with_original_error_view() {
    let rollback_message = Arc::new(Mutex::new(None));
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
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
                    *rollback_message.lock().expect(
                        "rollback message mutex should not be poisoned",
                    ) = Some(error.to_string());
                    Ok::<(), io::Error>(())
                }
            })
            .build();

    let report = executor.run(|| Err::<(), _>(io::Error::other("task failed")));

    match report.execution() {
        ExecutionOutcome::TaskFailed(error) => {
            assert_eq!(error.to_string(), "task failed");
        }
        _ => panic!("expected task failure"),
    }
    assert!(matches!(
        report.preparation(),
        PreparationOutcome::RolledBack
    ));
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
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
            .when(|| true)
            .prepare(|| Ok::<Vec<&'static str>, io::Error>(vec!["prepare"]))
            .commit({
                let committed_token = Arc::clone(&committed_token);
                move |token| {
                    *committed_token.lock().expect(
                        "committed token mutex should not be poisoned",
                    ) = Some(token);
                    Ok::<(), io::Error>(())
                }
            })
            .no_rollback()
            .build();

    let report = executor.run_with_token(|token| {
        token.push("task");
        Ok::<usize, io::Error>(token.len())
    });

    assert!(matches!(report.execution(), ExecutionOutcome::Success(2)));
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
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
            .when(|| true)
            .prepare(|| Ok::<(), io::Error>(()))
            .commit(|_| Err::<(), _>(io::Error::other("commit failed")))
            .no_rollback()
            .build();

    let report = executor.run(|| Ok::<u32, io::Error>(42));

    assert!(matches!(report.execution(), ExecutionOutcome::Success(42)));
    match report.preparation() {
        PreparationOutcome::CommitFailed(error) => {
            assert_eq!(error.to_string(), "commit failed");
        }
        _ => panic!("expected commit failure"),
    }
}

/// Verifies a captured commit panic remains on the preparation axis while the
/// task success remains intact.
#[test]
fn test_run_captures_commit_panic_without_overwriting_success() {
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
            .when(|| true)
            .catch_panics(true)
            .prepare(|| Ok::<(), io::Error>(()))
            .commit(|_| -> Result<(), io::Error> { panic!("commit panic") })
            .no_rollback()
            .build();

    let report = executor.run(|| Ok::<u32, io::Error>(42));

    assert!(matches!(report.execution(), ExecutionOutcome::Success(42)));
    assert!(matches!(
        report.preparation(),
        PreparationOutcome::CommitPanicked(panic)
            if panic.phase() == PanicPhase::Commit
    ));
}

/// Verifies captured callback execution preserves simultaneous task and
/// rollback errors.
#[test]
fn test_run_catching_task_and_rollback_errors_preserves_both() {
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
            .when(|| true)
            .catch_panics(true)
            .prepare(|| Ok::<(), io::Error>(()))
            .no_commit()
            .rollback(|_, _| Err::<(), _>(io::Error::other("rollback failed")))
            .build();

    let report = executor.run(|| Err::<(), _>(io::Error::other("task failed")));

    assert!(matches!(
        report.execution(),
        ExecutionOutcome::TaskFailed(error)
            if error.to_string() == "task failed"
    ));
    assert!(matches!(
        report.preparation(),
        PreparationOutcome::RollbackFailed(error)
            if error.to_string() == "rollback failed"
    ));
}

/// Verifies a cloned lifecycle executor shares callbacks while retaining an
/// independent invocation token.
#[test]
fn test_clone_shares_lifecycle_callbacks() {
    let commit_calls = Arc::new(AtomicUsize::new(0));
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
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

    let report = cloned.run(|| Ok::<u32, io::Error>(42));

    assert!(matches!(report.execution(), ExecutionOutcome::Success(42)));
    assert!(matches!(
        report.preparation(),
        PreparationOutcome::Committed
    ));
    assert_eq!(commit_calls.load(Ordering::Relaxed), 1);
}

/// Verifies rollback failure does not overwrite the original task error.
#[test]
fn test_run_rollback_failure_preserves_task_error() {
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
            .when(|| true)
            .prepare(|| Ok::<(), io::Error>(()))
            .no_commit()
            .rollback(|_, _| Err::<(), _>(io::Error::other("rollback failed")))
            .build();

    let report = executor.run(|| Err::<(), _>(io::Error::other("task failed")));

    assert!(matches!(
        report.execution(),
        ExecutionOutcome::TaskFailed(_)
    ));
    assert!(matches!(
        report.preparation(),
        PreparationOutcome::RollbackFailed(_)
    ));
}

/// Verifies captured task panic is rolled back and retains both outcome axes.
#[test]
fn test_run_captured_task_panic_rolls_back() {
    let rollback_phase = Arc::new(Mutex::new(None));
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
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
                    *rollback_phase.lock().expect(
                        "rollback phase mutex should not be poisoned",
                    ) = Some(panic.phase());
                    Ok::<(), io::Error>(())
                }
            })
            .build();

    let report: qubit_dcl::ExecutionReport<(), io::Error, io::Error> =
        executor.run(|| panic!("task panic"));

    assert!(matches!(
        report.execution(),
        ExecutionOutcome::Panicked(panic) if panic.phase() == PanicPhase::Task
    ));
    assert!(matches!(
        report.preparation(),
        PreparationOutcome::RolledBack
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
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
            .when(|| true)
            .catch_panics(true)
            .prepare(|| Ok::<(), io::Error>(()))
            .no_commit()
            .rollback(|_, _| panic!("rollback panic"))
            .build();

    let report: qubit_dcl::ExecutionReport<(), io::Error, io::Error> =
        executor.run(|| panic!("task panic"));

    assert!(matches!(
        report.execution(),
        ExecutionOutcome::Panicked(panic) if panic.phase() == PanicPhase::Task
    ));
    assert!(matches!(
        report.preparation(),
        PreparationOutcome::RollbackPanicked(panic)
            if panic.phase() == PanicPhase::Rollback
    ));
}

/// Verifies disabled capture rolls back a locked-phase panic before resuming
/// the original payload.
#[test]
fn test_run_uncaptured_task_panic_rolls_back_then_resumes_original() {
    let rollback_calls = Arc::new(AtomicUsize::new(0));
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
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
        let _: qubit_dcl::ExecutionReport<(), io::Error, io::Error> =
            executor.run(|| panic!("original task panic"));
    }));

    let payload = panic_result.expect_err("task panic should resume unwinding");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"original task panic"));
    assert_eq!(rollback_calls.load(Ordering::Relaxed), 1);
}

/// Verifies a rollback error cannot replace the original locked-phase panic
/// when panic capture is disabled.
#[test]
fn test_run_uncaptured_task_panic_outranks_rollback_error() {
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
            .when(|| true)
            .catch_panics(false)
            .prepare(|| Ok::<(), io::Error>(()))
            .no_commit()
            .rollback(|_, _| {
                Err::<(), _>(io::Error::other("secondary rollback error"))
            })
            .build();

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _: qubit_dcl::ExecutionReport<(), io::Error, io::Error> =
            executor.run(|| panic!("original task panic"));
    }));

    let payload = panic_result.expect_err("task panic should resume unwinding");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"original task panic"));
}

/// Verifies a secondary rollback panic cannot replace the original locked
/// phase panic when panic capture is disabled.
#[test]
fn test_run_uncaptured_task_panic_outranks_rollback_panic() {
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
            .when(|| true)
            .catch_panics(false)
            .prepare(|| Ok::<(), io::Error>(()))
            .no_commit()
            .rollback(|_, _| -> Result<(), io::Error> {
                panic!("secondary rollback panic")
            })
            .build();

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _: qubit_dcl::ExecutionReport<(), io::Error, io::Error> =
            executor.run(|| panic!("original task panic"));
    }));

    let payload = panic_result.expect_err("task panic should resume unwinding");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"original task panic"));
}

/// Verifies an uncaptured initial predicate panic propagates before prepare.
#[test]
fn test_run_uncaptured_initial_predicate_panic_propagates() {
    let prepare_calls = Arc::new(AtomicUsize::new(0));
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
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
        let _report = executor.run(|| Ok::<(), io::Error>(()));
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
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
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
        let _report = executor.run(|| Ok::<(), io::Error>(()));
    }));

    let payload = panic_result.expect_err("prepare panic should propagate");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"prepare panic"));
    assert_eq!(rollback_calls.load(Ordering::Relaxed), 0);
}

/// Verifies an uncaptured commit panic propagates only after task success and
/// lock release.
#[test]
fn test_run_uncaptured_commit_panic_propagates() {
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
            .when(|| true)
            .prepare(|| Ok::<(), io::Error>(()))
            .commit(|_| -> Result<(), io::Error> { panic!("commit panic") })
            .no_rollback()
            .build();

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _report = executor.run(|| Ok::<(), io::Error>(()));
    }));

    let payload = panic_result.expect_err("commit panic should propagate");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"commit panic"));
}

/// Verifies an uncaptured ordinary rollback panic propagates when no earlier
/// locked-phase panic exists.
#[test]
fn test_run_uncaptured_ordinary_rollback_panic_propagates() {
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
            .when(|| true)
            .prepare(|| Ok::<(), io::Error>(()))
            .no_commit()
            .rollback(|_, _| -> Result<(), io::Error> {
                panic!("rollback panic")
            })
            .build();

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _report =
            executor.run(|| Err::<(), _>(io::Error::other("task failed")));
    }));

    let payload = panic_result.expect_err("rollback panic should propagate");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"rollback panic"));
}

/// Verifies a no-rollback lifecycle still resumes the original locked-phase
/// panic after consuming its token.
#[test]
fn test_run_uncaptured_task_panic_without_rollback_resumes_original() {
    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
            .when(|| true)
            .prepare(|| Ok::<(), io::Error>(()))
            .commit(|_| Ok::<(), io::Error>(()))
            .no_rollback()
            .build();

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _: qubit_dcl::ExecutionReport<(), io::Error, io::Error> =
            executor.run(|| panic!("original task panic"));
    }));

    let payload = panic_result.expect_err("task panic should resume unwinding");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"original task panic"));
}

/// Verifies concurrent prepare calls retain distinct tokens for commit and
/// rollback.
#[test]
fn test_run_concurrent_prepare_tokens_do_not_cross_calls() {
    let gate = Arc::new(AtomicBool::new(true));
    let prepare_barrier = Arc::new(Barrier::new(2));
    let next_token = Arc::new(AtomicUsize::new(0));
    let committed = Arc::new(Mutex::new(Vec::new()));
    let rolled_back = Arc::new(Mutex::new(Vec::new()));
    let executor = Arc::new(
        LifecycleDoubleCheckedLockExecutor::builder(ArcMutex::new(()))
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
            thread::spawn(move || {
                executor.run(|| {
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
