// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public regression tests for execution inside the acquired lock.

use std::{
    io,
    sync::{
        Arc,
        atomic::{
            AtomicUsize,
            Ordering,
        },
    },
};

use qubit_dcl::{
    DclExecutor,
    ExecutionOutcome,
};

/// Verifies a failed locked check prevents task execution.
#[test]
fn test_locked_execution_rechecks_before_task() {
    let checks = Arc::new(AtomicUsize::new(0));
    let task_calls = AtomicUsize::new(0);
    let executor = DclExecutor::builder()
        .when({
            let checks = Arc::clone(&checks);
            move || checks.fetch_add(1, Ordering::Relaxed) == 0
        })
        .build();
    let outcome = executor.run(&std::sync::Mutex::new(()), || {
        task_calls.fetch_add(1, Ordering::Relaxed);
        Ok::<(), io::Error>(())
    });

    assert!(matches!(outcome, ExecutionOutcome::ConditionNotMet));
    assert_eq!(task_calls.load(Ordering::Relaxed), 0);
}
