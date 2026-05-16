/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! # Execution Context
//!
//! Provides execution context after double-checked lock task execution.
//!
use crate::double_checked::{
    execution_result::ExecutionResult,
    executor_error::ExecutorError,
};

/// Execution context (state after task execution)
///
/// This type provides result retrieval functionality after task execution.
///
/// Prepare lifecycle callbacks are configured on
/// [`super::DoubleCheckedLockExecutor`] and are already applied before an
/// `ExecutionContext` is returned. Task closures are responsible for their own
/// rollback, cleanup, and commit logic.
///
/// # Type Parameters
///
/// * `T` - The type of the task return value
/// * `E` - The type of the task error
///
pub struct ExecutionContext<T, E>
where
    E: std::fmt::Display,
{
    /// Result produced by the double-checked execution.
    result: ExecutionResult<T, E>,
}

impl<T, E> ExecutionContext<T, E>
where
    E: std::fmt::Display,
{
    /// Creates a new execution context.
    ///
    /// # Parameters
    ///
    /// * `result` - The execution result
    ///
    /// # Returns
    ///
    /// A context wrapping the supplied result.
    #[inline]
    pub(super) fn new(result: ExecutionResult<T, E>) -> Self {
        Self { result }
    }

    /// Gets the execution result (consumes the context)
    ///
    /// Prepare commit or rollback callbacks have already been executed by the
    /// builder before this context was created. This method does not trigger
    /// additional side effects.
    ///
    /// # Returns
    ///
    /// The owned [`ExecutionResult`] stored in this context.
    #[inline]
    pub fn get_result(self) -> ExecutionResult<T, E> {
        self.result
    }

    /// Checks the execution result (does not consume the context)
    ///
    /// # Returns
    ///
    /// A shared reference to the stored [`ExecutionResult`].
    #[inline]
    pub fn peek_result(&self) -> &ExecutionResult<T, E> {
        &self.result
    }

    /// Checks if execution was successful
    ///
    /// # Returns
    ///
    /// `true` if the stored result is [`ExecutionResult::Success`].
    #[inline]
    pub fn is_success(&self) -> bool {
        self.result.is_success()
    }
}

// Convenience methods for cases without return values
impl<E> ExecutionContext<(), E>
where
    E: std::fmt::Display,
{
    /// Completes execution (for operations without return values)
    ///
    /// Returns whether the execution was successful. This convenience method
    /// intentionally collapses both unmet conditions and execution failures to
    /// `false`; use [`Self::try_finish`] when the failure details must be
    /// preserved.
    ///
    /// # Returns
    ///
    /// `true` if the stored result is [`ExecutionResult::Success`] containing
    /// `()`.
    #[inline]
    pub fn finish(self) -> bool {
        let result = self.get_result();
        result.is_success()
    }

    /// Completes execution while preserving failure details.
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Execution succeeded with `()`.
    /// * `Ok(false)` - The double-checked condition was not met.
    /// * `Err(ExecutorError<E>)` - Execution failed, preserving the original
    ///   executor error.
    ///
    /// # Errors
    ///
    /// Returns the stored [`ExecutorError`] when the underlying result is
    /// [`ExecutionResult::Failed`].
    #[inline]
    pub fn try_finish(self) -> Result<bool, ExecutorError<E>> {
        match self.get_result() {
            ExecutionResult::Success(()) => Ok(true),
            ExecutionResult::ConditionNotMet => Ok(false),
            ExecutionResult::Failed(error) => Err(error),
        }
    }
}
