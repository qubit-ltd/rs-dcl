// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for captured lifecycle finalization outcomes.

use std::io;

use qubit_dcl::CapturedFinalizationOutcome;

/// Verifies every non-panic captured finalization state remains distinct.
#[test]
fn test_captured_finalization_outcome_non_panic_states_are_distinct() {
    let outcomes: [CapturedFinalizationOutcome<io::Error>; 3] = [
        CapturedFinalizationOutcome::NotRequired,
        CapturedFinalizationOutcome::Succeeded,
        CapturedFinalizationOutcome::Failed(io::Error::other("finalization")),
    ];

    assert!(matches!(outcomes[0], CapturedFinalizationOutcome::NotRequired));
    assert!(matches!(outcomes[1], CapturedFinalizationOutcome::Succeeded));
    assert!(matches!(outcomes[2], CapturedFinalizationOutcome::Failed(_)));
}
