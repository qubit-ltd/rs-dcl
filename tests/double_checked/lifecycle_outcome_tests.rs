// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for lifecycle execution outcomes.

use std::io;

use qubit_dcl::FinalizationOutcome;
use qubit_dcl::LifecycleOutcome;

/// Verifies every non-panic lifecycle branch remains distinguishable.
#[test]
fn test_lifecycle_outcome_non_panic_states_are_distinct() {
    let initial: LifecycleOutcome<(), io::Error, io::Error> =
        LifecycleOutcome::InitialConditionNotMet;
    let prepare: LifecycleOutcome<(), io::Error, io::Error> =
        LifecycleOutcome::PrepareFailed(io::Error::other("prepare"));
    let success: LifecycleOutcome<u32, io::Error, io::Error> =
        LifecycleOutcome::TaskSucceeded {
            value: 42,
            commit: FinalizationOutcome::Succeeded,
        };
    let second: LifecycleOutcome<(), io::Error, io::Error> =
        LifecycleOutcome::SecondConditionNotMet {
            rollback: FinalizationOutcome::Succeeded,
        };
    let task: LifecycleOutcome<(), io::Error, io::Error> =
        LifecycleOutcome::TaskFailed {
            error: io::Error::other("task"),
            rollback: FinalizationOutcome::Failed(io::Error::other("rollback")),
        };

    assert!(matches!(initial, LifecycleOutcome::InitialConditionNotMet));
    assert!(matches!(prepare, LifecycleOutcome::PrepareFailed(_)));
    assert!(matches!(
        success,
        LifecycleOutcome::TaskSucceeded {
            value: 42,
            commit: FinalizationOutcome::Succeeded,
        }
    ));
    assert!(matches!(
        second,
        LifecycleOutcome::SecondConditionNotMet {
            rollback: FinalizationOutcome::Succeeded,
        }
    ));
    assert!(matches!(
        task,
        LifecycleOutcome::TaskFailed {
            rollback: FinalizationOutcome::Failed(_),
            ..
        }
    ));
}
