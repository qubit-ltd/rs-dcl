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
        io,
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
        double_checked::ExecutionResult,
    };
    use qubit_lock::{
        ArcMutex,
        lock::Lock,
    };

    mod test_double_checked_lock {
        use super::*;

        #[test]
        fn test_call_with_runs_without_manual_executor_build() {
            let data = ArcMutex::new(10);
            let skip = Arc::new(AtomicBool::new(false));

            let updated = DoubleCheckedLock::on(data.clone())
                .when({
                    let skip = skip.clone();
                    move || !skip.load(Ordering::Acquire)
                })
                .call_with(|value: &mut i32| {
                    *value += 5;
                    Ok::<i32, io::Error>(*value)
                })
                .get_result();

            assert!(matches!(updated, ExecutionResult::Success(15)));
            assert_eq!(data.read(|value| *value), 15);
        }

        #[test]
        fn test_execute_with_returns_unmet_when_condition_fails() {
            let data = ArcMutex::new(10);

            let result = DoubleCheckedLock::on(data.clone())
                .when(|| false)
                .execute_with(|value: &mut i32| {
                    *value += 1;
                    Ok::<(), io::Error>(())
                })
                .get_result();

            assert!(matches!(result, ExecutionResult::ConditionNotMet));
            assert_eq!(data.read(|value| *value), 10);
        }

        #[test]
        fn test_prepare_pipeline_works_in_convenience_mode() {
            let data = ArcMutex::new(10);
            let prepared = Arc::new(AtomicBool::new(false));
            let committed = Arc::new(AtomicBool::new(false));
            let rolled_back = Arc::new(AtomicBool::new(false));

            let result = DoubleCheckedLock::on(data.clone())
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
                .commit_prepare({
                    let committed = committed.clone();
                    move || {
                        committed.store(true, Ordering::Release);
                        Ok::<(), io::Error>(())
                    }
                })
                .call_with(|value: &mut i32| {
                    *value += 3;
                    Ok::<i32, io::Error>(*value)
                })
                .get_result();

            assert!(matches!(result, ExecutionResult::Success(13)));
            assert!(prepared.load(Ordering::Acquire));
            assert!(committed.load(Ordering::Acquire));
            assert!(!rolled_back.load(Ordering::Acquire));
        }

        #[test]
        fn test_build_still_exposes_reusable_executor() {
            let data = ArcMutex::new(1);
            let executor = DoubleCheckedLock::on(data.clone()).when(|| true).build();

            let first = executor
                .call_with(|value: &mut i32| {
                    *value += 1;
                    Ok::<i32, io::Error>(*value)
                })
                .get_result();
            let second = executor
                .call_with(|value: &mut i32| {
                    *value += 1;
                    Ok::<i32, io::Error>(*value)
                })
                .get_result();

            assert!(matches!(first, ExecutionResult::Success(2)));
            assert!(matches!(second, ExecutionResult::Success(3)));
            assert_eq!(data.read(|value| *value), 3);
        }
    }
}
