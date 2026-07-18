// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Initial builder stage for the lifecycle DCL executor.

use std::{
    marker::PhantomData,
    sync::Arc,
};

use crate::double_checked::{
    LifecyclePredicateBuilder,
    internal::DclCore,
};

/// Builder stage that owns a lock but cannot proceed without a predicate.
#[doc(hidden)]
#[must_use = "the builder must be completed with when and lifecycle stages"]
pub struct LifecycleDoubleCheckedLockExecutorBuilder<L, T: ?Sized> {
    /// Lock supplied by the caller.
    lock: L,
    /// Associates the builder with the lock's protected type.
    marker: PhantomData<fn(&T)>,
}

impl<L, T: ?Sized> LifecycleDoubleCheckedLockExecutorBuilder<L, T> {
    /// Creates the initial lifecycle builder stage.
    ///
    /// # Parameters
    ///
    /// * `lock` - Lock used by the built executor.
    ///
    /// # Returns
    ///
    /// A builder requiring [`Self::when`].
    #[inline]
    pub(crate) fn new(lock: L) -> Self {
        Self {
            lock,
            marker: PhantomData,
        }
    }

    /// Sets the lock-free condition evaluated before and after locking.
    ///
    /// # Parameters
    ///
    /// * `predicate` - Thread-safe, zero-argument condition callback.
    ///
    /// # Returns
    ///
    /// A builder that accepts panic configuration and prepare.
    ///
    /// # Errors
    ///
    /// This method only stores `predicate`; lifecycle and task errors are
    /// reported by the built executor.
    ///
    /// # Panics
    ///
    /// This method does not invoke `predicate`. Predicate panics can only occur
    /// during executor execution.
    ///
    /// # Synchronization
    ///
    /// `predicate` must perform an atomic or equivalently synchronized read and
    /// must be safe for concurrent calls.
    ///
    /// # Locking
    ///
    /// `predicate` runs once without the executor lock and again while holding
    /// it. It must not acquire the same underlying lock itself.
    #[inline]
    pub fn when<F>(self, predicate: F) -> LifecyclePredicateBuilder<L, T>
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        LifecyclePredicateBuilder::new(DclCore::new(
            self.lock,
            Arc::new(predicate),
        ))
    }
}
