// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for lifecycle finalization outcomes.

use std::io;

use qubit_dcl::FinalizationOutcome;

/// Verifies every non-panic finalization state remains distinguishable.
#[test]
fn test_finalization_outcome_non_panic_states_are_distinct() {
    let outcomes: [FinalizationOutcome<io::Error>; 3] = [
        FinalizationOutcome::NotRequired,
        FinalizationOutcome::Succeeded,
        FinalizationOutcome::Failed(io::Error::other("finalization")),
    ];

    assert!(matches!(outcomes[0], FinalizationOutcome::NotRequired));
    assert!(matches!(outcomes[1], FinalizationOutcome::Succeeded));
    assert!(matches!(outcomes[2], FinalizationOutcome::Failed(_)));
}
