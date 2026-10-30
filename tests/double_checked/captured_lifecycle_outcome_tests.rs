// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for captured lifecycle execution outcomes.

use std::io;
use std::sync::Mutex;

use qubit_dcl::CapturedFinalizationOutcome;
use qubit_dcl::CapturedLifecycleOutcome;
use qubit_dcl::LifecycleDclExecutor;
use qubit_dcl::PanicPhase;

use crate::support::PanicOnDrop;

/// Verifies every non-panic captured lifecycle branch remains distinguishable.
#[test]
fn test_captured_lifecycle_outcome_non_panic_states_are_distinct() {
    let initial: CapturedLifecycleOutcome<(), io::Error, io::Error> = CapturedLifecycleOutcome::InitialConditionNotMet;
    let prepare: CapturedLifecycleOutcome<(), io::Error, io::Error> =
        CapturedLifecycleOutcome::PrepareFailed(io::Error::other("prepare"));
    let success: CapturedLifecycleOutcome<u32, io::Error, io::Error> = CapturedLifecycleOutcome::TaskSucceeded {
        value: 42,
        commit: CapturedFinalizationOutcome::Succeeded,
    };
    let second: CapturedLifecycleOutcome<(), io::Error, io::Error> = CapturedLifecycleOutcome::SecondConditionNotMet {
        rollback: CapturedFinalizationOutcome::Succeeded,
    };
    let task: CapturedLifecycleOutcome<(), io::Error, io::Error> = CapturedLifecycleOutcome::TaskFailed {
        error: io::Error::other("task"),
        rollback: CapturedFinalizationOutcome::Failed(io::Error::other("rollback")),
    };

    assert!(matches!(initial, CapturedLifecycleOutcome::InitialConditionNotMet));
    assert!(matches!(prepare, CapturedLifecycleOutcome::PrepareFailed(_)));
    assert!(matches!(
        success,
        CapturedLifecycleOutcome::TaskSucceeded {
            value: 42,
            commit: CapturedFinalizationOutcome::Succeeded,
        }
    ));
    assert!(matches!(
        second,
        CapturedLifecycleOutcome::SecondConditionNotMet {
            rollback: CapturedFinalizationOutcome::Succeeded,
        }
    ));
    assert!(matches!(
        task,
        CapturedLifecycleOutcome::TaskFailed {
            rollback: CapturedFinalizationOutcome::Failed(_),
            ..
        }
    ));
}

/// Verifies the new cleanup-panic outcome carries its dedicated phase.
#[test]
fn test_task_cleanup_panic_branch_is_distinct() {
    let executor = LifecycleDclExecutor::builder()
        .when(|| false)
        .prepare(|| Ok::<(), io::Error>(()))
        .commit(|_| Ok::<(), io::Error>(()))
        .no_rollback()
        .build();
    let captured = PanicOnDrop;
    let outcome = executor.run_catching(&Mutex::new(()), move || {
        drop(captured);
        Ok::<(), io::Error>(())
    });

    assert!(matches!(
        outcome,
        CapturedLifecycleOutcome::TaskCleanupPanicked(panic)
            if panic.phase() == PanicPhase::TaskCleanup
    ));
}
