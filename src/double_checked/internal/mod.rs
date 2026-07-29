// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private execution machinery shared by public DCL executors.

mod dcl_core;
mod lifecycle_finalization;
mod locked_execution;
mod panic_capture;
mod rollback_execution;

pub(crate) use dcl_core::DclCore;
pub(crate) use lifecycle_finalization::{
    finalize_commit,
    finalize_rollback,
};
pub(crate) use locked_execution::LockedExecution;
pub(crate) use panic_capture::catch_phase;
pub(crate) use rollback_execution::RollbackExecution;
