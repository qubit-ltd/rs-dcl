/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! # Qubit DCL
//!
//! Double-checked locking executor for Qubit Rust libraries.
//!

pub mod double_checked;
pub mod lock;

pub use double_checked::{
    CallbackError, DoubleCheckedLock, DoubleCheckedLockBuilder, DoubleCheckedLockExecutor,
    DoubleCheckedLockReadyBuilder, ExecutionContext, ExecutionLogger, ExecutionResult,
    ExecutorBuilder, ExecutorError, ExecutorLockBuilder, ExecutorReadyBuilder,
};
pub use qubit_lock::ArcMutex;
pub use qubit_lock::lock::Lock;
