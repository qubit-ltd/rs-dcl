/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
// qubit-style: allow explicit-imports
#[cfg(test)]
mod tests {
    use qubit_dcl::double_checked::{
        ExecutionResult,
        ExecutorError,
    };

    mod test_execution_result {
        use super::*;

        #[test]
        fn test_execution_result_success() {
            let result = ExecutionResult::<i32, String>::Success(42);

            assert!(result.is_success());
            assert!(!result.is_unmet());
            assert!(!result.is_failed());
        }

        #[test]
        fn test_execution_result_condition_not_met() {
            let result = ExecutionResult::<i32, String>::ConditionNotMet;

            assert!(!result.is_success());
            assert!(result.is_unmet());
            assert!(!result.is_failed());
        }

        #[test]
        fn test_execution_result_failed() {
            let error = ExecutorError::<String>::TaskFailed("Test error".to_string());
            let result = ExecutionResult::<i32, String>::Failed(error);

            assert!(!result.is_success());
            assert!(!result.is_unmet());
            assert!(result.is_failed());
        }

        #[test]
        fn test_execution_result_unwrap_success() {
            let result = ExecutionResult::<i32, String>::Success(100);
            assert_eq!(result.unwrap(), 100);
        }

        #[test]
        #[should_panic(expected = "Called unwrap on ExecutionResult::ConditionNotMet")]
        fn test_execution_result_unwrap_condition_not_met_panics() {
            let result = ExecutionResult::<i32, String>::ConditionNotMet;
            result.unwrap();
        }

        #[test]
        #[should_panic(expected = "Called unwrap on ExecutionResult::Failed")]
        fn test_execution_result_unwrap_failed_panics() {
            let error = ExecutorError::<String>::TaskFailed("Test error".to_string());
            let result = ExecutionResult::<i32, String>::Failed(error);
            result.unwrap();
        }

        #[test]
        fn test_execution_result_into_result_success() {
            let result = ExecutionResult::<i32, String>::Success(42);
            let converted = result.into_result();

            assert!(converted.is_ok());
            assert_eq!(
                converted.expect("success should convert to Ok(Some(value))"),
                Some(42),
            );
        }

        #[test]
        fn test_execution_result_into_result_condition_not_met() {
            let result = ExecutionResult::<i32, String>::ConditionNotMet;
            let converted = result.into_result();

            assert!(converted.is_ok());
            assert_eq!(converted.expect("unmet should convert to Ok(None)"), None,);
        }

        #[test]
        fn test_execution_result_into_result_failed() {
            let error = ExecutorError::<String>::TaskFailed("Test error".to_string());
            let result = ExecutionResult::<i32, String>::Failed(error);
            let converted = result.into_result();

            assert!(matches!(
                converted,
                Err(ExecutorError::TaskFailed(message)) if message == "Test error"
            ));
        }

        #[test]
        fn test_execution_result_failure_constructors() {
            let error = ExecutorError::TaskFailed("task failed".to_string());
            let task_result = ExecutionResult::<(), String>::from_executor_error(error);
            assert!(matches!(
                task_result,
                ExecutionResult::Failed(ExecutorError::TaskFailed(message))
                    if message == "task failed"
            ));

            let commit_result =
                ExecutionResult::<(), String>::prepare_commit_failed("commit failed");
            assert!(matches!(
                commit_result,
                ExecutionResult::Failed(ExecutorError::PrepareCommitFailed(callback))
                    if callback.message() == "commit failed"
            ));
        }

        #[test]
        fn test_execution_result_prepare_failed_constructor() {
            let result = ExecutionResult::<(), String>::prepare_failed("prepare failed");

            assert!(matches!(
                result,
                ExecutionResult::Failed(ExecutorError::PrepareFailed(callback))
                    if callback.message() == "prepare failed"
            ));
        }

        #[test]
        fn test_execution_result_prepare_failed_with_type_constructor() {
            let result = ExecutionResult::<(), String>::prepare_failed_with_type(
                "prepare",
                "prepare failed",
            );

            assert!(matches!(
                result,
                ExecutionResult::Failed(ExecutorError::PrepareFailed(callback))
                    if callback.message() == "prepare failed"
            ));
        }

        #[test]
        fn test_execution_result_prepare_commit_failed_with_type_constructor() {
            let result = ExecutionResult::<(), String>::prepare_commit_failed_with_type(
                "prepare_commit",
                "commit failed",
            );

            assert!(matches!(
                result,
                ExecutionResult::Failed(ExecutorError::PrepareCommitFailed(callback))
                    if callback.message() == "commit failed"
                        && callback.callback_type() == Some("prepare_commit")
            ));
        }

        #[test]
        fn test_execution_result_prepare_rollback_failed_constructor() {
            let result =
                ExecutionResult::<(), String>::prepare_rollback_failed("original", "rollback");

            assert!(matches!(
                result,
                ExecutionResult::Failed(ExecutorError::PrepareRollbackFailed {
                    original,
                    rollback,
                }) if original.message() == "original" && rollback.message() == "rollback"
            ));
        }

        #[test]
        fn test_execution_result_prepare_rollback_failed_with_type_constructor() {
            let result = ExecutionResult::<(), String>::prepare_rollback_failed_with_type(
                "prepare_rollback",
                "original",
                "rollback",
            );

            assert!(matches!(
                result,
                ExecutionResult::Failed(ExecutorError::PrepareRollbackFailed {
                    original,
                    rollback,
                }) if original.message() == "original"
                    && rollback.message() == "rollback"
                    && rollback.callback_type() == Some("prepare_rollback")
            ));
        }
    }
}
