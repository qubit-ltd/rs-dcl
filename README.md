# Qubit DCL

[![Rust CI](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-dcl/coverage-badge.json)](https://qubit-ltd.github.io/rs-dcl/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-dcl.svg?color=blue)](https://crates.io/crates/qubit-dcl)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

A standalone crate for **double-checked locking** over generic lock handles. It packages the usual “test outside the lock, lock, test again, run the task” sequence into a reusable `DoubleCheckedLockExecutor`, with an optional **prepare / rollback / commit** pipeline and structured results.

The crate re-exports [`ArcMutex`](https://crates.io/crates/qubit-lock) and the [`Lock`](https://crates.io/crates/qubit-lock) trait from `qubit-lock` so a typical app can depend on `qubit-dcl` alone.

## Features

- **`DoubleCheckedLockExecutor`**: one builder-built executor, many invocations; integrates with the [`qubit-function`](https://crates.io/crates/qubit-function) `Tester` and runnable traits.
- **`DoubleCheckedLock`**: one-shot convenience entry for `on(...).when(...).call*` style execution without keeping an executor variable.
- **Double-checked flow**: first condition check without the lock, optional pre-lock prepare, write lock, second check, then task; after the lock is released, optional prepare commit or rollback.
- **Execution API**: `call` / `execute` (no direct `&mut T` in the closure) and `call_with` / `execute_with` (mutable access to protected data).
- **Typed outcomes**: `ExecutionContext` and `ExecutionResult` distinguish success, “condition not met,” task failure, and prepare finalization failures (`ExecutorError`).
- **Logging hooks** via `log` and configurable `ExecutionLogger` on the builder for unmet conditions and prepare-step failures; each event can also be disabled from the builder chain.

## How it works

1. The condition **tester runs twice** (outside the lock, then again under the write lock). Anything the first read relies on must remain safe without this executor’s lock (for example atomics with appropriate orderings).
2. If the first test passes, an optional **prepare** runnable may run; then the lock is taken, the second test runs, and the user task runs with `&mut T` if applicable.
3. If prepare ran successfully, after releasing the lock the executor may run **commit** on full success, or **rollback** when the inner check or task did not succeed.

Panics from the tester, prepare callbacks, or task are not caught by default. Enable capture with `.catch_panics()`, or use `.with_panic_capture(flag)` when the setting comes from a boolean. Built executors also support `.with_panic_capture(flag)` and return a reconfigured executor. Tester and task panics become `ExecutorError::Panic`; prepare lifecycle panics become `PrepareFailed`, `PrepareCommitFailed`, or `PrepareRollbackFailed`. When prepare already succeeded, rollback can still run for captured task or second-check panics. When cloned executors run concurrently, several calls may complete prepare before one call wins the second condition check; losing calls run prepare rollback if it is configured.

## Installation

```toml
[dependencies]
qubit-dcl = "0.9"
```

`qubit-dcl` already depends on `qubit-lock` and re-exports `ArcMutex` and `Lock`; add a direct `qubit-lock` dependency only if you use types beyond those re-exports.

### Public API paths

Import the lock trait from the crate root:

```rust
use qubit_dcl::Lock;
```

The old compatibility path `qubit_dcl::lock::Lock` is no longer provided. Use the root re-export above, or import `qubit_lock::Lock` directly when your code intentionally depends on `qubit-lock`.

## Quick start

```rust
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use qubit_dcl::{DoubleCheckedLockExecutor, ArcMutex, Lock, ExecutionResult};

fn main() {
    let data = ArcMutex::new(10);
    let skip = Arc::new(AtomicBool::new(false));

    let executor = DoubleCheckedLockExecutor::builder()
        .on(data.clone())
        .when({
            let skip = skip.clone();
            move || !skip.load(Ordering::Acquire)
        })
        .build();

    let updated = executor
        .call_with(|value: &mut i32| {
            *value += 5;
            Ok::<i32, std::io::Error>(*value)
        })
        .get_result();

    assert!(matches!(updated, ExecutionResult::Success(15)));
    assert_eq!(data.read(|value| *value), 15);
}
```

### Side-effect–only run (`finish`)

For `execute` or `call` with no meaningful return value, you can use [`ExecutionContext::finish`](https://docs.rs/qubit-dcl) on `ExecutionContext<(), E>` to get a `bool` success:

```rust
use qubit_dcl::{DoubleCheckedLockExecutor, ArcMutex};

let data = ArcMutex::new(());
let ok = DoubleCheckedLockExecutor::builder()
    .on(data)
    .when(|| true)
    .build()
    .execute(|| Ok::<(), std::io::Error>(()))
    .finish();
assert!(ok);
```

`finish()` is intentionally lossy: both condition-not-met and execution failure
return `false`. Use `try_finish()` when you need to preserve task or prepare
errors:

```rust
use qubit_dcl::{DoubleCheckedLockExecutor, ArcMutex};

let data = ArcMutex::new(());
let ok = DoubleCheckedLockExecutor::builder()
    .on(data)
    .when(|| true)
    .build()
    .execute(|| Ok::<(), std::io::Error>(()))
    .try_finish()
    .expect("execution should not fail");
assert!(ok);
```

### One-shot convenience (`DoubleCheckedLock`)

When you do not need to keep a reusable executor, use `DoubleCheckedLock` for a shorter chain:

```rust
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use qubit_dcl::{DoubleCheckedLock, ArcMutex, ExecutionResult, Lock};

let data = ArcMutex::new(10);
let skip = Arc::new(AtomicBool::new(false));

let updated = DoubleCheckedLock::on(data.clone())
    .when({
        let skip = skip.clone();
        move || !skip.load(Ordering::Acquire)
    })
    .call_with(|value: &mut i32| {
        *value += 5;
        Ok::<i32, std::io::Error>(*value)
    })
    .get_result();

assert!(matches!(updated, ExecutionResult::Success(15)));
assert_eq!(data.read(|value| *value), 15);
```

### Example program

A runnable sample is under `examples/double_checked_lock_executor_demo.rs`:

```bash
cargo run --example double_checked_lock_executor_demo
```

## Builder API (summary)

- Start with `DoubleCheckedLockExecutor::builder()`.
- Attach a lock: `.on(lock)` where `L: Lock<T>`.
- Set the double-checked condition: `.when(tester)`.
- Configure panic capture with `.catch_panics()`, `.with_panic_capture(flag)`, or `.disable_catch_panics()`.
- Optionally: `.prepare`, `.rollback_prepare`, `.commit_prepare` for the prepare pipeline.
- Configure diagnostics with `.log_unmet_condition`, `.log_prepare_failure`, `.log_prepare_commit_failure`, `.log_prepare_rollback_failure`; disable them with the matching `.disable_*_logging` methods.
- Finish with `.build()`.

## Project layout

- `src/double_checked`: executor, builders, `ExecutionContext`, `ExecutionResult`, errors, and logging.
- `tests/double_checked` and `tests/docs`: unit and README consistency tests.

## Quality checks

```bash
cargo +nightly fmt -- --check --config-path .rs-ci/rustfmt.toml
cargo clippy --all-targets --all-features -- -D warnings
cargo test
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
./align-ci.sh
./ci-check.sh
./coverage.sh json
```

## License

Apache-2.0
