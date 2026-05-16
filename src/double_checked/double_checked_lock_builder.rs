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
    ///
    /// # Parameters
    ///
    /// * `level` - Log level used for unmet-condition messages.
    /// * `message` - Full log message emitted when the condition is not met.
    ///
    /// # Returns
    ///
    /// This builder with unmet-condition logging configured.
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn log_unmet_condition(mut self, level: log::Level, message: impl Into<String>) -> Self {
        self.inner = self.inner.log_unmet_condition(level, message);
        self
    }

    /// Disables logging when the double-checked condition is not met.
    ///
    /// # Returns
    ///
    /// This builder with unmet-condition logging disabled.
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn disable_unmet_condition_logging(mut self) -> Self {
        self.inner = self.inner.disable_unmet_condition_logging();
        self
    }

    /// Configures logging when the prepare action fails.
    ///
    /// # Parameters
    ///
    /// * `level` - Log level used for prepare failure messages.
    /// * `message_prefix` - Prefix placed before the prepare failure text.
    ///
    /// # Returns
    ///
    /// This builder with prepare failure logging configured.
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn log_prepare_failure(
        mut self,
        level: log::Level,
        message_prefix: impl Into<String>,
    ) -> Self {
        self.inner = self.inner.log_prepare_failure(level, message_prefix);
        self
    }

    /// Disables logging when the prepare action fails.
    ///
    /// # Returns
    ///
    /// This builder with prepare failure logging disabled.
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn disable_prepare_failure_logging(mut self) -> Self {
        self.inner = self.inner.disable_prepare_failure_logging();
        self
    }

    /// Configures logging when the prepare commit action fails.
    ///
    /// # Parameters
    ///
    /// * `level` - Log level used for prepare-commit failure messages.
    /// * `message_prefix` - Prefix placed before the prepare-commit failure
    ///   text.
    ///
    /// # Returns
    ///
    /// This builder with prepare-commit failure logging configured.
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn log_prepare_commit_failure(
        mut self,
        level: log::Level,
        message_prefix: impl Into<String>,
    ) -> Self {
        self.inner = self.inner.log_prepare_commit_failure(level, message_prefix);
        self
    }

    /// Disables logging when the prepare commit action fails.
    ///
    /// # Returns
    ///
    /// This builder with prepare-commit failure logging disabled.
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn disable_prepare_commit_failure_logging(mut self) -> Self {
        self.inner = self.inner.disable_prepare_commit_failure_logging();
        self
    }

    /// Configures logging when the prepare rollback action fails.
    ///
    /// # Parameters
    ///
    /// * `level` - Log level used for prepare-rollback failure messages.
    /// * `message_prefix` - Prefix placed before the prepare-rollback failure
    ///   text.
    ///
    /// # Returns
    ///
    /// This builder with prepare-rollback failure logging configured.
    #[inline]
    #[must_use = "assign or chain the returned builder"]
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
    ///
    /// # Returns
    ///
    /// This builder with prepare-rollback failure logging disabled.
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn disable_prepare_rollback_failure_logging(mut self) -> Self {
        self.inner = self.inner.disable_prepare_rollback_failure_logging();
        self
    }

    /// Enables panic capture for tester, prepare callbacks, and task execution.
    ///
    /// # Returns
    ///
    /// This builder with panic capture enabled.
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn catch_panics(mut self) -> Self {
        self.inner = self.inner.catch_panics();
        self
    }

    /// Derives a builder with panic capture enabled or disabled for tester,
    /// prepare callbacks, and task execution.
    ///
    /// # Parameters
    ///
    /// * `catch_panics` - `true` to capture panics as execution errors, or
    ///   `false` to let panics unwind.
    ///
    /// # Returns
    ///
    /// A reconfigured builder with the updated panic-capture setting.
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn with_panic_capture(mut self, catch_panics: bool) -> Self {
        self.inner = self.inner.with_panic_capture(catch_panics);
        self
    }

    /// Disables panic capture for tester, prepare callbacks, and task execution.
    ///
    /// # Returns
    ///
    /// This builder with panic capture disabled.
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn disable_catch_panics(mut self) -> Self {
        self.inner = self.inner.disable_catch_panics();
        self
    }

    /// Sets the required double-checked condition.
    ///
    /// # Parameters
    ///
    /// * `tester` - Condition tester executed before and after acquiring the
    ///   lock.
    ///
    /// # Returns
    ///
    /// A ready builder that can configure prepare callbacks or run the task.
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn when<Tst>(self, tester: Tst) -> DoubleCheckedLockReadyBuilder<L, T>
    where
        Tst: Tester + Send + Sync + 'static,
    {
        DoubleCheckedLockReadyBuilder {
            inner: self.inner.when(tester),
        }
    }
}
