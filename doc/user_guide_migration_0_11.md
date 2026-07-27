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

The lifecycle result model is also replaced rather than compatibility-aliased:

- `ExecutionReport<R, E, C>` and `PreparationOutcome<C>` are removed.
- `LifecycleDoubleCheckedLockExecutor::run` and `run_with_token` now return one
  exhaustive `LifecycleOutcome<R, E, C>`.
- Commit and rollback status is represented by
  `FinalizationOutcome<C>`.
- The basic executor continues to return `ExecutionOutcome<R, E>`, but its
  lifecycle-only `NotExecuted` variant is removed.

The principal state mapping is:

| Previous report state | New lifecycle outcome |
| --- | --- |
| Initial condition not met | `InitialConditionNotMet` |
| Initial condition check panicked | `InitialConditionCheckPanicked` |
| `NotExecuted` + prepare failure/panic | `PrepareFailed` / `PreparePanicked` |
| Task success + commit state | `TaskSucceeded { value, commit }` |
| Second condition not met + rollback state | `SecondConditionNotMet { rollback }` |
| Task failure + rollback state | `TaskFailed { error, rollback }` |
| Locked execution panic + rollback state | `ExecutionPanicked { panic, rollback }` |

Within the nested commit or rollback field, the old `*NotRequired`,
`Committed`/`RolledBack`, `*Failed`, and `*Panicked` variants map to
`FinalizationOutcome::{NotRequired, Succeeded, Failed, Panicked}`.

The removed `ArcMutex`, `ArcRwLock`, and related wrappers are not replaced in
`qubit-dcl`. Use native `std`, `parking_lot`, or Tokio lock types supported by
`qubit-lock`. A read-write lock must select a mode explicitly through its
`read_lock()` or `write_lock()` adapter before it can be passed as `Lock`.

The predicate remains a zero-argument, thread-safe callback. It must read an
atomic or equivalently synchronized gate. Every external transition that must
exclude the task must acquire the exact same underlying lock passed to `run`.

Passing a lock per invocation is intentional. The same executor can receive a
read mode in one thread and the paired write mode in another thread, provided
both modes come from the same RWLock. Their tasks may use different captured
data while consulting the same atomic state variable; the lock supplies
coordination rather than data ownership.

With panic capture enabled, a panic raised while dropping the guard after
normally completed locked work is reported as `PanicPhase::LockRelease`.
