/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Builder state after a lock has been attached for
//! [`super::DoubleCheckedLockExecutor`].
//!

use std::marker::PhantomData;

use qubit_function::Tester;

use super::{
    ExecutionLogger,
    executor_ready_builder::ExecutorReadyBuilder,
};
use qubit_lock::Lock;

/// Builder state after a lock has been attached.
///
/// Call [`Self::when`] to configure the required condition tester.
///
/// # Type Parameters
///
/// * `L` - The lock type implementing [`Lock<T>`].
/// * `T` - The data type protected by the lock.
///
#[derive(Clone)]
pub struct ExecutorLockBuilder<L, T> {
    /// The lock to store in the executor.
    pub(in crate::double_checked) lock: L,

    /// Logger carried forward to the ready builder state.
    pub(in crate::double_checked) logger: ExecutionLogger,

    /// Whether panics from tester, callbacks, and task are captured as result errors.
    pub(in crate::double_checked) catch_panics: bool,

    /// Carries the protected data type.
    pub(in crate::double_checked) _phantom: PhantomData<fn() -> T>,
}

impl<L, T> ExecutorLockBuilder<L, T>
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
        self.logger.set_unmet_condition(Some(level), message);
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
        self.logger.disable_unmet_condition();
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
        self.logger.set_prepare_failure(Some(level), message_prefix);
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
        self.logger.disable_prepare_failure();
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
        self.logger
            .set_prepare_commit_failure(Some(level), message_prefix);
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
        self.logger.disable_prepare_commit_failure();
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
        self.logger
            .set_prepare_rollback_failure(Some(level), message_prefix);
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
        self.logger.disable_prepare_rollback_failure();
        self
    }

    /// Configures the required double-checked condition.
    ///
    /// The tester is executed outside and inside the lock. State read by the
    /// outside check must be safe to access without this executor's lock.
    ///
    /// # Parameters
    ///
    /// * `tester` - The reusable condition tester.
    ///
    /// # Returns
    ///
    /// The builder state that can configure prepare callbacks and build the
    /// executor.
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn when<Tst>(self, tester: Tst) -> ExecutorReadyBuilder<L, T>
    where
        Tst: Tester + Send + Sync + 'static,
    {
        ExecutorReadyBuilder {
            lock: self.lock,
            tester: tester.into_arc(),
            logger: self.logger,
            prepare_action: None,
            rollback_prepare_action: None,
            commit_prepare_action: None,
            catch_panics: self.catch_panics,
            _phantom: PhantomData,
        }
    }

    /// Enables panic capture for tester, prepare callbacks, and task execution.
    ///
    /// # Returns
    ///
    /// This builder with panic capture enabled.
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn catch_panics(mut self) -> Self {
        self.catch_panics = true;
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
        self.catch_panics = catch_panics;
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
        self.catch_panics = false;
        self
    }
}
