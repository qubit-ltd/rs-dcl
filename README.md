# Qubit DCL

[![Rust CI](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-dcl/coverage-badge.json)](https://qubit-ltd.github.io/rs-dcl/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-dcl.svg?color=blue)](https://crates.io/crates/qubit-dcl)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

`qubit-dcl` packages the double-checked locking design pattern as reusable
executors. A lock-free predicate first rejects unnecessary work without taking
a lock. If it succeeds, the executor obtains the generic `qubit_lock::Lock`
passed to that call, checks the same predicate again, and runs an arbitrary
task inside that lock.

Version 0.11 is a deliberately breaking redesign. Executors retain the
predicate and callbacks, while every `run` call supplies its lock. One executor
can therefore coordinate through different lock implementations or instances.

## Concurrency contract

Correct DCL use requires all three rules below:

1. The predicate reads an atomic or equivalently synchronized gate. The usual
   protocol is an Acquire load paired with Release stores.
2. The predicate must not acquire the executor's lock and should not block.
3. The selected lock mode must match the task's semantics. `Lock` represents an
   acquisition mode, not necessarily an exclusive one. The executor holds one
   guard from that mode across the second check and task.

A shared read mode is valid when the task only reads state covered by the
protocol, every conflicting writer uses the paired write mode of the same
underlying lock, and callers do not require at-most-once task execution.
Multiple invocations may then pass the second check and run concurrently.

A task that changes the gate or protected state, consumes work, performs
one-time initialization, or otherwise requires serialization must use an
`ExclusiveLock` mode such as a mutex or write-mode adapter. An independent
compare-and-exchange protocol is also valid when it establishes the required
unique winner.

An atomic gate provides visibility; the selected lock mode supplies the
corresponding shared/exclusive coordination. `ptr::read_volatile` is intended
for volatile memory such as MMIO and is not a replacement for atomic
synchronization.

## Basic executor

Import lock types directly from their owning crate. The following example
deliberately uses a mutex, so its task runs through an exclusive acquisition
mode and may close the gate inside that guard:

```rust
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use qubit_dcl::{DoubleCheckedLockExecutor, ExecutionOutcome};
use parking_lot::Mutex;

let lock = Mutex::new(());
let gate = Arc::new(AtomicBool::new(true));
let executor = DoubleCheckedLockExecutor::builder()
    .when({
        let gate = Arc::clone(&gate);
        move || gate.load(Ordering::Acquire)
    })
    .build();

let outcome = executor.run(&lock, {
    let gate = Arc::clone(&gate);
    move || {
        // The second check has succeeded and this task already holds the
        // executor lock, so it can close the gate without reacquiring it.
        gate.store(false, Ordering::Release);
        Ok::<usize, io::Error>(42)
    }
});

assert!(matches!(outcome, ExecutionOutcome::Success(42)));
```

The first `false` result returns `ExecutionOutcome::ConditionNotMet` without
calling any lock method. A task error is returned unchanged as
`ExecutionOutcome::TaskFailed(E)`.

Read-only tasks may use a read-mode adapter. Multiple executions can overlap,
while writers using the paired write mode remain excluded:

```rust
use std::sync::{
    Arc,
    RwLock,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use qubit_dcl::{DoubleCheckedLockExecutor, ExecutionOutcome};
use qubit_lock::ReadWriteLock;

let lock = RwLock::new(());
let gate = Arc::new(AtomicBool::new(true));
let value = Arc::new(AtomicUsize::new(42));
let executor = DoubleCheckedLockExecutor::builder()
    .when({
        let gate = Arc::clone(&gate);
        move || gate.load(Ordering::Acquire)
    })
    .build();

let read_mode = lock.read_lock();
let outcome = executor.run(&read_mode, {
    let value = Arc::clone(&value);
    move || Ok::<usize, std::io::Error>(value.load(Ordering::Acquire))
});
assert!(matches!(outcome, ExecutionOutcome::Success(42)));

let _writer = lock.write();
value.store(43, Ordering::Release);
```

When another path updates the gate, it must use the same lock object:

```rust
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use qubit_dcl::DoubleCheckedLockExecutor;
use qubit_lock::Lock;

let lock = Arc::new(parking_lot::Mutex::new(()));
let gate = Arc::new(AtomicBool::new(true));
let executor = DoubleCheckedLockExecutor::builder()
    .when({
        let gate = Arc::clone(&gate);
        move || gate.load(Ordering::Acquire)
    })
    .build();

let guard = Lock::lock(&lock);
gate.store(false, Ordering::Release);
drop(guard);

let outcome = executor.run(&lock, || Ok::<(), std::io::Error>(()));
assert!(matches!(
    outcome,
    qubit_dcl::ExecutionOutcome::ConditionNotMet
));
```

## Lifecycle executor

`LifecycleDoubleCheckedLockExecutor` runs prepare after the first check and
before locking. Each invocation receives its own token `P`. After the task and
lock release, that token is consumed by commit or rollback.

```rust
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use qubit_dcl::{
    ExecutionOutcome,
    LifecycleDoubleCheckedLockExecutor,
    PreparationOutcome,
    RollbackCause,
};

let lock = std::sync::Mutex::new(());
let gate = Arc::new(AtomicBool::new(true));
let executor = LifecycleDoubleCheckedLockExecutor::builder()
    .when({
        let gate = Arc::clone(&gate);
        move || gate.load(Ordering::Acquire)
    })
    .catch_panics(true)
    .prepare(|| Ok::<Vec<&'static str>, io::Error>(vec!["prepared"]))
    .commit(|token| {
        assert_eq!(token, ["prepared", "task"]);
        Ok::<(), io::Error>(())
    })
    .rollback(|token, cause| {
        assert!(!token.is_empty());
        match cause {
            RollbackCause::ConditionNotMet => {}
            RollbackCause::TaskFailed(error) => eprintln!("task failed: {error}"),
            RollbackCause::Panicked(panic) => {
                eprintln!("panic in {:?}", panic.phase());
            }
        }
        Ok::<(), io::Error>(())
    })
    .build();

let report = executor.run_with_token(&lock, |token| {
    token.push("task");
    Ok::<usize, io::Error>(token.len())
});

assert!(matches!(
    report.execution(),
    ExecutionOutcome::Success(2)
));
assert!(matches!(
    report.preparation(),
    PreparationOutcome::Committed
));
```

The typestate builder permits exactly three lifecycle combinations:

- `prepare -> commit -> rollback -> build`
- `prepare -> commit -> no_rollback -> build`
- `prepare -> no_commit -> rollback -> build`

There is intentionally no `no_commit + no_rollback` combination. When no token
data is needed, prepare can return `()` and the caller can use `run` rather than
`run_with_token`.

## Outcome and panic semantics

`ExecutionReport<R, E, C>` retains two independent axes:

- `ExecutionOutcome<R, E>` reports condition checks, task success/error, or a
  captured panic.
- `PreparationOutcome<C>` reports prepare, commit, rollback, or an explicitly
  unnecessary finalizer.

A commit failure never erases task success, and a rollback failure never erases
the task error or panic that triggered it. `RollbackCause::TaskFailed` borrows
the original error during rollback while the report retains its owned value.

Panic capture is disabled by default. With `.catch_panics(true)`, panic metadata
includes the precise `PanicPhase` and original payload. The capture boundary is
outside the RAII guard's scope, so a standard-library lock observes unwinding
and is poisoned normally; parking-lot locks retain their normal non-poisoning
behavior.

## Installation

```toml
[dependencies]
qubit-dcl = "0.11"
qubit-lock = "0.11"
```

`qubit-dcl` does not re-export `Lock` or lock primitives owned by other crates.
Declare `qubit-lock` and the selected lock backend directly.

## Migration from 0.10

Version 0.11 moves locks from builder state to each executor call and adopts the
data-independent `qubit_lock::Lock` trait. See the
[0.11 migration guide](doc/user_guide_migration_0_11.md) for the complete
mapping.

## Testing

```bash
# Run tests with the default feature set
cargo test

# Run tests with all declared features
cargo test --all-features

# Project CI checks
./ci-check.sh

# Check code coverage
./coverage.sh
```

## License

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the
full license text.

## Contributing

Contributions are welcome. Please follow the Rust API guidelines, keep public
API documentation and tests current, and run `./align-ci.sh` to format code and
`./ci-check.sh` to satisfy CI requirements before submitting a pull request.

## Author

**Haixing Hu** - *Qubit Co. Ltd.*

Repository: [https://github.com/qubit-ltd/rs-dcl](https://github.com/qubit-ltd/rs-dcl)
