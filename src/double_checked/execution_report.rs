// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Orthogonal task and lifecycle reporting.

use crate::double_checked::{
    ExecutionOutcome,
    PreparationOutcome,
};

/// Retains both the guarded task outcome and its lifecycle finalization
/// outcome.
#[derive(Debug)]
#[must_use = "both execution and preparation outcomes must be inspected"]
pub struct ExecutionReport<R, E, C> {
    /// Outcome of the condition checks and guarded task.
    execution: ExecutionOutcome<R, E>,
    /// Outcome of prepare and its selected finalizer.
    preparation: PreparationOutcome<C>,
}

impl<R, E, C> ExecutionReport<R, E, C> {
    /// Creates a report without collapsing either outcome axis.
    ///
    /// # Parameters
    ///
    /// * `execution` - Condition and task outcome.
    /// * `preparation` - Prepare and finalization outcome.
    ///
    /// # Returns
    ///
    /// A report retaining both supplied values.
    #[inline]
    pub(crate) fn new(
        execution: ExecutionOutcome<R, E>,
        preparation: PreparationOutcome<C>,
    ) -> Self {
        Self {
            execution,
            preparation,
        }
    }

    /// Returns the condition and task outcome.
    ///
    /// # Returns
    ///
    /// A shared reference to the execution axis.
    #[inline(always)]
    pub fn execution(&self) -> &ExecutionOutcome<R, E> {
        &self.execution
    }

    /// Returns the prepare and finalization outcome.
    ///
    /// # Returns
    ///
    /// A shared reference to the preparation axis.
    #[inline(always)]
    pub fn preparation(&self) -> &PreparationOutcome<C> {
        &self.preparation
    }

    /// Consumes the report and returns both outcome axes.
    ///
    /// # Returns
    ///
    /// A tuple containing the execution outcome followed by the preparation
    /// outcome.
    #[inline(always)]
    pub fn into_parts(self) -> (ExecutionOutcome<R, E>, PreparationOutcome<C>) {
        (self.execution, self.preparation)
    }
}
