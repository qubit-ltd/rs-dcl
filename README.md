# Qubit DCL

[![Rust CI](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-dcl/coverage-badge.json)](https://qubit-ltd.github.io/rs-dcl/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-dcl.svg?color=blue)](https://crates.io/crates/qubit-dcl)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

Qubit DCL provides reusable synchronous double-checked-lock executors for Rust
applications that use an atomic or equivalently synchronized gate. It removes
repeated check-lock-check-task code while keeping lock ownership, protected
data, and memory-ordering decisions in the application.

A typical use case is lazy initialization: many request handlers may observe
that an expensive resource is still needed, but only the handler that passes
the second check under an exclusive lock should initialize it.

## Installation

Qubit DCL requires Rust 1.94 or later. The standard-library path uses the
backend-neutral `qubit-lock` capability and the `qubit-atomic` gate wrapper:

```toml
[dependencies]
qubit-dcl = "0.12"
qubit-atomic = "0.16"
qubit-lock = { version = "0.13", default-features = false }
```

The `parking-lot` backend is optional. Enable the matching feature in
`qubit-dcl` and `qubit-lock` when the application uses
`parking_lot` locks:

```toml
[dependencies]
qubit-dcl = { version = "0.12", features = ["parking-lot"] }
qubit-atomic = "0.16"
qubit-lock = "0.13"
parking_lot = "0.12"
```

`qubit-atomic` is an application-level choice for the gate; Qubit DCL does not
require it at runtime. `qubit-lock` is the lock capability used by the
executor, and the application declares the concrete lock backend directly.

## Quick Start

This example uses `ArcAtomic<bool>` from `qubit-atomic` and the exclusive
`write_lock()` adapter from `qubit-lock`:

```rust
use qubit_atomic::ArcAtomic;
use qubit_dcl::{DclExecutor, ExecutionOutcome};
use qubit_lock::ReadWriteLock;

let gate = ArcAtomic::new(true);
let rw_lock = std::sync::RwLock::new(());
let write_mode = rw_lock.write_lock();
let executor = DclExecutor::new({
    let gate = gate.clone();
    move || gate.load()
});

let outcome = executor.run(&write_mode, {
    let gate = gate.clone();
    move || {
        gate.store(false);
        Ok::<usize, std::io::Error>(42)
    }
});

assert!(matches!(outcome, ExecutionOutcome::Success(42)));
assert!(matches!(
    executor.run(&write_mode, || Ok::<(), std::io::Error>(())),
    ExecutionOutcome::ConditionNotMet
));
```

The first check is lock-free. The second check and task run under the same
guard selected by `write_lock()`. Once the task closes the gate, later calls
return `ExecutionOutcome::ConditionNotMet` before asking the lock for a guard.
A task error remains available as `ExecutionOutcome::TaskFailed(error)`.

## Two Execution Models

`DclExecutor` is the small, reusable path. Construct it with
`DclExecutor::new(predicate)`, pass a `qubit_lock::Lock` mode to each `run`
call, and choose `run_catching` when a caller needs captured `PanicInfo`.

`LifecycleDclExecutor` is for a per-invocation token that must be prepared
before locking and finalized after the guard is released. Its typestate builder
requires a predicate, a prepare callback, and a valid commit/rollback
combination. Use `run_with_token` when the task must mutate the token; use
`run` when it does not.

The public result model includes:

- `ExecutionOutcome` for basic success, rejected conditions, and task errors.
- `LifecycleOutcome` and `FinalizationOutcome` for successful and unsuccessful
  lifecycle paths.
- `RollbackCause` for the reason supplied to a rollback callback.
- `CapturedLifecycleOutcome`, `CapturedFinalizationOutcome`, `PanicInfo`, and
  `PanicPhase` for panic-aware lifecycle execution.

The complete workflows and result tables are in the
[English User Guide](doc/user_guide.md) and
[中文用户手册](doc/user_guide.zh_CN.md).

## Why This Project Exists

Double-checked locking is easy to describe and easy to duplicate incorrectly:
a fast path may acquire a lock unnecessarily, the second check may be omitted,
or a shared read lock may be mistaken for an at-most-once mechanism. Qubit DCL
makes the execution order explicit:

```text
initial predicate -> lock selected by this call -> second predicate -> task
```

The executor owns neither the lock nor its protected data. A single executor
can therefore be reused with compatible modes from one read-write lock, while
the application retains control of the data captured by each task.

## Contract and Limits

- The predicate must read an atomic or equivalently synchronized gate. The
  usual pairing is an Acquire load with Release stores; `ArcAtomic` supplies
  those defaults.
- The predicate must not acquire the same underlying lock and should not block.
- `write_lock()` or another exclusive mode is required when the task changes
  the gate or protected protocol, consumes work, or needs serialized execution.
- `read_lock()` is valid only for a read-only task whose conflicting writers use
  the paired write mode of the same underlying lock. Shared calls may overlap;
  they do not provide at-most-once execution.
- External code that changes the gate or protected state must coordinate through
  the same underlying lock used by conflicting executor calls.
- Qubit DCL does not own a lock, expose protected data, cache task results,
  select the application's memory ordering, or turn a non-atomic predicate into
  a synchronized one.

## Related Qubit Crates

- [rs-atomic](https://github.com/qubit-ltd/rs-atomic) provides convenient
  atomic values and shared-owner wrappers such as `ArcAtomic<bool>` for gates.
- [rs-lock](https://github.com/qubit-ltd/rs-lock) provides backend-neutral
  synchronous lock capabilities, including `ReadWriteLock`, `read_lock()`, and
  `write_lock()`.
- [rs-dcl](https://github.com/qubit-ltd/rs-dcl) combines a synchronized gate
  with a caller-selected lock mode and double-checked execution policy.

## Learn More

- Read the full [English User Guide](doc/user_guide.md).
- 阅读[中文用户手册](doc/user_guide.zh_CN.md)。
- Browse the [API reference](https://docs.rs/qubit-dcl).
- 阅读[中文 README](README.zh_CN.md)。
- See the related [rs-atomic](https://github.com/qubit-ltd/rs-atomic) and
  [rs-lock](https://github.com/qubit-ltd/rs-lock) repositories.

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
