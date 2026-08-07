// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Standard-library lock coverage for the minimal feature selection.

use std::io;
use std::sync::Mutex;

use qubit_dcl::DclExecutor;
use qubit_dcl::ExecutionOutcome;
use qubit_dcl::FinalizationOutcome;
use qubit_dcl::LifecycleDclExecutor;
use qubit_dcl::LifecycleOutcome;

/// Verifies both executors support standard-library mutexes without the
/// optional parking-lot lock implementations.
#[test]
fn test_standard_mutexes_work_without_parking_lot_feature() {
    let basic_lock = Mutex::new(());
    let basic_executor = DclExecutor::new(|| true);
    let basic_outcome =
        basic_executor.run(&basic_lock, || Ok::<usize, io::Error>(7));
    assert!(matches!(basic_outcome, ExecutionOutcome::Success(7)));

    let lifecycle_lock = Mutex::new(());
    let lifecycle_executor = LifecycleDclExecutor::builder()
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
