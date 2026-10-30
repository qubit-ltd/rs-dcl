// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Behavioral tests for the transaction protocol shown in the user guides.

use std::io;
use std::sync::Arc;
use std::sync::Barrier;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use qubit_dcl::FinalizationOutcome;
use qubit_dcl::LifecycleDclExecutor;
use qubit_dcl::LifecycleOutcome;
use qubit_dcl::RollbackCause;

use crate::support::run_test_in_subprocess;

#[derive(Clone)]
struct Database {
    committed: Arc<Mutex<Vec<String>>>,
    fail_next_commit: Arc<AtomicBool>,
    commit_barriers: Option<(Arc<Barrier>, Arc<Barrier>)>,
}

impl Database {
    fn new() -> Self {
        Self {
            committed: Arc::default(),
            fail_next_commit: Arc::new(AtomicBool::new(true)),
            commit_barriers: None,
        }
    }

    fn with_commit_barriers(entered: Arc<Barrier>, release: Arc<Barrier>) -> Self {
        Self {
            commit_barriers: Some((entered, release)),
            ..Self::new()
        }
    }

    fn committed(&self) -> Vec<String> {
        self.committed
            .lock()
            .expect("database state should not be poisoned")
            .clone()
    }
}

struct Transaction {
    database: Database,
    pending: Vec<String>,
}

impl Transaction {
    fn commit(self) -> Result<(), io::Error> {
        if let Some((entered, release)) = &self.database.commit_barriers {
            entered.wait();
            release.wait();
        }
        if self.database.fail_next_commit.swap(false, Ordering::AcqRel) {
            return Err(io::Error::other("commit failed before writing"));
        }
        self.database
            .committed
            .lock()
            .map_err(|_| io::Error::other("database state is poisoned"))?
            .extend(self.pending);
        Ok(())
    }

    fn rollback(self) -> Result<(), io::Error> {
        Ok(())
    }
}

fn executor(work_available: Arc<AtomicBool>, database: Database) -> LifecycleDclExecutor<Transaction, io::Error> {
    LifecycleDclExecutor::builder()
        .when({
            let work_available = Arc::clone(&work_available);
            move || work_available.load(Ordering::Acquire)
        })
        .prepare({
            let database = database.clone();
            move || {
                Ok::<Transaction, io::Error>(Transaction {
                    database: database.clone(),
                    pending: Vec::new(),
                })
            }
        })
        .commit(|transaction| transaction.commit())
        .rollback(|transaction, _cause: RollbackCause<'_>| transaction.rollback())
        .build()
}

#[test]
fn test_commit_failure_reopens_gate_and_second_call_commits_once() {
    let work_available = Arc::new(AtomicBool::new(true));
    let transaction_lock = Mutex::new(());
    let database = Database::new();
    let executor = executor(Arc::clone(&work_available), database.clone());

    let first = executor.run_with_token(&transaction_lock, |transaction| {
        transaction.pending.push("record".to_owned());
        work_available.store(false, Ordering::Release);
        Ok::<(), io::Error>(())
    });
    match first {
        LifecycleOutcome::TaskSucceeded {
            commit: FinalizationOutcome::Failed(error),
            ..
        } => assert_eq!(error.to_string(), "commit failed before writing"),
        other => panic!("expected the injected pre-write commit failure, got {other:?}"),
    }
    assert!(database.committed().is_empty());
    assert!(!work_available.load(Ordering::Acquire));

    {
        let _guard = transaction_lock
            .lock()
            .expect("coordination lock should not be poisoned");
        work_available.store(true, Ordering::Release);
    }

    let second = executor.run_with_token(&transaction_lock, |transaction| {
        transaction.pending.push("record".to_owned());
        work_available.store(false, Ordering::Release);
        Ok::<(), io::Error>(())
    });
    assert!(matches!(
        second,
        LifecycleOutcome::TaskSucceeded {
            commit: FinalizationOutcome::Succeeded,
            ..
        }
    ));
    assert_eq!(database.committed(), ["record"]);
    assert!(!work_available.load(Ordering::Acquire));
}

#[test]
fn test_task_error_rollback_reopens_gate_under_coordination_lock() {
    let work_available = Arc::new(AtomicBool::new(true));
    let transaction_lock = Mutex::new(());
    let database = Database::new();
    let executor = executor(Arc::clone(&work_available), database.clone());

    let outcome = executor.run_with_token(&transaction_lock, |_transaction| {
        work_available.store(false, Ordering::Release);
        Err::<(), _>(io::Error::other("task failed"))
    });
    assert!(matches!(
        outcome,
        LifecycleOutcome::TaskFailed {
            rollback: FinalizationOutcome::Succeeded,
            ..
        }
    ));
    {
        let _guard = transaction_lock
            .lock()
            .expect("coordination lock should not be poisoned");
        work_available.store(true, Ordering::Release);
    }

    assert!(work_available.load(Ordering::Acquire));
    assert!(database.committed().is_empty());
}

#[rustfmt::skip]
fn call_during_commit_sees_claimed_gate_before_database_write_scenario() {
    let work_available = Arc::new(AtomicBool::new(true));
    let transaction_lock = Arc::new(Mutex::new(()));
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let database = Database::with_commit_barriers(Arc::clone(&entered), Arc::clone(&release));
    database.fail_next_commit.store(false, Ordering::Release);
    let executor = executor(Arc::clone(&work_available), database.clone());
    let worker_executor = executor.clone();
    let worker_gate = Arc::clone(&work_available);
    let worker_lock = Arc::clone(&transaction_lock);

    let worker = thread::spawn(move || {
        worker_executor.run_with_token(&worker_lock, |transaction| {
            transaction.pending.push("record".to_owned());
            worker_gate.store(false, Ordering::Release);
            Ok::<(), io::Error>(())
        })
    });

    entered.wait();
    assert!(database.committed().is_empty());
    let concurrent = executor.run_with_token(&transaction_lock, |_transaction| -> Result<(), io::Error> {
        panic!("claimed work must not run concurrently")
    });
    assert!(matches!(concurrent, LifecycleOutcome::InitialConditionNotMet));
    assert!(database.committed().is_empty());

    release.wait();
    assert!(matches!(
        worker.join().expect("worker should finish"),
        LifecycleOutcome::TaskSucceeded {
            commit: FinalizationOutcome::Succeeded,
            ..
        }
    ));
    assert_eq!(database.committed(), ["record"]);
}

#[test]
fn test_call_during_commit_sees_claimed_gate_before_database_write() {
    run_test_in_subprocess(
        "docs::lifecycle_transaction_tests::test_call_during_commit_sees_claimed_gate_before_database_write",
        Duration::from_secs(30),
        call_during_commit_sees_claimed_gate_before_database_write_scenario,
    )
    .expect("commit protocol scenario must finish successfully");
}
