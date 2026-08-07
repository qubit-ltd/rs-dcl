// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Commit-configured lifecycle builder stage.

use std::sync::Arc;

use crate::double_checked::CommitCallback;
use crate::double_checked::LifecycleReadyBuilder;
use crate::double_checked::PrepareCallback;
use crate::double_checked::RollbackCallback;
use crate::double_checked::internal::DclCore;

/// Builder stage requiring a rollback/no-rollback choice.
///
/// # Type Parameters
///
/// * `P` - Per-invocation token type produced by `prepare`.
/// * `C` - Error type returned by lifecycle callbacks.
#[doc(hidden)]
#[must_use = "the commit stage requires rollback or no_rollback"]
pub struct LifecycleCommitBuilder<P, C> {
    /// DCL predicate configuration.
    core: DclCore,
    /// Per-invocation token producer.
    prepare: PrepareCallback<P, C>,
    /// Successful-path token consumer.
    commit: CommitCallback<P, C>,
}

impl<P, C> LifecycleCommitBuilder<P, C> {
    /// Creates the commit-configured builder stage.
    ///
    /// # Parameters
    ///
    /// * `core` - DCL predicate configuration.
    /// * `prepare` - Erased prepare callback.
    /// * `commit` - Erased commit callback.
    ///
    /// # Type Parameters
    ///
    /// * `P` - Per-invocation token type.
    /// * `C` - Lifecycle callback error type.
    ///
    /// # Returns
    ///
    /// A builder requiring a rollback choice.
    #[inline]
    pub(crate) fn new(
        core: DclCore,
        prepare: PrepareCallback<P, C>,
        commit: CommitCallback<P, C>,
    ) -> Self {
        Self {
            core,
            prepare,
            commit,
        }
    }

    /// Sets the unsuccessful-path token consumer.
    ///
    /// # Parameters
    ///
    /// * `rollback` - Callback receiving the token and structured failure cause
    ///   after lock release.
    ///
    /// # Type Parameters
    ///
    /// * `F` - Rollback callback type.
    ///
    /// # Returns
    ///
    /// A complete builder ready to build.
    ///
    /// # Errors
    ///
    /// This method only stores `rollback`; an error returned when rollback
    /// later runs is preserved in [`crate::FinalizationOutcome::Failed`].
    ///
    /// # Panics
    ///
    /// This method does not invoke `rollback`. Panic behavior is selected at
    /// execution time by calling `run` or `run_catching`.
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
    pub fn rollback<F>(self, rollback: F) -> LifecycleReadyBuilder<P, C>
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
            Some(self.commit),
            Some(rollback),
        )
    }

    /// Declares that unsuccessful invocations do not require rollback.
    ///
    /// # Returns
    ///
    /// A complete builder ready to build.
    #[inline]
    pub fn no_rollback(self) -> LifecycleReadyBuilder<P, C> {
        LifecycleReadyBuilder::new(
            self.core,
            self.prepare,
            Some(self.commit),
            None,
        )
    }
}
