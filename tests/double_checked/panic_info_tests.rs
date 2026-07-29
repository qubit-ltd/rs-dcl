// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for captured panic metadata.

use std::{
    io,
    panic::panic_any,
    sync::Mutex,
};

use qubit_dcl::{
    DoubleCheckedLockExecutor,
    ExecutionOutcome,
    PanicPhase,
};

/// Verifies a string payload remains inspectable and recoverable by value.
#[test]
fn test_panic_info_preserves_string_payload() {
    let executor = DoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(true)
        .build();
    let outcome: ExecutionOutcome<(), io::Error> = executor
        .run(&Mutex::new(()), || {
            panic_any(String::from("owned panic"))
        });

    let ExecutionOutcome::Panicked(panic) = outcome else {
        panic!("expected captured task panic");
    };
    assert_eq!(panic.phase(), PanicPhase::Task);
    assert_eq!(panic.message(), Some("owned panic"));
    assert_eq!(
        panic.payload().downcast_ref::<String>().map(String::as_str),
        Some("owned panic")
    );

    let payload = panic.into_payload();
    let message = payload
        .downcast::<String>()
        .expect("payload should remain an owned String");
    assert_eq!(*message, "owned panic");
}

/// Verifies unknown payloads are retained without fabricating a message.
#[test]
fn test_panic_info_preserves_non_string_payload_without_message() {
    let executor = DoubleCheckedLockExecutor::builder()
        .when(|| true)
        .catch_panics(true)
        .build();
    let outcome: ExecutionOutcome<(), io::Error> =
        executor.run(&Mutex::new(()), || panic_any(123_u32));

    let ExecutionOutcome::Panicked(panic) = outcome else {
        panic!("expected captured task panic");
    };
    assert_eq!(panic.message(), None);
    assert_eq!(panic.payload().downcast_ref::<u32>(), Some(&123));
    assert!(format!("{panic:?}").contains("PanicInfo"));
}
