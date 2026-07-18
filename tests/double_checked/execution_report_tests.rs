// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for orthogonal lifecycle execution reports.

use std::io;

use qubit_dcl::{
    ExecutionOutcome,
    LifecycleDoubleCheckedLockExecutor,
    PreparationOutcome,
};

/// Verifies borrowed accessors expose both report axes without consuming them.
#[test]
fn test_execution_report_accessors_preserve_both_axes() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<(), io::Error>(()))
        .commit(|_| Err::<(), _>(io::Error::other("commit failed")))
        .no_rollback()
        .build();

    let report =
        executor.run(&parking_lot::Mutex::new(()), || Ok::<u32, io::Error>(42));

    assert!(matches!(report.execution(), ExecutionOutcome::Success(42)));
    assert!(matches!(
        report.preparation(),
        PreparationOutcome::CommitFailed(_)
    ));
}

/// Verifies consuming a report returns both original owned outcomes.
#[test]
fn test_execution_report_into_parts_preserves_owned_errors() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<(), io::Error>(()))
        .no_commit()
        .rollback(|_, _| Err::<(), _>(io::Error::other("rollback failed")))
        .build();

    let report = executor.run(&parking_lot::Mutex::new(()), || {
        Err::<(), _>(io::Error::other("task failed"))
    });
    let (execution, preparation) = report.into_parts();

    match execution {
        ExecutionOutcome::TaskFailed(error) => {
            assert_eq!(error.to_string(), "task failed");
        }
        _ => panic!("expected task failure"),
    }
    match preparation {
        PreparationOutcome::RollbackFailed(error) => {
            assert_eq!(error.to_string(), "rollback failed");
        }
        _ => panic!("expected rollback failure"),
    }
}
