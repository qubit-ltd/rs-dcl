// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared panic capture helpers.

use std::panic::{
    AssertUnwindSafe,
    catch_unwind,
};

use crate::double_checked::{
    PanicInfo,
    PanicPhase,
};

/// Executes an operation behind a panic boundary with a fixed phase.
///
/// # Parameters
///
/// * `phase` - Phase assigned if `operation` unwinds.
/// * `operation` - Operation to execute.
///
/// # Type Parameters
///
/// * `R` - Operation result type.
/// * `F` - Operation callback type.
///
/// # Returns
///
/// `Ok(R)` when the operation returns, or `Err(PanicInfo)` retaining the
/// original panic payload.
///
/// # Errors
///
/// Returns [`Err`] when `operation` panics.
#[inline]
pub(crate) fn catch_phase<R, F>(
    phase: PanicPhase,
    operation: F,
) -> Result<R, PanicInfo>
where
    F: FnOnce() -> R,
{
    catch_unwind(AssertUnwindSafe(operation))
        .map_err(|payload| PanicInfo::from_payload(phase, payload))
}
