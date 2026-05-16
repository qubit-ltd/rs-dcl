/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Convenience ready-builder state for [`super::DoubleCheckedLock`].

use std::fmt::Display;

use qubit_function::{
    Callable,
    CallableWith,
    Runnable,
    RunnableWith,
};

use super::{
    DoubleCheckedLockExecutor,
    ExecutionContext,
    executor_ready_builder::ExecutorReadyBuilder,
};
use crate::lock::Lock;

/// Convenience builder state with tester attached.
#[derive(Clone)]
pub struct DoubleCheckedLockReadyBuilder<L, T> {
    /// Reusable-executor builder delegated to by the convenience API.
    pub(in crate::double_checked) inner: ExecutorReadyBuilder<L, T>,
}

impl<L, T> DoubleCheckedLockReadyBuilder<L, T>
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

    /// Sets the prepare action.
    ///
    /// # Parameters
    ///
    /// * `prepare_action` - Fallible action to run after the first condition
    ///   check and before locking.
    ///
    /// # Returns
    ///
    /// This builder with prepare configured.
    ///
    /// # Errors
    ///
    /// This builder method does not return errors. If `prepare_action` later
    /// returns an error during execution, the execution result becomes
    /// [`super::ExecutionResult::Failed`] with
    /// [`super::ExecutorError::PrepareFailed`].
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn prepare<Rn, E>(mut self, prepare_action: Rn) -> Self
    where
        Rn: Runnable<E> + Send + 'static,
        E: Display,
    {
        self.inner = self.inner.prepare(prepare_action);
        self
    }

    /// Sets the rollback action for prepare.
    ///
    /// # Parameters
    ///
    /// * `rollback_prepare_action` - Fallible action to run when prepare
    ///   completed but the second condition check or task fails.
    ///
    /// # Returns
    ///
    /// This builder with prepare rollback configured.
    ///
    /// # Errors
    ///
    /// This builder method does not return errors. If
    /// `rollback_prepare_action` later returns an error during execution, the
    /// execution result becomes [`super::ExecutionResult::Failed`] with
    /// [`super::ExecutorError::PrepareRollbackFailed`].
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn rollback_prepare<Rn, E>(mut self, rollback_prepare_action: Rn) -> Self
    where
        Rn: Runnable<E> + Send + 'static,
        E: Display,
    {
        self.inner = self.inner.rollback_prepare(rollback_prepare_action);
        self
    }

    /// Sets the commit action for prepare.
    ///
    /// # Parameters
    ///
    /// * `commit_prepare_action` - Fallible action to run when prepare
    ///   completed and the task succeeds.
    ///
    /// # Returns
    ///
    /// This builder with prepare commit configured.
    ///
    /// # Errors
    ///
    /// This builder method does not return errors. If `commit_prepare_action`
    /// later returns an error during execution, the execution result becomes
    /// [`super::ExecutionResult::Failed`] with
    /// [`super::ExecutorError::PrepareCommitFailed`].
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn commit_prepare<Rn, E>(mut self, commit_prepare_action: Rn) -> Self
    where
        Rn: Runnable<E> + Send + 'static,
        E: Display,
    {
        self.inner = self.inner.commit_prepare(commit_prepare_action);
        self
    }

    /// Builds a reusable [`DoubleCheckedLockExecutor`].
    ///
    /// # Returns
    ///
    /// A reusable executor containing the configured lock, tester, logger, and
    /// prepare lifecycle callbacks.
    #[inline]
    #[must_use = "use the returned executor"]
    pub fn build(self) -> DoubleCheckedLockExecutor<L, T> {
        self.inner.build()
    }

    /// Runs a callable task with one-shot executor creation.
    ///
    /// # Parameters
    ///
    /// * `task` - Zero-argument callable executed after both condition checks
    ///   pass.
    ///
    /// # Returns
    ///
    /// An [`ExecutionContext`] containing success, unmet-condition, or failure
    /// information.
    #[inline]
    pub fn call<C, R, E>(self, task: C) -> ExecutionContext<R, E>
    where
        C: Callable<R, E>,
        E: Display,
    {
        self.inner.build().call(task)
    }

    /// Runs a runnable task with one-shot executor creation.
    ///
    /// # Parameters
    ///
    /// * `task` - Zero-argument runnable executed after both condition checks
    ///   pass.
    ///
    /// # Returns
    ///
    /// An [`ExecutionContext`] containing success, unmet-condition, or failure
    /// information.
    #[inline]
    pub fn execute<Rn, E>(self, task: Rn) -> ExecutionContext<(), E>
    where
        Rn: Runnable<E>,
        E: Display,
    {
        self.inner.build().execute(task)
    }

    /// Runs a callable task with mutable protected data.
    ///
    /// # Parameters
    ///
    /// * `task` - Callable receiving `&mut T` after both condition checks pass.
    ///
    /// # Returns
    ///
    /// An [`ExecutionContext`] containing success, unmet-condition, or failure
    /// information.
    #[inline]
    pub fn call_with<C, R, E>(self, task: C) -> ExecutionContext<R, E>
    where
        C: CallableWith<T, R, E>,
        E: Display,
    {
        self.inner.build().call_with(task)
    }

    /// Runs a runnable task with mutable protected data.
    ///
    /// # Parameters
    ///
    /// * `task` - Runnable receiving `&mut T` after both condition checks pass.
    ///
    /// # Returns
    ///
    /// An [`ExecutionContext`] containing success, unmet-condition, or failure
    /// information.
    #[inline]
    pub fn execute_with<Rn, E>(self, task: Rn) -> ExecutionContext<(), E>
    where
        Rn: RunnableWith<T, E>,
        E: Display,
    {
        self.inner.build().execute_with(task)
    }
}
