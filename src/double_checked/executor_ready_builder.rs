/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Ready builder for [`super::DoubleCheckedLockExecutor`] (tester set, optional
//! prepare hooks).
//!

use std::{
    fmt::Display,
    marker::PhantomData,
};

use qubit_function::{
    ArcRunnable,
    ArcTester,
    Runnable,
};

use super::{
    CallbackError,
    ExecutionLogger,
    double_checked_lock_executor::DoubleCheckedLockExecutor,
};
use crate::lock::Lock;

/// Builder state after the required condition tester has been configured.
///
/// This state can configure prepare lifecycle callbacks and build the final
/// [`DoubleCheckedLockExecutor`].
///
/// # Type Parameters
///
/// * `L` - The lock type implementing [`Lock<T>`].
/// * `T` - The data type protected by the lock.
///
#[derive(Clone)]
pub struct ExecutorReadyBuilder<L, T> {
    /// The lock to store in the executor.
    pub(in crate::double_checked) lock: L,

    /// Required condition tester.
    pub(in crate::double_checked) tester: ArcTester,

    /// Logger used by the executor.
    pub(in crate::double_checked) logger: ExecutionLogger,

    /// Optional action executed after the first check and before locking.
    pub(in crate::double_checked) prepare_action: Option<ArcRunnable<CallbackError>>,

    /// Optional action executed when prepare must be rolled back.
    pub(in crate::double_checked) rollback_prepare_action: Option<ArcRunnable<CallbackError>>,

    /// Optional action executed when prepare should be committed.
    pub(in crate::double_checked) commit_prepare_action: Option<ArcRunnable<CallbackError>>,

    /// Whether panics from tester, callbacks, and task are converted to errors.
    pub(in crate::double_checked) catch_panics: bool,

    /// Carries the protected data type.
    pub(in crate::double_checked) _phantom: PhantomData<fn() -> T>,
}

impl<L, T> ExecutorReadyBuilder<L, T>
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
    pub fn disable_prepare_rollback_failure_logging(mut self) -> Self {
        self.logger.disable_prepare_rollback_failure();
        self
    }

    /// Sets the prepare action.
    ///
    /// The action runs after the first condition check succeeds and before the
    /// lock is acquired. If it succeeds, the executor will later run either
    /// rollback or commit according to the final task result.
    ///
    /// Errors returned by this action are converted to [`String`] and reported
    /// by execution methods as [`super::ExecutionResult::Failed`].
    ///
    /// # Parameters
    ///
    /// * `prepare_action` - The fallible action to run before locking.
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
    pub fn prepare<Rn, E>(mut self, prepare_action: Rn) -> Self
    where
        Rn: Runnable<E> + Send + 'static,
        E: Display,
    {
        let mut action = prepare_action;
        self.prepare_action = Some(ArcRunnable::new(move || {
            action
                .run()
                .map_err(|error| CallbackError::with_callback_type("prepare", error))
        }));
        self
    }

    /// Sets the rollback action for a successfully completed prepare action.
    ///
    /// Errors returned by this action are converted to [`String`] and replace
    /// the original execution result with a prepare-rollback failure.
    ///
    /// # Parameters
    ///
    /// * `rollback_prepare_action` - The action to run if the second condition
    ///   check or task execution fails after prepare succeeds.
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
    pub fn rollback_prepare<Rn, E>(mut self, rollback_prepare_action: Rn) -> Self
    where
        Rn: Runnable<E> + Send + 'static,
        E: Display,
    {
        let mut action = rollback_prepare_action;
        self.rollback_prepare_action = Some(ArcRunnable::new(move || {
            action
                .run()
                .map_err(|error| CallbackError::with_callback_type("prepare_rollback", error))
        }));
        self
    }

    /// Sets the commit action for a successfully completed prepare action.
    ///
    /// Errors returned by this action are converted to [`String`] and replace
    /// an otherwise successful execution result with a prepare-commit failure.
    ///
    /// # Parameters
    ///
    /// * `commit_prepare_action` - The action to run if the task succeeds after
    ///   prepare succeeds.
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
    pub fn commit_prepare<Rn, E>(mut self, commit_prepare_action: Rn) -> Self
    where
        Rn: Runnable<E> + Send + 'static,
        E: Display,
    {
        let mut action = commit_prepare_action;
        self.commit_prepare_action = Some(ArcRunnable::new(move || {
            action
                .run()
                .map_err(|error| CallbackError::with_callback_type("prepare_commit", error))
        }));
        self
    }

    /// Enables panic capture for tester, prepare callbacks, and task execution.
    ///
    /// When enabled, panic payloads are converted to
    /// [`super::executor_error::ExecutorError::Panic`] and surfaced through
    /// [`super::ExecutionResult`].
    ///
    /// # Returns
    ///
    /// This builder with panic capture enabled.
    #[inline]
    pub fn catch_panics(mut self) -> Self {
        self.catch_panics = true;
        self
    }

    /// Sets whether panic capture for tester, prepare callbacks, and task
    /// execution is enabled.
    ///
    /// # Parameters
    ///
    /// * `catch_panics` - `true` to capture panics as execution errors, or
    ///   `false` to let panics unwind.
    ///
    /// # Returns
    ///
    /// This builder with the updated panic-capture setting.
    #[inline]
    pub fn set_catch_panics(mut self, catch_panics: bool) -> Self {
        self.catch_panics = catch_panics;
        self
    }

    /// Disables panic capture for tester, prepare callbacks, and task execution.
    ///
    /// # Returns
    ///
    /// This builder with panic capture disabled.
    #[inline]
    pub fn disable_catch_panics(mut self) -> Self {
        self.catch_panics = false;
        self
    }

    /// Builds the reusable executor.
    ///
    /// # Returns
    ///
    /// A [`DoubleCheckedLockExecutor`] containing the configured lock, tester,
    /// execution logger, and prepare lifecycle callbacks.
    #[inline]
    pub fn build(self) -> DoubleCheckedLockExecutor<L, T> {
        DoubleCheckedLockExecutor::new(self)
    }
}
