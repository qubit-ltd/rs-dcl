// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared double-checked locking core.

use std::{
    cell::Cell,
    panic::{
        AssertUnwindSafe,
        catch_unwind,
    },
    sync::Arc,
};

use qubit_lock::Lock;

use crate::double_checked::{
    PanicInfo,
    PanicPhase,
    internal::{
        LockedExecution,
        catch_phase,
    },
};

/// Owns the predicate and panic-capture configuration shared by both public
/// executors.
pub(crate) struct DclCore {
    /// Lock-free predicate invoked before and after lock acquisition.
    predicate: Arc<dyn Fn() -> bool + Send + Sync + 'static>,
    /// Whether public calls convert panics into structured outcomes.
    catch_panics: bool,
}

impl DclCore {
    /// Creates a DCL core with panic capture disabled.
    ///
    /// # Parameters
    ///
    /// * `predicate` - Lock-free condition checked twice.
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
            catch_panics: false,
        }
    }

    /// Returns whether public calls should convert panics into outcomes.
    ///
    /// # Returns
    ///
    /// `true` when panic capture is enabled.
    #[inline(always)]
    pub(crate) fn catch_panics(&self) -> bool {
        self.catch_panics
    }

    /// Reconfigures panic capture while preserving the predicate.
    ///
    /// # Parameters
    ///
    /// * `catch_panics` - Whether public calls should capture panics.
    ///
    /// # Returns
    ///
    /// The reconfigured core.
    #[inline(always)]
    pub(crate) fn with_catch_panics(mut self, catch_panics: bool) -> Self {
        self.catch_panics = catch_panics;
        self
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
    /// # Returns
    ///
    /// The locked-phase result.
    ///
    /// # Panics
    ///
    /// Propagates lock, predicate, and task panics through the concrete lock
    /// implementation.
    pub(crate) fn execute_locked<L, R, E, F>(
        &self,
        lock: &L,
        task: F,
    ) -> LockedExecution<R, E>
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
    /// Shares the erased predicate and copies panic configuration.
    #[inline]
    fn clone(&self) -> Self {
        Self {
            predicate: self.predicate.clone(),
            catch_panics: self.catch_panics,
        }
    }
}
