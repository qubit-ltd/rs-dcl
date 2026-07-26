// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Prepare lifecycle outcomes for double-checked execution.

use crate::double_checked::PanicInfo;

/// Describes the lifecycle work performed around a guarded task.
#[derive(Debug)]
#[must_use = "the preparation outcome must be inspected"]
pub enum PreparationOutcome<C> {
    /// The initial condition failed before prepare started.
    NotStarted,
    /// Prepare returned the original lifecycle error and produced no token.
    PrepareFailed(C),
    /// Prepare panicked and produced no usable token.
    PreparePanicked(PanicInfo),
    /// The successful path explicitly declared that no commit was required.
    CommitNotRequired,
    /// Commit consumed the token successfully.
    Committed,
    /// Commit consumed the token and returned the original lifecycle error.
    CommitFailed(C),
    /// Commit finalization panicked while consuming the token.
    CommitPanicked(PanicInfo),
    /// The unsuccessful path explicitly declared that no rollback was
    /// required.
    RollbackNotRequired,
    /// Rollback consumed the token successfully.
    RolledBack,
    /// Rollback consumed the token and returned the original lifecycle error.
    RollbackFailed(C),
    /// Rollback finalization panicked while consuming the token.
    RollbackPanicked(PanicInfo),
}
