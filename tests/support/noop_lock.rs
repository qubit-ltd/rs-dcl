// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Non-blocking lock test double for sequential executor coverage.

use qubit_lock::{
    Lock,
    TryLockError,
};

/// Lock test double whose guard has no synchronization side effects.
pub struct NoopLock;

/// Guard returned by [`NoopLock`].
pub struct NoopGuard;

impl Lock for NoopLock {
    type Guard<'a> = NoopGuard;

    /// Returns a no-op guard for a sequential test invocation.
    #[inline(always)]
    fn lock(&self) -> Self::Guard<'_> {
        NoopGuard
    }

    /// Returns a no-op guard without blocking.
    #[inline(always)]
    fn try_lock(&self) -> Result<Self::Guard<'_>, TryLockError> {
        Ok(NoopGuard)
    }
}
