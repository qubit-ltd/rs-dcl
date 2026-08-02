// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Prepare-configured lifecycle builder stage.

use std::sync::Arc;

use crate::double_checked::{
    CommitCallback,
    LifecycleCommitBuilder,
    LifecycleRollbackBuilder,
    PrepareCallback,
    internal::DclCore,
};

/// Builder stage requiring a successful-path finalizer choice.
///
/// # Type Parameters
///
/// * `P` - Per-invocation token type produced by `prepare`.
/// * `C` - Error type returned by lifecycle callbacks.
#[doc(hidden)]
#[must_use = "the prepare stage requires commit or no_commit"]
pub struct LifecyclePrepareBuilder<P, C> {
    /// DCL predicate configuration.
    core: DclCore,
    /// Per-invocation token producer.
    prepare: PrepareCallback<P, C>,
}

impl<P, C> LifecyclePrepareBuilder<P, C> {
    /// Creates the prepare-configured builder stage.
    ///
    /// # Parameters
    ///
    /// * `core` - DCL predicate configuration.
    /// * `prepare` - Erased prepare callback.
    ///
    /// # Type Parameters
    ///
    /// * `P` - Per-invocation token type.
    /// * `C` - Lifecycle callback error type.
    ///
    /// # Returns
    ///
    /// A builder requiring `commit` or `no_commit`.
    #[inline]
    pub(crate) fn new(core: DclCore, prepare: PrepareCallback<P, C>) -> Self {
        Self { core, prepare }
    }

    /// Sets the successful-path token consumer.
    ///
    /// # Parameters
    ///
    /// * `commit` - Callback invoked after a successful task and lock release.
    ///
    /// # Type Parameters
    ///
    /// * `F` - Commit callback type.
    ///
    /// # Returns
    ///
    /// A builder requiring `rollback` or `no_rollback`.
    ///
    /// # Errors
    ///
    /// This method only stores `commit`; an error returned when commit later
    /// runs is preserved in [`crate::FinalizationOutcome::Failed`].
    ///
    /// # Panics
    ///
    /// This method does not invoke `commit`. Panic behavior is selected at
    /// execution time by calling `run` or `run_catching`.
    ///
    /// # Synchronization
    ///
    /// `commit` is stored as a shared `Fn` callback and may be called
    /// concurrently for separate invocations.
    ///
    /// # Locking
    ///
    /// `commit` runs after the task and after the executor lock is released.
    /// It does not automatically reacquire that lock.
    #[inline]
    pub fn commit<F>(self, commit: F) -> LifecycleCommitBuilder<P, C>
    where
        F: Fn(P) -> Result<(), C> + Send + Sync + 'static,
    {
        let commit: CommitCallback<P, C> = Arc::new(commit);
        LifecycleCommitBuilder::new(self.core, self.prepare, commit)
    }

    /// Declares that successful invocations do not require commit.
    ///
    /// # Returns
    ///
    /// A builder that still requires rollback; the invalid no-commit plus
    /// no-rollback combination is not representable.
    #[inline]
    pub fn no_commit(self) -> LifecycleRollbackBuilder<P, C> {
        LifecycleRollbackBuilder::new(self.core, self.prepare)
    }
}
