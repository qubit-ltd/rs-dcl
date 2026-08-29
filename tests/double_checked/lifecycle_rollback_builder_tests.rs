// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the lifecycle rollback builder stage.

use std::io;

use qubit_dcl::FinalizationOutcome;
use qubit_dcl::LifecycleDclExecutor;
use qubit_dcl::LifecycleOutcome;

/// Verifies rollback configuration receives a failed task outcome.
#[test]
fn test_rollback_builder_configures_rollback() {
    let executor = LifecycleDclExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<u32, io::Error>(1))
        .no_commit()
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();
    let outcome = executor.run(&std::sync::Mutex::new(()), || Err::<(), _>(io::Error::other("task")));

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskFailed {
            rollback: FinalizationOutcome::Succeeded,
            ..
        }
    ));
}
