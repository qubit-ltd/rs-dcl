/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! # Execution Result
//!
//! Provides the task execution result enum for double-checked locking.
//!
use std::fmt;

use crate::double_checked::{CallbackError, ExecutorError};

/// Task execution result
///
/// Represents the result of executing a task using an enum to clearly distinguish
/// between success, unmet conditions, and failure.
///
/// # Type Parameters
///
/// * `T` - The type of the return value when execution succeeds
/// * `E` - The type of the error when execution fails
///
/// # Examples
///
/// ```rust
/// use qubit_dcl::double_checked::{ExecutionResult, ExecutorError};
///
/// let success: ExecutionResult<i32, String> = ExecutionResult::success(42);
/// if let ExecutionResult::Success(val) = success {
///     println!("Value: {}", val);
/// }
///
/// let unmet: ExecutionResult<i32, String> = ExecutionResult::unmet();
///
/// let failed: ExecutionResult<i32, String> =
///     ExecutionResult::task_failed("Task failed".to_string());
/// ```
///
#[derive(Debug)]
pub enum ExecutionResult<T, E> {
    /// Execution succeeded with a value
    Success(T),

    /// Double-checked locking condition was not met
    ConditionNotMet,

    /// Execution failed with an error
    Failed(ExecutorError<E>),
}

impl<T, E> ExecutionResult<T, E> {
    /// Builds [`ExecutionResult::Success`] with `value`.
    ///
    /// # Parameters
    ///
    /// * `value` - Successful task value.
    ///
    /// # Returns
    ///
    /// A success result containing `value`.
    #[inline]
    pub fn success(value: T) -> Self {
        ExecutionResult::Success(value)
    }

    /// Builds [`ExecutionResult::ConditionNotMet`].
    ///
    /// # Returns
    ///
    /// A result representing a failed double-check condition.
    #[inline]
    pub fn unmet() -> Self {
        ExecutionResult::ConditionNotMet
    }

    /// Builds a failed result with [`ExecutorError::TaskFailed`].
    ///
    /// # Parameters
    ///
    /// * `err` - Error returned by the executed task.
    ///
    /// # Returns
    ///
    /// A failed result wrapping the task error.
    #[inline]
    pub fn task_failed(err: E) -> Self {
        ExecutionResult::Failed(ExecutorError::TaskFailed(err))
    }

    /// Builds a failed result with [`ExecutorError::PrepareFailed`].
    ///
    /// Accepts any [`std::fmt::Display`] value (including [`std::error::Error`] and [`String`]);
    /// the message is stored in a [`CallbackError`] wrapper.
    ///
    /// # Parameters
    ///
    /// * `msg` - Prepare error message or displayable error value.
    ///
    /// # Returns
    ///
    /// A failed result containing the prepare failure message.
    #[inline]
    pub fn prepare_failed(msg: impl fmt::Display) -> Self {
        ExecutionResult::Failed(ExecutorError::PrepareFailed(CallbackError::from_display(
            msg,
        )))
    }

    /// Builds a failed result with [`ExecutorError::PrepareCommitFailed`].
    ///
    /// # Parameters
    ///
    /// * `msg` - Commit error message or displayable error value.
    ///
    /// # Returns
    ///
    /// A failed result containing the prepare-commit failure message.
    #[inline]
    pub fn prepare_commit_failed(msg: impl fmt::Display) -> Self {
        ExecutionResult::Failed(ExecutorError::PrepareCommitFailed(
            CallbackError::from_display(msg),
        ))
    }

    /// Builds a failed result with [`ExecutorError::PrepareFailed`] and explicit
    /// callback type metadata.
    ///
    /// The callback type can later be read from
    /// [`ExecutorError::callback_type`].
    ///
    /// # Parameters
    ///
    /// * `callback_type` - Callback type tag, e.g. `"prepare"`.
    /// * `msg` - Failure message.
    #[inline]
    pub fn prepare_failed_with_type(
        callback_type: &'static str,
        msg: impl std::fmt::Display,
    ) -> Self {
        ExecutionResult::Failed(ExecutorError::PrepareFailed(CallbackError::with_type(
            callback_type,
            msg,
        )))
    }

    /// Builds a failed result with [`ExecutorError::PrepareRollbackFailed`].
    ///
    /// # Parameters
    ///
    /// * `original` - Original failure that triggered prepare rollback.
    /// * `rollback` - Failure produced by the rollback action.
    ///
    /// # Returns
    ///
    /// A failed result containing both original and rollback messages.
    #[inline]
    pub fn prepare_rollback_failed(
        original: impl Into<String>,
        rollback: impl Into<String>,
    ) -> Self {
        ExecutionResult::Failed(ExecutorError::PrepareRollbackFailed {
            original: CallbackError::from_display(original.into()),
            rollback: CallbackError::from_display(rollback.into()),
        })
    }

    /// Wraps an arbitrary [`ExecutorError`] as [`ExecutionResult::Failed`].
    ///
    /// # Parameters
    ///
    /// * `err` - Executor error to store in the failed result.
    ///
    /// # Returns
    ///
    /// A failed result containing `err`.
    #[inline]
    pub fn from_executor_error(err: ExecutorError<E>) -> Self {
        ExecutionResult::Failed(err)
    }

    /// Checks if the execution was successful.
    ///
    /// # Returns
    ///
    /// `true` if this result is [`ExecutionResult::Success`].
    #[inline]
    pub fn is_success(&self) -> bool {
        matches!(self, ExecutionResult::Success(_))
    }

    /// Checks if the condition was not met.
    ///
    /// # Returns
    ///
    /// `true` if this result is [`ExecutionResult::ConditionNotMet`].
    #[inline]
    pub fn is_unmet(&self) -> bool {
        matches!(self, ExecutionResult::ConditionNotMet)
    }

    /// Checks if the execution failed.
    ///
    /// # Returns
    ///
    /// `true` if this result is [`ExecutionResult::Failed`].
    #[inline]
    pub fn is_failed(&self) -> bool {
        matches!(self, ExecutionResult::Failed(_))
    }

    /// Converts the result to a standard Result
    ///
    /// # Returns
    ///
    /// * `Ok(Some(T))` - If execution was successful
    /// * `Ok(None)` - If condition was not met
    /// * `Err(ExecutorError<E>)` - If execution failed
    ///
    /// # Errors
    ///
    /// Returns the stored [`ExecutorError`] when this value is
    /// [`ExecutionResult::Failed`].
    #[inline]
    pub fn into_result(self) -> Result<Option<T>, ExecutorError<E>> {
        match self {
            ExecutionResult::Success(v) => Ok(Some(v)),
            ExecutionResult::ConditionNotMet => Ok(None),
            ExecutionResult::Failed(e) => Err(e),
        }
    }
}

impl<T, E> ExecutionResult<T, E>
where
    E: fmt::Display,
{
    /// Unwraps the success value, panicking if not successful
    ///
    /// # Returns
    ///
    /// The success value stored in [`ExecutionResult::Success`].
    ///
    /// # Panics
    ///
    /// Panics if this result is [`ExecutionResult::ConditionNotMet`] or
    /// [`ExecutionResult::Failed`].
    #[inline]
    pub fn unwrap(self) -> T {
        match self {
            ExecutionResult::Success(v) => v,
            ExecutionResult::ConditionNotMet => {
                panic!("Called unwrap on ExecutionResult::ConditionNotMet")
            }
            ExecutionResult::Failed(e) => {
                panic!("Called unwrap on ExecutionResult::Failed: {}", e)
            }
        }
    }
}
