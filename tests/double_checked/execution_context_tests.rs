/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::io;

use qubit_dcl::double_checked::{
    DoubleCheckedLockExecutor,
    ExecutionResult,
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
