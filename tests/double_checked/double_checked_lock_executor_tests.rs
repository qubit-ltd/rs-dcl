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
        cell::Cell,
        io,
        panic::{
            AssertUnwindSafe,
            catch_unwind,
            panic_any,
        },
        rc::Rc,
        sync::{
            Arc,
            Barrier,
            atomic::{
                AtomicBool,
                AtomicUsize,
                Ordering,
            },
        },
        thread,
    };

    use qubit_dcl::{
        DoubleCheckedLockExecutor,
        double_checked::{
            ExecutionContext,
            ExecutionResult,
            ExecutorError,
        },
    };
    use qubit_lock::{
        ArcMutex,
        lock::Lock,
    };

    mod test_double_checked_lock_executor {
        use super::*;

        fn no_op_task() -> Result<(), io::Error> {
            Ok(())
        }

        fn increment_unit_task(value: &mut i32) -> Result<(), io::Error> {
            *value += 1;
            Ok(())
        }

        /// Returns a deterministic zero-argument task error.
        fn failing_no_op_task() -> Result<(), io::Error> {
            Err(io::Error::other("zero argument task failed"))
        }

        /// Panics from a zero-argument task.
        fn panicking_no_op_task() -> Result<(), io::Error> {
            panic!("zero argument task panic");
        }

        /// Returns a deterministic mutable task error.
        fn failing_increment_unit_task(_value: &mut i32) -> Result<(), io::Error> {
            Err(io::Error::other("mutable task failed"))
        }

        /// Panics from a mutable task.
        fn panicking_increment_unit_task(_value: &mut i32) -> Result<(), io::Error> {
            panic!("mutable task panic");
        }

        /// Increments the mutable value and returns it.
        fn increment_value_task(value: &mut i32) -> Result<i32, io::Error> {
            *value += 1;
            Ok(*value)
        }

        /// Returns a deterministic mutable value task error.
        fn failing_increment_value_task(_value: &mut i32) -> Result<i32, io::Error> {
            Err(io::Error::other("mutable value task failed"))
        }

        /// Panics from a mutable value task.
        fn panicking_increment_value_task(_value: &mut i32) -> Result<i32, io::Error> {
            panic!("mutable value task panic");
        }

        #[test]
        fn test_call_with_executes_reusable_task_with_mutable_data() {
            let data = ArcMutex::new(10);
            let active = Arc::new(AtomicBool::new(false));
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data.clone())
                .when({
                    let active = active.clone();
                    move || !active.load(Ordering::Acquire)
                })
                .build();

            let first = executor
                .call_with(|value: &mut i32| {
                    *value += 5;
                    Ok::<i32, io::Error>(*value)
                })
                .get_result();
            let second = executor
                .call_with(|value: &mut i32| {
                    *value += 7;
                    Ok::<i32, io::Error>(*value)
                })
                .get_result();

            assert!(matches!(first, ExecutionResult::Success(15)));
            assert!(matches!(second, ExecutionResult::Success(22)));
            assert_eq!(data.read(|value| *value), 22);
        }

        #[test]
        fn test_execute_with_reports_condition_not_met_without_lock_mutation() {
            let data = ArcMutex::new(10);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data.clone())
                .when(|| false)
                .build();

            let result = executor
                .execute_with(increment_unit_task as fn(&mut i32) -> Result<(), io::Error>)
                .get_result();

            assert!(matches!(result, ExecutionResult::ConditionNotMet));
            assert_eq!(data.read(|value| *value), 10);
        }

        #[test]
        fn test_call_and_execute_run_zero_argument_tasks_as_executor_api() {
            let data = ArcMutex::new(0);
            let counter = Arc::new(AtomicUsize::new(0));
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data.clone())
                .when(|| true)
                .build();

            let value = executor
                .call(|| Ok::<i32, io::Error>(42))
                .get_result()
                .unwrap();
            let executed = executor
                .execute({
                    let counter = counter.clone();
                    move || {
                        counter.fetch_add(1, Ordering::AcqRel);
                        Ok::<(), io::Error>(())
                    }
                })
                .get_result();

            assert_eq!(value, 42);
            assert!(matches!(executed, ExecutionResult::Success(())));
            assert_eq!(counter.load(Ordering::Acquire), 1);
        }

        #[test]
        fn test_function_pointer_tasks_cover_error_and_panic_paths() {
            let failed = DoubleCheckedLockExecutor::builder()
                .on(ArcMutex::new(0))
                .when(|| true)
                .build()
                .execute(failing_no_op_task as fn() -> Result<(), io::Error>)
                .get_result();
            match failed {
                ExecutionResult::Failed(ExecutorError::TaskFailed(error)) => {
                    assert_eq!(error.to_string(), "zero argument task failed");
                }
                _ => panic!("expected zero argument task failure"),
            }

            let panicked = DoubleCheckedLockExecutor::builder()
                .on(ArcMutex::new(0))
                .when(|| true)
                .catch_panics()
                .build()
                .execute(panicking_no_op_task as fn() -> Result<(), io::Error>)
                .get_result();
            assert!(matches!(
                panicked,
                ExecutionResult::Failed(ExecutorError::Panic(_))
            ));

            let succeeded_with = DoubleCheckedLockExecutor::builder()
                .on(ArcMutex::new(0))
                .when(|| true)
                .build()
                .execute_with(increment_unit_task as fn(&mut i32) -> Result<(), io::Error>)
                .get_result();
            assert!(matches!(succeeded_with, ExecutionResult::Success(())));

            let failed_with = DoubleCheckedLockExecutor::builder()
                .on(ArcMutex::new(0))
                .when(|| true)
                .build()
                .execute_with(failing_increment_unit_task as fn(&mut i32) -> Result<(), io::Error>)
                .get_result();
            match failed_with {
                ExecutionResult::Failed(ExecutorError::TaskFailed(error)) => {
                    assert_eq!(error.to_string(), "mutable task failed");
                }
                _ => panic!("expected mutable task failure"),
            }

            let panicked_with = DoubleCheckedLockExecutor::builder()
                .on(ArcMutex::new(0))
                .when(|| true)
                .catch_panics()
                .build()
                .execute_with(
                    panicking_increment_unit_task as fn(&mut i32) -> Result<(), io::Error>,
                )
                .get_result();
            assert!(matches!(
                panicked_with,
                ExecutionResult::Failed(ExecutorError::Panic(_))
            ));

            let called_with = DoubleCheckedLockExecutor::builder()
                .on(ArcMutex::new(0))
                .when(|| true)
                .build()
                .call_with(increment_value_task as fn(&mut i32) -> Result<i32, io::Error>)
                .get_result();
            assert!(matches!(called_with, ExecutionResult::Success(1)));

            let call_failed_with = DoubleCheckedLockExecutor::builder()
                .on(ArcMutex::new(0))
                .when(|| true)
                .build()
                .call_with(failing_increment_value_task as fn(&mut i32) -> Result<i32, io::Error>)
                .get_result();
            match call_failed_with {
                ExecutionResult::Failed(ExecutorError::TaskFailed(error)) => {
                    assert_eq!(error.to_string(), "mutable value task failed");
                }
                _ => panic!("expected mutable value task failure"),
            }

            let call_panicked_with = DoubleCheckedLockExecutor::builder()
                .on(ArcMutex::new(0))
                .when(|| true)
                .catch_panics()
                .build()
                .call_with(panicking_increment_value_task as fn(&mut i32) -> Result<i32, io::Error>)
                .get_result();
            assert!(matches!(
                call_panicked_with,
                ExecutionResult::Failed(ExecutorError::Panic(_))
            ));
        }

        #[test]
        fn test_inherent_call_returns_execution_context() {
            let data = ArcMutex::new(0);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .build();

            let context: ExecutionContext<i32, io::Error> =
                executor.call(|| Ok::<i32, io::Error>(7));

            assert!(matches!(context.get_result(), ExecutionResult::Success(7)));
        }

        #[test]
        fn test_execution_context_peek_success_and_finish() {
            let data = ArcMutex::new(0);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data.clone())
                .when(|| true)
                .build();

            let context = executor.call(|| Ok::<i32, io::Error>(3));
            assert!(context.is_success());
            assert!(matches!(context.peek_result(), ExecutionResult::Success(3)));
            assert!(matches!(context.get_result(), ExecutionResult::Success(3)));

            let finished = executor
                .execute(no_op_task as fn() -> Result<(), io::Error>)
                .finish();
            assert!(finished);

            let skipped = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| false)
                .build()
                .execute(no_op_task as fn() -> Result<(), io::Error>)
                .finish();
            assert!(!skipped);
        }

        #[test]
        fn test_configured_logger_logs_condition_not_met() {
            let data = ArcMutex::new(10);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data.clone())
                .when(|| false)
                .log_unmet_condition(log::Level::Info, "condition not met")
                .build();

            let result = executor
                .execute_with(increment_unit_task as fn(&mut i32) -> Result<(), io::Error>)
                .get_result();

            assert!(matches!(result, ExecutionResult::ConditionNotMet));
            assert_eq!(data.read(|value| *value), 10);
        }

        #[test]
        fn test_call_accepts_non_send_non_static_task() {
            let data = ArcMutex::new(10);
            let local_increment = 5;
            let marker = Rc::new(Cell::new(0));
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data.clone())
                .when(|| true)
                .build();

            let result = executor
                .call_with(|value: &mut i32| {
                    marker.set(local_increment);
                    *value += marker.get();
                    Ok::<i32, io::Error>(*value)
                })
                .get_result();

            assert!(matches!(result, ExecutionResult::Success(15)));
            assert_eq!(data.read(|value| *value), 15);
            assert_eq!(marker.get(), 5);
        }

        #[test]
        fn test_concurrent_calls_prepare_all_then_roll_back_losing_second_checks() {
            const WORKERS: usize = 4;

            let data = ArcMutex::new(0);
            let should_run = Arc::new(AtomicBool::new(true));
            let tester_calls = Arc::new(AtomicUsize::new(0));
            let first_check_gate = Arc::new(Barrier::new(WORKERS));
            let prepare_count = Arc::new(AtomicUsize::new(0));
            let rollback_count = Arc::new(AtomicUsize::new(0));
            let task_count = Arc::new(AtomicUsize::new(0));

            let executor = Arc::new(
                DoubleCheckedLockExecutor::builder()
                    .on(data.clone())
                    .when({
                        let should_run = should_run.clone();
                        let tester_calls = tester_calls.clone();
                        let first_check_gate = first_check_gate.clone();
                        move || {
                            let call_index = tester_calls.fetch_add(1, Ordering::AcqRel);
                            let current = should_run.load(Ordering::Acquire);
                            if call_index < WORKERS {
                                first_check_gate.wait();
                                return current;
                            }
                            should_run.load(Ordering::Acquire)
                        }
                    })
                    .prepare({
                        let prepare_count = prepare_count.clone();
                        move || {
                            prepare_count.fetch_add(1, Ordering::AcqRel);
                            Ok::<(), io::Error>(())
                        }
                    })
                    .rollback_prepare({
                        let rollback_count = rollback_count.clone();
                        move || {
                            rollback_count.fetch_add(1, Ordering::AcqRel);
                            Ok::<(), io::Error>(())
                        }
                    })
                    .build(),
            );

            let handles = (0..WORKERS)
                .map(|_| {
                    let executor = executor.clone();
                    let should_run = should_run.clone();
                    let task_count = task_count.clone();
                    thread::spawn(move || {
                        executor
                            .call_with(move |value: &mut i32| {
                                task_count.fetch_add(1, Ordering::AcqRel);
                                *value += 1;
                                should_run.store(false, Ordering::Release);
                                Ok::<i32, io::Error>(*value)
                            })
                            .get_result()
                    })
                })
                .collect::<Vec<_>>();

            let results = handles
                .into_iter()
                .map(|handle| handle.join().expect("worker thread should not panic"))
                .collect::<Vec<_>>();

            let success_count = results
                .iter()
                .filter(|result| matches!(result, ExecutionResult::Success(_)))
                .count();
            let unmet_count = results
                .iter()
                .filter(|result| matches!(result, ExecutionResult::ConditionNotMet))
                .count();

            assert_eq!(success_count, 1);
            assert_eq!(unmet_count, WORKERS - 1);
            assert_eq!(data.read(|value| *value), 1);
            assert_eq!(task_count.load(Ordering::Acquire), 1);
            assert_eq!(prepare_count.load(Ordering::Acquire), WORKERS);
            assert_eq!(rollback_count.load(Ordering::Acquire), WORKERS - 1);
        }

        #[test]
        fn test_task_panic_propagates_without_prepare_rollback() {
            let data = ArcMutex::new(10);
            let prepared = Arc::new(AtomicBool::new(false));
            let rolled_back = Arc::new(AtomicBool::new(false));
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .prepare({
                    let prepared = prepared.clone();
                    move || {
                        prepared.store(true, Ordering::Release);
                        Ok::<(), io::Error>(())
                    }
                })
                .rollback_prepare({
                    let rolled_back = rolled_back.clone();
                    move || {
                        rolled_back.store(true, Ordering::Release);
                        Ok::<(), io::Error>(())
                    }
                })
                .build();

            let caught = catch_unwind(AssertUnwindSafe(|| {
                executor.execute_with(|_value: &mut i32| -> Result<(), io::Error> {
                    panic!("task panic");
                });
            }));

            assert!(caught.is_err());
            assert!(prepared.load(Ordering::Acquire));
            assert!(!rolled_back.load(Ordering::Acquire));
        }

        #[test]
        fn test_task_panic_returns_panic_error_when_catch_panics_enabled() {
            let data = ArcMutex::new(10);
            let prepared = Arc::new(AtomicBool::new(false));
            let rolled_back = Arc::new(AtomicBool::new(false));
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .catch_panics()
                .prepare({
                    let prepared = prepared.clone();
                    move || {
                        prepared.store(true, Ordering::Release);
                        Ok::<(), io::Error>(())
                    }
                })
                .rollback_prepare({
                    let rolled_back = rolled_back.clone();
                    move || {
                        rolled_back.store(true, Ordering::Release);
                        Ok::<(), io::Error>(())
                    }
                })
                .build();

            let result = executor
                .execute_with(|_value: &mut i32| -> Result<(), io::Error> {
                    panic!("task panic");
                })
                .get_result();

            assert!(matches!(
                result,
                ExecutionResult::Failed(ExecutorError::Panic(_))
            ));
            assert!(prepared.load(Ordering::Acquire));
            assert!(rolled_back.load(Ordering::Acquire));
        }

        #[test]
        fn test_set_catch_panics_on_executor_catches_task_panic() {
            let data = ArcMutex::new(10);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .build()
                .set_catch_panics(true);

            let result = executor
                .execute(|| -> Result<(), io::Error> {
                    panic!("executor set_catch_panics");
                })
                .get_result();

            assert!(matches!(
                result,
                ExecutionResult::Failed(ExecutorError::Panic(_))
            ));
        }

        #[test]
        fn test_disable_catch_panics_on_executor_allows_panic() {
            let data = ArcMutex::new(10);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .catch_panics()
                .build()
                .set_catch_panics(false);

            let caught = catch_unwind(AssertUnwindSafe(|| {
                executor
                    .execute(|| -> Result<(), io::Error> {
                        panic!("executor panic should propagate");
                    })
                    .get_result();
            }));

            assert!(caught.is_err());
        }

        #[test]
        #[allow(deprecated)]
        fn test_with_catch_panics_alias_and_getter() {
            let data = ArcMutex::new(10);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .build()
                .with_catch_panics(true);

            assert!(executor.catch_panics());

            let result = executor
                .execute(|| -> Result<(), io::Error> {
                    panic!("executor with_catch_panics");
                })
                .get_result();

            assert!(matches!(
                result,
                ExecutionResult::Failed(ExecutorError::Panic(_))
            ));
        }

        #[test]
        fn test_task_panic_with_string_payload_is_captured() {
            let data = ArcMutex::new(10);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .catch_panics()
                .build();

            let result = executor
                .execute_with(|_value: &mut i32| -> Result<(), io::Error> {
                    panic!("{}", String::from("string payload"));
                })
                .get_result();

            let message = match result {
                ExecutionResult::Failed(ExecutorError::Panic(error)) => error.message().to_string(),
                _ => panic!("expected panic error"),
            };

            assert!(message.contains("string payload"));
        }

        #[test]
        fn test_task_panic_with_non_string_payload_is_captured() {
            let data = ArcMutex::new(10);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .catch_panics()
                .build();

            let result = executor
                .execute_with(|_value: &mut i32| -> Result<(), io::Error> {
                    panic_any(vec![1, 2, 3]);
                })
                .get_result();

            let message = match result {
                ExecutionResult::Failed(ExecutorError::Panic(error)) => error.message().to_string(),
                _ => panic!("expected panic error"),
            };

            assert!(message.contains("Any"));
        }

        #[test]
        fn test_first_check_panic_with_catch_panics_returns_panic_error() {
            let data = ArcMutex::new(10);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| panic!("first check panic"))
                .catch_panics()
                .build();

            let result = executor
                .execute_with(|_value: &mut i32| -> Result<(), io::Error> {
                    Ok::<(), io::Error>(())
                })
                .get_result();

            let message = match result {
                ExecutionResult::Failed(ExecutorError::Panic(error)) => error.message().to_string(),
                _ => panic!("expected panic error"),
            };

            assert!(message.contains("first check panic"));
        }

        #[test]
        fn test_second_check_panic_with_catch_panics_returns_panic_error() {
            let checks = Arc::new(AtomicUsize::new(0));
            let executor = DoubleCheckedLockExecutor::builder()
                .on(ArcMutex::new(10))
                .when({
                    let checks = checks.clone();
                    move || {
                        let called = checks.fetch_add(1, Ordering::AcqRel);
                        if called == 0 {
                            true
                        } else {
                            panic!("second check panic");
                        }
                    }
                })
                .catch_panics()
                .build();

            let result = executor
                .execute_with(|_value: &mut i32| -> Result<(), io::Error> {
                    Ok::<(), io::Error>(())
                })
                .get_result();

            let message = match result {
                ExecutionResult::Failed(ExecutorError::Panic(error)) => error.message().to_string(),
                _ => panic!("expected panic error"),
            };

            assert_eq!(checks.load(Ordering::Acquire), 2);
            assert!(message.contains("second check panic"));
        }

        #[test]
        fn test_prepare_action_panic_with_catch_panics_returns_prepare_failed() {
            let data = ArcMutex::new(10);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .catch_panics()
                .prepare(|| -> Result<(), io::Error> {
                    panic!("prepare action panic");
                })
                .build();

            let result = executor
                .execute_with(|_value: &mut i32| -> Result<(), io::Error> {
                    Ok::<(), io::Error>(())
                })
                .get_result();

            assert!(matches!(
                result,
                ExecutionResult::Failed(ExecutorError::PrepareFailed(callback_error))
                    if callback_error.message() == "prepare action panic"
            ));
        }

        #[test]
        fn test_prepare_commit_action_panic_with_catch_panics_replaces_success() {
            let data = ArcMutex::new(10);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .catch_panics()
                .prepare(|| Ok::<(), io::Error>(()))
                .commit_prepare(|| -> Result<(), io::Error> {
                    panic!("commit panic");
                })
                .build();

            let result = executor
                .execute_with(|_value: &mut i32| -> Result<(), io::Error> {
                    Ok::<(), io::Error>(())
                })
                .get_result();

            let message = match result {
                ExecutionResult::Failed(ExecutorError::PrepareCommitFailed(error)) => {
                    error.message().to_string()
                }
                _ => panic!("expected commit failed error"),
            };

            assert!(message.contains("commit panic"));
        }

        #[test]
        fn test_prepare_rollback_action_panic_with_catch_panics_replaces_result() {
            let data = ArcMutex::new(10);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .catch_panics()
                .prepare(|| Ok::<(), io::Error>(()))
                .rollback_prepare(|| -> Result<(), io::Error> {
                    panic!("rollback panic");
                })
                .build();

            let result = executor
                .execute_with(|_value: &mut i32| -> Result<(), io::Error> {
                    Err(io::Error::other("task failed"))
                })
                .get_result();

            let message = match result {
                ExecutionResult::Failed(ExecutorError::PrepareRollbackFailed {
                    rollback, ..
                }) => rollback.message().to_string(),
                _ => panic!("expected rollback failed error"),
            };

            assert!(message.contains("rollback panic"));
        }

        #[test]
        fn test_prepare_success_without_commit_keeps_success_result() {
            let data = ArcMutex::new(1);
            let prepare_called = Arc::new(AtomicUsize::new(0));
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .prepare({
                    let prepare_called = prepare_called.clone();
                    move || {
                        prepare_called.fetch_add(1, Ordering::AcqRel);
                        Ok::<(), io::Error>(())
                    }
                })
                .build();

            let result = executor
                .call_with(|value: &mut i32| {
                    *value += 1;
                    Ok::<i32, io::Error>(*value)
                })
                .get_result();

            assert!(matches!(result, ExecutionResult::Success(2)));
            assert_eq!(prepare_called.load(Ordering::Acquire), 1);
        }

        #[test]
        fn test_execute_with_returning_prepare_commit_error_keeps_original_error_message() {
            let data = ArcMutex::new(1);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .catch_panics()
                .prepare(|| Ok::<(), io::Error>(()))
                .commit_prepare(|| Err::<(), io::Error>(io::Error::other("commit callback error")))
                .build();

            let result = executor
                .execute_with(|value: &mut i32| {
                    *value += 1;
                    Ok::<(), io::Error>(())
                })
                .get_result();

            match result {
                ExecutionResult::Failed(ExecutorError::PrepareCommitFailed(error)) => {
                    assert!(error.message().contains("commit callback error"));
                }
                _ => panic!("expected prepare commit failed result"),
            }
        }

        #[test]
        fn test_execute_with_returning_prepare_rollback_error_keeps_original_message() {
            let data = ArcMutex::new(1);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .catch_panics()
                .prepare(|| Ok::<(), io::Error>(()))
                .rollback_prepare(|| {
                    Err::<(), io::Error>(io::Error::other("rollback callback error"))
                })
                .build();

            let result = executor
                .execute_with(|value: &mut i32| {
                    *value += 1;
                    Err::<(), io::Error>(io::Error::other("task failed"))
                })
                .get_result();

            match result {
                ExecutionResult::Failed(error) => {
                    assert_eq!(error.callback_type(), Some("prepare_rollback"));
                    match error {
                        ExecutorError::PrepareRollbackFailed { rollback, .. } => {
                            assert!(rollback.message().contains("rollback callback error"));
                        }
                        _ => panic!("expected prepare rollback failed result"),
                    }
                }
                _ => panic!("expected prepare rollback failed result"),
            }
        }

        #[test]
        fn test_execute_with_prepare_failed_task_error_preserves_task_failure_if_no_rollback() {
            let data = ArcMutex::new(1);
            let prepare_calls = Arc::new(AtomicUsize::new(0));
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .catch_panics()
                .prepare({
                    let prepare_calls = prepare_calls.clone();
                    move || {
                        prepare_calls.fetch_add(1, Ordering::AcqRel);
                        Ok::<(), io::Error>(())
                    }
                })
                .build();

            let result = executor
                .execute_with(|value: &mut i32| {
                    *value += 1;
                    Err::<(), io::Error>(io::Error::other("task failed"))
                })
                .get_result();

            let task = match result {
                ExecutionResult::Failed(ExecutorError::TaskFailed(error)) => error,
                _ => panic!("expected task failed result"),
            };

            assert_eq!(prepare_calls.load(Ordering::Acquire), 1);
            assert_eq!(task.to_string(), "task failed");
        }

        #[test]
        fn test_execute_unit_with_returning_prepare_commit_error_keeps_original_error_message() {
            let data = ArcMutex::new(1);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .catch_panics()
                .prepare(|| Ok::<(), io::Error>(()))
                .commit_prepare(|| {
                    Err::<(), io::Error>(io::Error::other("unit commit callback error"))
                })
                .build();

            let result = executor
                .execute_with(|value: &mut i32| {
                    *value += 1;
                    Ok::<(), io::Error>(())
                })
                .get_result();

            match result {
                ExecutionResult::Failed(ExecutorError::PrepareCommitFailed(error)) => {
                    assert!(error.message().contains("unit commit callback error"));
                }
                _ => panic!("expected prepare commit failed result"),
            }
        }

        #[test]
        fn test_execute_unit_with_returning_prepare_rollback_error_preserves_task_failure() {
            let data = ArcMutex::new(1);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .catch_panics()
                .prepare(|| Ok::<(), io::Error>(()))
                .rollback_prepare(|| {
                    Err::<(), io::Error>(io::Error::other("unit rollback callback error"))
                })
                .build();

            let result = executor
                .execute_with(|value: &mut i32| {
                    *value += 1;
                    Err::<(), io::Error>(io::Error::other("task failed"))
                })
                .get_result();

            match result {
                ExecutionResult::Failed(ExecutorError::PrepareRollbackFailed {
                    rollback, ..
                }) => {
                    assert!(rollback.message().contains("unit rollback callback error"));
                }
                _ => panic!("expected prepare rollback failed result"),
            }
        }

        #[test]
        fn test_prepare_commit_success_keeps_success_result() {
            let data = ArcMutex::new(1);
            let prepare_called = Arc::new(AtomicUsize::new(0));
            let commit_called = Arc::new(AtomicUsize::new(0));
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .prepare({
                    let prepare_called = prepare_called.clone();
                    move || {
                        prepare_called.fetch_add(1, Ordering::AcqRel);
                        Ok::<(), io::Error>(())
                    }
                })
                .commit_prepare({
                    let commit_called = commit_called.clone();
                    move || {
                        commit_called.fetch_add(1, Ordering::AcqRel);
                        Ok::<(), io::Error>(())
                    }
                })
                .build();

            let result = executor
                .call_with(|value: &mut i32| {
                    *value += 1;
                    Ok::<i32, io::Error>(*value)
                })
                .get_result();

            assert!(matches!(result, ExecutionResult::Success(2)));
            assert_eq!(prepare_called.load(Ordering::Acquire), 1);
            assert_eq!(commit_called.load(Ordering::Acquire), 1);
        }

        #[test]
        fn test_execute_with_second_check_unmet_runs_prepare_rollback_success() {
            let should_pass = Arc::new(AtomicBool::new(true));
            let rollback_called = Arc::new(AtomicUsize::new(0));
            let executor = DoubleCheckedLockExecutor::builder()
                .on(ArcMutex::new(1))
                .when({
                    let should_pass = should_pass.clone();
                    move || should_pass.fetch_and(false, Ordering::AcqRel)
                })
                .prepare(|| Ok::<(), io::Error>(()))
                .rollback_prepare({
                    let rollback_called = rollback_called.clone();
                    move || {
                        rollback_called.fetch_add(1, Ordering::AcqRel);
                        Ok::<(), io::Error>(())
                    }
                })
                .build();

            let result = executor
                .execute_with(|_value: &mut i32| -> Result<(), io::Error> {
                    Ok::<(), io::Error>(())
                })
                .get_result();

            assert!(matches!(result, ExecutionResult::ConditionNotMet));
            assert_eq!(rollback_called.load(Ordering::Acquire), 1);
        }

        #[test]
        fn test_task_panic_with_owned_string_payload_is_captured() {
            let data = ArcMutex::new(10);
            let executor = DoubleCheckedLockExecutor::builder()
                .on(data)
                .when(|| true)
                .catch_panics()
                .build();

            let result = executor
                .execute_with(|_value: &mut i32| -> Result<(), io::Error> {
                    panic_any(String::from("owned string payload"));
                })
                .get_result();

            let message = match result {
                ExecutionResult::Failed(ExecutorError::Panic(error)) => error.message().to_string(),
                _ => panic!("expected panic error"),
            };

            assert!(message.contains("owned string payload"));
        }
    }
}
