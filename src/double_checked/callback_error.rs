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

    /// Callback type label, when available.
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
