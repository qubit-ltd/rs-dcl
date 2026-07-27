// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Lock test double whose guard panics when released.

use qubit_lock::{
    Lock,
    TryLockError,
};

/// Lock that always acquires successfully and panics when its guard is dropped.
pub struct PanickingReleaseLock;

/// Guard that simulates a panic while releasing its lock.
pub struct PanickingReleaseGuard;

impl Drop for PanickingReleaseGuard {
    /// Panics to simulate a lock backend failing during guard release.
    fn drop(&mut self) {
        panic!("lock release panic");
    }
}

impl Lock for PanickingReleaseLock {
    type Guard<'a> = PanickingReleaseGuard;

    /// Returns a guard that panics when dropped.
    #[inline(always)]
    fn lock(&self) -> Self::Guard<'_> {
        PanickingReleaseGuard
    }

    /// Returns a guard that panics when dropped.
    #[inline(always)]
    fn try_lock(&self) -> Result<Self::Guard<'_>, TryLockError> {
        Ok(PanickingReleaseGuard)
    }
}
