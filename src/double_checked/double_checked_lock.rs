/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! # Double-Checked Lock Convenience API
//!
//! Provides a one-shot convenience wrapper around
//! [`super::DoubleCheckedLockExecutor`].
//!
//! # Author
//!
//! Haixing Hu

use std::fmt::Display;

use qubit_function::{
    Callable,
    CallableWith,
    Runnable,
    RunnableWith,
    Tester,
};

use super::{
    DoubleCheckedLockExecutor,
    ExecutionContext,
    executor_lock_builder::ExecutorLockBuilder,
    executor_ready_builder::ExecutorReadyBuilder,
};
use crate::lock::Lock;

/// Entry type for one-shot double-checked lock execution.
///
/// This API is useful when you do not need to keep a reusable executor
/// instance. It delegates to [`DoubleCheckedLockExecutor`] internally.
///
/// # Examples
///
/// ```rust
/// use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
///
/// use qubit_dcl::{ArcMutex, DoubleCheckedLock, Lock};
///
/// let data = ArcMutex::new(10);
/// let skip = Arc::new(AtomicBool::new(false));
///
/// let result = DoubleCheckedLock::on(data.clone())
///     .when({
///         let skip = skip.clone();
///         move || !skip.load(Ordering::Acquire)
///     })
///     .call_with(|value: &mut i32| {
///         *value += 5;
///         Ok::<i32, std::io::Error>(*value)
///     })
///     .get_result();
///
/// assert!(result.is_success());
/// assert_eq!(data.read(|value| *value), 15);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct DoubleCheckedLock;

impl DoubleCheckedLock {
    /// Starts one-shot double-checked lock configuration by attaching a lock.
    #[inline]
    pub fn on<L, T>(lock: L) -> DoubleCheckedLockLockBuilder<L, T>
    where
        L: Lock<T>,
    {
        DoubleCheckedLockLockBuilder {
            inner: DoubleCheckedLockExecutor::builder().on(lock),
        }
    }
}

/// Convenience builder state with lock attached.
#[derive(Clone)]
pub struct DoubleCheckedLockLockBuilder<L, T> {
    inner: ExecutorLockBuilder<L, T>,
}

impl<L, T> DoubleCheckedLockLockBuilder<L, T>
where
    L: Lock<T>,
{
    /// Configures logging when the double-checked condition is not met.
    #[inline]
    pub fn log_unmet_condition(mut self, level: log::Level, message: impl Into<String>) -> Self {
        self.inner = self.inner.log_unmet_condition(level, message);
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

/// Convenience builder state with tester attached.
#[derive(Clone)]
pub struct DoubleCheckedLockReadyBuilder<L, T> {
    inner: ExecutorReadyBuilder<L, T>,
}

impl<L, T> DoubleCheckedLockReadyBuilder<L, T>
where
    L: Lock<T>,
{
    /// Configures logging when the double-checked condition is not met.
    #[inline]
    pub fn log_unmet_condition(mut self, level: log::Level, message: impl Into<String>) -> Self {
        self.inner = self.inner.log_unmet_condition(level, message);
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

    /// Sets the prepare action.
    #[inline]
    pub fn prepare<Rn, E>(mut self, prepare_action: Rn) -> Self
    where
        Rn: Runnable<E> + Send + 'static,
        E: Display + Send + 'static,
    {
        self.inner = self.inner.prepare(prepare_action);
        self
    }

    /// Sets the rollback action for prepare.
    #[inline]
    pub fn rollback_prepare<Rn, E>(mut self, rollback_prepare_action: Rn) -> Self
    where
        Rn: Runnable<E> + Send + 'static,
        E: Display + Send + 'static,
    {
        self.inner = self.inner.rollback_prepare(rollback_prepare_action);
        self
    }

    /// Sets the commit action for prepare.
    #[inline]
    pub fn commit_prepare<Rn, E>(mut self, commit_prepare_action: Rn) -> Self
    where
        Rn: Runnable<E> + Send + 'static,
        E: Display + Send + 'static,
    {
        self.inner = self.inner.commit_prepare(commit_prepare_action);
        self
    }

    /// Builds a reusable [`DoubleCheckedLockExecutor`].
    #[inline]
    pub fn build(self) -> DoubleCheckedLockExecutor<L, T> {
        self.inner.build()
    }

    /// Runs a callable task with one-shot executor creation.
    #[inline]
    pub fn call<C, R, E>(self, task: C) -> ExecutionContext<R, E>
    where
        C: Callable<R, E> + Send + 'static,
        R: Send + 'static,
        E: Display + Send + 'static,
    {
        self.inner.build().call(task)
    }

    /// Runs a runnable task with one-shot executor creation.
    #[inline]
    pub fn execute<Rn, E>(self, task: Rn) -> ExecutionContext<(), E>
    where
        Rn: Runnable<E> + Send + 'static,
        E: Display + Send + 'static,
    {
        self.inner.build().execute(task)
    }

    /// Runs a callable task with mutable protected data.
    #[inline]
    pub fn call_with<C, R, E>(self, task: C) -> ExecutionContext<R, E>
    where
        C: CallableWith<T, R, E> + Send + 'static,
        R: Send + 'static,
        E: Display + Send + 'static,
    {
        self.inner.build().call_with(task)
    }

    /// Runs a runnable task with mutable protected data.
    #[inline]
    pub fn execute_with<Rn, E>(self, task: Rn) -> ExecutionContext<(), E>
    where
        Rn: RunnableWith<T, E> + Send + 'static,
        E: Display + Send + 'static,
    {
        self.inner.build().execute_with(task)
    }
}
