/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! # Qubit DCL
//!
//! Double-checked locking executor for Qubit Rust libraries.
//!
//! # Author
//!
//! Haixing Hu

pub mod double_checked;
pub mod lock;

pub use double_checked::{
    CallbackError, DoubleCheckedLock, DoubleCheckedLockBuilder, DoubleCheckedLockExecutor,
    DoubleCheckedLockReadyBuilder, ExecutionContext, ExecutionLogger, ExecutionResult,
    ExecutorBuilder, ExecutorError, ExecutorLockBuilder, ExecutorReadyBuilder,
};
pub use qubit_lock::ArcMutex;
pub use qubit_lock::lock::Lock;
