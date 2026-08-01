// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the lifecycle predicate builder stage.

use std::io;

use qubit_dcl::{
    LifecycleDclExecutor,
    LifecycleOutcome,
};

/// Verifies the predicate stage accepts panic configuration and preparation.
#[test]
fn test_predicate_builder_configures_prepare() {
    let executor = LifecycleDclExecutor::builder()
        .when(|| true)
        
        .prepare(|| Err::<(), _>(io::Error::other("prepare")))
        .no_commit()
        .rollback(|_, _| Ok::<(), io::Error>(()))
        .build();
    let outcome =
        executor.run(&std::sync::Mutex::new(()), || Ok::<(), io::Error>(()));

    assert!(matches!(outcome, LifecycleOutcome::PrepareFailed(_)));
}
