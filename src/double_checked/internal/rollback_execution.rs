// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private unsuccessful execution state used to drive lifecycle rollback.

use std::error::Error;

use crate::double_checked::{
    FinalizationOutcome,
    LifecycleOutcome,
    RollbackCause,
};

/// Represents the locked execution state that requires lifecycle rollback.
///
/// # Type Parameters
///
/// * `E` - Task error type.
pub(crate) enum RollbackExecution<E> {
    /// The second condition check returned `false`.
    ConditionNotMet,
    /// The task returned its original error.
    TaskFailed(E),
}

impl<E> RollbackExecution<E>
where
    E: Error + Send + Sync + 'static,
{
    /// Creates the borrowed cause passed to the rollback callback.
    ///
    /// # Returns
    ///
    /// A cause borrowing any task error or panic metadata from this state.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Task error type stored in the unsuccessful execution state.
    #[inline]
    pub(crate) fn cause(&self) -> RollbackCause<'_> {
        match self {
            Self::ConditionNotMet => RollbackCause::ConditionNotMet,
            Self::TaskFailed(error) => RollbackCause::TaskFailed(error),
        }
    }

    /// Combines this unsuccessful execution state with its rollback outcome.
    ///
    /// # Parameters
    ///
    /// * `rollback` - Finalization outcome produced after the executor lock was
    ///   released.
    ///
    /// # Returns
    ///
    /// The terminal lifecycle outcome preserving the original execution state.
    ///
    /// # Type Parameters
    ///
    /// * `R` - Successful task result type.
    /// * `C` - Lifecycle callback error type.
    #[inline]
    pub(crate) fn into_outcome<R, C>(
        self,
        rollback: FinalizationOutcome<C>,
    ) -> LifecycleOutcome<R, E, C> {
        match self {
            Self::ConditionNotMet => {
                LifecycleOutcome::SecondConditionNotMet { rollback }
            }
            Self::TaskFailed(error) => {
                LifecycleOutcome::TaskFailed { error, rollback }
            }
        }
    }
}
