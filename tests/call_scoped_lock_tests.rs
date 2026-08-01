// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regression tests for call-scoped, data-independent locks.

use std::{
    convert::Infallible,
    sync::{
        Arc,
        Mutex,
    },
};

use qubit_dcl::{
    DclExecutor,
    ExecutionOutcome,
};

/// Verifies one executor accepts a different generic lock on every call.
#[test]
fn test_executor_accepts_different_lock_types_per_call() {
    let executor = DclExecutor::new(|| true);
    let borrowed_lock = Mutex::new(());
    let shared_lock = Arc::new(Mutex::new(()));

    let first = executor.run(&borrowed_lock, || Ok::<_, Infallible>(1_u32));
    let second = executor.run(&shared_lock, || Ok::<_, Infallible>(2_u32));

    assert!(matches!(first, ExecutionOutcome::Success(1)));
    assert!(matches!(second, ExecutionOutcome::Success(2)));
}
