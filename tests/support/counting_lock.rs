// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Lock test double that records every acquisition method call.

use std::sync::{
    Arc,
    atomic::{
        AtomicUsize,
        Ordering,
    },
};

use qubit_lock::{
    ArcMutex,
    Lock,
    TryLockError,
};

/// Wraps a real parking-lot mutex and counts calls to the `Lock` API.
#[derive(Clone)]
pub struct CountingLock<T> {
    /// Real lock used to preserve production locking behavior.
    inner: ArcMutex<T>,
    /// Shared acquisition-method invocation counter.
    calls: Arc<AtomicUsize>,
}

impl<T> CountingLock<T> {
    /// Creates a counting lock protecting `value`.
    ///
    /// # Parameters
    ///
    /// * `value` - Initial protected value.
    ///
    /// # Returns
    ///
    /// A lock with a zeroed call counter.
    #[inline]
    pub fn new(value: T) -> Self {
        Self {
            inner: ArcMutex::new(value),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Returns the number of `Lock` methods called through any clone.
    ///
    /// # Returns
    ///
    /// The current call count.
    #[inline(always)]
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }

    /// Records one lock API invocation.
    #[inline(always)]
    fn record_call(&self) {
        self.calls.fetch_add(1, Ordering::Relaxed);
    }
}

impl<T> Lock<T> for CountingLock<T> {
    /// Records and delegates a read operation.
    #[inline(always)]
    fn with_read<R, F>(&self, operation: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        self.record_call();
        self.inner.with_read(operation)
    }

    /// Records and delegates a write operation.
    #[inline(always)]
    fn with_write<R, F>(&self, operation: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        self.record_call();
        self.inner.with_write(operation)
    }

    /// Records and delegates a non-blocking read operation.
    #[inline(always)]
    fn try_with_read<R, F>(&self, operation: F) -> Result<R, TryLockError>
    where
        F: FnOnce(&T) -> R,
    {
        self.record_call();
        self.inner.try_with_read(operation)
    }

    /// Records and delegates a non-blocking write operation.
    #[inline(always)]
    fn try_with_write<R, F>(&self, operation: F) -> Result<R, TryLockError>
    where
        F: FnOnce(&mut T) -> R,
    {
        self.record_call();
        self.inner.try_with_write(operation)
    }
}
