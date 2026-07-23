// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Reusable lifecycle-aware double-checked lock executor.

use std::{
    error::Error,
    panic::{
        AssertUnwindSafe,
        catch_unwind,
        resume_unwind,
    },
    sync::Arc,
};

use qubit_lock::Lock;

use crate::double_checked::{
    ExecutionOutcome,
    ExecutionReport,
    LifecycleDoubleCheckedLockExecutorBuilder,
    PanicInfo,
    PanicPhase,
    PreparationOutcome,
    RollbackCause,
    internal::{
        DclCore,
        LockedExecution,
        catch_phase,
    },
};

/// Shared prepare callback type used by lifecycle executor clones.
pub(crate) type PrepareCallback<P, C> =
    Arc<dyn Fn() -> Result<P, C> + Send + Sync + 'static>;

/// Shared commit callback type used by lifecycle executor clones.
pub(crate) type CommitCallback<P, C> =
    Arc<dyn Fn(P) -> Result<(), C> + Send + Sync + 'static>;

/// Shared rollback callback type used by lifecycle executor clones.
pub(crate) type RollbackCallback<P, C> = Arc<
    dyn for<'a> Fn(P, RollbackCause<'a>) -> Result<(), C>
        + Send
        + Sync
        + 'static,
>;

/// Executes a DCL task with per-invocation prepare and finalization callbacks.
///
/// Prepare runs after the lock-free check and before lock acquisition. Commit
/// or rollback consumes the token after the executor lock has been released.
#[must_use = "an executor does nothing until run or run_with_token is called"]
pub struct LifecycleDoubleCheckedLockExecutor<P, C> {
    /// Shared DCL predicate and panic configuration.
    core: DclCore,
    /// Callback that creates one token for each prepared invocation.
    prepare: PrepareCallback<P, C>,
    /// Optional successful-path finalizer.
    commit: Option<CommitCallback<P, C>>,
    /// Optional unsuccessful-path finalizer.
    rollback: Option<RollbackCallback<P, C>>,
}

impl LifecycleDoubleCheckedLockExecutor<(), ()> {
    /// Starts building a lifecycle executor.
    ///
    /// # Returns
    ///
    /// A typestate builder requiring a predicate and lifecycle callbacks.
    #[inline]
    pub fn builder() -> LifecycleDoubleCheckedLockExecutorBuilder {
        LifecycleDoubleCheckedLockExecutorBuilder::new()
    }
}

impl<P, C> LifecycleDoubleCheckedLockExecutor<P, C> {
    /// Creates a built lifecycle executor from typestate-validated parts.
    ///
    /// # Parameters
    ///
    /// * `core` - Complete predicate and panic configuration.
    /// * `prepare` - Per-invocation token producer.
    /// * `commit` - Optional successful-path finalizer.
    /// * `rollback` - Optional unsuccessful-path finalizer.
    ///
    /// # Returns
    ///
    /// A reusable lifecycle executor.
    #[inline]
    pub(crate) fn from_parts(
        core: DclCore,
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
    /// Runs a task that does not need direct access to the prepare token.
    ///
    /// # Parameters
    ///
    /// * `lock` - Generic synchronous lock used for this invocation.
    /// * `task` - One-shot task executed inside the lock after the second
    ///   condition check.
    ///
    /// # Returns
    ///
    /// A report retaining both the execution and lifecycle outcomes.
    ///
    /// # Errors
    ///
    /// Task and lifecycle errors are returned inside their respective report
    /// axes and never overwrite one another.
    ///
    /// # Panics
    ///
    /// When panic capture is disabled, propagates callback, lock, predicate,
    /// and task panics. A panic after prepare in the locked phase first
    /// releases the lock and attempts rollback before the original panic is
    /// resumed.
    ///
    /// # Synchronization
    ///
    /// The predicate must read an atomic or equivalently synchronized gate.
    /// Prepare, commit, and rollback are shared `Fn` callbacks and may be
    /// called concurrently by separate invocations.
    ///
    /// # Locking
    ///
    /// The initial check, prepare, commit, and rollback run outside the
    /// executor lock. The second check and task share one guard from the
    /// supplied acquisition mode. A shared/read mode permits concurrent tasks
    /// and is valid only for read-only protected protocols without at-most-once
    /// requirements. Gate mutation, protected writes, or serialized execution
    /// require an [`qubit_lock::ExclusiveLock`] mode. Lifecycle callbacks do
    /// not automatically reacquire that lock.
    #[inline(always)]
    pub fn run<L, R, E, F>(&self, lock: &L, task: F) -> ExecutionReport<R, E, C>
    where
        L: Lock + ?Sized,
        E: Error + Send + Sync + 'static,
        F: FnOnce() -> Result<R, E>,
    {
        self.run_with_token(lock, move |_| task())
    }

    /// Runs a task with mutable access to its invocation's prepare token.
    ///
    /// The token exists on the invocation stack rather than in shared executor
    /// state. The task runs while the executor guard is held; commit or
    /// rollback later consumes the same token outside that guard.
    ///
    /// # Parameters
    ///
    /// * `lock` - Generic synchronous lock used for this invocation.
    /// * `task` - One-shot task receiving the invocation token by mutable
    ///   reference.
    ///
    /// # Returns
    ///
    /// A report retaining both the execution and lifecycle outcomes.
    ///
    /// # Errors
    ///
    /// Task and lifecycle errors are returned inside their respective report
    /// axes and never overwrite one another.
    ///
    /// # Panics
    ///
    /// Uses the same panic behavior as [`Self::run`].
    ///
    /// # Synchronization
    ///
    /// Each invocation owns its token on its call stack. The callback objects
    /// are shared and may be invoked concurrently, so captured mutable state
    /// requires its own synchronization.
    ///
    /// # Locking
    ///
    /// Token mutation by `task` occurs while the executor guard is held.
    /// Because each token belongs to one invocation, mutating the token alone
    /// does not require exclusive lock acquisition. Mutating captured gate or
    /// protected shared state does require an [`qubit_lock::ExclusiveLock`]
    /// mode or a separate uniqueness mechanism. Commit and rollback consume the
    /// token only after the guard has been released.
    #[inline]
    pub fn run_with_token<L, R, E, F>(
        &self,
        lock: &L,
        task: F,
    ) -> ExecutionReport<R, E, C>
    where
        L: Lock + ?Sized,
        E: Error + Send + Sync + 'static,
        F: FnOnce(&mut P) -> Result<R, E>,
    {
        if self.core.catch_panics() {
            self.run_catching(lock, task)
        } else {
            self.run_propagating(lock, task)
        }
    }

    /// Executes every lifecycle phase with structured panic capture enabled.
    ///
    /// # Parameters
    ///
    /// * `lock` - Lock used for this invocation.
    /// * `task` - Token-aware task to run in the locked phase.
    ///
    /// # Returns
    ///
    /// A complete report, including any captured panic metadata.
    fn run_catching<L, R, E, F>(
        &self,
        lock: &L,
        task: F,
    ) -> ExecutionReport<R, E, C>
    where
        L: Lock + ?Sized,
        E: Error + Send + Sync + 'static,
        F: FnOnce(&mut P) -> Result<R, E>,
    {
        match self.core.check_initial_catching() {
            Ok(true) => {}
            Ok(false) => {
                return ExecutionReport::new(
                    ExecutionOutcome::ConditionNotMet,
                    PreparationOutcome::NotStarted,
                );
            }
            Err(panic) => {
                return ExecutionReport::new(
                    ExecutionOutcome::Panicked(panic),
                    PreparationOutcome::NotStarted,
                );
            }
        }

        let mut token =
            match catch_phase(PanicPhase::Prepare, || (self.prepare)()) {
                Ok(Ok(token)) => token,
                Ok(Err(error)) => {
                    return ExecutionReport::new(
                        ExecutionOutcome::NotExecuted,
                        PreparationOutcome::PrepareFailed(error),
                    );
                }
                Err(panic) => {
                    return ExecutionReport::new(
                        ExecutionOutcome::NotExecuted,
                        PreparationOutcome::PreparePanicked(panic),
                    );
                }
            };

        match self.core.execute_locked_catching(lock, || task(&mut token)) {
            Ok(LockedExecution::ConditionNotMet) => {
                self.finish_rollback(token, ExecutionOutcome::ConditionNotMet)
            }
            Ok(LockedExecution::Task(Ok(value))) => {
                self.finish_commit(token, value)
            }
            Ok(LockedExecution::Task(Err(error))) => {
                self.finish_rollback(token, ExecutionOutcome::TaskFailed(error))
            }
            Err(panic) => {
                self.finish_rollback(token, ExecutionOutcome::Panicked(panic))
            }
        }
    }

    /// Executes uncaptured phases directly while retaining a temporary outer
    /// boundary around the locked phase for rollback.
    ///
    /// # Parameters
    ///
    /// * `lock` - Lock used for this invocation.
    /// * `task` - Token-aware task to run in the locked phase.
    ///
    /// # Returns
    ///
    /// A complete report when no callback or locked phase panics.
    ///
    /// # Panics
    ///
    /// Propagates initial-check, prepare, commit, and ordinary rollback panics.
    /// A locked-phase panic is resumed after rollback is attempted.
    fn run_propagating<L, R, E, F>(
        &self,
        lock: &L,
        task: F,
    ) -> ExecutionReport<R, E, C>
    where
        L: Lock + ?Sized,
        E: Error + Send + Sync + 'static,
        F: FnOnce(&mut P) -> Result<R, E>,
    {
        if !self.core.check_initial() {
            return ExecutionReport::new(
                ExecutionOutcome::ConditionNotMet,
                PreparationOutcome::NotStarted,
            );
        }
        let mut token = match (self.prepare)() {
            Ok(token) => token,
            Err(error) => {
                return ExecutionReport::new(
                    ExecutionOutcome::NotExecuted,
                    PreparationOutcome::PrepareFailed(error),
                );
            }
        };

        match self.core.execute_locked_catching(lock, || task(&mut token)) {
            Ok(LockedExecution::ConditionNotMet) => {
                self.finish_rollback(token, ExecutionOutcome::ConditionNotMet)
            }
            Ok(LockedExecution::Task(Ok(value))) => {
                self.finish_commit(token, value)
            }
            Ok(LockedExecution::Task(Err(error))) => {
                self.finish_rollback(token, ExecutionOutcome::TaskFailed(error))
            }
            Err(panic) => self.rollback_then_resume(token, panic),
        }
    }

    /// Finalizes a successful task without holding the executor lock.
    ///
    /// # Parameters
    ///
    /// * `token` - Token produced for this invocation.
    /// * `value` - Successful task value.
    ///
    /// # Returns
    ///
    /// A report combining success with commit or no-commit status.
    ///
    /// # Panics
    ///
    /// Propagates a commit panic when panic capture is disabled.
    fn finish_commit<R, E>(
        &self,
        token: P,
        value: R,
    ) -> ExecutionReport<R, E, C> {
        let preparation = match &self.commit {
            Some(commit) if self.core.catch_panics() => {
                match catch_phase(PanicPhase::Commit, || commit(token)) {
                    Ok(Ok(())) => PreparationOutcome::Committed,
                    Ok(Err(error)) => PreparationOutcome::CommitFailed(error),
                    Err(panic) => PreparationOutcome::CommitPanicked(panic),
                }
            }
            Some(commit) => match commit(token) {
                Ok(()) => PreparationOutcome::Committed,
                Err(error) => PreparationOutcome::CommitFailed(error),
            },
            None => {
                drop(token);
                PreparationOutcome::CommitNotRequired
            }
        };
        ExecutionReport::new(ExecutionOutcome::Success(value), preparation)
    }

    /// Finalizes an unsuccessful invocation without holding the executor lock.
    ///
    /// # Parameters
    ///
    /// * `token` - Token produced for this invocation.
    /// * `execution` - Condition, task-error, or captured-panic outcome that
    ///   requires rollback.
    ///
    /// # Returns
    ///
    /// A report retaining `execution` independently from rollback status.
    ///
    /// # Panics
    ///
    /// Propagates a rollback panic when panic capture is disabled.
    fn finish_rollback<R, E>(
        &self,
        token: P,
        execution: ExecutionOutcome<R, E>,
    ) -> ExecutionReport<R, E, C>
    where
        E: Error + Send + Sync + 'static,
    {
        let preparation = match &self.rollback {
            Some(rollback) => {
                let cause = Self::rollback_cause(&execution);
                if self.core.catch_panics() {
                    match catch_phase(PanicPhase::Rollback, || {
                        rollback(token, cause)
                    }) {
                        Ok(Ok(())) => PreparationOutcome::RolledBack,
                        Ok(Err(error)) => {
                            PreparationOutcome::RollbackFailed(error)
                        }
                        Err(panic) => {
                            PreparationOutcome::RollbackPanicked(panic)
                        }
                    }
                } else {
                    match rollback(token, cause) {
                        Ok(()) => PreparationOutcome::RolledBack,
                        Err(error) => PreparationOutcome::RollbackFailed(error),
                    }
                }
            }
            None => {
                drop(token);
                PreparationOutcome::RollbackNotRequired
            }
        };
        ExecutionReport::new(execution, preparation)
    }

    /// Creates the borrowed rollback cause for a completed execution outcome.
    ///
    /// # Parameters
    ///
    /// * `execution` - Outcome requiring rollback.
    ///
    /// # Returns
    ///
    /// A cause borrowing any task error or panic metadata from `execution`.
    ///
    /// # Panics
    ///
    /// Panics if called with an outcome that does not require rollback. This is
    /// an internal invariant enforced by the state-machine dispatch.
    #[inline]
    fn rollback_cause<R, E>(
        execution: &ExecutionOutcome<R, E>,
    ) -> RollbackCause<'_>
    where
        E: Error + Send + Sync + 'static,
    {
        match execution {
            ExecutionOutcome::ConditionNotMet => RollbackCause::ConditionNotMet,
            ExecutionOutcome::TaskFailed(error) => {
                RollbackCause::TaskFailed(error)
            }
            ExecutionOutcome::Panicked(panic) => RollbackCause::Panicked(panic),
            ExecutionOutcome::Success(_) | ExecutionOutcome::NotExecuted => {
                unreachable!("only failed locked outcomes require rollback")
            }
        }
    }

    /// Attempts rollback after a locked-phase panic and then resumes the
    /// original unwind payload.
    ///
    /// Secondary rollback errors, rollback panics, or token destructor panics
    /// are intentionally discarded so they cannot replace the original panic
    /// when capture is disabled.
    ///
    /// # Parameters
    ///
    /// * `token` - Token produced for the panicking invocation.
    /// * `panic` - Original locked-phase panic metadata.
    ///
    /// # Panics
    ///
    /// Always resumes the original panic payload after rollback is attempted.
    fn rollback_then_resume(&self, token: P, panic: PanicInfo) -> ! {
        if let Some(rollback) = &self.rollback {
            let _secondary = catch_unwind(AssertUnwindSafe(|| {
                rollback(token, RollbackCause::Panicked(&panic))
            }));
        } else {
            let _secondary = catch_unwind(AssertUnwindSafe(|| drop(token)));
        }
        resume_unwind(panic.into_payload())
    }
}

impl<P, C> Clone for LifecycleDoubleCheckedLockExecutor<P, C> {
    /// Shares all erased callbacks.
    #[inline]
    fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
            prepare: Arc::clone(&self.prepare),
            commit: self.commit.as_ref().map(Arc::clone),
            rollback: self.rollback.as_ref().map(Arc::clone),
        }
    }
}
