// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for lifecycle preparation outcomes.

use std::io;

use qubit_dcl::PreparationOutcome;

/// Verifies every non-panic lifecycle terminal state remains distinguishable.
#[test]
fn test_preparation_outcome_non_panic_states_are_distinct() {
    let outcomes: [PreparationOutcome<io::Error>; 8] = [
        PreparationOutcome::NotStarted,
        PreparationOutcome::PrepareFailed(io::Error::other("prepare")),
        PreparationOutcome::CommitNotRequired,
        PreparationOutcome::Committed,
        PreparationOutcome::CommitFailed(io::Error::other("commit")),
        PreparationOutcome::RollbackNotRequired,
        PreparationOutcome::RolledBack,
        PreparationOutcome::RollbackFailed(io::Error::other("rollback")),
    ];

    assert!(matches!(outcomes[0], PreparationOutcome::NotStarted));
    assert!(matches!(outcomes[1], PreparationOutcome::PrepareFailed(_)));
    assert!(matches!(outcomes[2], PreparationOutcome::CommitNotRequired));
    assert!(matches!(outcomes[3], PreparationOutcome::Committed));
    assert!(matches!(outcomes[4], PreparationOutcome::CommitFailed(_)));
    assert!(matches!(
        outcomes[5],
        PreparationOutcome::RollbackNotRequired
    ));
    assert!(matches!(outcomes[6], PreparationOutcome::RolledBack));
    assert!(matches!(outcomes[7], PreparationOutcome::RollbackFailed(_)));
}
