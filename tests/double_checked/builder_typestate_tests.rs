// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Runtime construction tests for legal lifecycle typestate combinations.

use std::io;

use qubit_dcl::{
    FinalizationOutcome,
    LifecycleDoubleCheckedLockExecutor,
    LifecycleOutcome,
};

/// Verifies the complete prepare/commit/rollback combination builds and runs.
#[test]
fn test_builder_full_lifecycle_combination() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<u32, io::Error>(1))
        .commit(|_| Ok::<(), io::Error>(()))
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();

    let outcome =
        executor.run(&std::sync::Mutex::new(()), || Ok::<u32, io::Error>(7));

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskSucceeded {
            value: 7,
            commit: FinalizationOutcome::Succeeded,
        }
    ));
}

/// Verifies a lifecycle with commit but no rollback builds and reports its
/// explicit no-rollback path.
#[test]
fn test_builder_commit_without_rollback_combination() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<u32, io::Error>(1))
        .commit(|_| Ok::<(), io::Error>(()))
        .no_rollback()
        .build();

    let outcome = executor.run(&std::sync::Mutex::new(()), || {
        Err::<(), _>(io::Error::other("task"))
    });

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskFailed {
            rollback: FinalizationOutcome::NotRequired,
            ..
        }
    ));
}

/// Verifies a lifecycle with rollback but no commit builds and reports its
/// explicit no-commit path.
#[test]
fn test_builder_rollback_without_commit_combination() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<u32, io::Error>(1))
        .no_commit()
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();

    let outcome =
        executor.run(&std::sync::Mutex::new(()), || Ok::<u32, io::Error>(7));

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskSucceeded {
            value: 7,
            commit: FinalizationOutcome::NotRequired,
        }
    ));
}
