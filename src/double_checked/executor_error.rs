/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! # Executor Error
//!
//! Provides executor error types for the double-checked lock executor.
//!

use std::error::Error;
use std::fmt;

use super::CallbackError;

/// Executor error types.
///
/// # Type Parameters
///
/// * `E` - The original error type from task execution
///
/// # Examples
///
/// ```rust
/// use qubit_dcl::double_checked::ExecutorError;
/// use qubit_dcl::double_checked::CallbackError;
///
/// let error: ExecutorError<String> =
///     ExecutorError::TaskFailed("task failed".to_string());
/// println!("Error: {}", error);
///
/// let error_with_msg: ExecutorError<String> =
///     ExecutorError::PrepareFailed(CallbackError::from_display("Service is not running"));
/// println!("Error: {}", error_with_msg);
/// ```
///
///
#[derive(Debug)]
pub enum ExecutorError<E> {
    /// Task execution failed with original error
    TaskFailed(E),

    /// Task execution panicked.
    Panic(CallbackError),

    /// Preparation action failed
    PrepareFailed(CallbackError),

    /// Commit action for a successfully completed prepare action failed.
    PrepareCommitFailed(CallbackError),

    /// Rollback action for a successfully completed prepare action failed.
    PrepareRollbackFailed {
        /// The original error that triggered the rollback
        original: CallbackError,
        /// The error that occurred during prepare rollback
        rollback: CallbackError,
    },
}

impl<E> fmt::Display for ExecutorError<E>
where
    E: fmt::Display,
{
    /// Formats this executor error for user-facing diagnostics.
    ///
    /// # Parameters
    ///
    /// * `f` - Formatter receiving the human-readable error text.
    ///
    /// # Returns
    ///
    /// [`fmt::Result`] from writing the formatted error text.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutorError::TaskFailed(e) => {
                write!(f, "Task execution failed: {}", e)
            }
            ExecutorError::Panic(error) => {
                write!(f, "Execution panicked: {}", error)
            }
            ExecutorError::PrepareFailed(msg) => {
                write!(f, "Preparation action failed: {}", msg)
            }
            ExecutorError::PrepareCommitFailed(msg) => {
                write!(f, "Prepare commit action failed: {}", msg)
            }
            #[rustfmt::skip]
            ExecutorError::PrepareRollbackFailed { original, rollback } => write!(f, "Prepare rollback failed: original error = {original}, rollback error = {rollback}"),
        }
    }
}

impl<E> ExecutorError<E> {
    /// Returns the callback type label, when the error comes from a callback and
    /// the type is available.
    ///
    /// This returns `None` for task failures and callback errors without
    /// associated type labels.
    ///
    /// # Returns
    ///
    /// `Some(label)` for callback failures that carry callback type metadata,
    /// or `None` for task failures and untyped callback failures.
    #[inline]
    pub fn callback_type(&self) -> Option<&'static str> {
        match self {
            ExecutorError::TaskFailed(_) => None,
            ExecutorError::Panic(error) => error.callback_type(),
            ExecutorError::PrepareFailed(error) => error.callback_type(),
            ExecutorError::PrepareCommitFailed(error) => error.callback_type(),
            ExecutorError::PrepareRollbackFailed { original, rollback } => {
                rollback.callback_type().or(original.callback_type())
            }
        }
    }
}

impl<E> Error for ExecutorError<E>
where
    E: Error + 'static,
{
    /// Returns the underlying task error as the standard error source.
    ///
    /// Prepare lifecycle failures store their messages as strings and therefore
    /// do not expose a structured source error.
    ///
    /// # Returns
    ///
    /// `Some(error)` for [`ExecutorError::TaskFailed`], or `None` for panic and
    /// prepare lifecycle failures.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ExecutorError::TaskFailed(error) => Some(error),
            ExecutorError::Panic(_) => None,
            ExecutorError::PrepareFailed(_) => None,
            ExecutorError::PrepareCommitFailed(_) => None,
            ExecutorError::PrepareRollbackFailed { .. } => None,
        }
    }
}
