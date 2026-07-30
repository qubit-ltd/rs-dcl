// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Initial builder stage for the basic DCL executor.

use crate::double_checked::{
    DoubleCheckedLockExecutorReadyBuilder,
    internal::DclCore,
};

/// Builder stage that cannot build until a predicate is set.
#[doc(hidden)]
#[must_use = "the builder must be completed with when and build"]
pub struct DoubleCheckedLockExecutorBuilder;

impl DoubleCheckedLockExecutorBuilder {
    /// Creates the initial builder stage.
    ///
    /// # Returns
    ///
    /// A builder that requires [`Self::when`].
    #[inline]
    pub(crate) fn new() -> Self {
        Self
    }

    /// Sets the lock-free condition evaluated before and after locking.
    ///
    /// The predicate must use atomic or equivalent synchronization and must
    /// not acquire the same underlying lock supplied to the executor.
    ///
    /// # Parameters
    ///
    /// * `predicate` - Thread-safe, zero-argument condition callback.
    ///
    /// # Returns
    ///
    /// A ready builder with panic capture disabled by default.
    ///
    /// # Errors
    ///
    /// This method only stores `predicate`; any error-producing work belongs
    /// in the later task and is reported by the executor.
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
    pub fn when<F>(self, predicate: F) -> DoubleCheckedLockExecutorReadyBuilder
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        DoubleCheckedLockExecutorReadyBuilder::new(DclCore::new(predicate))
    }
}
