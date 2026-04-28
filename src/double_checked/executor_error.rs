/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! # Executor Error
//!
//! Provides executor error types for the double-checked lock executor.
//!
//! # Author
//!
//! Haixing Hu
// qubit-style: allow multiple-public-types

use std::error::Error;
use std::fmt;

/// Common error information for prepare lifecycle callbacks.
///
/// This keeps the error type name together with the message so callers can
/// classify failures without depending on fragile string matching.
///
/// # Examples
///
/// ```rust
/// use qubit_dcl::double_checked::CallbackError;
///
/// let prepare_error = CallbackError::with_type("prepare", "Resource is locked");
/// assert_eq!(prepare_error.callback_type(), Some("prepare"));
/// println!("prepare_error = {:?}", prepare_error);
/// ```
#[derive(Debug, Clone)]
pub struct CallbackError {
    /// Error message produced by the callback.
    message: String,

    /// Concrete type name, when available.
    callback_type: Option<&'static str>,
}

impl CallbackError {
    /// Builds a callback error without type metadata.
    #[inline]
    pub fn from_display<T: fmt::Display>(error: T) -> Self {
        Self {
            message: error.to_string(),
            callback_type: None,
        }
    }

    /// Builds a callback error with explicit callback type metadata.
    #[inline]
    pub fn with_type<T: fmt::Display>(source_type: &'static str, error: T) -> Self {
        Self {
            message: error.to_string(),
            callback_type: Some(source_type),
        }
    }

    /// Returns the raw message.
    #[inline]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the callback type label, when available.
    #[inline]
    pub fn callback_type(&self) -> Option<&'static str> {
        self.callback_type
    }

    /// Returns whether the callback type label is set.
    #[inline]
    pub fn is_typed(&self) -> bool {
        self.callback_type.is_some()
    }
}

impl fmt::Display for CallbackError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.callback_type {
            Some(callback_type) => write!(f, "{}: {}", callback_type, self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

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
/// # Author
///
/// Haixing Hu
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
            ExecutorError::PrepareRollbackFailed { original, rollback } => {
                write!(
                    f,
                    "Prepare rollback failed: original error = {}, rollback error = {}",
                    original, rollback
                )
            }
        }
    }
}

impl<E> ExecutorError<E> {
    /// Returns the callback type label, when the error comes from a callback and
    /// the type is available.
    ///
    /// This returns `None` for task failures and callback errors without
    /// associated type labels.
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
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ExecutorError::TaskFailed(error) => Some(error),
            ExecutorError::Panic(_)
            | ExecutorError::PrepareFailed(_)
            | ExecutorError::PrepareCommitFailed(_)
            | ExecutorError::PrepareRollbackFailed { .. } => None,
        }
    }
}
