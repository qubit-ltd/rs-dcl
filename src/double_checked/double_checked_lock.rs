/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! # Double-Checked Lock Convenience API
//!
//! Provides a one-shot convenience wrapper around
//! [`super::DoubleCheckedLockExecutor`].
//!

use super::{
    DoubleCheckedLockBuilder,
    DoubleCheckedLockExecutor,
};
use crate::lock::Lock;

/// Entry type for one-shot double-checked lock execution.
///
/// This API is useful when you do not need to keep a reusable executor
/// instance. It delegates to [`DoubleCheckedLockExecutor`] internally.
///
/// # Examples
///
/// ```rust
/// use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
///
/// use qubit_dcl::{ArcMutex, DoubleCheckedLock, Lock};
///
/// let data = ArcMutex::new(10);
/// let skip = Arc::new(AtomicBool::new(false));
///
/// let result = DoubleCheckedLock::on(data.clone())
///     .when({
///         let skip = skip.clone();
///         move || !skip.load(Ordering::Acquire)
///     })
///     .call_with(|value: &mut i32| {
///         *value += 5;
///         Ok::<i32, std::io::Error>(*value)
///     })
///     .get_result();
///
/// assert!(result.is_success());
/// assert_eq!(data.read(|value| *value), 15);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct DoubleCheckedLock;

impl DoubleCheckedLock {
    /// Starts one-shot double-checked lock configuration by attaching a lock.
    ///
    /// # Parameters
    ///
    /// * `lock` - Lock handle protecting the data used by the one-shot
    ///   execution.
    ///
    /// # Returns
    ///
    /// A convenience builder that can configure the double-checked condition.
    #[inline]
    #[must_use = "assign or chain the returned builder"]
    pub fn on<L, T>(lock: L) -> DoubleCheckedLockBuilder<L, T>
    where
        L: Lock<T>,
    {
        DoubleCheckedLockBuilder {
            inner: DoubleCheckedLockExecutor::builder().on(lock),
        }
    }
}
