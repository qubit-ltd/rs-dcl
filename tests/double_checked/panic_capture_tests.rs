// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public regression tests for internal panic capture.

use std::io;

use qubit_dcl::{
    DoubleCheckedLockExecutor,
    ExecutionOutcome,
    PanicPhase,
};

/// Verifies a captured task panic retains its phase and message.
#[test]
fn test_panic_capture_retains_task_context() {
    let executor = DoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(true)
        .build();
    let outcome: ExecutionOutcome<(), io::Error> =
        executor.run(&parking_lot::Mutex::new(()), || panic!("captured"));

    assert!(matches!(
        outcome,
        ExecutionOutcome::Panicked(panic)
            if panic.phase() == PanicPhase::Task
                && panic.message() == Some("captured")
    ));
}
