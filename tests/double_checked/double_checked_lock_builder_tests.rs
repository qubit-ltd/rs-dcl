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
    use std::{
        io,
        panic::{
            AssertUnwindSafe,
            catch_unwind,
        },
    };

    use qubit_dcl::{
        DoubleCheckedLock,
        double_checked::{
            ExecutionResult,
            ExecutorError,
        },
    };
    use qubit_lock::ArcMutex;

    mod test_double_checked_lock_builder {
        use super::*;

        #[test]
        fn test_log_methods_are_chainable_before_call() {
            let data = ArcMutex::new(1);

            let result = DoubleCheckedLock::on(data)
                .log_unmet_condition(log::Level::Info, "condition not met")
                .log_prepare_failure(log::Level::Warn, "prepare failed")
                .log_prepare_commit_failure(log::Level::Error, "prepare commit failed")
                .log_prepare_rollback_failure(log::Level::Debug, "prepare rollback failed")
                .disable_unmet_condition_logging()
                .disable_prepare_failure_logging()
                .disable_prepare_commit_failure_logging()
                .disable_prepare_rollback_failure_logging()
                .when(|| true)
                .call(|| Ok::<i32, io::Error>(42))
                .get_result();

            assert!(matches!(result, ExecutionResult::Success(42)));
        }

        #[test]
        fn test_catch_panics_catches_task_panic() {
            let data = ArcMutex::new(10);
            let result = DoubleCheckedLock::on(data)
                .catch_panics()
                .when(|| true)
                .execute_with(|_value: &mut i32| -> Result<(), io::Error> {
                    panic!("panic in lock builder");
                })
                .get_result();

            assert!(matches!(
                result,
                ExecutionResult::Failed(ExecutorError::Panic(_))
            ));
        }

        #[test]
        fn test_with_panic_capture_catches_task_panic() {
            let data = ArcMutex::new(10);
            let result = DoubleCheckedLock::on(data)
                .with_panic_capture(true)
                .when(|| true)
                .execute_with(|_value: &mut i32| -> Result<(), io::Error> {
                    panic!("panic with panic capture");
                })
                .get_result();

            assert!(matches!(
                result,
                ExecutionResult::Failed(ExecutorError::Panic(_))
            ));
        }

        #[test]
        fn test_disable_catch_panics_allows_panic() {
            let data = ArcMutex::new(10);
            let caught = catch_unwind(AssertUnwindSafe(|| {
                DoubleCheckedLock::on(data)
                    .catch_panics()
                    .disable_catch_panics()
                    .when(|| true)
                    .execute_with(|_value: &mut i32| -> Result<(), io::Error> {
                        panic!("panic from lock builder disable_catch_panics");
                    })
                    .get_result();
            }));

            assert!(caught.is_err());
        }
    }
}
