// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Reusable lifecycle-aware double-checked lock executor.

use std::{
    error::Error,
    panic::{
        AssertUnwindSafe,
        catch_unwind,
    },
    sync::Arc,
};

use qubit_lock::Lock;

use crate::double_checked::{
    CapturedFinalizationOutcome,
    CapturedLifecycleOutcome,
    FinalizationOutcome,
    LifecycleDclExecutorBuilder,
    LifecycleOutcome,
    PanicInfo,
    PanicPhase,
    RollbackCause,
    internal::{
        DclCore,
        CapturedRollbackExecution,
        LockedExecution,
        RollbackExecution,
        catch_phase,
        finalize_commit,
        finalize_commit_catching,
        finalize_rollback,
        finalize_rollback_catching,
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
///
/// The executor intentionally owns neither a lock nor protected data. The same
/// executor may be invoked with the read and write modes obtained from one
/// RWLock. A read-only task and a write task can operate on different captured
/// data while consulting the same atomic gate; their paired modes on the same
/// underlying lock provide the required coordination. This is why each
/// [`Self::run`] or [`Self::run_with_token`] call supplies its lock mode.
#[must_use = "an executor does nothing until run or run_with_token is called"]
pub struct LifecycleDclExecutor<P, C> {
    /// Shared DCL predicate.
    core: DclCore,
    /// Callback that creates one token for each prepared invocation.
    prepare: PrepareCallback<P, C>,
    /// Optional successful-path finalizer.
    commit: Option<CommitCallback<P, C>>,
    /// Optional unsuccessful-path finalizer.
    rollback: Option<RollbackCallback<P, C>>,
}

impl LifecycleDclExecutor<(), ()> {
    /// Starts building a lifecycle executor.
    ///
    /// # Returns
    ///
    /// A typestate builder requiring a predicate and lifecycle callbacks.
    #[inline]
    pub fn builder() -> LifecycleDclExecutorBuilder {
        LifecycleDclExecutorBuilder::new()
    }
}

impl<P, C> LifecycleDclExecutor<P, C> {
    /// Creates a built lifecycle executor from typestate-validated parts.
    ///
    /// # Parameters
    ///
    /// * `core` - Predicate configuration.
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
    ///   check.
    ///
    /// # Returns
    ///
    /// The single terminal lifecycle outcome.
    #[inline(always)]
    pub fn run<L, R, E, F>(&self, lock: &L, task: F) -> LifecycleOutcome<R, E, C>
    where
        L: Lock + ?Sized,
        E: Error + Send + Sync + 'static,
        F: FnOnce() -> Result<R, E>,
    {
        self.run_with_token(lock, move |_| task())
    }

    /// Runs a task with direct access to its invocation token.
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
    /// The single terminal lifecycle outcome.
    #[inline]
    pub fn run_with_token<L, R, E, F>(
        &self,
        lock: &L,
        task: F,
    ) -> LifecycleOutcome<R, E, C>
    where
        L: Lock + ?Sized,
        E: Error + Send + Sync + 'static,
        F: FnOnce(&mut P) -> Result<R, E>,
    {
        if !self.core.check_initial() {
            return LifecycleOutcome::InitialConditionNotMet;
        }
        let mut token = match (self.prepare)() {
            Ok(token) => token,
            Err(error) => {
                return LifecycleOutcome::PrepareFailed(error);
            }
        };

        match self.core.execute_locked_catching(lock, || task(&mut token)) {
            Ok(LockedExecution::ConditionNotMet) => {
                self.finish_rollback(token, RollbackExecution::ConditionNotMet)
            }
            Ok(LockedExecution::Task(Ok(value))) => {
                self.finish_commit(token, value)
            }
            Ok(LockedExecution::Task(Err(error))) => {
                self.finish_rollback(token, RollbackExecution::TaskFailed(error))
            }
            Err(panic) => self.rollback_then_resume(token, panic),
        }
    }

    /// Runs a task with panic capture enabled.
    ///
    /// # Parameters
    ///
    /// * `lock` - Lock used for this invocation.
    /// * `task` - Task to run in the locked phase.
    ///
    /// # Returns
    ///
    /// Captured lifecycle outcome when execution crosses panic boundaries.
    pub fn run_catching<L, R, E, F>(&self, lock: &L, task: F) -> CapturedLifecycleOutcome<R, E, C>
    where
        L: Lock + ?Sized,
        E: Error + Send + Sync + 'static,
        F: FnOnce() -> Result<R, E>,
    {
        self.run_with_token_catching(lock, move |_| task())
    }

    /// Runs a task with panic capture enabled.
    ///
    /// # Parameters
    ///
    /// * `lock` - Lock used for this invocation.
    /// * `task` - Token-aware task to run in the locked phase.
    ///
    /// # Returns
    ///
    /// Captured lifecycle outcome preserving task/rollback panics.
    pub fn run_with_token_catching<L, R, E, F>(
        &self,
        lock: &L,
        task: F,
    ) -> CapturedLifecycleOutcome<R, E, C>
    where
        L: Lock + ?Sized,
        E: Error + Send + Sync + 'static,
        F: FnOnce(&mut P) -> Result<R, E>,
    {
        match self.core.check_initial_catching() {
            Ok(true) => {}
            Ok(false) => {
                return CapturedLifecycleOutcome::InitialConditionNotMet;
            }
            Err(panic) => {
                return CapturedLifecycleOutcome::InitialConditionCheckPanicked(panic);
            }
        }

        let mut token = match catch_phase(PanicPhase::Prepare, || (self.prepare)()) {
            Ok(Ok(token)) => token,
            Ok(Err(error)) => {
                return CapturedLifecycleOutcome::PrepareFailed(error);
            }
            Err(panic) => {
                return CapturedLifecycleOutcome::PreparePanicked(panic);
            }
        };

        match self.core.execute_locked_catching(lock, || task(&mut token)) {
            Ok(LockedExecution::ConditionNotMet) => {
                self.finish_rollback_catching(
                    token,
                    CapturedRollbackExecution::ConditionNotMet,
                )
            }
            Ok(LockedExecution::Task(Ok(value))) => {
                self.finish_commit_catching(token, value)
            }
            Ok(LockedExecution::Task(Err(error))) => self.finish_rollback_catching(
                token,
                CapturedRollbackExecution::TaskFailed(error),
            ),
            Err(panic) => self.finish_rollback_catching(
                token,
                CapturedRollbackExecution::Panicked(panic),
            ),
        }
    }

    /// Finalizes a successful task without holding the executor lock.
    #[inline]
    fn finish_commit<R, E>(&self, token: P, value: R) -> LifecycleOutcome<R, E, C> {
        let commit = finalize_commit(self.commit.as_deref(), token);
        LifecycleOutcome::TaskSucceeded { value, commit }
    }

    /// Finalizes a successful task without holding the executor lock.
    fn finish_commit_catching<R, E>(
        &self,
        token: P,
        value: R,
    ) -> CapturedLifecycleOutcome<R, E, C> {
        let commit = finalize_commit_catching(self.commit.as_deref(), token);
        CapturedLifecycleOutcome::TaskSucceeded { value, commit }
    }

    /// Finalizes an unsuccessful invocation without holding the executor lock.
    #[inline]
    fn finish_rollback<R, E>(
        &self,
        token: P,
        execution: RollbackExecution<E>,
    ) -> LifecycleOutcome<R, E, C>
    where
        E: Error + Send + Sync + 'static,
    {
        let cause = execution.cause();
        let rollback = finalize_rollback(self.rollback.as_deref(), token, cause);
        execution.into_outcome(rollback)
    }

    /// Finalizes an unsuccessful invocation without holding the executor lock.
    fn finish_rollback_catching<R, E>(
        &self,
        token: P,
        execution: CapturedRollbackExecution<E>,
    ) -> CapturedLifecycleOutcome<R, E, C> {
        let cause = execution.cause();
        let rollback = finalize_rollback_catching(self.rollback.as_deref(), token, cause);
        execution.into_outcome(rollback)
    }

    /// Attempts rollback after a locked-phase panic and then resumes the
    /// original unwind payload.
    #[inline]
    fn rollback_then_resume(&self, token: P, panic: PanicInfo) -> ! {
        if let Some(rollback) = &self.rollback {
            Self::discard_secondary(|| {
                let _ = rollback(token, RollbackCause::Panicked(&panic));
            });
        } else {
            Self::discard_secondary(|| drop(token));
        }
        panic.resume_unwind()
    }

    /// Runs and discards secondary cleanup work without allowing its result,
    /// panic payload, or destructor panic to replace an earlier panic.
    #[inline]
    fn discard_secondary<F>(operation: F)
    where
        F: FnOnce(),
    {
        let secondary = catch_unwind(AssertUnwindSafe(|| {
            let _ = catch_unwind(AssertUnwindSafe(operation));
        }));
        if let Err(payload) = secondary {
            std::mem::forget(payload);
        }
    }
}

impl<P, C> Clone for LifecycleDclExecutor<P, C> {
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
