// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::io;

use qubit_dcl::double_checked::{
    DoubleCheckedLockExecutor,
    ExecutionResult,
    ExecutorError,
};
use qubit_lock::ArcMutex;

/// Test result inspection and consumption through the execution context.
#[test]
fn test_execution_context_peek_get_and_finish() {
    let executor = DoubleCheckedLockExecutor::builder()
        .on(ArcMutex::new(0))
        .when(|| true)
        .build();

    let context = executor.call(|| Ok::<i32, io::Error>(7));
    assert!(context.is_success());
    assert!(matches!(context.peek_result(), ExecutionResult::Success(7)));
    assert!(matches!(context.get_result(), ExecutionResult::Success(7)));

    assert!(executor.execute(|| Ok::<(), io::Error>(())).finish(),);
}

/// Test fallible completion without losing task failure details.
#[test]
fn test_execution_context_try_finish_preserves_failure() {
    let executor = DoubleCheckedLockExecutor::builder()
        .on(ArcMutex::new(0))
        .when(|| true)
        .build();

    let success = executor.execute(|| Ok::<(), io::Error>(())).try_finish();
    assert!(matches!(success, Ok(true)));

    let unmet = DoubleCheckedLockExecutor::builder()
        .on(ArcMutex::new(0))
        .when(|| false)
        .build()
        .execute(|| Ok::<(), io::Error>(()))
        .try_finish();
    assert!(matches!(unmet, Ok(false)));

    let failed = executor
        .execute(|| Err::<(), io::Error>(io::Error::other("task failed")))
        .try_finish();
    assert!(matches!(
        failed,
        Err(ExecutorError::TaskFailed(error)) if error.to_string() == "task failed"
    ));
}
