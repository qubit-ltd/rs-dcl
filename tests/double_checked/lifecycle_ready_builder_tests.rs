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
    ExecutionOutcome,
    LifecycleDoubleCheckedLockExecutor,
};

/// Verifies a ready lifecycle builder produces a reusable executor.
#[test]
fn test_ready_builder_builds_reusable_executor() {
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<(), io::Error>(()))
        .no_commit()
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();
    let lock = parking_lot::Mutex::new(());

    for value in [1, 2] {
        let report = executor.run(&lock, || Ok::<u32, io::Error>(value));
        assert!(
            matches!(report.execution(), ExecutionOutcome::Success(v) if *v == value)
        );
    }
}
