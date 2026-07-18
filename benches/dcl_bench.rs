// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Criterion benchmarks for DCL fast and locked paths.

use std::{
    convert::Infallible,
    hint::black_box,
    sync::{
        Arc,
        atomic::{
            AtomicBool,
            Ordering,
        },
    },
};

use criterion::{
    Criterion,
    criterion_group,
    criterion_main,
};
use qubit_dcl::DoubleCheckedLockExecutor;
use qubit_lock::{
    ArcMutex,
    Lock,
};

/// Benchmarks raw predicates, executor paths, and equivalent handwritten DCL.
///
/// # Parameters
///
/// * `criterion` - Criterion benchmark registry.
fn benchmark_dcl(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("double_checked_lock");

    let raw_gate = AtomicBool::new(false);
    group.bench_function("raw_atomic_predicate", |bencher| {
        bencher.iter(|| black_box(raw_gate.load(Ordering::Acquire)));
    });

    let false_gate = Arc::new(AtomicBool::new(false));
    let false_executor = DoubleCheckedLockExecutor::builder(ArcMutex::new(()))
        .when({
            let false_gate = Arc::clone(&false_gate);
            move || false_gate.load(Ordering::Acquire)
        })
        .build();
    group.bench_function("executor_fast_false", |bencher| {
        bencher.iter(|| {
            black_box(false_executor.run(|| Ok::<usize, Infallible>(1)))
        });
    });

    let success_executor =
        DoubleCheckedLockExecutor::builder(ArcMutex::new(()))
            .when(|| true)
            .build();
    group.bench_function("executor_uncontended_success", |bencher| {
        bencher.iter(|| {
            black_box(
                success_executor.run(|| Ok::<usize, Infallible>(black_box(1))),
            )
        });
    });

    let handwritten_gate = AtomicBool::new(true);
    let handwritten_lock = ArcMutex::new(());
    group.bench_function("handwritten_uncontended_dcl", |bencher| {
        bencher.iter(|| {
            let result = if handwritten_gate.load(Ordering::Acquire) {
                handwritten_lock.with_write(|_| {
                    if handwritten_gate.load(Ordering::Acquire) {
                        black_box(1)
                    } else {
                        0
                    }
                })
            } else {
                0
            };
            black_box(result)
        });
    });

    group.finish();
}

criterion_group!(benches, benchmark_dcl);
criterion_main!(benches);
