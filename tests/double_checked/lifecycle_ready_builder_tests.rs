// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the ready lifecycle builder stage.

use std::io;

use qubit_dcl::{
    FinalizationOutcome,
    LifecycleDclExecutor,
    LifecycleOutcome,
};

/// Verifies a ready lifecycle builder produces a reusable executor.
#[test]
fn test_ready_builder_builds_reusable_executor() {
    let executor = LifecycleDclExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<(), io::Error>(()))
        .no_commit()
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();
    let lock = std::sync::Mutex::new(());

    for value in [1, 2] {
        let outcome = executor.run(&lock, || Ok::<u32, io::Error>(value));
        assert!(matches!(
            outcome,
            LifecycleOutcome::TaskSucceeded {
                value: actual,
                commit: FinalizationOutcome::NotRequired,
            } if actual == value
        ));
    }
}
