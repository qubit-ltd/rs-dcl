// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the initial lifecycle builder stage.

use std::io;

use qubit_dcl::{
    ExecutionOutcome,
    LifecycleDoubleCheckedLockExecutor,
};

/// Verifies the lifecycle builder starts with predicate selection.
#[test]
fn test_lifecycle_builder_accepts_predicate() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| false)
        .prepare(|| Ok::<(), io::Error>(()))
        .no_commit()
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();
    let report =
        executor.run(&parking_lot::Mutex::new(()), || Ok::<(), io::Error>(()));

    assert!(matches!(
        report.execution(),
        ExecutionOutcome::ConditionNotMet
    ));
}
