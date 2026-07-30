// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the lifecycle commit builder stage.

use std::io;

use qubit_dcl::{
    FinalizationOutcome,
    LifecycleDclExecutor,
    LifecycleOutcome,
};

/// Verifies the commit stage supports the explicit no-rollback branch.
#[test]
fn test_commit_builder_selects_no_rollback() {
    let executor = LifecycleDclExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<u32, io::Error>(1))
        .commit(|_| Ok::<(), io::Error>(()))
        .no_rollback()
        .build();
    let outcome =
        executor.run(&std::sync::Mutex::new(()), || Ok::<(), io::Error>(()));

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskSucceeded {
            commit: FinalizationOutcome::Succeeded,
            ..
        }
    ));
}
