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
    use std::{error::Error, io};

    use qubit_dcl::double_checked::{CallbackError, ExecutorError};

    mod test_executor_error {
        use super::*;

        #[test]
        fn test_executor_error_task_failed_display() {
            let error = ExecutorError::<String>::TaskFailed("Task failed".to_string());
            let display = format!("{}", error);
            assert_eq!(display, "Task execution failed: Task failed");
        }

        #[test]
        fn test_executor_error_prepare_failed_display() {
            let error: ExecutorError<String> =
                ExecutorError::PrepareFailed(CallbackError::from_display("Prepare failed"));
            let display = format!("{}", error);
            assert_eq!(display, "Preparation action failed: Prepare failed");
        }

        #[test]
        fn test_executor_error_panic_display() {
            let error: ExecutorError<String> =
                ExecutorError::Panic(CallbackError::from_display("Task panicked"));
            let display = format!("{}", error);
            assert_eq!(display, "Execution panicked: Task panicked");
        }

        #[test]
        fn test_executor_error_prepare_commit_failed_display() {
            let error: ExecutorError<String> =
                ExecutorError::PrepareCommitFailed(CallbackError::from_display("Commit failed"));
            let display = format!("{}", error);
            assert_eq!(display, "Prepare commit action failed: Commit failed");
        }

        #[test]
        fn test_executor_error_prepare_rollback_failed_display() {
            let error: ExecutorError<String> = ExecutorError::PrepareRollbackFailed {
                original: CallbackError::from_display("Original error"),
                rollback: CallbackError::from_display("Rollback error"),
            };
            let display = format!("{}", error);
            assert_eq!(
                display,
                "Prepare rollback failed: original error = Original error, rollback error = Rollback error"
            );
        }

        #[test]
        fn test_executor_error_debug() {
            let error = ExecutorError::<String>::TaskFailed("Debug test".to_string());
            let debug_str = format!("{:?}", error);
            assert!(debug_str.contains("TaskFailed"));
            assert!(debug_str.contains("Debug test"));
        }

        #[test]
        fn test_executor_error_with_io_error() {
            let io_error = io::Error::other("IO error");
            let error = ExecutorError::<io::Error>::TaskFailed(io_error);
            let display = format!("{}", error);
            assert!(display.contains("Task execution failed"));
        }

        #[test]
        fn test_executor_error_callback_failures_display_with_io_error_type() {
            let panic_error =
                ExecutorError::<io::Error>::Panic(CallbackError::from_display("task panicked"));
            let prepare_error = ExecutorError::<io::Error>::PrepareFailed(
                CallbackError::from_display("prepare failed"),
            );
            let commit_error = ExecutorError::<io::Error>::PrepareCommitFailed(
                CallbackError::from_display("commit failed"),
            );
            let rollback_error = ExecutorError::<io::Error>::PrepareRollbackFailed {
                original: CallbackError::from_display("task failed"),
                rollback: CallbackError::from_display("rollback failed"),
            };

            assert_eq!(panic_error.to_string(), "Execution panicked: task panicked");
            assert_eq!(
                prepare_error.to_string(),
                "Preparation action failed: prepare failed"
            );
            assert_eq!(
                commit_error.to_string(),
                "Prepare commit action failed: commit failed"
            );
            assert_eq!(
                rollback_error.to_string(),
                "Prepare rollback failed: original error = task failed, rollback error = rollback failed"
            );
        }

        #[test]
        fn test_executor_error_is_error_trait() {
            let error = ExecutorError::<io::Error>::TaskFailed(io::Error::other("Test"));
            let _error_trait: &dyn std::error::Error = &error;
        }

        #[test]
        fn test_executor_error_source_exposes_task_error() {
            let error = ExecutorError::<io::Error>::TaskFailed(io::Error::other("source error"));

            let source = error.source().expect("task error should be source");
            assert_eq!(source.to_string(), "source error");
        }

        #[test]
        fn test_executor_error_source_is_none_for_prepare_failures() {
            let error = ExecutorError::<io::Error>::PrepareFailed(CallbackError::from_display(
                "prepare failed",
            ));

            assert!(error.source().is_none());
        }

        #[test]
        fn test_executor_error_source_is_none_for_callback_failures() {
            let panic_error =
                ExecutorError::<io::Error>::Panic(CallbackError::from_display("task panicked"));
            let commit_error = ExecutorError::<io::Error>::PrepareCommitFailed(
                CallbackError::from_display("commit failed"),
            );
            let rollback_error = ExecutorError::<io::Error>::PrepareRollbackFailed {
                original: CallbackError::from_display("task failed"),
                rollback: CallbackError::from_display("rollback failed"),
            };

            assert!(panic_error.source().is_none());
            assert!(commit_error.source().is_none());
            assert!(rollback_error.source().is_none());
        }

        #[test]
        fn test_executor_error_callback_type_for_rollback_error_prefers_rollback_type() {
            let error = ExecutorError::<String>::PrepareRollbackFailed {
                original: CallbackError::with_type("prepare", "prepare failed"),
                rollback: CallbackError::with_type("rollback", "rollback failed"),
            };

            assert_eq!(error.callback_type(), Some("rollback"));
        }

        #[test]
        fn test_executor_error_callback_type_for_rollback_error_falls_back_to_original_type() {
            let error = ExecutorError::<String>::PrepareRollbackFailed {
                original: CallbackError::with_type("prepare", "prepare failed"),
                rollback: CallbackError::from_display("rollback failed"),
            };

            assert_eq!(error.callback_type(), Some("prepare"));
        }

        #[test]
        fn test_executor_error_callback_type_returns_none_for_task_error() {
            let error: ExecutorError<String> = ExecutorError::TaskFailed("task failed".to_string());
            assert_eq!(error.callback_type(), None);
        }

        #[test]
        fn test_executor_error_callback_type_for_prepare_failure() {
            let error: ExecutorError<String> =
                ExecutorError::PrepareFailed(CallbackError::with_type("prepare", "prepare failed"));

            assert_eq!(error.callback_type(), Some("prepare"));
        }

        #[test]
        fn test_executor_error_callback_type_for_prepare_commit_failure() {
            let error: ExecutorError<String> = ExecutorError::PrepareCommitFailed(
                CallbackError::with_type("prepare_commit", "prepare commit failed"),
            );

            assert_eq!(error.callback_type(), Some("prepare_commit"));
        }

        #[test]
        fn test_executor_error_callback_type_reflects_panic_error_type() {
            let error: ExecutorError<String> =
                ExecutorError::Panic(CallbackError::with_type("task", "task panicked"));
            assert_eq!(error.callback_type(), Some("task"));
        }
    }
}
