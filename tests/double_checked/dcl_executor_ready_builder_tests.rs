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
    DclExecutor,
    PanicPhase,
};

/// Verifies the ready builder applies panic-capture configuration.
#[test]
fn test_ready_builder_enables_panic_capture() {
    let executor = DclExecutor::builder()
        .when(|| true)
        
        .build();
    let panic = executor
        .run_catching(&std::sync::Mutex::new(()), || panic!("task"))
        .unwrap_err();
    assert_eq!(panic.phase(), PanicPhase::Task);
}
