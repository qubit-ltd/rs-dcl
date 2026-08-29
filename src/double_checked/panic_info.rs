// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Captured panic metadata for double-checked execution.

use std::any::Any;
use std::fmt;
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::panic::resume_unwind;

use crate::double_checked::PanicPhase;

/// Preserves a captured panic payload together with its execution phase.
///
/// Unknown payload types remain available through [`Self::payload`] and
/// [`Self::into_payload`] but are deliberately not formatted by `Debug`.
/// Dropping this value discards any payload-destructor panic so a captured
/// panic does not re-propagate while its structured outcome is disposed.
#[must_use = "captured panic metadata must be inspected or resumed"]
pub struct PanicInfo {
    /// Phase in which the panic was captured.
    phase: PanicPhase,
    /// String message extracted from common panic payload types.
    message: Option<String>,
    /// Original panic payload used for inspection or resumed unwinding.
    payload: Option<Box<dyn Any + Send + 'static>>,
}

impl PanicInfo {
    /// Creates panic metadata from a caught unwind payload.
    ///
    /// # Parameters
    ///
    /// * `phase` - Phase active when unwinding crossed the capture boundary.
    /// * `payload` - Original payload returned by `catch_unwind`.
    ///
    /// # Returns
    ///
    /// Panic metadata that retains ownership of `payload`.
    #[inline]
    pub(crate) fn from_payload(phase: PanicPhase, payload: Box<dyn Any + Send + 'static>) -> Self {
        let message = payload
            .downcast_ref::<&str>()
            .map(|message| (*message).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned());
        Self {
            phase,
            message,
            payload: Some(payload),
        }
    }

    /// Returns the phase in which the panic occurred.
    ///
    /// # Returns
    ///
    /// The captured panic phase.
    #[inline(always)]
    pub fn phase(&self) -> PanicPhase {
        self.phase
    }

    /// Returns the message extracted from a string panic payload.
    ///
    /// # Returns
    ///
    /// `Some(message)` for `&str` and `String` payloads, or `None` for other
    /// payload types.
    #[inline(always)]
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Returns the original panic payload by reference.
    ///
    /// # Returns
    ///
    /// The payload retained from `catch_unwind`.
    ///
    /// # Panics
    ///
    /// Panics if the payload has already been consumed. This is an internal
    /// invariant because the method only borrows `self`.
    #[inline(always)]
    pub fn payload(&self) -> &(dyn Any + Send + 'static) {
        self.payload
            .as_deref()
            .expect("PanicInfo payload must exist while it is borrowed")
    }

    /// Consumes the metadata and returns the original panic payload.
    ///
    /// # Returns
    ///
    /// The owned payload suitable for `resume_unwind`.
    ///
    /// # Panics
    ///
    /// Panics if the payload has already been consumed. This is an internal
    /// invariant because the method consumes `self` only once.
    #[inline(always)]
    pub fn into_payload(mut self) -> Box<dyn Any + Send + 'static> {
        self.payload
            .take()
            .expect("PanicInfo payload must exist before it is consumed")
    }

    /// Resumes panicking with the captured payload.
    ///
    /// This does not rebuild or adapt the payload.
    ///
    /// # Panics
    ///
    /// Always unwinds with the original captured payload.
    #[inline]
    pub fn resume_unwind(self) -> ! {
        resume_unwind(self.into_payload())
    }
}

impl Drop for PanicInfo {
    /// Discards the retained payload without allowing a destructor panic to
    /// escape from structured panic metadata disposal.
    fn drop(&mut self) {
        let Some(payload) = self.payload.take() else {
            return;
        };
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
            std::mem::forget(payload);
        }
    }
}

impl fmt::Debug for PanicInfo {
    /// Formats the panic phase and optional string message without attempting
    /// to format an unknown payload type.
    ///
    /// # Errors
    ///
    /// Returns the formatter error if writing the debug representation fails.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PanicInfo")
            .field("phase", &self.phase)
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}
