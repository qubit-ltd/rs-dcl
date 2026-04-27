/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! # Double-Checked Lock Executor
//!
//! Provides a double-checked lock executor for executing tasks with condition
//! checking and prepare lifecycle support.
//!
//! # Author
//!
//! Haixing Hu

mod double_checked_lock;
mod double_checked_lock_executor;
mod execution_context;
mod execution_logger;
mod execution_result;
mod executor_builder;
mod executor_error;
mod executor_lock_builder;
mod executor_ready_builder;

pub use double_checked_lock::{
    DoubleCheckedLock, DoubleCheckedLockBuilder, DoubleCheckedLockReadyBuilder,
};
pub use double_checked_lock_executor::DoubleCheckedLockExecutor;
pub use execution_context::ExecutionContext;
pub use execution_logger::ExecutionLogger;
pub use execution_result::ExecutionResult;
pub use executor_builder::ExecutorBuilder;
pub use executor_error::{CallbackError, ExecutorError};
pub use executor_lock_builder::ExecutorLockBuilder;
pub use executor_ready_builder::ExecutorReadyBuilder;
