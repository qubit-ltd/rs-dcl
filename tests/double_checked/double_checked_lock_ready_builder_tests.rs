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
        sync::{
            Arc,
            atomic::{
                AtomicBool,
                Ordering,
            },
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

    mod test_double_checked_lock_ready_builder {
        use super::*;

        #[test]
        fn test_log_methods_are_chainable_before_execute() {
            let data = ArcMutex::new(1);
            let executed = Arc::new(AtomicBool::new(false));

            let result = DoubleCheckedLock::on(data)
                .when(|| true)
                .log_unmet_condition(log::Level::Info, "condition not met")
                .log_prepare_failure(log::Level::Warn, "prepare failed")
                .log_prepare_commit_failure(log::Level::Error, "prepare commit failed")
                .log_prepare_rollback_failure(log::Level::Debug, "prepare rollback failed")
                .disable_unmet_condition_logging()
                .disable_prepare_failure_logging()
                .disable_prepare_commit_failure_logging()
                .disable_prepare_rollback_failure_logging()
                .execute({
                    let executed = executed.clone();
                    move || {
                        executed.store(true, Ordering::Release);
                        Ok::<(), io::Error>(())
                    }
                })
                .get_result();

            assert!(matches!(result, ExecutionResult::Success(())));
            assert!(executed.load(Ordering::Acquire));
        }

        #[test]
        fn test_catch_panics_catches_task_panic() {
            let data = ArcMutex::new(10);
            let result = DoubleCheckedLock::on(data)
                .when(|| true)
                .catch_panics()
                .prepare(|| Ok::<(), io::Error>(()))
                .execute_with(|_value: &mut i32| -> Result<(), io::Error> {
                    panic!("panic in convenient API task");
                })
                .get_result();

            assert!(matches!(
                result,
                ExecutionResult::Failed(ExecutorError::Panic(_))
            ));
        }

        #[test]
        fn test_with_panic_capture_false_allows_panic() {
            let data = ArcMutex::new(10);
            let result = catch_unwind(AssertUnwindSafe(|| {
                DoubleCheckedLock::on(data)
                    .when(|| true)
                    .with_panic_capture(true)
                    .with_panic_capture(false)
                    .execute_with(|_value: &mut i32| -> Result<(), io::Error> {
                        panic!("panic should propagate");
                    })
                    .get_result()
            }));

            assert!(result.is_err());
        }

        #[test]
        fn test_disable_catch_panics_allows_panic() {
            let data = ArcMutex::new(10);
            let caught = catch_unwind(AssertUnwindSafe(|| {
                DoubleCheckedLock::on(data)
                    .when(|| true)
                    .catch_panics()
                    .disable_catch_panics()
                    .execute_with(|_value: &mut i32| -> Result<(), io::Error> {
                        panic!("panic from ready builder disable_catch_panics");
                    })
                    .get_result();
            }));

            assert!(caught.is_err());
        }
    }
}
