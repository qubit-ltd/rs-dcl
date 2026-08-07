// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Loom models for DCL gate and lock interleavings.

use std::io;
use std::sync::TryLockError as StdTryLockError;

use loom::model;
use loom::sync::Arc;
use loom::sync::Mutex;
use loom::sync::MutexGuard;
use loom::sync::atomic::AtomicBool;
use loom::sync::atomic::AtomicUsize;
use loom::sync::atomic::Ordering;
use loom::thread;
use qubit_dcl::DclExecutor;
use qubit_dcl::ExecutionOutcome;
use qubit_dcl::LifecycleDclExecutor;
use qubit_lock::Lock;
use qubit_lock::TryLockError;

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

impl<T> Lock for LoomLock<T>
where
    T: Send,
{
    type Guard<'a>
        = MutexGuard<'a, T>
    where
        Self: 'a;

    /// Records and acquires the modeled mutex.
    fn lock(&self) -> Self::Guard<'_> {
        self.record_call();
        self.inner.lock().expect("loom lock should not be poisoned")
    }

    /// Records and attempts immediate modeled acquisition.
    fn try_lock(&self) -> Result<Self::Guard<'_>, TryLockError> {
        self.record_call();
        match self.inner.try_lock() {
            Ok(guard) => Ok(guard),
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
        let executor = DclExecutor::new(|| false);

        let outcome = executor.run(&lock, || Ok::<(), io::Error>(()));

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
        let lock = LoomLock::new(());
        let executor = Arc::new(DclExecutor::new({
            let gate = Arc::clone(&gate);
            move || gate.load(Ordering::Acquire)
        }));

        let handles = (0..2)
            .map(|_| {
                let executor = Arc::clone(&executor);
                let gate = Arc::clone(&gate);
                let task_calls = Arc::clone(&task_calls);
                let lock = lock.clone();
                thread::spawn(move || {
                    matches!(
                        executor.run(&lock, || {
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
        let executor = DclExecutor::new({
            let gate = Arc::clone(&gate);
            let checks = Arc::clone(&checks);
            move || {
                checks.fetch_add(1, Ordering::Relaxed);
                gate.load(Ordering::Acquire)
            }
        });

        let guard = Lock::lock(&lock);
        let worker_task_calls = Arc::clone(&task_calls);
        let task_lock = lock.clone();
        let handle = thread::spawn(move || {
            executor.run(&task_lock, || {
                worker_task_calls.fetch_add(1, Ordering::Relaxed);
                Ok::<(), io::Error>(())
            })
        });
        while checks.load(Ordering::Acquire) == 0 {
            thread::yield_now();
        }
        gate.store(false, Ordering::Release);
        drop(guard);

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
        let lock = LoomLock::new(());
        let executor = Arc::new(
            LifecycleDclExecutor::builder()
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
        let first_lock = lock.clone();
        let first = thread::spawn(move || {
            first_executor.run_with_token(&first_lock, |token| {
                *token = 0;
                Ok::<(), io::Error>(())
            })
        });
        let second_executor = Arc::clone(&executor);
        let second_lock = lock.clone();
        let second = thread::spawn(move || {
            second_executor.run_with_token(&second_lock, |token| {
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
