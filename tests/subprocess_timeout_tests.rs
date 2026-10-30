// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
#[expect(dead_code, unused_imports)]
mod support;

use std::time::Duration;

use support::run_test_in_subprocess;

#[test]
fn test_subprocess_success() {
    let result = run_test_in_subprocess("test_subprocess_success", Duration::from_secs(30), || {});
    assert!(result.is_ok(), "successful scenario must complete: {result:?}");
}

#[test]
fn test_subprocess_panic_is_reported() {
    let result = run_test_in_subprocess("test_subprocess_panic_is_reported", Duration::from_secs(30), || {
        panic!("expected child failure")
    });
    let error = result.expect_err("child panic must fail");
    assert!(error.contains("test_subprocess_panic_is_reported"));
    assert!(error.contains("exit"));
}

#[test]
fn test_subprocess_timeout_is_reported() {
    let result = run_test_in_subprocess("test_subprocess_timeout_is_reported", Duration::from_secs(2), || {
        loop {
            std::thread::park();
        }
    });
    let error = result.expect_err("blocked child must time out");
    assert!(error.contains("test_subprocess_timeout_is_reported"));
    assert!(error.contains("timed out"));
}

#[test]
fn test_subprocess_zero_exit_is_rejected() {
    let result = run_test_in_subprocess("test_subprocess_zero_exit_is_rejected", Duration::from_secs(30), || {
        std::process::exit(0)
    });
    assert!(result.is_err(), "exit 0 does not prove scenario completion");
}
