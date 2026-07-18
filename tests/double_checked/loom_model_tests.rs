// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Loom models for DCL gate and lock interleavings.

use std::{
    io,
    sync::TryLockError as StdTryLockError,
};

use loom::{
    model,
    sync::{
        Arc,
        Mutex,
        atomic::{
            AtomicBool,
            AtomicUsize,
            Ordering,
        },
    },
    thread,
};
use qubit_dcl::{
    DoubleCheckedLockExecutor,
    ExecutionOutcome,
    LifecycleDoubleCheckedLockExecutor,
};
use qubit_lock::{
    Lock,
    TryLockError,
};

/// Loom-aware lock implementation used only by model tests.
#[derive(Clone)]
struct LoomLock<T> {
    /// Shared modeled mutex.
    inner: Arc<Mutex<T>>,
    /// Number of calls through the `Lock` API.
    calls: Arc<AtomicUsize>,
}

impl<T> LoomLock<T> {
    /// Creates a modeled lock protecting `value`.
    ///
    /// # Parameters
    ///
    /// * `value` - Initial protected value.
    ///
    /// # Returns
    ///
    /// A modeled lock with zero recorded calls.
    fn new(value: T) -> Self {
        Self {
            inner: Arc::new(Mutex::new(value)),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Returns the modeled lock call count.
    ///
    /// # Returns
    ///
    /// The number of calls made through any clone.
    fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }

    /// Records one modeled lock call.
    fn record_call(&self) {
        self.calls.fetch_add(1, Ordering::Relaxed);
    }
}

impl<T> Lock<T> for LoomLock<T> {
    /// Records and executes a modeled read operation.
    fn with_read<R, F>(&self, operation: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        self.record_call();
        let guard =
            self.inner.lock().expect("loom lock should not be poisoned");
        operation(&guard)
    }

    /// Records and executes a modeled write operation.
    fn with_write<R, F>(&self, operation: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        self.record_call();
        let mut guard =
            self.inner.lock().expect("loom lock should not be poisoned");
        operation(&mut guard)
    }

    /// Records and attempts a modeled read operation without blocking.
    fn try_with_read<R, F>(&self, operation: F) -> Result<R, TryLockError>
    where
        F: FnOnce(&T) -> R,
    {
        self.record_call();
        match self.inner.try_lock() {
            Ok(guard) => Ok(operation(&guard)),
            Err(StdTryLockError::WouldBlock) => Err(TryLockError::WouldBlock),
            Err(StdTryLockError::Poisoned(_)) => Err(TryLockError::Poisoned),
        }
    }

    /// Records and attempts a modeled write operation without blocking.
    fn try_with_write<R, F>(&self, operation: F) -> Result<R, TryLockError>
    where
        F: FnOnce(&mut T) -> R,
    {
        self.record_call();
        match self.inner.try_lock() {
            Ok(mut guard) => Ok(operation(&mut guard)),
            Err(StdTryLockError::WouldBlock) => Err(TryLockError::WouldBlock),
            Err(StdTryLockError::Poisoned(_)) => Err(TryLockError::Poisoned),
        }
    }
}

/// Verifies every explored outer-false path avoids the lock completely.
#[test]
fn test_loom_initial_false_has_zero_lock_calls() {
    model(|| {
        let lock = LoomLock::new(());
        let executor = DoubleCheckedLockExecutor::builder(lock.clone())
            .when(|| false)
            .build();

        let outcome = executor.run(|| Ok::<(), io::Error>(()));

        assert!(matches!(outcome, ExecutionOutcome::ConditionNotMet));
        assert_eq!(lock.calls(), 0);
    });
}

/// Verifies two competing calls cannot both execute after one task closes the
/// gate inside the executor lock.
#[test]
fn test_loom_task_gate_change_allows_one_success() {
    model(|| {
        let gate = Arc::new(AtomicBool::new(true));
        let task_calls = Arc::new(AtomicUsize::new(0));
        let executor = Arc::new(
            DoubleCheckedLockExecutor::builder(LoomLock::new(()))
                .when({
                    let gate = Arc::clone(&gate);
                    move || gate.load(Ordering::Acquire)
                })
                .build(),
        );

        let handles = (0..2)
            .map(|_| {
                let executor = Arc::clone(&executor);
                let gate = Arc::clone(&gate);
                let task_calls = Arc::clone(&task_calls);
                thread::spawn(move || {
                    matches!(
                        executor.run(|| {
                            task_calls.fetch_add(1, Ordering::Relaxed);
                            gate.store(false, Ordering::Release);
                            Ok::<(), io::Error>(())
                        }),
                        ExecutionOutcome::Success(())
                    )
                })
            })
            .collect::<Vec<_>>();

        let success_count = handles
            .into_iter()
            .map(|handle| handle.join().expect("loom worker should not panic"))
            .filter(|succeeded| *succeeded)
            .count();
        assert_eq!(success_count, 1);
        assert_eq!(task_calls.load(Ordering::Relaxed), 1);
    });
}

/// Verifies a same-lock external gate transition is observed by the waiting
/// invocation's second check.
#[test]
fn test_loom_external_same_lock_transition_blocks_stale_task() {
    model(|| {
        let lock = LoomLock::new(());
        let gate = Arc::new(AtomicBool::new(true));
        let checks = Arc::new(AtomicUsize::new(0));
        let task_calls = Arc::new(AtomicUsize::new(0));
        let executor = DoubleCheckedLockExecutor::builder(lock.clone())
            .when({
                let gate = Arc::clone(&gate);
                let checks = Arc::clone(&checks);
                move || {
                    checks.fetch_add(1, Ordering::Relaxed);
                    gate.load(Ordering::Acquire)
                }
            })
            .build();

        let handle = lock.with_write(|_| {
            let task_calls = Arc::clone(&task_calls);
            let handle = thread::spawn(move || {
                executor.run(|| {
                    task_calls.fetch_add(1, Ordering::Relaxed);
                    Ok::<(), io::Error>(())
                })
            });
            while checks.load(Ordering::Acquire) == 0 {
                thread::yield_now();
            }
            gate.store(false, Ordering::Release);
            handle
        });

        let outcome = handle.join().expect("loom worker should not panic");
        assert!(matches!(outcome, ExecutionOutcome::ConditionNotMet));
        assert_eq!(task_calls.load(Ordering::Relaxed), 0);
    });
}

/// Verifies concurrent prepared calls retain distinct task-updated tokens
/// through their commit paths.
#[test]
fn test_loom_prepare_tokens_do_not_cross_invocations() {
    model(|| {
        let committed_mask = Arc::new(AtomicUsize::new(0));
        let executor = Arc::new(
            LifecycleDoubleCheckedLockExecutor::builder(LoomLock::new(()))
                .when(|| true)
                .prepare(|| Ok::<usize, io::Error>(usize::MAX))
                .commit({
                    let committed_mask = Arc::clone(&committed_mask);
                    move |token| {
                        committed_mask.fetch_or(1 << token, Ordering::Relaxed);
                        Ok::<(), io::Error>(())
                    }
                })
                .no_rollback()
                .build(),
        );

        let first_executor = Arc::clone(&executor);
        let first = thread::spawn(move || {
            first_executor.run_with_token(|token| {
                *token = 0;
                Ok::<(), io::Error>(())
            })
        });
        let second_executor = Arc::clone(&executor);
        let second = thread::spawn(move || {
            second_executor.run_with_token(|token| {
                *token = 1;
                Ok::<(), io::Error>(())
            })
        });

        let _first_report =
            first.join().expect("first loom worker should not panic");
        let _second_report =
            second.join().expect("second loom worker should not panic");
        let committed = committed_mask.load(Ordering::Relaxed);
        assert_eq!(committed, 0b11);
    });
}
