// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Predicate-configured lifecycle builder stage.

use std::sync::Arc;

use crate::double_checked::{
    LifecyclePrepareBuilder,
    PrepareCallback,
    internal::DclCore,
};

/// Builder stage that requires prepare before finalizer selection.
#[doc(hidden)]
#[must_use = "the predicate stage must be completed with prepare"]
pub struct LifecyclePredicateBuilder<L, T: ?Sized> {
    /// DCL configuration containing the lock and predicate.
    core: DclCore<L, T>,
}

impl<L, T: ?Sized> LifecyclePredicateBuilder<L, T> {
    /// Creates the predicate-configured lifecycle stage.
    ///
    /// # Parameters
    ///
    /// * `core` - Lock and predicate configuration.
    ///
    /// # Returns
    ///
    /// A builder awaiting prepare.
    #[inline]
    pub(crate) fn new(core: DclCore<L, T>) -> Self {
        Self { core }
    }

    /// Configures whether all lifecycle phases convert panics into report
    /// outcomes.
    ///
    /// # Parameters
    ///
    /// * `catch_panics` - `true` to capture panics, or `false` to propagate
    ///   them subject to locked-phase rollback.
    ///
    /// # Returns
    ///
    /// The reconfigured predicate stage.
    #[inline(always)]
    pub fn catch_panics(mut self, catch_panics: bool) -> Self {
        self.core = self.core.with_catch_panics(catch_panics);
        self
    }

    /// Sets the callback that creates one token per prepared invocation.
    ///
    /// # Parameters
    ///
    /// * `prepare` - Concurrently callable token-producing callback.
    ///
    /// # Returns
    ///
    /// A builder requiring a commit/no-commit choice.
    ///
    /// # Errors
    ///
    /// This method only stores `prepare`; an error returned when prepare later
    /// runs is preserved in [`crate::PreparationOutcome::PrepareFailed`].
    ///
    /// # Panics
    ///
    /// This method does not invoke `prepare`. Its panic behavior is determined
    /// by the built executor's panic-capture setting.
    ///
    /// # Synchronization
    ///
    /// `prepare` is stored as a shared `Fn` callback and may run concurrently
    /// for separate executor invocations.
    ///
    /// # Locking
    ///
    /// `prepare` runs after the lock-free check but before lock acquisition. It
    /// must manage any separate locks it needs and must not assume the executor
    /// lock is held.
    #[inline]
    pub fn prepare<P, C, F>(
        self,
        prepare: F,
    ) -> LifecyclePrepareBuilder<L, T, P, C>
    where
        F: Fn() -> Result<P, C> + Send + Sync + 'static,
    {
        let prepare: PrepareCallback<P, C> = Arc::new(prepare);
        LifecyclePrepareBuilder::new(self.core, prepare)
    }
}
