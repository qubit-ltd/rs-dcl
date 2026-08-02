// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Complete outcomes for lifecycle-aware double-checked execution.

use crate::double_checked::FinalizationOutcome;

/// Describes the single valid terminal state of a lifecycle invocation.
///
/// Commit outcomes occur only after task success. Rollback outcomes occur only
/// after preparation succeeded and locked execution did not complete
/// successfully, so invalid task/finalization combinations are not
/// representable.
///
/// # Type Parameters
///
/// * `R` - Value type returned by the guarded task.
/// * `E` - Error type returned by the guarded task.
/// * `C` - Error type returned by lifecycle callbacks.
#[derive(Debug)]
#[must_use = "the lifecycle outcome must be inspected"]
pub enum LifecycleOutcome<R, E, C> {
    /// The lock-free condition check returned `false` before prepare started.
    InitialConditionNotMet,
    /// Prepare returned its original lifecycle error without producing a token.
    PrepareFailed(C),
    /// The task returned a value and the token followed the commit path.
    TaskSucceeded {
        /// Original value returned by the task.
        value: R,
        /// Outcome of commit or of dropping a token that required no commit.
        commit: FinalizationOutcome<C>,
    },
    /// The second condition check returned `false` after prepare completed.
    SecondConditionNotMet {
        /// Outcome of rollback or of dropping a token that required no
        /// rollback.
        rollback: FinalizationOutcome<C>,
    },
    /// The task returned its original error and the token followed rollback.
    TaskFailed {
        /// Original error returned by the task.
        error: E,
        /// Outcome of rollback or of dropping a token that required no
        /// rollback.
        rollback: FinalizationOutcome<C>,
    },
}
