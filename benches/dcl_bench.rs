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
        Barrier,
        Mutex,
        atomic::{
            AtomicU8,
            AtomicUsize,
            Ordering,
        },
    },
    thread,
    time::Duration,
};

use criterion::{
    BenchmarkGroup,
    Criterion,
    Throughput,
    criterion_group,
    criterion_main,
    measurement::WallTime,
};
use parking_lot::Mutex as ParkingLotMutex;
use qubit_dcl::{
    DclExecutor,
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

/// Executes the normal DCL path with a statically dispatched predicate.
///
/// `predicate` is evaluated before and after acquiring `lock`. The function
/// returns `true` only when both checks succeed and `task` runs successfully.
/// This benchmark-only helper is the zero-cost upper bound for a typed public
/// executor; it is not part of the library API.
#[inline]
fn run_typed_dcl<L, P, F>(predicate: &P, lock: &L, task: F) -> bool
where
    L: Lock + ?Sized,
    P: Fn() -> bool + ?Sized,
    F: FnOnce(),
{
    if !predicate() {
        return false;
    }
    let _guard = lock.lock();
    if !predicate() {
        return false;
    }
    task();
    true
}

/// Registers predicate-only measurements for a fixed executor state.
///
/// The three entries use identical atomic state reads and differ only in how
/// the closure is represented. This isolates static and trait-object dispatch
/// from locking and task work.
fn benchmark_predicate_representations_for_state(
    group: &mut BenchmarkGroup<'_, WallTime>,
    state_name: &str,
    state_value: u8,
) {
    let state = Arc::new(AtomicU8::new(state_value));
    let direct_state = Arc::clone(&state);
    let direct = move || direct_state.load(Ordering::Acquire) == RUNNING;
    let static_arc_state = Arc::clone(&state);
    let static_arc =
        Arc::new(move || static_arc_state.load(Ordering::Acquire) == RUNNING);
    let dynamic_state = Arc::clone(&state);
    let dynamic: Arc<dyn Fn() -> bool + Send + Sync> =
        Arc::new(move || dynamic_state.load(Ordering::Acquire) == RUNNING);

    group.bench_function(
        format!("predicate/{state_name}/direct_closure"),
        |bencher| {
            bencher.iter(|| black_box(direct()));
        },
    );
    group.bench_function(
        format!("predicate/{state_name}/arc_static"),
        |bencher| {
            bencher.iter(|| black_box(static_arc()));
        },
    );
    group.bench_function(
        format!("predicate/{state_name}/arc_dynamic"),
        |bencher| {
            bencher.iter(|| black_box(dynamic()));
        },
    );
}

/// Registers predicate representation measurements for running and shut-down
/// states.
fn benchmark_predicate_representations(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("dcl_cost_breakdown");
    benchmark_predicate_representations_for_state(
        &mut group, "running", RUNNING,
    );
    benchmark_predicate_representations_for_state(
        &mut group,
        "shut_down",
        SHUT_DOWN,
    );
    group.finish();
}

/// Registers current and statically typed DCL measurements for one backend.
///
/// Both paths evaluate the predicate twice on acceptance and once on rejection.
/// The typed entry isolates the potential benefit of making the predicate a
/// generic type parameter instead of the current `Arc<dyn Fn()>` type erasure.
fn benchmark_typed_dcl_backend<L>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    backend: &str,
    lock: &L,
) where
    L: Lock + ?Sized,
{
    for (state_name, state_value) in
        [("running", RUNNING), ("shut_down", SHUT_DOWN)]
    {
        let state = Arc::new(AtomicU8::new(state_value));
        let typed_state = Arc::clone(&state);
        let typed_predicate =
            move || typed_state.load(Ordering::Acquire) == RUNNING;
        let executor_state = Arc::clone(&state);
        let executor = DclExecutor::new(move || {
            executor_state.load(Ordering::Acquire) == RUNNING
        });
        let submitted = AtomicUsize::new(0);

        group.bench_function(
            format!("typed/{backend}/{state_name}"),
            |bencher| {
                bencher.iter(|| {
                    black_box(run_typed_dcl(&typed_predicate, lock, || {
                        submitted.fetch_add(1, Ordering::Relaxed);
                    }))
                });
            },
        );
        group.bench_function(
            format!("erased/{backend}/{state_name}"),
            |bencher| {
                bencher.iter(|| {
                    black_box(executor.run(lock, || {
                        submitted.fetch_add(1, Ordering::Relaxed);
                        Ok::<(), Infallible>(())
                    }))
                });
            },
        );
    }
}

/// Registers typed-versus-erased DCL measurements for both supported locks.
fn benchmark_typed_dcl(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("dcl_dispatch_cost");
    let std_lock = Mutex::new(());
    benchmark_typed_dcl_backend(&mut group, "std_mutex", &std_lock);
    let parking_lot_lock = ParkingLotMutex::new(());
    benchmark_typed_dcl_backend(
        &mut group,
        "parking_lot_mutex",
        &parking_lot_lock,
    );
    group.finish();
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
    let executor = DclExecutor::new(move || {
        predicate_state.load(Ordering::Acquire) == RUNNING
    });
    let panic_predicate_state = Arc::clone(&state);
    let catching_executor = DclExecutor::new(move || {
        panic_predicate_state.load(Ordering::Acquire) == RUNNING
    });

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
    group.bench_function(
        format!("{backend}/running/qubit_dcl_catching"),
        |bencher| {
            bencher.iter(|| {
                black_box(catching_executor.run_catching(lock, || {
                    submitted.fetch_add(1, Ordering::Relaxed);
                    Ok::<(), Infallible>(())
                }))
            });
        },
    );

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
    group.bench_function(
        format!("{backend}/shut_down/qubit_dcl_catching"),
        |bencher| {
            bencher.iter(|| {
                let outcome = catching_executor.run_catching(lock, || {
                    submitted.fetch_add(1, Ordering::Relaxed);
                    Ok::<(), Infallible>(())
                });
                black_box(matches!(outcome, Ok(ExecutionOutcome::Success(()))))
            });
        },
    );
}

/// Executes a concurrent DCL round and returns the elapsed wall-clock time.
///
/// All workers wait at the barrier before issuing `iterations` calls, so the
/// elapsed duration includes contention on `lock` but excludes thread creation.
/// `executor`, `lock`, and `state` must remain valid for all scoped workers.
fn run_contention_round<L>(
    executor: &DclExecutor,
    lock: &L,
    worker_count: usize,
    iterations: u64,
    submitted: &AtomicUsize,
) -> Duration
where
    L: Lock + Sync + ?Sized,
{
    let start_barrier = Barrier::new(worker_count + 1);
    let start = thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| {
                start_barrier.wait();
                for _ in 0..iterations {
                    let _ = black_box(executor.run(lock, || {
                        submitted.fetch_add(1, Ordering::Relaxed);
                        Ok::<(), Infallible>(())
                    }));
                }
            });
        }
        let start = std::time::Instant::now();
        start_barrier.wait();
        start
    });
    start.elapsed()
}

/// Executes a concurrent DCL round with a fixed per-worker rejection ratio.
///
/// Each worker owns its state and executor so a state transition before one
/// invocation cannot alter another worker's second check. Updating the state
/// is deliberately part of every ratio measurement; its fixed cost keeps the
/// compared ratios on the same workload shape.
fn run_rejection_ratio_round<L>(
    lock: &L,
    worker_count: usize,
    iterations: u64,
    rejected_percentage: u8,
    submitted: &AtomicUsize,
) -> Duration
where
    L: Lock + Sync + ?Sized,
{
    let start_barrier = Barrier::new(worker_count + 1);
    let start = thread::scope(|scope| {
        for _ in 0..worker_count {
            let state = Arc::new(AtomicU8::new(RUNNING));
            let predicate_state = Arc::clone(&state);
            let executor = DclExecutor::new(move || {
                predicate_state.load(Ordering::Acquire) == RUNNING
            });
            let barrier = &start_barrier;
            scope.spawn(move || {
                barrier.wait();
                for iteration in 0..iterations {
                    let state_value =
                        if iteration % 100 < u64::from(rejected_percentage) {
                            SHUT_DOWN
                        } else {
                            RUNNING
                        };
                    state.store(state_value, Ordering::Release);
                    let _ = black_box(executor.run(lock, || {
                        submitted.fetch_add(1, Ordering::Relaxed);
                        Ok::<(), Infallible>(())
                    }));
                }
            });
        }
        let start = std::time::Instant::now();
        start_barrier.wait();
        start
    });
    start.elapsed()
}

/// Registers concurrent accepted and rejected DCL throughput for one backend.
///
/// Criterion's iteration count becomes the per-worker operation count. The
/// group throughput is therefore configured as the worker count so reported
/// rates represent total executor calls.
fn benchmark_contention_backend<L>(
    criterion: &mut Criterion,
    backend: &str,
    lock: &L,
) where
    L: Lock + Sync + ?Sized,
{
    for worker_count in [1_usize, 2, 4, 8] {
        for (state_name, state_value) in
            [("running", RUNNING), ("shut_down", SHUT_DOWN)]
        {
            let state = Arc::new(AtomicU8::new(state_value));
            let predicate_state = Arc::clone(&state);
            let executor = DclExecutor::new(move || {
                predicate_state.load(Ordering::Acquire) == RUNNING
            });
            let submitted = AtomicUsize::new(0);
            let mut group = criterion.benchmark_group(format!(
                "dcl_contention/{backend}/{state_name}/{worker_count}_workers"
            ));
            group.throughput(Throughput::Elements(worker_count as u64));
            group.bench_function("qubit_dcl", |bencher| {
                bencher.iter_custom(|iterations| {
                    run_contention_round(
                        &executor,
                        lock,
                        worker_count,
                        iterations,
                        &submitted,
                    )
                });
            });
            group.finish();
        }
    }
}

/// Registers concurrent DCL throughput for both supported mutex backends.
fn benchmark_contention(criterion: &mut Criterion) {
    let std_lock = Mutex::new(());
    benchmark_contention_backend(criterion, "std_mutex", &std_lock);
    let parking_lot_lock = ParkingLotMutex::new(());
    benchmark_contention_backend(
        criterion,
        "parking_lot_mutex",
        &parking_lot_lock,
    );
}

/// Registers four-worker throughput measurements at several rejection ratios.
///
/// The worker count is kept fixed so this benchmark isolates the effect of
/// reducing lock acquisition frequency while preserving concurrent callers.
fn benchmark_rejection_ratios_backend<L>(
    criterion: &mut Criterion,
    backend: &str,
    lock: &L,
) where
    L: Lock + Sync + ?Sized,
{
    const WORKER_COUNT: usize = 4;

    for rejected_percentage in [0_u8, 50, 90, 100] {
        let submitted = AtomicUsize::new(0);
        let mut group = criterion.benchmark_group(format!(
            "dcl_rejection_ratio/{backend}/{rejected_percentage}_percent_rejected"
        ));
        group.throughput(Throughput::Elements(WORKER_COUNT as u64));
        group.bench_function("qubit_dcl", |bencher| {
            bencher.iter_custom(|iterations| {
                run_rejection_ratio_round(
                    lock,
                    WORKER_COUNT,
                    iterations,
                    rejected_percentage,
                    &submitted,
                )
            });
        });
        group.finish();
    }
}

/// Registers rejection-ratio throughput measurements for both mutex backends.
fn benchmark_rejection_ratios(criterion: &mut Criterion) {
    let std_lock = Mutex::new(());
    benchmark_rejection_ratios_backend(criterion, "std_mutex", &std_lock);
    let parking_lot_lock = ParkingLotMutex::new(());
    benchmark_rejection_ratios_backend(
        criterion,
        "parking_lot_mutex",
        &parking_lot_lock,
    );
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

criterion_group!(
    benches,
    benchmark_dcl,
    benchmark_predicate_representations,
    benchmark_typed_dcl,
    benchmark_contention,
    benchmark_rejection_ratios,
);
criterion_main!(benches);
