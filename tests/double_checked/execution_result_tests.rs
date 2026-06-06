// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_dcl::double_checked::{
    ExecutionResult,
    ExecutorError,
};

/// Test execution result constructors and conversion helpers.
#[test]
fn test_execution_result_constructors_and_into_result() {
    let success = ExecutionResult::<i32, String>::success(42);
    assert!(success.is_success());
    assert_eq!(
        success
            .into_result()
            .expect("success should convert to Ok(Some(value))"),
        Some(42),
    );

    let unmet = ExecutionResult::<i32, String>::unmet();
    assert!(unmet.is_unmet());
    assert_eq!(
        unmet
            .into_result()
            .expect("unmet should convert to Ok(None)"),
        None,
    );

    let failed =
        ExecutionResult::<i32, String>::task_failed("failed".to_string());
    assert!(failed.is_failed());
    assert!(matches!(
        failed.into_result(),
        Err(ExecutorError::TaskFailed(message)) if message == "failed"
    ));
}
