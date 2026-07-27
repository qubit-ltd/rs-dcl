// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for panic phase classification.

use qubit_dcl::PanicPhase;

/// Verifies every public panic phase remains distinct.
#[test]
fn test_panic_phases_are_distinct() {
    let phases = [
        PanicPhase::InitialConditionCheck,
        PanicPhase::Prepare,
        PanicPhase::LockAcquisition,
        PanicPhase::SecondConditionCheck,
        PanicPhase::Task,
        PanicPhase::LockRelease,
        PanicPhase::Commit,
        PanicPhase::Rollback,
    ];

    for (index, phase) in phases.iter().enumerate() {
        assert!(!phases[index + 1..].contains(phase));
    }
}
