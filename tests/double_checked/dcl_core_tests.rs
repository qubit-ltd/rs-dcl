// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public regression tests for the internal DCL core.

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
    DoubleCheckedLockExecutor,
    ExecutionOutcome,
};

/// Verifies the shared core evaluates the predicate before and after locking.
#[test]
fn test_core_performs_two_condition_checks() {
    let checks = Arc::new(AtomicUsize::new(0));
    let executor = DoubleCheckedLockExecutor::builder()
        .when({
            let checks = Arc::clone(&checks);
            move || {
                checks.fetch_add(1, Ordering::Relaxed);
                true
            }
        })
        .build();
    let outcome =
        executor.run(&parking_lot::Mutex::new(()), || Ok::<(), io::Error>(()));

    assert!(matches!(outcome, ExecutionOutcome::Success(())));
    assert_eq!(checks.load(Ordering::Relaxed), 2);
}
