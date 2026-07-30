# Qubit DCL User Guide

[中文](user_guide.zh_CN.md) · [README](../README.md) ·
[API reference](https://docs.rs/qubit-dcl) ·
[0.11 migration guide](user_guide_migration_0_11.md)

This guide explains how to use `qubit-dcl` 0.11 to avoid unnecessary
serialized work while preserving a correct synchronization protocol. It is for
Rust developers who already have an atomic or equivalently synchronized gate
and need a reusable double-checked execution policy around it.

## Conceptual Model

A `DoubleCheckedLockExecutor` stores a predicate. A call to `run` follows this
sequence:

```text
read synchronized gate
  false ──> ExecutionOutcome::ConditionNotMet
  true  ──> acquire this call's Lock
               └─> read the same gate again
                     false ──> ConditionNotMet
                     true  ──> run task under that guard
```

The first check is lock-free. The second check and task share one guard from
the `qubit_lock::Lock` supplied to that invocation. The executor owns neither
the lock nor its protected data, so the same executor can be reused with
different calls and compatible lock modes.

The predicate must read an atomic or equivalently synchronized gate. The usual
protocol is an Acquire load paired with Release stores. The predicate must not
acquire the same underlying lock and should not block.

## Scenario: Initialize Expensive State Once

Suppose several request handlers may lazily initialize an expensive resource.
The first handler that observes an open gate initializes it and closes the
gate. Later handlers should return without acquiring the mutex.

Add the crate and a lock backend:

```toml
[dependencies]
qubit-dcl = { version = "0.11", features = ["parking-lot"] }
qubit-lock = "0.13"
parking_lot = "0.12"
```

Enable the optional `parking-lot` feature for the matching `qubit-lock`
support. For only standard-library locks, no DCL feature is needed:

```toml
qubit-dcl = "0.11"
qubit-lock = { version = "0.13", default-features = false }
```

Build one executor and pass the mutex on each call:

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
        // This task already holds the mutex selected for this invocation.
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

The first successful call performs the task while holding the mutex. The
second call sees the closed gate during its lock-free check, so it does not
call the lock implementation. A `TaskFailed(E)` outcome preserves the
original task error; it is not converted into a library-wide error type.

## Choosing a Lock Mode

`Lock` represents an acquisition mode, not necessarily exclusivity.

`run` deliberately accepts `Lock` rather than `ExclusiveLock` and follows the
same control flow for shared and exclusive modes. This lets callers pass either
the read or write mode from one RWLock. Rust cannot prove that the task closure
is read-only, so a caller supplying a shared mode must uphold that contract.

Use an exclusive mode, such as a mutex or a write-mode adapter, when the task
changes the gate or protected state, consumes work, initializes something only
once, or otherwise needs serialization. An independent compare-and-exchange
protocol is also valid when it establishes the required unique winner.

A shared read mode is valid only when all of the following hold:

1. The task is read-only with respect to the protected protocol.
2. Every conflicting writer uses the paired write mode of the same underlying
   lock.
3. The caller does not require at-most-once task execution.

Several shared calls can pass the second check and run concurrently. This is
intentional, not a duplicate-execution bug.

One executor may receive paired modes from the same read-write lock. The
actions may capture different data, provided their shared gate protocol uses
the same underlying lock for conflicting updates:

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
let read_value = Arc::new(AtomicUsize::new(42));
let write_value = Arc::new(AtomicUsize::new(0));
let executor = DoubleCheckedLockExecutor::builder()
    .when({
        let gate = Arc::clone(&gate);
        move || gate.load(Ordering::Acquire)
    })
    .build();

let read_mode = lock.read_lock();
assert!(matches!(
    executor.run(&read_mode, {
        let read_value = Arc::clone(&read_value);
        move || Ok::<usize, std::io::Error>(read_value.load(Ordering::Acquire))
    }),
    ExecutionOutcome::Success(42)
));

let write_mode = lock.write_lock();
assert!(matches!(
    executor.run(&write_mode, {
        let gate = Arc::clone(&gate);
        let write_value = Arc::clone(&write_value);
        move || {
            write_value.store(43, Ordering::Release);
            gate.store(false, Ordering::Release);
            Ok::<(), std::io::Error>(())
        }
    }),
    ExecutionOutcome::Success(())
));
```

External code that changes the gate must acquire the same underlying lock used
by the conflicting `run` calls. An atomic gate supplies visibility; it does not
replace lock-based coordination. In particular, `ptr::read_volatile` is for
volatile memory such as MMIO and is not synchronization.

## Lifecycle Execution

Use `LifecycleDoubleCheckedLockExecutor` when each invocation needs a token
created after the first check and finalized after locked execution. Its
typestate builder only exposes these valid configurations:

- `prepare -> commit -> rollback -> build`
- `prepare -> commit -> no_rollback -> build`
- `prepare -> no_commit -> rollback -> build`

There is deliberately no `no_commit + no_rollback` configuration. `prepare`
runs before lock acquisition. The task mutates its invocation-local token
under the executor guard. After that guard is released, `commit` consumes a
successful token and `rollback` consumes a token from an unsuccessful path.

```rust
use std::{
    io,
    sync::{Arc, atomic::{AtomicBool, Ordering}},
};

use qubit_dcl::{
    FinalizationOutcome,
    LifecycleDoubleCheckedLockExecutor,
    LifecycleOutcome,
    RollbackCause,
};

let lock = std::sync::Mutex::new(());
let gate = Arc::new(AtomicBool::new(true));
let executor = LifecycleDoubleCheckedLockExecutor::builder()
    .when({
        let gate = Arc::clone(&gate);
        move || gate.load(Ordering::Acquire)
    })
    .prepare(|| Ok::<Vec<&'static str>, io::Error>(vec!["prepared"]))
    .commit(|token| {
        assert_eq!(token, ["prepared", "task"]);
        Ok::<(), io::Error>(())
    })
    .rollback(|token, cause| {
        assert!(!token.is_empty());
        if let RollbackCause::TaskFailed(error) = cause {
            eprintln!("task failed: {error}");
        }
        Ok::<(), io::Error>(())
    })
    .build();

let outcome = executor.run_with_token(&lock, |token| {
    token.push("task");
    Ok::<usize, io::Error>(token.len())
});
assert!(matches!(
    outcome,
    LifecycleOutcome::TaskSucceeded {
        value: 2,
        commit: FinalizationOutcome::Succeeded,
    }
));
```

Use `run` when the task does not need the token. Use `run_with_token` only
when the task must mutate it. Callback objects are shared and can be invoked
concurrently, so any mutable state captured by prepare, commit, or rollback
needs its own synchronization.

## Outcomes and Panic Handling

The basic executor returns `ExecutionOutcome<R, E>`:

| Variant | Meaning |
| --- | --- |
| `Success(R)` | The task ran under the selected guard and returned a value. |
| `ConditionNotMet` | Either condition check returned false. |
| `TaskFailed(E)` | The task returned its unchanged error. |
| `Panicked(PanicInfo)` | A configured panic boundary captured a panic. |

The lifecycle executor returns one exhaustive `LifecycleOutcome<R, E, C>`.
It distinguishes the first check, prepare, second check, task failure, task
success, and panic paths. Finalization fields use
`FinalizationOutcome<C>`: `NotRequired`, `Succeeded`, `Failed(C)`, or
`Panicked(PanicInfo)`. Commit failure never erases task success; rollback
failure never erases the original task error or panic.

Panic capture is disabled by default, so predicate, lock, callback, and task
panics propagate normally. Enable `catch_panics(true)` to receive
`PanicInfo` with a `PanicPhase`. The capture boundary is outside the RAII
guard's scope: standard locks preserve normal poisoning behavior, while
parking-lot retains its normal non-poisoning behavior. A panic while explicitly
releasing a normally completed guard is classified as `PanicPhase::LockRelease`.

Capture works only with an unwinding panic strategy. With `panic = "abort"`,
the process terminates before an outcome can be returned or unwind-based
rollback can run. `PanicInfo` provides classification and payload transport,
not recovery or a transaction boundary: side effects completed before the
panic remain, and lifecycle rollback can itself fail or panic. A captured
outcome does not prove that application invariants were restored; verify or
reestablish them before reusing the affected state.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| A task runs more than once. | Use an exclusive mode or an explicit atomic uniqueness protocol; a shared mode permits overlap. |
| A task runs after an external state transition. | Ensure that transition uses the same underlying lock and a Release store paired with the predicate's Acquire load. |
| A deadlock occurs. | Confirm the predicate does not acquire the executor's lock and callbacks do not accidentally reacquire it. |
| Rollback cannot explain failure. | Match the `RollbackCause` and retain the original error/panic from `LifecycleOutcome`. |
| A standard mutex is poisoned. | Check whether panic capture crossed a standard-lock guard; poisoning is intentionally preserved. |

## Limitations and Best Practices

- This crate is synchronous and accepts a synchronous `qubit_lock::Lock`.
- It does not discover locks, own protected data, cache task results, or select
  a memory-ordering protocol for the application.
- It does not make a non-atomic predicate safe or turn a shared lock mode into
  exclusive execution.
- Keep predicates small, non-blocking, and free of acquisition of the same
  lock. Treat executor callbacks as reusable concurrent functions.
- Prefer the basic executor until prepare/commit/rollback ownership is a real
  requirement.

## Further Reading

- Return to the [English README](../README.md) or [中文 README](../README.zh_CN.md).
- Read the [API reference](https://docs.rs/qubit-dcl).
- For upgrades from 0.10, read the
  [migration guide](user_guide_migration_0_11.md).
