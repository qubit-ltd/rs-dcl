// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the lifecycle prepare builder stage.

use std::io;
use std::sync::Mutex;

use qubit_dcl::FinalizationOutcome;
use qubit_dcl::LifecycleDclExecutor;
use qubit_dcl::LifecycleOutcome;

/// Verifies the prepare stage supports the explicit no-commit branch.
#[test]
fn test_prepare_builder_selects_no_commit() {
    let executor = LifecycleDclExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<u32, io::Error>(1))
        .no_commit()
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();
    let outcome = executor.run(&Mutex::new(()), || Ok::<(), io::Error>(()));

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskSucceeded {
            commit: FinalizationOutcome::NotRequired,
            ..
        }
    ));
}
