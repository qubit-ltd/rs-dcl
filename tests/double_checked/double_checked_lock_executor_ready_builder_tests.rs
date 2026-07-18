// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the ready basic-executor builder stage.

use std::io;

use qubit_dcl::{
    DoubleCheckedLockExecutor,
    ExecutionOutcome,
};

/// Verifies the ready builder applies panic-capture configuration.
#[test]
fn test_ready_builder_enables_panic_capture() {
    let executor = DoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(true)
        .build();
    let outcome: ExecutionOutcome<(), io::Error> =
        executor.run(&parking_lot::Mutex::new(()), || panic!("task"));

    assert!(matches!(outcome, ExecutionOutcome::Panicked(_)));
}
