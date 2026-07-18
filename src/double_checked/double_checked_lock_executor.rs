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
    DoubleCheckedLockExecutorBuilder,
    ExecutionOutcome,
    internal::DclCore,
};

/// Executes arbitrary tasks using the double-checked locking pattern.
///
/// The first predicate call is lock-free. If it succeeds, the executor obtains
/// a write lock, evaluates the same predicate again, and runs the task while
/// still holding that lock. The protected `T` is deliberately not exposed to
/// either the predicate or task.
#[must_use = "an executor does nothing until run is called"]
pub struct DoubleCheckedLockExecutor<L, T: ?Sized> {
    /// Shared lock, predicate, and panic configuration.
    core: DclCore<L, T>,
}

impl DoubleCheckedLockExecutor<(), ()> {
    /// Starts building an executor around `lock`.
    ///
    /// # Parameters
    ///
    /// * `lock` - Generic synchronous lock used for the protected phase.
    ///
    /// # Returns
    ///
    /// A typestate builder that requires a predicate before it can build.
    #[inline]
    pub fn builder<L, T: ?Sized>(
        lock: L,
    ) -> DoubleCheckedLockExecutorBuilder<L, T>
    where
        L: Lock<T>,
    {
        DoubleCheckedLockExecutorBuilder::new(lock)
    }
}

impl<L, T: ?Sized> DoubleCheckedLockExecutor<L, T> {
    /// Creates a built executor from its configured core.
    ///
    /// # Parameters
    ///
    /// * `core` - Complete lock and predicate configuration.
    ///
    /// # Returns
    ///
    /// A reusable executor.
    #[inline]
    pub(crate) fn from_core(core: DclCore<L, T>) -> Self {
        Self { core }
    }
}

impl<L, T: ?Sized> DoubleCheckedLockExecutor<L, T>
where
    L: Lock<T>,
{
    /// Runs `task` only when both condition checks succeed.
    ///
    /// The first condition check performs no lock operation. The second check
    /// and task run inside one `with_write` call. A task that needs to change
    /// the gate may safely do so through captured atomic state because it is
    /// already inside the executor's critical section. Gate changes made after
    /// the task or on other system paths must acquire the same underlying lock.
    ///
    /// # Parameters
    ///
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
    /// as [`ExecutionOutcome::Panicked`].
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
    /// `task` share one write-lock critical section. The predicate must not
    /// acquire the same underlying lock. A task may update its captured gate
    /// directly; later or external gate updates that must exclude the task must
    /// acquire that same underlying lock.
    pub fn run<R, E, F>(&self, task: F) -> ExecutionOutcome<R, E>
    where
        F: FnOnce() -> Result<R, E>,
    {
        if self.core.catch_panics() {
            match self.core.check_initial_catching() {
                Ok(true) => {}
                Ok(false) => return ExecutionOutcome::ConditionNotMet,
                Err(panic) => return ExecutionOutcome::Panicked(panic),
            }
            match self.core.execute_locked_catching(task) {
                Ok(locked) => locked.into_outcome(),
                Err(panic) => ExecutionOutcome::Panicked(panic),
            }
        } else {
            if !self.core.check_initial() {
                return ExecutionOutcome::ConditionNotMet;
            }
            self.core.execute_locked(task).into_outcome()
        }
    }
}

impl<L, T: ?Sized> Clone for DoubleCheckedLockExecutor<L, T>
where
    L: Clone,
{
    /// Clones the lock handle and shares the predicate configuration.
    #[inline]
    fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
        }
    }
}
