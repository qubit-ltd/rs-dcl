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
    marker::PhantomData,
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

/// Thread-safe erased predicate shared by executor clones.
pub(crate) type Predicate = dyn Fn() -> bool + Send + Sync + 'static;

/// Owns the lock, erased predicate, and panic-capture configuration shared by
/// both public executors.
pub(crate) struct DclCore<L, T: ?Sized> {
    /// Lock whose write path encloses the second check and task.
    lock: L,
    /// Lock-free predicate invoked before and after lock acquisition.
    predicate: Arc<Predicate>,
    /// Whether public calls convert panics into structured outcomes.
    catch_panics: bool,
    /// Associates the lock's protected type without storing or exposing it.
    marker: PhantomData<fn(&T)>,
}

impl<L, T: ?Sized> DclCore<L, T> {
    /// Creates a DCL core with panic capture disabled.
    ///
    /// # Parameters
    ///
    /// * `lock` - Lock used for the protected phase.
    /// * `predicate` - Lock-free condition checked twice.
    ///
    /// # Returns
    ///
    /// A core ready to be configured or built into an executor.
    #[inline]
    pub(crate) fn new(lock: L, predicate: Arc<Predicate>) -> Self {
        Self {
            lock,
            predicate,
            catch_panics: false,
            marker: PhantomData,
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

    /// Reconfigures panic capture while preserving the lock and predicate.
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
}

impl<L, T: ?Sized> DclCore<L, T>
where
    L: Lock<T>,
{
    /// Acquires the write lock, checks the condition again, and optionally runs
    /// the task without catching panics.
    ///
    /// # Parameters
    ///
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
    pub(crate) fn execute_locked<R, E, F>(
        &self,
        task: F,
    ) -> LockedExecution<R, E>
    where
        F: FnOnce() -> Result<R, E>,
    {
        self.lock.with_write(|_| {
            if !(self.predicate)() {
                LockedExecution::ConditionNotMet
            } else {
                LockedExecution::Task(task())
            }
        })
    }

    /// Executes the complete locked phase behind one outer panic boundary.
    ///
    /// Keeping the boundary outside `with_write` ensures an unwind from the
    /// task crosses the concrete lock guard before it is converted into
    /// metadata, preserving standard-lock poisoning behavior.
    ///
    /// # Parameters
    ///
    /// * `task` - Task to run only when the second check succeeds.
    ///
    /// # Returns
    ///
    /// The locked result or panic metadata with the most precise active phase.
    pub(crate) fn execute_locked_catching<R, E, F>(
        &self,
        task: F,
    ) -> Result<LockedExecution<R, E>, PanicInfo>
    where
        F: FnOnce() -> Result<R, E>,
    {
        let phase = Cell::new(PanicPhase::LockAcquisition);
        catch_unwind(AssertUnwindSafe(|| {
            self.lock.with_write(|_| {
                phase.set(PanicPhase::SecondConditionCheck);
                if !(self.predicate)() {
                    LockedExecution::ConditionNotMet
                } else {
                    phase.set(PanicPhase::Task);
                    LockedExecution::Task(task())
                }
            })
        }))
        .map_err(|payload| PanicInfo::from_payload(phase.get(), payload))
    }
}

impl<L, T: ?Sized> Clone for DclCore<L, T>
where
    L: Clone,
{
    /// Clones the lock handle and shares the erased predicate.
    #[inline]
    fn clone(&self) -> Self {
        Self {
            lock: self.lock.clone(),
            predicate: Arc::clone(&self.predicate),
            catch_panics: self.catch_panics,
            marker: PhantomData,
        }
    }
}
