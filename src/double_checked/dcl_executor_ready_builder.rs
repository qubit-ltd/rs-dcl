// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Ready builder stage for the basic DCL executor.

use crate::double_checked::{
    DclExecutor,
    internal::DclCore,
};

/// Builder stage containing every required basic-executor component.
#[doc(hidden)]
#[must_use = "the ready builder must be consumed by build"]
pub struct DclExecutorReadyBuilder {
    /// Shared DCL predicate.
    core: DclCore,
}

impl DclExecutorReadyBuilder {
    /// Creates a ready builder from a configured DCL core.
    ///
    /// # Parameters
    ///
    /// * `core` - Lock-free predicate configuration.
    ///
    /// # Returns
    ///
    /// A builder that can be built into an executor.
    #[inline]
    pub(crate) fn new(core: DclCore) -> Self {
        Self { core }
    }

    /// Builds a reusable basic DCL executor.
    ///
    /// # Returns
    ///
    /// An executor sharing the configured predicate across cloned handles.
    #[inline]
    pub fn build(self) -> DclExecutor {
        DclExecutor::from_core(self.core)
    }
}
