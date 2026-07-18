// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Ready builder stage for the basic DCL executor.

use crate::double_checked::{
    DoubleCheckedLockExecutor,
    internal::DclCore,
};

/// Builder stage containing every required basic-executor component.
#[doc(hidden)]
#[must_use = "the ready builder must be consumed by build"]
pub struct DoubleCheckedLockExecutorReadyBuilder {
    /// Shared DCL configuration being built.
    core: DclCore,
}

impl DoubleCheckedLockExecutorReadyBuilder {
    /// Creates a ready builder from a configured DCL core.
    ///
    /// # Parameters
    ///
    /// * `core` - Lock and predicate configuration.
    ///
    /// # Returns
    ///
    /// A builder that can be configured further or built.
    #[inline]
    pub(crate) fn new(core: DclCore) -> Self {
        Self { core }
    }

    /// Configures whether executor calls convert panics into outcomes.
    ///
    /// # Parameters
    ///
    /// * `catch_panics` - `true` to capture panics, or `false` to propagate
    ///   them through the caller.
    ///
    /// # Returns
    ///
    /// The reconfigured ready builder.
    #[inline(always)]
    pub fn catch_panics(mut self, catch_panics: bool) -> Self {
        self.core = self.core.with_catch_panics(catch_panics);
        self
    }

    /// Builds a reusable basic DCL executor.
    ///
    /// # Returns
    ///
    /// An executor sharing the configured predicate across cloned handles.
    #[inline]
    pub fn build(self) -> DoubleCheckedLockExecutor {
        DoubleCheckedLockExecutor::from_core(self.core)
    }
}
