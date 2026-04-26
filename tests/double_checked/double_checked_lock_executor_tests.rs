/*******************************************************************************
 *
 *    Copyright (c) 2025 - 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        io,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
        sync::{
            Arc, Barrier,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        thread,
    };

    use qubit_dcl::{
        DoubleCheckedLockExecutor,
        double_checked::{ExecutionContext, ExecutionResult},
    };
    use qubit_lock::{ArcMutex, lock::Lock};

    mod test_double_checked_lock_executor {
        use super::*;

        fn no_op_task() -> Result<(), io::Error> {
            Ok(())
        }

        fn increment_unit_task(value: &mut i32) -> Result<(), io::Error> {
            *value += 1;
            Ok(())
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
    }
}
