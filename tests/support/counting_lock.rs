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
    Mutex,
    TryLockError as StdTryLockError,
    atomic::{
        AtomicUsize,
        Ordering,
    },
};

use qubit_lock::{
    Lock,
    TryLockError,
};

use super::CountingGuard;

/// Wraps a real standard-library mutex and counts calls to the `Lock` API.
#[derive(Clone)]
pub struct CountingLock {
    /// Real lock used to preserve production locking behavior.
    inner: Arc<Mutex<()>>,
    /// Shared acquisition-method invocation counter.
    calls: Arc<AtomicUsize>,
}

impl CountingLock {
    /// Creates a counting lock.
    ///
    /// # Returns
    ///
    /// A lock with a zeroed call counter.
    #[inline]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(())),
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

impl Lock for CountingLock {
    type Guard<'a> = CountingGuard<'a>;

    /// Records and delegates blocking acquisition.
    #[inline(always)]
    fn lock(&self) -> Self::Guard<'_> {
        self.record_call();
        CountingGuard::new(
            Mutex::lock(self.inner.as_ref())
                .expect("counting lock mutex should not be poisoned"),
        )
    }

    /// Records and delegates immediate acquisition.
    #[inline(always)]
    fn try_lock(&self) -> Result<Self::Guard<'_>, TryLockError> {
        self.record_call();
        match Mutex::try_lock(self.inner.as_ref()) {
            Ok(guard) => Ok(CountingGuard::new(guard)),
            Err(StdTryLockError::WouldBlock) => Err(TryLockError::WouldBlock),
            Err(StdTryLockError::Poisoned(_)) => {
                panic!("counting lock mutex should not be poisoned")
            }
        }
    }
}
