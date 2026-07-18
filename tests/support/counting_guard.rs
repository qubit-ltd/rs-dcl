// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! RAII guard returned by the counting lock test double.

use parking_lot::MutexGuard;

/// Retains the underlying mutex guard until this value is dropped.
pub struct CountingGuard<'a> {
    /// Underlying guard whose destructor releases the test mutex.
    _guard: MutexGuard<'a, ()>,
}

impl<'a> CountingGuard<'a> {
    /// Wraps an acquired parking-lot guard.
    ///
    /// # Parameters
    ///
    /// * `guard` - Guard to retain for this value's lifetime.
    ///
    /// # Returns
    ///
    /// A counting-lock guard.
    #[inline(always)]
    pub(super) fn new(guard: MutexGuard<'a, ()>) -> Self {
        Self { _guard: guard }
    }
}
