/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! # Callback Error
//!
//! Provides callback error information for double-checked execution.
//!

use std::fmt;

/// Common error information for prepare lifecycle callbacks.
///
/// This keeps a semantic callback type label together with the message so
/// callers can classify failures without depending on fragile string matching.
///
/// # Examples
///
/// ```rust
/// use qubit_dcl::double_checked::CallbackError;
///
/// let prepare_error = CallbackError::with_callback_type("prepare", "Resource is locked");
/// assert_eq!(prepare_error.callback_type(), Some("prepare"));
/// println!("prepare_error = {:?}", prepare_error);
/// ```
#[derive(Debug, Clone)]
pub struct CallbackError {
    /// Error message produced by the callback.
    message: String,

    /// Callback type label, when available.
    callback_type: Option<&'static str>,
}

impl CallbackError {
    /// Builds a callback error without type metadata.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Displayable error or message value to store.
    ///
    /// # Parameters
    ///
    /// * `error` - Error value whose display text becomes this callback
    ///   error's message.
    ///
    /// # Returns
    ///
    /// A callback error with no callback type label.
    #[inline]
    pub fn from_display<T: fmt::Display>(error: T) -> Self {
        Self {
            message: error.to_string(),
            callback_type: None,
        }
    }

    /// Builds a callback error with explicit callback type metadata.
    ///
    /// # Parameters
    ///
    /// * `callback_type` - Semantic callback type label, such as `"prepare"`
    ///   or `"prepare_rollback"`.
    /// * `error` - Error message or displayable error value produced by the
    ///   callback.
    ///
    /// # Returns
    ///
    /// A callback error with `callback_type` stored as metadata.
    #[inline]
    pub fn with_callback_type<T: fmt::Display>(callback_type: &'static str, error: T) -> Self {
        Self {
            message: error.to_string(),
            callback_type: Some(callback_type),
        }
    }

    /// Returns the raw message.
    ///
    /// # Returns
    ///
    /// The message captured from the original displayable error value.
    #[inline]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the callback type label, when available.
    ///
    /// # Returns
    ///
    /// `Some(label)` when this error was built with callback type metadata, or
    /// `None` when no metadata is available.
    #[inline]
    pub fn callback_type(&self) -> Option<&'static str> {
        self.callback_type
    }

    /// Returns whether the callback type label is set.
    ///
    /// # Returns
    ///
    /// `true` when [`Self::callback_type`] would return [`Some`].
    #[inline]
    pub fn is_typed(&self) -> bool {
        self.callback_type.is_some()
    }
}

impl fmt::Display for CallbackError {
    /// Formats this callback error for user-facing diagnostics.
    ///
    /// # Parameters
    ///
    /// * `f` - Formatter receiving the formatted callback error.
    ///
    /// # Returns
    ///
    /// [`fmt::Result`] from writing the callback label and message.
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.callback_type {
            Some(callback_type) => write!(f, "{}: {}", callback_type, self.message),
            None => write!(f, "{}", self.message),
        }
    }
}
