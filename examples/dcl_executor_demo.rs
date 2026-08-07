// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Demonstrates basic and lifecycle-aware double-checked execution.

use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use qubit_dcl::DclExecutor;
use qubit_dcl::ExecutionOutcome;
use qubit_dcl::FinalizationOutcome;
use qubit_dcl::LifecycleDclExecutor;
use qubit_dcl::LifecycleOutcome;

/// Runs basic and lifecycle DCL examples using atomic gates.
fn main() {
    let gate = Arc::new(AtomicBool::new(true));
    let lock = std::sync::Mutex::new(());
    let executor = DclExecutor::new({
        let gate = Arc::clone(&gate);
        move || gate.load(Ordering::Acquire)
    });
    let outcome = executor.run(&lock, {
        let gate = Arc::clone(&gate);
        move || {
            gate.store(false, Ordering::Release);
            Ok::<usize, io::Error>(42)
        }
    });
    assert!(matches!(outcome, ExecutionOutcome::Success(42)));

    let lifecycle_lock = std::sync::Mutex::new(());
    let lifecycle_executor = LifecycleDclExecutor::builder()
        .when(|| true)
        .prepare(|| Ok::<Vec<&'static str>, io::Error>(vec!["prepare"]))
        .commit(|token| {
            assert_eq!(token, ["prepare", "task"]);
            Ok::<(), io::Error>(())
        })
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();
    let outcome = lifecycle_executor.run_with_token(&lifecycle_lock, |token| {
        token.push("task");
        Ok::<usize, io::Error>(token.len())
    });
    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskSucceeded {
            value: 2,
            commit: FinalizationOutcome::Succeeded,
        }
    ));
}
