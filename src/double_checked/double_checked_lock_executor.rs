/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! # Double-Checked Lock Executor
//!
//! Provides a reusable executor for double-checked locking workflows.
//!

use std::{
    any::Any,
    fmt::Display,
    marker::PhantomData,
    panic::{
        self,
        AssertUnwindSafe,
    },
};

use qubit_function::{
    ArcRunnable,
    ArcTester,
    Callable,
    CallableWith,
    Runnable,
    RunnableWith,
    Tester,
};

use super::{
    ExecutionContext,
    ExecutionLogger,
    ExecutionResult,
    ExecutorError,
    executor_builder::ExecutorBuilder,
    executor_ready_builder::ExecutorReadyBuilder,
};
use qubit_lock::Lock;

/// Reusable double-checked lock executor.
///
/// The executor owns the lock handle, condition tester, execution logger, and
/// optional prepare lifecycle callbacks. Each execution performs:
///
/// 1. A first condition check outside the lock.
/// 2. Optional prepare action.
/// 3. Lock acquisition.
/// 4. A second condition check inside the lock.
/// 5. The submitted task.
/// 6. Optional prepare commit or rollback after the lock is released.
///
/// The tester is intentionally run both outside and inside the lock. Any state
/// read by the first check must therefore use atomics or another synchronization
/// mechanism that is safe without this executor's lock.
///
/// # Type Parameters
///
/// * `L` - The lock type implementing [`Lock<T>`].
/// * `T` - The data type protected by the lock.
///
/// # Examples
///
/// Use [`DoubleCheckedLockExecutor::builder`] to attach a lock (for example
/// [`crate::ArcMutex`]), set a [`Tester`] with
/// [`ExecutorLockBuilder::when`](super::ExecutorLockBuilder::when), then call
/// [`Self::call`], [`Self::execute`], [`Self::call_with`], or
/// [`Self::execute_with`] on the built executor.
///
/// Panics from the tester, prepare callbacks, or task can be captured by
/// configuring the builder or by deriving a reconfigured executor with
/// [`with_panic_capture`](Self::with_panic_capture). Tester and task panics
/// are reported as [`super::ExecutorError::Panic`]. Prepare lifecycle panics
/// are reported through the corresponding prepare, commit, or rollback error
/// variants, so rollback can still be executed after captured task or second
/// condition-check panics.
///
/// Cloned executors share their configured prepare callbacks. Concurrent calls
/// may therefore complete prepare in several threads before one call wins the
/// second condition check; calls that lose the second check run prepare rollback
/// if it is configured.
///
/// ```rust
/// use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
///
/// use qubit_dcl::{ArcMutex, DoubleCheckedLockExecutor, Lock};
/// use qubit_dcl::double_checked::ExecutionResult;
///
/// fn main() {
///     let data = ArcMutex::new(10);
///     let skip = Arc::new(AtomicBool::new(false));
///
///     let executor = DoubleCheckedLockExecutor::builder()
///         .on(data.clone())
///         .when({
///             let skip = skip.clone();
///             move || !skip.load(Ordering::Acquire)
///         })
///         .build();
///
///     let updated = executor
///         .call_with(|value: &mut i32| {
///             *value += 5;
///             Ok::<i32, std::io::Error>(*value)
///         })
///         .get_result();
///
///     assert!(matches!(updated, ExecutionResult::Success(15)));
///     assert_eq!(data.read(|value| *value), 15);
///
///     skip.store(true, Ordering::Release);
///     let skipped = executor
///         .call_with(|value: &mut i32| {
///             *value += 1;
///             Ok::<i32, std::io::Error>(*value)
///         })
///         .get_result();
///
///     assert!(matches!(skipped, ExecutionResult::ConditionNotMet));
///     assert_eq!(data.read(|value| *value), 15);
/// }
/// ```
///
#[derive(Clone)]
pub struct DoubleCheckedLockExecutor<L = (), T = ()> {
    /// The lock protecting the target data.
    lock: L,

    /// Condition checked before and after acquiring the lock.
    tester: ArcTester,

    /// Logger for unmet conditions and prepare lifecycle failures.
    logger: ExecutionLogger,

    /// Optional action executed after the first check and before locking.
    prepare_action: Option<ArcRunnable<CallbackError>>,

    /// Optional action executed when prepare must be rolled back.
    rollback_prepare_action: Option<ArcRunnable<CallbackError>>,

    /// Optional action executed when prepare should be committed.
    commit_prepare_action: Option<ArcRunnable<CallbackError>>,

    /// Whether panics from tester, callbacks, and task are captured as errors.
    catch_panics: bool,

    /// Carries the protected data type.
    _phantom: PhantomData<fn() -> T>,
}

impl DoubleCheckedLockExecutor<(), ()> {
    /// Creates a builder for a reusable double-checked lock executor.
    ///
    /// # Returns
    ///
    /// A builder in the initial state. Attach a lock with
    /// [`ExecutorBuilder::on`], then configure a tester with
    /// [`ExecutorLockBuilder::when`](super::ExecutorLockBuilder::when).
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn builder() -> ExecutorBuilder {
        ExecutorBuilder::default()
    }
}

impl<L, T> DoubleCheckedLockExecutor<L, T>
where
    L: Lock<T>,
{
    /// Assembles an executor from the ready builder state.
    ///
    /// # Parameters
    ///
    /// * `builder` - Ready builder carrying the lock, tester, logger, and
    ///   prepare lifecycle callbacks.
    ///
    /// # Returns
    ///
    /// A reusable executor containing the supplied builder state.
    #[inline]
    #[must_use = "use the returned executor"]
    pub fn new(builder: ExecutorReadyBuilder<L, T>) -> Self {
        Self {
            lock: builder.lock,
            tester: builder.tester,
            logger: builder.logger,
            prepare_action: builder.prepare_action,
            rollback_prepare_action: builder.rollback_prepare_action,
            commit_prepare_action: builder.commit_prepare_action,
            catch_panics: builder.catch_panics,
            _phantom: builder._phantom,
        }
    }

    /// Executes a zero-argument callable while holding the write lock.
    ///
    /// Use [`Self::call_with`] when the task needs direct mutable access to the
    /// protected data.
    ///
    /// # Parameters
    ///
    /// * `task` - The callable task to execute after both condition checks pass.
    ///
    /// # Returns
    ///
    /// An [`ExecutionContext`] containing success, unmet-condition, or failure
    /// information.
    #[inline]
    pub fn call<C, R, E>(&self, task: C) -> ExecutionContext<R, E>
    where
        C: Callable<R, E>,
        E: Display,
    {
        let mut task = task;
        let result = self.execute_with_write_lock(move |_data| task.call());
        ExecutionContext::new(result)
    }

    /// Executes a zero-argument runnable while holding the write lock.
    ///
    /// # Parameters
    ///
    /// * `task` - The runnable task to execute after both condition checks pass.
    ///
    /// # Returns
    ///
    /// An [`ExecutionContext`] containing success, unmet-condition, or failure
    /// information.
    #[inline]
    pub fn execute<Rn, E>(&self, task: Rn) -> ExecutionContext<(), E>
    where
        Rn: Runnable<E>,
        E: Display,
    {
        let mut task = task;
        let result = self.execute_with_write_lock(move |_data| task.run());
        ExecutionContext::new(result)
    }

    /// Executes a callable with mutable access to the protected data.
    ///
    /// # Parameters
    ///
    /// * `task` - The callable receiving `&mut T` after both condition checks
    ///   pass.
    ///
    /// # Returns
    ///
    /// An [`ExecutionContext`] containing success, unmet-condition, or failure
    /// information.
    #[inline]
    pub fn call_with<C, R, E>(&self, task: C) -> ExecutionContext<R, E>
    where
        C: CallableWith<T, R, E>,
        E: Display,
    {
        let mut task = task;
        let result = self.execute_with_write_lock(move |data| task.call_with(data));
        ExecutionContext::new(result)
    }

    /// Executes a runnable with mutable access to the protected data.
    ///
    /// # Parameters
    ///
    /// * `task` - The runnable receiving `&mut T` after both condition checks
    ///   pass.
    ///
    /// # Returns
    ///
    /// An [`ExecutionContext`] containing success, unmet-condition, or failure
    /// information.
    #[inline]
    pub fn execute_with<Rn, E>(&self, task: Rn) -> ExecutionContext<(), E>
    where
        Rn: RunnableWith<T, E>,
        E: Display,
    {
        let mut task = task;
        let result = self.execute_with_write_lock(move |data| task.run_with(data));
        ExecutionContext::new(result)
    }

    /// Derives an executor with panic capture enabled or disabled for tester,
    /// callbacks, and task execution.
    ///
    /// # Parameters
    ///
    /// * `catch_panics` - `true` to convert panic payloads into execution
    ///   errors, or `false` to let panics unwind normally.
    ///
    /// # Returns
    ///
    /// A reconfigured executor with the updated panic-capture setting.
    #[inline]
    #[must_use = "assign or chain the returned executor"]
    pub fn with_panic_capture(mut self, catch_panics: bool) -> Self {
        self.catch_panics = catch_panics;
        self
    }

    /// Returns whether panic capture is enabled.
    ///
    /// # Returns
    ///
    /// `true` when tester, prepare callback, and task panics are converted into
    /// execution errors instead of unwinding.
    #[inline]
    pub fn catch_panics(&self) -> bool {
        self.catch_panics
    }

    /// Runs the configured double-checked sequence under a write lock.
    ///
    /// # Parameters
    ///
    /// * `task` - The task to run with mutable access after both condition
    ///   checks pass.
    ///
    /// # Returns
    ///
    /// The final execution result, including prepare finalization.
    ///
    /// # Errors
    ///
    /// Task errors are captured as [`ExecutionResult::Failed`] with
    /// [`super::ExecutorError::TaskFailed`]. Prepare, commit, and rollback
    /// failures are also captured in the returned [`ExecutionResult`] rather
    /// than returned as a separate `Result`.
    fn execute_with_write_lock<R, E, F>(&self, task: F) -> ExecutionResult<R, E>
    where
        E: Display,
        F: FnOnce(&mut T) -> Result<R, E>,
    {
        let first_check = match self.try_run("tester", || self.tester.test()) {
            Ok(v) => v,
            Err(error) => {
                return ExecutionResult::from_executor_error(ExecutorError::Panic(error));
            }
        };

        if !first_check {
            self.log_unmet_condition();
            return ExecutionResult::unmet();
        }

        let prepare_completed = match self.run_prepare_action() {
            Ok(completed) => completed,
            Err(error) => {
                return ExecutionResult::from_executor_error(ExecutorError::PrepareFailed(error));
            }
        };

        let result = self.lock.write(|data| {
            let passed = match self.try_run("tester", || self.tester.test()) {
                Ok(v) => v,
                Err(error) => {
                    return ExecutionResult::from_executor_error(ExecutorError::Panic(error));
                }
            };
            if !passed {
                return ExecutionResult::unmet();
            }

            match self.try_run("task", || task(data)) {
                Ok(Ok(value)) => ExecutionResult::success(value),
                Ok(Err(error)) => ExecutionResult::task_failed(error),
                Err(error) => ExecutionResult::from_executor_error(ExecutorError::Panic(error)),
            }
        });

        if result.is_unmet() {
            self.log_unmet_condition();
        }

        if prepare_completed {
            self.finalize_prepare(result)
        } else {
            result
        }
    }

    /// Executes the optional prepare action.
    ///
    /// # Returns
    ///
    /// `Ok(true)` if prepare exists and succeeds, `Ok(false)` if no prepare
    /// action is configured, or `Err(message)` if prepare fails.
    ///
    /// # Errors
    ///
    /// Returns [`CallbackError`] when the prepare action returns an error or
    /// panics while panic capture is enabled.
    fn run_prepare_action(&self) -> Result<bool, CallbackError> {
        let Some(mut prepare_action) = self.prepare_action.clone() else {
            return Ok(false);
        };

        match self.try_run("prepare", move || prepare_action.run()) {
            Ok(Ok(_)) => Ok(true),
            Ok(Err(error)) => {
                self.logger.log_prepare_failed(&error);
                Err(error)
            }
            Err(error) => {
                self.logger.log_prepare_failed(&error);
                Err(error)
            }
        }
    }

    /// Commits or rolls back a successfully completed prepare action.
    ///
    /// This method runs after the write lock has been released.
    ///
    /// # Parameters
    ///
    /// * `result` - Result produced by the condition check and task execution.
    ///
    /// # Returns
    ///
    /// `result` unchanged when no finalization action fails. Returns a failed
    /// result when prepare commit or prepare rollback fails.
    fn finalize_prepare<R, E>(&self, mut result: ExecutionResult<R, E>) -> ExecutionResult<R, E>
    where
        E: Display,
    {
        if result.is_success() {
            if let Some(mut commit_prepare_action) = self.commit_prepare_action.clone() {
                match self.try_run("prepare_commit", move || commit_prepare_action.run()) {
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => {
                        self.logger.log_prepare_commit_failed(&error);
                        result = ExecutionResult::from_executor_error(ExecutorError::PrepareCommitFailed(error));
                    }
                    Err(error) => {
                        self.logger.log_prepare_commit_failed(&error);
                        result = ExecutionResult::from_executor_error(ExecutorError::PrepareCommitFailed(error));
                    }
                }
            }
            return result;
        }

        let original = if let ExecutionResult::Failed(error) = &result {
            original_failure_to_callback_error(error)
        } else {
            CallbackError::from_display("Condition not met")
        };

        if let Some(mut rollback_prepare_action) = self.rollback_prepare_action.clone() {
            match self.try_run("prepare_rollback", move || rollback_prepare_action.run()) {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => {
                    self.logger.log_prepare_rollback_failed(&error);
                    result = ExecutionResult::from_executor_error(ExecutorError::PrepareRollbackFailed {
                        original,
                        rollback: error,
                    });
                }
                Err(error) => {
                    self.logger.log_prepare_rollback_failed(&error);
                    result = ExecutionResult::from_executor_error(ExecutorError::PrepareRollbackFailed {
                        original,
                        rollback: error,
                    });
                }
            }
        }
        result
    }

    /// Runs a callback with optional panic capture.
    ///
    /// # Parameters
    ///
    /// * `callback_type` - Semantic label used if a captured panic is converted
    ///   into [`CallbackError`].
    /// * `callback` - Callback to execute.
    ///
    /// # Returns
    ///
    /// `Ok(value)` when the callback returns normally, or `Err(error)` when
    /// panic capture is enabled and the callback panics.
    ///
    /// # Errors
    ///
    /// Returns [`CallbackError`] only when `catch_panics` is enabled and
    /// `callback` panics.
    fn try_run<R>(&self, callback_type: &'static str, callback: impl FnOnce() -> R) -> Result<R, CallbackError> {
        if !self.catch_panics {
            return Ok(callback());
        }

        match panic::catch_unwind(AssertUnwindSafe(callback)) {
            Ok(result) => Ok(result),
            Err(payload) => {
                let message = panic_payload_to_message(&*payload);
                Err(CallbackError::with_callback_type(callback_type, message))
            }
        }
    }

    /// Logs that the double-checked condition was not met.
    ///
    /// This method has no return value.
    fn log_unmet_condition(&self) {
        self.logger.log_unmet_condition();
    }
}

type CallbackError = super::callback_error::CallbackError;

/// Converts the original execution failure into rollback metadata.
///
/// # Parameters
///
/// * `error` - Failure that caused prepare rollback to run.
///
/// # Returns
///
/// Callback error metadata suitable for the `original` field of
/// [`ExecutorError::PrepareRollbackFailed`].
fn original_failure_to_callback_error<E>(error: &ExecutorError<E>) -> CallbackError
where
    E: Display,
{
    match error {
        ExecutorError::Panic(error) => error.clone(),
        _ => CallbackError::from_display(error),
    }
}

/// Converts a panic payload into a stable diagnostic message.
///
/// # Parameters
///
/// * `payload` - Panic payload captured from `catch_unwind`.
///
/// # Returns
///
/// The string payload when available, or a stable generic message for
/// non-string panic payloads.
fn panic_payload_to_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.to_string()
    } else {
        "non-string panic payload".to_string()
    }
}
