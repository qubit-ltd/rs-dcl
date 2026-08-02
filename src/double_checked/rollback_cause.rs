// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Structured causes supplied to lifecycle rollback callbacks.

use std::error::Error;

use crate::double_checked::PanicInfo;

/// Explains why a prepared invocation requires rollback.
///
/// Error and panic values are borrowed only for the duration of the rollback
/// callback so the execution report can retain their original owned values.
///
/// The `'a` lifetime is the duration for which the rollback callback may
/// inspect the borrowed error or panic metadata.
#[derive(Debug)]
pub enum RollbackCause<'a> {
    /// The second condition check failed after prepare completed.
    ConditionNotMet,
    /// The guarded task returned an error.
    TaskFailed(
        /// Borrowed view of the original task error.
        &'a (dyn Error + Send + Sync + 'static),
    ),
    /// Lock acquisition, the second check, the task, or lock release panicked.
    Panicked(
        /// Borrowed metadata for the original captured panic.
        &'a PanicInfo,
    ),
}
