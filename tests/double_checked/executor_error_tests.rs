/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
use std::{
    error::Error,
    io,
};

use qubit_dcl::double_checked::{
    CallbackError,
    ExecutorError,
};

/// Test executor error display, source, and callback type accessors.
#[test]
fn test_executor_error_display_source_and_callback_type() {
    let task_error = ExecutorError::<io::Error>::TaskFailed(io::Error::other("task failed"));
    assert_eq!(task_error.to_string(), "Task execution failed: task failed");
    assert_eq!(
        task_error
            .source()
            .expect("task failure should expose source")
            .to_string(),
        "task failed",
    );
    assert_eq!(task_error.callback_type(), None);

    let prepare_error: ExecutorError<io::Error> =
        ExecutorError::PrepareFailed(CallbackError::with_type("prepare", "prepare failed"));
    assert_eq!(prepare_error.callback_type(), Some("prepare"));
    assert_eq!(
        prepare_error.to_string(),
        "Preparation action failed: prepare: prepare failed",
    );
}
