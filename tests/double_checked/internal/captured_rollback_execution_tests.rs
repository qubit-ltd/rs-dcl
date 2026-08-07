// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for captured lifecycle rollback execution paths.

use std::io;

use qubit_dcl::CapturedFinalizationOutcome;
use qubit_dcl::CapturedLifecycleOutcome;
use qubit_dcl::LifecycleDclExecutor;
use qubit_dcl::RollbackCause;

/// Verifies a captured task error preserves both the error and rollback result.
#[test]
fn test_captured_task_failure_preserves_error_and_rollback() {
    let executor = LifecycleDclExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<(), io::Error>(()))
        .no_commit()
        .rollback(|_, cause| {
            assert!(matches!(cause, RollbackCause::TaskFailed(_)));
            Ok::<(), io::Error>(())
        })
        .build();

    let outcome = executor.run_catching(&std::sync::Mutex::new(()), || {
        Err::<(), _>(io::Error::other("task"))
    });

    assert!(matches!(
        outcome,
        CapturedLifecycleOutcome::TaskFailed {
            error,
            rollback: CapturedFinalizationOutcome::Succeeded,
        } if error.to_string() == "task"
    ));
}
