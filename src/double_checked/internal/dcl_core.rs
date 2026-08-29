// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared double-checked locking core.

use std::cell::Cell;
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::sync::Arc;

use qubit_lock::Lock;

use crate::double_checked::PanicInfo;
use crate::double_checked::PanicPhase;
use crate::double_checked::internal::LockedExecution;
use crate::double_checked::internal::catch_phase;

/// Owns the predicate shared by both public executors.
pub(crate) struct DclCore {
    /// Lock-free predicate invoked before and after lock acquisition.
    predicate: Arc<dyn Fn() -> bool + Send + Sync + 'static>,
}

impl DclCore {
    /// Creates a DCL core with the configured predicate.
    ///
    /// # Parameters
    ///
    /// * `predicate` - Lock-free condition checked twice.
    ///
    /// # Type Parameters
    ///
    /// * `F` - Thread-safe predicate callback type.
    ///
    /// # Returns
    ///
    /// A core ready to be configured or built into an executor.
    #[inline]
    pub(crate) fn new<F>(predicate: F) -> Self
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        Self {
            predicate: Arc::new(predicate),
        }
    }

    /// Performs the initial lock-free condition check.
    ///
    /// # Returns
    ///
    /// The predicate result.
    ///
    /// # Panics
    ///
    /// Propagates a predicate panic.
    #[inline(always)]
    pub(crate) fn check_initial(&self) -> bool {
        (self.predicate)()
    }

    /// Performs the initial check and captures any panic.
    ///
    /// # Returns
    ///
    /// The predicate result or panic metadata classified as
    /// [`PanicPhase::InitialConditionCheck`].
    ///
    /// # Errors
    ///
    /// Returns [`Err`] when the predicate panics.
    #[inline]
    pub(crate) fn check_initial_catching(&self) -> Result<bool, PanicInfo> {
        catch_phase(PanicPhase::InitialConditionCheck, || self.check_initial())
    }

    /// Acquires the lock, checks the condition again, and optionally runs
    /// the task without catching panics.
    ///
    /// # Parameters
    ///
    /// * `lock` - Lock used for the protected phase of this invocation.
    /// * `task` - Task to run only when the second check succeeds.
    ///
    /// # Type Parameters
    ///
    /// * `L` - Lock type used for the protected phase.
    /// * `R` - Successful task result type.
    /// * `E` - Task error type.
    /// * `F` - One-shot task callback type.
    ///
    /// # Returns
    ///
    /// The locked-phase result.
    ///
    /// # Panics
    ///
    /// Propagates lock, predicate, and task panics through the concrete lock
    /// implementation.
    pub(crate) fn execute_locked<L, R, E, F>(&self, lock: &L, task: F) -> LockedExecution<R, E>
    where
        L: Lock + ?Sized,
        F: FnOnce() -> Result<R, E>,
    {
        let _guard = lock.lock();
        if !(self.predicate)() {
            LockedExecution::ConditionNotMet
        } else {
            LockedExecution::Task(task())
        }
    }

    /// Executes the complete locked phase behind one outer panic boundary.
    ///
    /// Keeping the boundary outside the RAII guard ensures an unwind from the
    /// task crosses the concrete lock guard before it is converted into
    /// metadata, preserving standard-lock poisoning behavior.
    ///
    /// # Parameters
    ///
    /// * `lock` - Lock used for the protected phase of this invocation.
    /// * `task` - Task to run only when the second check succeeds.
    ///
    /// # Returns
    ///
    /// The locked result or panic metadata with the most precise active phase.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] when lock acquisition or release, the second predicate
    /// check, or the task panics.
    pub(crate) fn execute_locked_catching<L, R, E, F>(
        &self,
        lock: &L,
        task: F,
    ) -> Result<LockedExecution<R, E>, PanicInfo>
    where
        L: Lock + ?Sized,
        F: FnOnce() -> Result<R, E>,
    {
        let phase = Cell::new(PanicPhase::LockAcquisition);
        catch_unwind(AssertUnwindSafe(|| {
            let guard = lock.lock();
            phase.set(PanicPhase::SecondConditionCheck);
            let outcome = if !(self.predicate)() {
                LockedExecution::ConditionNotMet
            } else {
                phase.set(PanicPhase::Task);
                LockedExecution::Task(task())
            };
            phase.set(PanicPhase::LockRelease);
            drop(guard);
            outcome
        }))
        .map_err(|payload| PanicInfo::from_payload(phase.get(), payload))
    }
}

impl Clone for DclCore {
    /// Shares the erased predicate.
    #[inline]
    fn clone(&self) -> Self {
        Self {
            predicate: self.predicate.clone(),
        }
    }
}
