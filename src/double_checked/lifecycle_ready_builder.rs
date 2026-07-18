// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Complete lifecycle builder stage.

use crate::double_checked::{
    CommitCallback,
    LifecycleDoubleCheckedLockExecutor,
    PrepareCallback,
    RollbackCallback,
    internal::DclCore,
};

/// Typestate-validated lifecycle builder ready to build.
#[doc(hidden)]
#[must_use = "the ready builder must be consumed by build"]
pub struct LifecycleReadyBuilder<L, T: ?Sized, P, C> {
    /// DCL lock and predicate configuration.
    core: DclCore<L, T>,
    /// Per-invocation token producer.
    prepare: PrepareCallback<P, C>,
    /// Optional successful-path token consumer.
    commit: Option<CommitCallback<P, C>>,
    /// Optional unsuccessful-path token consumer.
    rollback: Option<RollbackCallback<P, C>>,
}

impl<L, T: ?Sized, P, C> LifecycleReadyBuilder<L, T, P, C> {
    /// Creates a complete typestate-validated builder.
    ///
    /// # Parameters
    ///
    /// * `core` - DCL lock and predicate configuration.
    /// * `prepare` - Erased prepare callback.
    /// * `commit` - Selected commit callback, if required.
    /// * `rollback` - Selected rollback callback, if required.
    ///
    /// # Returns
    ///
    /// A builder exposing only [`Self::build`].
    #[inline]
    pub(crate) fn new(
        core: DclCore<L, T>,
        prepare: PrepareCallback<P, C>,
        commit: Option<CommitCallback<P, C>>,
        rollback: Option<RollbackCallback<P, C>>,
    ) -> Self {
        Self {
            core,
            prepare,
            commit,
            rollback,
        }
    }

    /// Builds a reusable lifecycle DCL executor.
    ///
    /// # Returns
    ///
    /// An executor sharing callbacks while producing an independent token for
    /// each invocation.
    #[inline]
    pub fn build(self) -> LifecycleDoubleCheckedLockExecutor<L, T, P, C> {
        LifecycleDoubleCheckedLockExecutor::from_parts(
            self.core,
            self.prepare,
            self.commit,
            self.rollback,
        )
    }
}
