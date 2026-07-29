// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Standard-library lock coverage for the minimal feature selection.

use std::{
    io,
    sync::Mutex,
};

use qubit_dcl::{
    DoubleCheckedLockExecutor,
    ExecutionOutcome,
    FinalizationOutcome,
    LifecycleDoubleCheckedLockExecutor,
    LifecycleOutcome,
};

/// Verifies both executors support standard-library mutexes without the
/// optional parking-lot lock implementations.
#[test]
fn test_standard_mutexes_work_without_parking_lot_feature() {
    let basic_lock = Mutex::new(());
    let basic_executor =
        DoubleCheckedLockExecutor::builder().when(|| true).build();
    let basic_outcome =
        basic_executor.run(&basic_lock, || Ok::<usize, io::Error>(7));
    assert!(matches!(basic_outcome, ExecutionOutcome::Success(7)));

    let lifecycle_lock = Mutex::new(());
    let lifecycle_executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<(), io::Error>(()))
        .commit(|_| Ok::<(), io::Error>(()))
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();
    let lifecycle_outcome =
        lifecycle_executor.run(&lifecycle_lock, || Ok::<usize, io::Error>(11));
    assert!(matches!(
        lifecycle_outcome,
        LifecycleOutcome::TaskSucceeded {
            value: 11,
            commit: FinalizationOutcome::Succeeded,
        }
    ));
}
