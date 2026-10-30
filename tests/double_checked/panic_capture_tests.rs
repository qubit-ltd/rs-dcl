// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public regression tests for internal panic capture.

use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use qubit_dcl::CapturedFinalizationOutcome;
use qubit_dcl::CapturedLifecycleOutcome;
use qubit_dcl::DclExecutor;
use qubit_dcl::LifecycleDclExecutor;
use qubit_dcl::PanicPhase;
use qubit_dcl::RollbackCause;

use crate::support::PanicOnDrop;

/// Verifies a captured task panic retains its phase and message.
#[test]
fn test_panic_capture_retains_task_context() {
    let executor = DclExecutor::new(|| true);
    let panic = executor
        .run_catching(&Mutex::new(()), || -> Result<(), std::io::Error> { panic!("captured") })
        .unwrap_err();

    assert_eq!(panic.phase(), PanicPhase::Task);
    assert_eq!(panic.message(), Some("captured"));
}

/// Classifies a skipped task destructor after releasing the guard.
#[test]
fn test_basic_second_false_task_cleanup_phase() {
    let checks = Arc::new(AtomicUsize::new(0));
    let executor = DclExecutor::new({
        let checks = Arc::clone(&checks);
        move || checks.fetch_add(1, Ordering::Relaxed) == 0
    });
    let lock = Mutex::new(());
    let task_calls = AtomicUsize::new(0);
    let task_calls_ref = &task_calls;
    let captured = PanicOnDrop;
    let panic = executor
        .run_catching(&lock, move || {
            task_calls_ref.fetch_add(1, Ordering::Relaxed);
            drop(captured);
            Ok::<(), io::Error>(())
        })
        .expect_err("skipped task destructor panic must be captured");

    assert_eq!(panic.phase(), PanicPhase::TaskCleanup);
    assert_eq!(panic.message(), Some("secondary token drop panic"));
    assert_eq!(checks.load(Ordering::Relaxed), 2);
    assert_eq!(task_calls.load(Ordering::Relaxed), 0);
    assert!(!lock.is_poisoned());
    assert!(lock.try_lock().is_ok(), "guard must already be released");
}

/// Preserves prepared-token rollback for both captured lifecycle entry points.
#[test]
fn test_lifecycle_second_false_task_cleanup_phase() {
    for with_token in [false, true] {
        let checks = Arc::new(AtomicUsize::new(0));
        let rollback_calls = Arc::new(AtomicUsize::new(0));
        let lock = Arc::new(Mutex::new(()));
        let executor = LifecycleDclExecutor::builder()
            .when({
                let checks = Arc::clone(&checks);
                move || checks.fetch_add(1, Ordering::Relaxed) == 0
            })
            .prepare(|| Ok::<u32, io::Error>(7))
            .no_commit()
            .rollback({
                let lock = Arc::clone(&lock);
                let rollback_calls = Arc::clone(&rollback_calls);
                move |token, cause| {
                    assert_eq!(token, 7);
                    assert!(matches!(cause, RollbackCause::Panicked(panic)
                        if panic.phase() == PanicPhase::TaskCleanup));
                    assert!(lock.try_lock().is_ok(), "rollback must run outside the guard");
                    rollback_calls.fetch_add(1, Ordering::Relaxed);
                    Ok::<(), io::Error>(())
                }
            })
            .build();
        let task_calls = AtomicUsize::new(0);
        let task_calls_ref = &task_calls;
        let captured = PanicOnDrop;
        let outcome = if with_token {
            executor.run_with_token_catching(lock.as_ref(), move |_| {
                task_calls_ref.fetch_add(1, Ordering::Relaxed);
                drop(captured);
                Ok::<(), io::Error>(())
            })
        } else {
            executor.run_catching(lock.as_ref(), move || {
                task_calls_ref.fetch_add(1, Ordering::Relaxed);
                drop(captured);
                Ok::<(), io::Error>(())
            })
        };
        assert!(matches!(outcome,
            CapturedLifecycleOutcome::ExecutionPanicked {
                panic,
                rollback: CapturedFinalizationOutcome::Succeeded,
            } if panic.phase() == PanicPhase::TaskCleanup
                && panic.message() == Some("secondary token drop panic")
        ));
        assert_eq!(checks.load(Ordering::Relaxed), 2);
        assert_eq!(task_calls.load(Ordering::Relaxed), 0);
        assert_eq!(rollback_calls.load(Ordering::Relaxed), 1);
        assert!(!lock.is_poisoned());
        assert!(lock.try_lock().is_ok());
    }
}
