// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Task execution outcomes for double-checked execution.

/// Describes whether and how the guarded task was executed.
///
/// # Type Parameters
///
/// * `R` - Value type returned by the guarded task.
/// * `E` - Error type returned by the guarded task.
#[derive(Debug)]
#[must_use = "the execution outcome must be inspected"]
pub enum ExecutionOutcome<R, E> {
    /// The task ran while holding the executor lock and returned `R`.
    Success(R),
    /// A lock-free or lock-protected condition check returned `false`.
    ConditionNotMet,
    /// The task ran and returned its original error value.
    TaskFailed(E),
}

impl<R, E> ExecutionOutcome<R, E> {
    /// Converts the outcome to a result whose success value is optional.
    ///
    /// `Success(value)` becomes `Ok(Some(value))`, `ConditionNotMet` becomes
    /// `Ok(None)`, and `TaskFailed(error)` becomes `Err(error)`.
    ///
    /// # Type Parameters
    ///
    /// * `R` - Successful task result type.
    /// * `E` - Task error type.
    ///
    /// # Returns
    ///
    /// A result preserving the task error and distinguishing a rejected
    /// condition from a successful task.
    #[inline]
    pub fn into_result(self) -> Result<Option<R>, E> {
        match self {
            Self::Success(value) => Ok(Some(value)),
            Self::ConditionNotMet => Ok(None),
            Self::TaskFailed(error) => Err(error),
        }
    }
}
