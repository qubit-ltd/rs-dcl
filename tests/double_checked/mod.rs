// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
#[cfg(feature = "parking-lot")]
mod builder_typestate_tests;
#[cfg(feature = "parking-lot")]
mod concurrency_tests;
#[cfg(feature = "parking-lot")]
mod dcl_core_tests;
#[cfg(feature = "parking-lot")]
mod double_checked_lock_executor_builder_tests;
#[cfg(feature = "parking-lot")]
mod double_checked_lock_executor_ready_builder_tests;
#[cfg(feature = "parking-lot")]
mod double_checked_lock_executor_tests;
mod execution_outcome_tests;
mod finalization_outcome_tests;
mod internal;
#[cfg(feature = "parking-lot")]
mod lifecycle_commit_builder_tests;
#[cfg(feature = "parking-lot")]
mod lifecycle_double_checked_lock_executor_builder_tests;
#[cfg(feature = "parking-lot")]
mod lifecycle_double_checked_lock_executor_tests;
mod lifecycle_outcome_tests;
#[cfg(feature = "parking-lot")]
mod lifecycle_predicate_builder_tests;
#[cfg(feature = "parking-lot")]
mod lifecycle_prepare_builder_tests;
#[cfg(feature = "parking-lot")]
mod lifecycle_ready_builder_tests;
#[cfg(feature = "parking-lot")]
mod lifecycle_rollback_builder_tests;
#[cfg(feature = "parking-lot")]
mod locked_execution_tests;
#[cfg(feature = "parking-lot")]
mod loom_model_tests;
mod panic_capture_tests;
mod panic_info_tests;
mod panic_phase_tests;
mod rollback_cause_tests;
