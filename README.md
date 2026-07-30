# Qubit DCL

[![Rust CI](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-dcl/coverage-badge.json)](https://qubit-ltd.github.io/rs-dcl/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-dcl.svg?color=blue)](https://crates.io/crates/qubit-dcl)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

Qubit DCL provides reusable double-checked-locking executors for Rust. It
helps a concurrent application skip work that is already unnecessary without
turning a synchronization protocol into ad-hoc, duplicated lock code. You
provide an atomic or equivalently synchronized gate and choose the lock mode
for each execution; the executor performs a lock-free check, rechecks under
that mode, then runs the task only when both checks succeed.

## Installation

```toml
[dependencies]
qubit-dcl = { version = "0.11", features = ["parking-lot"] }
qubit-lock = "0.13"
parking_lot = "0.12"
```

Qubit DCL requires Rust 1.94 or later. Enable the optional `parking-lot`
feature when using that backend. Applications using only standard-library locks
need no DCL feature:

```toml
qubit-dcl = "0.11"
qubit-lock = { version = "0.13", default-features = false }
```

Qubit DCL does not re-export `qubit_lock::Lock` or lock primitives owned by
other crates. Declare `qubit-lock` and the selected lock backend directly.

## Quick Start

Assume several request handlers may initialize one expensive resource. The
first handler closes an atomic gate while holding the mutex; later handlers
observe the closed gate and return without attempting lock acquisition.

```rust
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use parking_lot::Mutex;
use qubit_dcl::{DoubleCheckedLockExecutor, ExecutionOutcome};

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
        // The task already holds this invocation's mutex.
        gate.store(false, Ordering::Release);
        Ok::<usize, io::Error>(42)
    }
});

assert!(matches!(outcome, ExecutionOutcome::Success(42)));
assert!(matches!(
    executor.run(&lock, || Ok::<(), io::Error>(())),
    ExecutionOutcome::ConditionNotMet
));
```

The first `false` predicate result returns
`ExecutionOutcome::ConditionNotMet` without calling any lock method. A task
error is preserved as `ExecutionOutcome::TaskFailed(E)`.

## Why This Crate Exists

Double-checked locking is simple to describe but easy to implement
inconsistently: a fast path may acquire a lock accidentally, the second check
may be omitted, or a shared read lock may be treated as an at-most-once
mechanism. Qubit DCL makes the execution sequence explicit while keeping lock
ownership and protected data in the calling application.

The lock supplied to `run` is a coordination mode, not executor-owned state.
This lets a reader and writer use the same executor with paired modes from one
RWLock when their tasks follow the same gate protocol.

## What It Provides

- `DoubleCheckedLockExecutor` for a reusable predicate, a caller-selected
  `qubit_lock::Lock`, and structured `ExecutionOutcome` values.
- `LifecycleDoubleCheckedLockExecutor` for workflows that prepare a
  per-invocation token, then commit or roll it back after locked execution.
- `LifecycleOutcome`, `FinalizationOutcome`, `RollbackCause`, `PanicInfo`,
  and `PanicPhase` for inspecting every terminal lifecycle path.
- Optional `catch_panics(true)` support when a caller needs structured panic
  metadata instead of propagation.

It does not own a lock, expose protected data, cache task results, choose the
application's memory ordering, or make a shared lock mode exclusive. A gate
that changes protected state or requires at-most-once execution needs an
exclusive lock mode or a separate uniqueness protocol.

## Learn More

- Read the full [User Guide](doc/user_guide.md) for the synchronization
  contract, read/write coordination, lifecycle tokens, panic handling,
  troubleshooting, and limitations.
- 阅读[中文用户手册](doc/user_guide.zh_CN.md)。
- Browse the [API reference](https://docs.rs/qubit-dcl).
- Read the [0.11 migration guide](doc/user_guide_migration_0_11.md) or
  [中文迁移指南](doc/user_guide_migration_0_11.zh_CN.md).
- 阅读[中文 README](README.zh_CN.md)。

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
