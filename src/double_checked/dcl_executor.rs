// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Reusable basic double-checked lock executor.

use qubit_lock::Lock;

use crate::double_checked::{
    DclExecutorBuilder,
    ExecutionOutcome,
    internal::DclCore,
};

/// Executes arbitrary tasks using the double-checked locking pattern.
///
/// The first predicate call is lock-free. If it succeeds, the executor obtains
/// the supplied lock, evaluates the same predicate again, and runs the task
/// while still holding that lock. The lock's protected data is deliberately
/// not exposed to either the predicate or task.
///
/// The supplied [`Lock`] selects an acquisition mode, not necessarily an
/// exclusive one. Shared modes allow multiple tasks to overlap; exclusive
/// modes serialize the second check and task.
///
/// The executor intentionally does not own a lock or protected data. One
/// common use invokes the same executor with the read and write modes obtained
/// from one RWLock: a reader runs a read-only task under the read mode, while a
/// writer runs a write task under the paired write mode. The two tasks may use
/// different captured data while consulting the same atomic gate. Passing the
/// mode to each [`Self::run`] call preserves this coordination-only role.
#[must_use = "an executor does nothing until run is called"]
pub struct DclExecutor {
    /// Shared predicate and panic configuration.
    core: DclCore,
}

impl DclExecutor {
    /// Starts building an executor.
    ///
    /// # Returns
    ///
    /// A typestate builder that requires a predicate before it can build.
    #[inline]
    pub fn builder() -> DclExecutorBuilder {
        DclExecutorBuilder::new()
    }

    /// Creates a built executor from its configured core.
    ///
    /// # Parameters
    ///
    /// * `core` - Complete predicate and panic configuration.
    ///
    /// # Returns
    ///
    /// A reusable executor.
    #[inline]
    pub(crate) fn from_core(core: DclCore) -> Self {
        Self { core }
    }
    /// Runs `task` only when both condition checks succeed.
    ///
    /// The first condition check performs no lock operation. The second check
    /// and task run under one RAII guard. That guard may represent shared or
    /// exclusive acquisition.
    ///
    /// This method intentionally accepts [`Lock`] rather than
    /// [`qubit_lock::ExclusiveLock`]. It uses the same execution algorithm for
    /// every supplied acquisition mode and cannot determine whether `task` is
    /// read-only. When a shared mode is supplied, upholding the read-only
    /// contract is the caller's responsibility; Rust's type system does not
    /// enforce it.
    ///
    /// A shared mode is valid when the task is read-only with respect to the
    /// protected protocol, every conflicting writer uses the paired exclusive
    /// mode of the same underlying lock, and the caller does not require
    /// at-most-once task execution. Multiple shared-mode invocations may pass
    /// the second check and run their tasks concurrently.
    ///
    /// A task that changes the gate or protected state, or otherwise requires
    /// serialized or at-most-once execution, must receive a lock mode
    /// implementing [`qubit_lock::ExclusiveLock`] or use a separate uniqueness
    /// mechanism such as compare-and-exchange.
    ///
    /// The same executor may be called concurrently with read and write modes
    /// from one RWLock. Correctness depends on those modes sharing the same
    /// underlying lock, not on the tasks accessing the same data: a read-only
    /// task and a write task may capture different objects while sharing the
    /// predicate's state variable.
    ///
    /// # Parameters
    ///
    /// * `lock` - Generic synchronous lock used for this invocation.
    /// * `task` - One-shot task executed inside the lock after the second
    ///   check.
    ///
    /// # Returns
    ///
    /// A structured condition, task, or captured-panic outcome.
    ///
    /// # Errors
    ///
    /// A task error is preserved in [`ExecutionOutcome::TaskFailed`]; this
    /// method does not collapse or transform it.
    ///
    /// # Panics
    ///
    /// When panic capture is disabled, propagates panics from the predicate,
    /// lock implementation, or task. When enabled, those panics are returned
    /// as [`ExecutionOutcome::Panicked`]. A guard-drop panic after locked work
    /// completes is classified as [`crate::PanicPhase::LockRelease`].
    /// Capture requires an unwinding panic strategy; with `panic = "abort"`,
    /// the process terminates without returning an outcome.
    ///
    /// Capturing a panic classifies and transports its payload. It does not
    /// undo earlier side effects, restore application invariants, or make
    /// poisoned state safe to reuse. The caller must verify or reestablish
    /// those invariants before continuing.
    ///
    /// # Synchronization
    ///
    /// The predicate must read an atomic or equivalently synchronized gate.
    /// Use an Acquire load paired with Release stores unless the application
    /// requires stronger ordering.
    ///
    /// # Locking
    ///
    /// The first predicate call does not acquire `lock`. The second call and
    /// `task` share one guard from the supplied acquisition mode. The predicate
    /// must not acquire the same underlying lock.
    ///
    /// With a shared/read mode, the task must remain read-only relative to the
    /// protected protocol and all conflicting updates must acquire the paired
    /// write mode. With an exclusive mode, the task may update its captured
    /// gate directly; later or external conflicting updates must acquire the
    /// same underlying lock.
    pub fn run<L, R, E, F>(&self, lock: &L, task: F) -> ExecutionOutcome<R, E>
    where
        L: Lock + ?Sized,
        F: FnOnce() -> Result<R, E>,
    {
        if self.core.catch_panics() {
            match self.core.check_initial_catching() {
                Ok(true) => {}
                Ok(false) => return ExecutionOutcome::ConditionNotMet,
                Err(panic) => return ExecutionOutcome::Panicked(panic),
            }
            match self.core.execute_locked_catching(lock, task) {
                Ok(locked) => locked.into_outcome(),
                Err(panic) => ExecutionOutcome::Panicked(panic),
            }
        } else {
            if !self.core.check_initial() {
                return ExecutionOutcome::ConditionNotMet;
            }
            self.core.execute_locked(lock, task).into_outcome()
        }
    }
}

impl Clone for DclExecutor {
    /// Shares the predicate configuration.
    #[inline]
    fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
        }
    }
}
