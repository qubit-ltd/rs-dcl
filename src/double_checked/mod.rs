// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Double-checked locking executors and structured outcomes.

mod captured_finalization_outcome;
mod captured_lifecycle_outcome;
mod dcl_executor;
mod execution_outcome;
mod finalization_outcome;
mod internal;
mod lifecycle_commit_builder;
mod lifecycle_dcl_executor;
mod lifecycle_dcl_executor_builder;
mod lifecycle_outcome;
mod lifecycle_predicate_builder;
mod lifecycle_prepare_builder;
mod lifecycle_ready_builder;
mod lifecycle_rollback_builder;
mod panic_info;
mod panic_phase;
mod rollback_cause;

pub use captured_finalization_outcome::CapturedFinalizationOutcome;
pub use captured_lifecycle_outcome::CapturedLifecycleOutcome;
pub use dcl_executor::DclExecutor;
pub use execution_outcome::ExecutionOutcome;
pub use finalization_outcome::FinalizationOutcome;
#[doc(hidden)]
pub use lifecycle_commit_builder::LifecycleCommitBuilder;
pub use lifecycle_dcl_executor::LifecycleDclExecutor;
pub(crate) use lifecycle_dcl_executor::{
    CommitCallback,
    PrepareCallback,
    RollbackCallback,
};
#[doc(hidden)]
pub use lifecycle_dcl_executor_builder::LifecycleDclExecutorBuilder;
pub use lifecycle_outcome::LifecycleOutcome;
#[doc(hidden)]
pub use lifecycle_predicate_builder::LifecyclePredicateBuilder;
#[doc(hidden)]
pub use lifecycle_prepare_builder::LifecyclePrepareBuilder;
#[doc(hidden)]
pub use lifecycle_ready_builder::LifecycleReadyBuilder;
#[doc(hidden)]
pub use lifecycle_rollback_builder::LifecycleRollbackBuilder;
pub use panic_info::PanicInfo;
pub use panic_phase::PanicPhase;
pub use rollback_cause::RollbackCause;
