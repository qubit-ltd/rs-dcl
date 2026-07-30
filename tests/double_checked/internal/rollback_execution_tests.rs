// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Behavior tests for lifecycle rollback execution state.

use std::{
    io,
    sync::{
        Arc,
        atomic::{
            AtomicBool,
            Ordering,
        },
    },
};

use qubit_dcl::{
    FinalizationOutcome,
    LifecycleDclExecutor,
    LifecycleOutcome,
};

/// Verifies a second condition failure maps to rollback after prepare
/// completes.
#[test]
fn test_second_condition_failure_reports_not_required_rollback() {
    let gate = Arc::new(AtomicBool::new(true));
    let executor = LifecycleDclExecutor::builder()
        .when({
            let gate = Arc::clone(&gate);
            move || gate.load(Ordering::Acquire)
        })
        .prepare({
            let gate = Arc::clone(&gate);
            move || {
                gate.store(false, Ordering::Release);
                Ok::<(), io::Error>(())
            }
        })
        .commit(|_| Ok::<(), io::Error>(()))
        .no_rollback()
        .build();

    let outcome =
        executor.run(&std::sync::Mutex::new(()), || Ok::<(), io::Error>(()));

    assert!(matches!(
        outcome,
        LifecycleOutcome::SecondConditionNotMet {
            rollback: FinalizationOutcome::NotRequired,
        }
    ));
}
