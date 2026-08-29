# Qubit DCL

[![Rust CI](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-dcl/coverage-badge.json)](https://qubit-ltd.github.io/rs-dcl/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-dcl.svg?color=blue)](https://crates.io/crates/qubit-dcl)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

Qubit DCL provides reusable synchronous double-checked execution for work that
is usually unnecessary but must be serialized when it becomes necessary. It
keeps the check-lock-check-task sequence in one place while the application
continues to own the gate, coordination lock, business data, and side effects.

## Installation

Qubit DCL requires Rust 1.94 or later. A standard-library gate and mutex need
only the following application dependency:

```toml
[dependencies]
qubit-dcl = "0.13"
```

No atomic helper crate or direct `qubit-lock` dependency is required for this
minimal path.

## Quick Start

Suppose a configuration change marks a service-routing cache as stale. Several
request threads can notice the stale flag at once, but the configuration should
be loaded only once. The example uses only Qubit DCL and standard-library
synchronization primitives:

```rust
use std::{
    convert::Infallible,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
};

use qubit_dcl::{DclExecutor, ExecutionOutcome};

let refresh_needed = Arc::new(AtomicBool::new(true));
let refresh_lock = Arc::new(Mutex::new(()));
let routes = Arc::new(RwLock::new(Vec::<String>::new()));
let load_count = Arc::new(AtomicUsize::new(0));

let executor = Arc::new(DclExecutor::new({
    let refresh_needed = Arc::clone(&refresh_needed);
    move || refresh_needed.load(Ordering::Acquire)
}));

let workers = (0..2)
    .map(|_| {
        let executor = Arc::clone(&executor);
        let refresh_lock = Arc::clone(&refresh_lock);
        let refresh_needed = Arc::clone(&refresh_needed);
        let routes = Arc::clone(&routes);
        let load_count = Arc::clone(&load_count);
        thread::spawn(move || {
            executor.run(&refresh_lock, || {
                load_count.fetch_add(1, Ordering::Relaxed);
                *routes.write().expect("route cache should not be poisoned") =
                    vec!["api-v2.internal:443".to_owned()];
                refresh_needed.store(false, Ordering::Release);
                Ok::<(), Infallible>(())
            })
        })
    })
    .collect::<Vec<_>>();

let outcomes = workers
    .into_iter()
    .map(|worker| worker.join().expect("refresh worker should not panic"))
    .collect::<Vec<_>>();

assert_eq!(
    outcomes
        .iter()
        .filter(|outcome| matches!(outcome, ExecutionOutcome::Success(())))
        .count(),
    1,
);
assert_eq!(
    outcomes
        .iter()
        .filter(|outcome| matches!(outcome, ExecutionOutcome::ConditionNotMet))
        .count(),
    1,
);
assert_eq!(load_count.load(Ordering::Relaxed), 1);
assert_eq!(
    *routes.read().expect("route cache should not be poisoned"),
    ["api-v2.internal:443"],
);
```

One worker refreshes the cache. The other either observes the closed gate on
the fast path or reaches the second check after waiting for the mutex; in both
cases it skips the expensive load.

## How It Works

```text
if the initial predicate is false:
    return ConditionNotMet without acquiring the lock

acquire the lock supplied by the caller

if the second predicate is false while holding the lock:
    return ConditionNotMet without running the task

run the task while holding the same lock guard

if the task succeeds:
    return Success
otherwise:
    return TaskFailed
```

The initial predicate avoids lock acquisition when no work is needed, while
the second check closes the race between the first observation and acquiring
the lock. Returning from either locked branch releases the guard through RAII.

## Choosing an Executor

- `DclExecutor` runs a task under a caller-supplied lock when both predicate
  checks pass. It returns `ExecutionOutcome`, preserving task errors without
  wrapping them.
- `LifecycleDclExecutor` adds a per-invocation token: prepare it before lock
  acquisition, use or mutate it during the task, release the guard, and then
  commit or roll it back. Its typestate builder prevents incomplete
  finalization policies.

Use `run_catching` or `run_with_token_catching` when the caller needs structured
`PanicInfo` instead of normal panic propagation.

## Contract and Limits

- The predicate must read an atomic or equivalently synchronized gate. An
  Acquire load paired with Release stores is a common starting protocol.
- The predicate must not acquire the same underlying lock and should not block.
- Use `Mutex`, `write_lock()`, or another exclusive mode when the task changes
  the gate, consumes work, modifies the protected protocol, or requires
  at-most-once execution.
- A shared `read_lock()` permits overlapping tasks and does not provide
  at-most-once execution.
- Code outside the executor that changes the gate or conflicting protected
  state must coordinate through the same underlying lock.
- Qubit DCL does not own locks or business data, cache task results, retry
  failures, choose memory ordering, or make an unsynchronized predicate safe.

## Optional Integrations

The minimal example above needs none of these additions:

- Add [`qubit-atomic`](https://github.com/qubit-ltd/rs-atomic) only when its
  atomic values or shared-owner wrappers, such as `ArcAtomic<bool>`, are useful
  to the application.
- Add [`qubit-lock`](https://github.com/qubit-ltd/rs-lock) directly only when
  application code calls capabilities such as `ReadWriteLock::read_lock()` or
  `write_lock()`. Qubit DCL already uses it internally.
- To pass a `parking_lot` lock directly, enable Qubit DCL's matching feature and
  add the selected backend:

  ```toml
  [dependencies]
  qubit-dcl = { version = "0.13", features = ["parking-lot"] }
  parking_lot = "0.12"
  ```

## Learn More

- Follow the complete workflows in the [English User Guide](doc/user_guide.md)
  or [中文用户手册](doc/user_guide.zh_CN.md).
- Browse the [API reference](https://docs.rs/qubit-dcl).
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
