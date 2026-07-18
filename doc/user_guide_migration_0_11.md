# Migrating qubit-dcl from 0.10 to 0.11

Version 0.11 separates reusable DCL policy from lock ownership. Executors keep
the predicate and lifecycle callbacks; callers provide a data-independent
`qubit_lock::Lock` to each invocation.

Before:

```rust,ignore
let executor = DoubleCheckedLockExecutor::builder(lock)
    .when(predicate)
    .build();
let outcome = executor.run(task);
```

After:

```rust,ignore
let executor = DoubleCheckedLockExecutor::builder()
    .when(predicate)
    .build();
let outcome = executor.run(&lock, task);
```

Apply the same change to lifecycle execution:

- `LifecycleDoubleCheckedLockExecutor::builder(lock)` becomes `builder()`.
- `run(task)` becomes `run(&lock, task)`.
- `run_with_token(task)` becomes `run_with_token(&lock, task)`.

The removed `ArcMutex`, `ArcRwLock`, and related wrappers are not replaced in
`qubit-dcl`. Use native `std`, `parking_lot`, or Tokio lock types supported by
`qubit-lock`. A read-write lock must select a mode explicitly through its
`read_lock()` or `write_lock()` adapter before it can be passed as `Lock`.

The predicate remains a zero-argument, thread-safe callback. It must read an
atomic or equivalently synchronized gate. Every external transition that must
exclude the task must acquire the exact same underlying lock passed to `run`.

