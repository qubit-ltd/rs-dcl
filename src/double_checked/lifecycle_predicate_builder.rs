// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
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
pub struct LifecyclePredicateBuilder {
    /// Shared DCL predicate.
    core: DclCore,
}

impl LifecyclePredicateBuilder {
    /// Creates the predicate-configured lifecycle stage.
    ///
    /// # Parameters
    ///
    /// * `core` - Predicate configuration.
    ///
    /// # Returns
    ///
    /// A builder awaiting prepare.
    #[inline]
    pub(crate) fn new(core: DclCore) -> Self {
        Self { core }
    }

    /// Sets the callback that creates one token per prepared invocation.
    ///
    /// # Parameters
    ///
    /// * `prepare` - Concurrently callable token-producing callback.
    ///
    /// # Returns
    ///
    /// A builder requiring `commit` or `no_commit`.
    ///
    /// # Errors
    ///
    /// This method only stores `prepare`; an error returned when prepare later
    /// runs is preserved in [`crate::LifecycleOutcome::PrepareFailed`].
    ///
    /// # Panics
    ///
    /// This method does not invoke `prepare`. Its panic behavior is determined
    /// during invocation of the built executor.
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
    pub fn prepare<P, C, F>(self, prepare: F) -> LifecyclePrepareBuilder<P, C>
    where
        F: Fn() -> Result<P, C> + Send + Sync + 'static,
    {
        let prepare: PrepareCallback<P, C> = Arc::new(prepare);
        LifecyclePrepareBuilder::new(self.core, prepare)
    }
}
