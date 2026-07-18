// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Result produced entirely within the executor lock.

use crate::double_checked::ExecutionOutcome;

/// Distinguishes a failed second check from an executed task result.
#[must_use = "the locked execution result must be converted into an outcome"]
pub(crate) enum LockedExecution<R, E> {
    /// The condition was no longer satisfied after the lock was acquired.
    ConditionNotMet,
    /// The task ran while the executor lock was held.
    Task(Result<R, E>),
}

impl<R, E> LockedExecution<R, E> {
    /// Converts a completed locked phase into its public execution outcome.
    ///
    /// # Returns
    ///
    /// The corresponding condition, success, or task-failure outcome.
    #[inline]
    pub(crate) fn into_outcome(self) -> ExecutionOutcome<R, E> {
        match self {
            Self::ConditionNotMet => ExecutionOutcome::ConditionNotMet,
            Self::Task(Ok(value)) => ExecutionOutcome::Success(value),
            Self::Task(Err(error)) => ExecutionOutcome::TaskFailed(error),
        }
    }
}
