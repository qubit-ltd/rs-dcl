// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Benchmarks the downstream-style submission and shutdown state machine.

use std::{
    convert::Infallible,
    hint::black_box,
    sync::{
        Arc,
        Mutex,
        atomic::{
            AtomicU8,
            AtomicUsize,
            Ordering,
        },
    },
};

use criterion::{
    BenchmarkGroup,
    Criterion,
    criterion_group,
    criterion_main,
    measurement::WallTime,
};
use parking_lot::Mutex as ParkingLotMutex;
use qubit_dcl::{
    DoubleCheckedLockExecutor,
    ExecutionOutcome,
};
use qubit_lock::Lock;

/// Executor accepts new work in this state.
const RUNNING: u8 = 0;
/// Executor rejects new work after shutdown begins.
const SHUT_DOWN: u8 = 1;

/// Runs a submission after acquiring the lock before checking state.
#[inline]
fn submit_lock_first<L>(
    state: &AtomicU8,
    lock: &L,
    submitted: &AtomicUsize,
) -> bool
where
    L: Lock + ?Sized,
{
    let _guard = lock.lock();
    if state.load(Ordering::Acquire) != RUNNING {
        return false;
    }
    submitted.fetch_add(1, Ordering::Relaxed);
    true
}

/// Runs the handwritten double-checked submission path.
#[inline]
fn submit_handwritten_dcl<L>(
    state: &AtomicU8,
    lock: &L,
    submitted: &AtomicUsize,
) -> bool
where
    L: Lock + ?Sized,
{
    if state.load(Ordering::Acquire) != RUNNING {
        return false;
    }
    let _guard = lock.lock();
    if state.load(Ordering::Acquire) != RUNNING {
        return false;
    }
    submitted.fetch_add(1, Ordering::Relaxed);
    true
}

/// Registers running and shut-down submission paths for one lock backend.
fn benchmark_submission_backend<L>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    backend: &str,
    lock: &L,
) where
    L: Lock + ?Sized,
{
    let state = Arc::new(AtomicU8::new(RUNNING));
    let submitted = AtomicUsize::new(0);
    let predicate_state = Arc::clone(&state);
    let executor = DoubleCheckedLockExecutor::builder()
        .when(move || predicate_state.load(Ordering::Acquire) == RUNNING)
        .build();

    group.bench_function(format!("{backend}/running/lock_first"), |bencher| {
        bencher.iter(|| black_box(submit_lock_first(&state, lock, &submitted)));
    });
    group.bench_function(
        format!("{backend}/running/handwritten_dcl"),
        |bencher| {
            bencher.iter(|| {
                black_box(submit_handwritten_dcl(&state, lock, &submitted))
            });
        },
    );
    group.bench_function(format!("{backend}/running/qubit_dcl"), |bencher| {
        bencher.iter(|| {
            black_box(executor.run(lock, || {
                submitted.fetch_add(1, Ordering::Relaxed);
                Ok::<(), Infallible>(())
            }))
        });
    });

    state.store(SHUT_DOWN, Ordering::Release);
    group.bench_function(
        format!("{backend}/shut_down/lock_first"),
        |bencher| {
            bencher.iter(|| {
                black_box(submit_lock_first(&state, lock, &submitted))
            });
        },
    );
    group.bench_function(
        format!("{backend}/shut_down/handwritten_dcl"),
        |bencher| {
            bencher.iter(|| {
                black_box(submit_handwritten_dcl(&state, lock, &submitted))
            });
        },
    );
    group.bench_function(format!("{backend}/shut_down/qubit_dcl"), |bencher| {
        bencher.iter(|| {
            let outcome = executor.run(lock, || {
                submitted.fetch_add(1, Ordering::Relaxed);
                Ok::<(), Infallible>(())
            });
            black_box(matches!(outcome, ExecutionOutcome::Success(())))
        });
    });
}

/// Benchmarks submission gates matching executor shutdown coordination.
fn benchmark_dcl(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("executor_submission_gate");
    let std_lock = Mutex::new(());
    benchmark_submission_backend(&mut group, "std_mutex", &std_lock);
    let parking_lot_lock = ParkingLotMutex::new(());
    benchmark_submission_backend(
        &mut group,
        "parking_lot_mutex",
        &parking_lot_lock,
    );
    group.finish();
}

criterion_group!(benches, benchmark_dcl);
criterion_main!(benches);
