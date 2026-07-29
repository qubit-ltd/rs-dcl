// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Behavior tests for lifecycle finalization without callbacks.

use std::{
    io,
    sync::{
        Arc,
        atomic::{
            AtomicUsize,
            Ordering,
        },
    },
};

use qubit_dcl::{
    FinalizationOutcome,
    LifecycleDoubleCheckedLockExecutor,
    LifecycleOutcome,
};

/// Token that records when lifecycle finalization drops it.
struct DropToken {
    /// Shared counter incremented when the token is dropped.
    drops: Arc<AtomicUsize>,
}

impl Drop for DropToken {
    /// Records one finalization drop.
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}

/// Verifies a successful invocation drops its token when no commit callback is
/// configured.
#[test]
fn test_no_commit_drops_token_and_reports_not_required() {
    let drops = Arc::new(AtomicUsize::new(0));
    let executor = LifecycleDoubleCheckedLockExecutor::builder()
        .when(|| true)
        .prepare({
            let drops = Arc::clone(&drops);
            move || {
                Ok::<DropToken, io::Error>(DropToken {
                    drops: Arc::clone(&drops),
                })
            }
        })
        .no_commit()
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();

    let outcome =
        executor.run(&std::sync::Mutex::new(()), || Ok::<(), io::Error>(()));

    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskSucceeded {
            value: (),
            commit: FinalizationOutcome::NotRequired,
        }
    ));
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}
