// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! No-commit lifecycle builder stage requiring rollback.

use std::sync::Arc;

use crate::double_checked::{
    LifecycleReadyBuilder,
    PrepareCallback,
    RollbackCallback,
    internal::DclCore,
};

/// Builder stage representing `no_commit` and requiring rollback.
#[doc(hidden)]
#[must_use = "the no-commit stage requires rollback"]
pub struct LifecycleRollbackBuilder<L, T: ?Sized, P, C> {
    /// DCL lock and predicate configuration.
    core: DclCore<L, T>,
    /// Per-invocation token producer.
    prepare: PrepareCallback<P, C>,
}

impl<L, T: ?Sized, P, C> LifecycleRollbackBuilder<L, T, P, C> {
    /// Creates the no-commit builder stage.
    ///
    /// # Parameters
    ///
    /// * `core` - DCL lock and predicate configuration.
    /// * `prepare` - Erased prepare callback.
    ///
    /// # Returns
    ///
    /// A builder requiring rollback.
    #[inline]
    pub(crate) fn new(
        core: DclCore<L, T>,
        prepare: PrepareCallback<P, C>,
    ) -> Self {
        Self { core, prepare }
    }

    /// Sets the required unsuccessful-path token consumer.
    ///
    /// # Parameters
    ///
    /// * `rollback` - Callback receiving the token and structured failure cause
    ///   after lock release.
    ///
    /// # Returns
    ///
    /// A complete builder with no commit and a configured rollback.
    ///
    /// # Errors
    ///
    /// This method only stores `rollback`; an error returned when rollback
    /// later runs is preserved in
    /// [`crate::PreparationOutcome::RollbackFailed`].
    ///
    /// # Panics
    ///
    /// This method does not invoke `rollback`. Its panic behavior is determined
    /// by the built executor's panic-capture setting.
    ///
    /// # Synchronization
    ///
    /// `rollback` is stored as a shared `Fn` callback and may be called
    /// concurrently for separate invocations. Its borrowed cause is valid only
    /// for that callback invocation.
    ///
    /// # Locking
    ///
    /// `rollback` runs after the executor lock is released and does not
    /// automatically reacquire it.
    #[inline]
    pub fn rollback<F>(self, rollback: F) -> LifecycleReadyBuilder<L, T, P, C>
    where
        F: for<'a> Fn(P, crate::RollbackCause<'a>) -> Result<(), C>
            + Send
            + Sync
            + 'static,
    {
        let rollback: RollbackCallback<P, C> = Arc::new(rollback);
        LifecycleReadyBuilder::new(
            self.core,
            self.prepare,
            None,
            Some(rollback),
        )
    }
}
