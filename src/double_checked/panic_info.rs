// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Captured panic metadata for double-checked execution.

use std::{
    any::Any,
    fmt,
};

use crate::double_checked::PanicPhase;

/// Preserves a captured panic payload together with its execution phase.
///
/// Unknown payload types remain available through [`Self::payload`] and
/// [`Self::into_payload`] but are deliberately not formatted by `Debug`.
pub struct PanicInfo {
    /// Phase in which the panic was captured.
    phase: PanicPhase,
    /// String message extracted from common panic payload types.
    message: Option<String>,
    /// Original panic payload used for inspection or resumed unwinding.
    payload: Box<dyn Any + Send + 'static>,
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
    pub(crate) fn from_payload(
        phase: PanicPhase,
        payload: Box<dyn Any + Send + 'static>,
    ) -> Self {
        let message = payload
            .downcast_ref::<&str>()
            .map(|message| (*message).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned());
        Self {
            phase,
            message,
            payload,
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
    #[inline(always)]
    pub fn payload(&self) -> &(dyn Any + Send + 'static) {
        self.payload.as_ref()
    }

    /// Consumes the metadata and returns the original panic payload.
    ///
    /// # Returns
    ///
    /// The owned payload suitable for `resume_unwind`.
    #[inline(always)]
    pub fn into_payload(self) -> Box<dyn Any + Send + 'static> {
        self.payload
    }
}

impl fmt::Debug for PanicInfo {
    /// Formats the panic phase and optional string message without attempting
    /// to format an unknown payload type.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PanicInfo")
            .field("phase", &self.phase)
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}
