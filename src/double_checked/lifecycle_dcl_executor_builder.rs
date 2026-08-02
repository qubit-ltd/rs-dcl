// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Initial builder stage for the lifecycle DCL executor.

use crate::double_checked::{
    LifecyclePredicateBuilder,
    internal::DclCore,
};

/// Builder stage that cannot proceed without a predicate.
#[doc(hidden)]
#[must_use = "the builder must be completed with when and lifecycle stages"]
pub struct LifecycleDclExecutorBuilder;

impl LifecycleDclExecutorBuilder {
    /// Creates the initial lifecycle builder stage.
    ///
    /// # Returns
    ///
    /// A builder requiring [`Self::when`].
    #[inline]
    pub(crate) fn new() -> Self {
        Self
    }

    /// Sets the lock-free condition evaluated before and after locking.
    ///
    /// # Parameters
    ///
    /// * `predicate` - Thread-safe, zero-argument condition callback.
    ///
    /// # Type Parameters
    ///
    /// * `F` - Thread-safe predicate callback type.
    ///
    /// # Returns
    ///
    /// A builder that accepts lifecycle preparation.
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
    pub fn when<F>(self, predicate: F) -> LifecyclePredicateBuilder
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        LifecyclePredicateBuilder::new(DclCore::new(predicate))
    }
}
