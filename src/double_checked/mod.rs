// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Double-checked locking executors and structured outcomes.

mod double_checked_lock_executor;
mod double_checked_lock_executor_builder;
mod double_checked_lock_executor_ready_builder;
mod execution_outcome;
mod execution_report;
mod internal;
mod lifecycle_commit_builder;
mod lifecycle_double_checked_lock_executor;
mod lifecycle_double_checked_lock_executor_builder;
mod lifecycle_predicate_builder;
mod lifecycle_prepare_builder;
mod lifecycle_ready_builder;
mod lifecycle_rollback_builder;
mod panic_info;
mod panic_phase;
mod preparation_outcome;
mod rollback_cause;

pub use double_checked_lock_executor::DoubleCheckedLockExecutor;
#[doc(hidden)]
pub use double_checked_lock_executor_builder::DoubleCheckedLockExecutorBuilder;
#[doc(hidden)]
pub use double_checked_lock_executor_ready_builder::DoubleCheckedLockExecutorReadyBuilder;
pub use execution_outcome::ExecutionOutcome;
pub use execution_report::ExecutionReport;
#[doc(hidden)]
pub use lifecycle_commit_builder::LifecycleCommitBuilder;
pub use lifecycle_double_checked_lock_executor::LifecycleDoubleCheckedLockExecutor;
pub(crate) use lifecycle_double_checked_lock_executor::{
    CommitCallback,
    PrepareCallback,
    RollbackCallback,
};
#[doc(hidden)]
pub use lifecycle_double_checked_lock_executor_builder::LifecycleDoubleCheckedLockExecutorBuilder;
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
pub use preparation_outcome::PreparationOutcome;
pub use rollback_cause::RollbackCause;
