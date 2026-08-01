// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Task execution outcomes for double-checked execution.

/// Describes whether and how the guarded task was executed.
#[derive(Debug)]
#[must_use = "the execution outcome must be inspected"]
pub enum ExecutionOutcome<R, E> {
    /// The task ran while holding the executor lock and returned `R`.
    Success(R),
    /// A lock-free or lock-protected condition check returned `false`.
    ConditionNotMet,
    /// The task ran and returned its original error value.
    TaskFailed(E),
}
