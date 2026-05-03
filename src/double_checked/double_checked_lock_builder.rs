/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Convenience builder state for [`super::DoubleCheckedLock`].

use qubit_function::Tester;

use super::{
    DoubleCheckedLockReadyBuilder,
    executor_lock_builder::ExecutorLockBuilder,
};
use crate::lock::Lock;

/// Convenience builder state with lock attached.
#[derive(Clone)]
pub struct DoubleCheckedLockBuilder<L, T> {
    /// Reusable-executor builder delegated to by the convenience API.
    pub(in crate::double_checked) inner: ExecutorLockBuilder<L, T>,
}

impl<L, T> DoubleCheckedLockBuilder<L, T>
where
    L: Lock<T>,
{
    /// Configures logging when the double-checked condition is not met.
    #[inline]
    pub fn log_unmet_condition(mut self, level: log::Level, message: impl Into<String>) -> Self {
        self.inner = self.inner.log_unmet_condition(level, message);
        self
    }

    /// Disables logging when the double-checked condition is not met.
    #[inline]
    pub fn disable_unmet_condition_logging(mut self) -> Self {
        self.inner = self.inner.disable_unmet_condition_logging();
        self
    }

    /// Configures logging when the prepare action fails.
    #[inline]
    pub fn log_prepare_failure(
        mut self,
        level: log::Level,
        message_prefix: impl Into<String>,
    ) -> Self {
        self.inner = self.inner.log_prepare_failure(level, message_prefix);
        self
    }

    /// Disables logging when the prepare action fails.
    #[inline]
    pub fn disable_prepare_failure_logging(mut self) -> Self {
        self.inner = self.inner.disable_prepare_failure_logging();
        self
    }

    /// Configures logging when the prepare commit action fails.
    #[inline]
    pub fn log_prepare_commit_failure(
        mut self,
        level: log::Level,
        message_prefix: impl Into<String>,
    ) -> Self {
        self.inner = self.inner.log_prepare_commit_failure(level, message_prefix);
        self
    }

    /// Disables logging when the prepare commit action fails.
    #[inline]
    pub fn disable_prepare_commit_failure_logging(mut self) -> Self {
        self.inner = self.inner.disable_prepare_commit_failure_logging();
        self
    }

    /// Configures logging when the prepare rollback action fails.
    #[inline]
    pub fn log_prepare_rollback_failure(
        mut self,
        level: log::Level,
        message_prefix: impl Into<String>,
    ) -> Self {
        self.inner = self
            .inner
            .log_prepare_rollback_failure(level, message_prefix);
        self
    }

    /// Disables logging when the prepare rollback action fails.
    #[inline]
    pub fn disable_prepare_rollback_failure_logging(mut self) -> Self {
        self.inner = self.inner.disable_prepare_rollback_failure_logging();
        self
    }

    /// Enables panic capture for tester, prepare callbacks, and task execution.
    #[inline]
    pub fn catch_panics(mut self) -> Self {
        self.inner = self.inner.catch_panics();
        self
    }

    /// Sets whether panic capture for tester, prepare callbacks, and task
    /// execution is enabled.
    #[inline]
    pub fn set_catch_panics(mut self, catch_panics: bool) -> Self {
        self.inner = self.inner.set_catch_panics(catch_panics);
        self
    }

    /// Disables panic capture for tester, prepare callbacks, and task execution.
    #[inline]
    pub fn disable_catch_panics(mut self) -> Self {
        self.inner = self.inner.disable_catch_panics();
        self
    }

    /// Sets the required double-checked condition.
    #[inline]
    pub fn when<Tst>(self, tester: Tst) -> DoubleCheckedLockReadyBuilder<L, T>
    where
        Tst: Tester + Send + Sync + 'static,
    {
        DoubleCheckedLockReadyBuilder {
            inner: self.inner.when(tester),
        }
    }
}
