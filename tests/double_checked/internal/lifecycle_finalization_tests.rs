// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Behavior tests for lifecycle finalization branches.

use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use qubit_dcl::FinalizationOutcome;
use qubit_dcl::LifecycleDclExecutor;
use qubit_dcl::LifecycleOutcome;

/// Token that records when lifecycle finalization drops it.
struct DropToken {
    /// Shared counter incremented when the token is dropped.
    drops: Arc<AtomicUsize>,
}

impl Drop for DropToken {
    /// Records one finalization drop.
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}

/// Verifies a successful invocation drops its token when no commit callback is
/// configured.
#[test]
fn test_no_commit_drops_token_and_reports_not_required() {
    let drops = Arc::new(AtomicUsize::new(0));
    let executor = LifecycleDclExecutor::builder()
        .when(|| true)
        .prepare({
            let drops = Arc::clone(&drops);
            move || {
                Ok::<DropToken, io::Error>(DropToken {
                    drops: Arc::clone(&drops),
                })
            }
        })
        .no_commit()
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();

    let outcome = executor.run(&std::sync::Mutex::new(()), || Ok::<(), io::Error>(()));

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskSucceeded {
            value: (),
            commit: FinalizationOutcome::NotRequired,
        }
    ));
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}

/// Verifies panic capture preserves an ordinary commit error.
#[test]
fn test_catching_commit_error_reports_failure() {
    let executor = LifecycleDclExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<(), io::Error>(()))
        .commit(|_| Err::<(), _>(io::Error::other("commit failed")))
        .no_rollback()
        .build();

    let outcome = executor.run(&std::sync::Mutex::new(()), || Ok::<u32, io::Error>(42));

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskSucceeded {
            value: 42,
            commit: FinalizationOutcome::Failed(error),
        } if error.to_string() == "commit failed"
    ));
}

/// Verifies panic capture drops a token normally when no commit callback is
/// configured.
#[test]
fn test_catching_no_commit_drops_token_and_reports_not_required() {
    let drops = Arc::new(AtomicUsize::new(0));
    let executor = LifecycleDclExecutor::builder()
        .when(|| true)
        .prepare({
            let drops = Arc::clone(&drops);
            move || {
                Ok::<DropToken, io::Error>(DropToken {
                    drops: Arc::clone(&drops),
                })
            }
        })
        .no_commit()
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();

    let outcome = executor.run(&std::sync::Mutex::new(()), || Ok::<(), io::Error>(()));

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskSucceeded {
            value: (),
            commit: FinalizationOutcome::NotRequired,
        }
    ));
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}

/// Verifies panic capture drops a token normally when no rollback callback is
/// configured.
#[test]
fn test_catching_no_rollback_drops_token_and_reports_not_required() {
    let drops = Arc::new(AtomicUsize::new(0));
    let executor = LifecycleDclExecutor::builder()
        .when(|| true)
        .prepare({
            let drops = Arc::clone(&drops);
            move || {
                Ok::<DropToken, io::Error>(DropToken {
                    drops: Arc::clone(&drops),
                })
            }
        })
        .commit(|_| Ok::<(), io::Error>(()))
        .no_rollback()
        .build();

    let outcome = executor.run(&std::sync::Mutex::new(()), || {
        Err::<(), _>(io::Error::other("task failed"))
    });

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskFailed {
            error,
            rollback: FinalizationOutcome::NotRequired,
        } if error.to_string() == "task failed"
    ));
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}
