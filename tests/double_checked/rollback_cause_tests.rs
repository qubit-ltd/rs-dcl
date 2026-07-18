// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for structured rollback causes.

use std::{
    error::Error,
    io,
};

use qubit_dcl::RollbackCause;

/// Verifies the condition failure cause carries no fabricated error.
#[test]
fn test_rollback_cause_condition_not_met_is_distinct() {
    let cause = RollbackCause::ConditionNotMet;

    assert!(matches!(cause, RollbackCause::ConditionNotMet));
}

/// Verifies rollback can inspect the original concrete task error by
/// reference.
#[test]
fn test_rollback_cause_task_failed_borrows_original_error() {
    let error = io::Error::other("task failed");
    let error_view: &(dyn Error + Send + Sync + 'static) = &error;
    let cause = RollbackCause::TaskFailed(error_view);

    match cause {
        RollbackCause::TaskFailed(view) => {
            let original = view
                .downcast_ref::<io::Error>()
                .expect("rollback should receive the original io::Error");
            assert!(std::ptr::eq(original, &error));
        }
        _ => panic!("expected a task failure rollback cause"),
    }
}
