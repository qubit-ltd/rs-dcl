// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for execution outcomes.

use std::io;

use qubit_dcl::ExecutionOutcome;

/// Verifies that a successful task preserves its return value.
#[test]
fn test_execution_outcome_success_preserves_value() {
    let outcome: ExecutionOutcome<u32, io::Error> =
        ExecutionOutcome::Success(42);

    assert!(matches!(outcome, ExecutionOutcome::Success(42)));
}

/// Verifies that both condition exits remain distinguishable from task
/// failures.
#[test]
fn test_execution_outcome_condition_not_met_is_distinct() {
    let outcome: ExecutionOutcome<(), io::Error> =
        ExecutionOutcome::ConditionNotMet;

    assert!(matches!(outcome, ExecutionOutcome::ConditionNotMet));
}

/// Verifies that a lifecycle prepare failure can report that no task ran.
#[test]
fn test_execution_outcome_not_executed_is_distinct() {
    let outcome: ExecutionOutcome<(), io::Error> =
        ExecutionOutcome::NotExecuted;

    assert!(matches!(outcome, ExecutionOutcome::NotExecuted));
}

/// Verifies that a task error is returned without string conversion.
#[test]
fn test_execution_outcome_task_failed_preserves_error() {
    let outcome: ExecutionOutcome<(), io::Error> =
        ExecutionOutcome::TaskFailed(io::Error::other("task failed"));

    match outcome {
        ExecutionOutcome::TaskFailed(error) => {
            assert_eq!(error.kind(), io::ErrorKind::Other);
            assert_eq!(error.to_string(), "task failed");
        }
        _ => panic!("expected a task failure"),
    }
}
