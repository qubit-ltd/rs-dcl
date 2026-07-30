// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the initial basic-executor builder stage.

use std::io;

use qubit_dcl::{
    DclExecutor,
    ExecutionOutcome,
};

/// Verifies the initial builder accepts a predicate and produces an executor.
#[test]
fn test_builder_accepts_predicate() {
    let executor = DclExecutor::builder().when(|| true).build();
    let outcome =
        executor.run(&std::sync::Mutex::new(()), || Ok::<u32, io::Error>(7));

    assert!(matches!(outcome, ExecutionOutcome::Success(7)));
}
