// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public regression tests for internal panic capture.

use std::{
    io,
    sync::Mutex,
};

use qubit_dcl::{
    DclExecutor,
    ExecutionOutcome,
    PanicPhase,
};

/// Verifies a captured task panic retains its phase and message.
#[test]
fn test_panic_capture_retains_task_context() {
    let executor = DclExecutor::builder()
        .when(|| true)
        
        .build();
    let panic = executor
        .run_catching(&Mutex::new(()), || panic!("captured"))
        .unwrap_err();

    assert_eq!(panic.phase(), PanicPhase::Task);
    assert_eq!(panic.message(), Some("captured"));
}
