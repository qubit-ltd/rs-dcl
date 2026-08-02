# Qubit DCL User Guide

[中文](user_guide.zh_CN.md) · [README](../README.md) ·
[API reference](https://docs.rs/qubit-dcl)

This guide applies to qubit-dcl 0.12. It is for Rust developers who need a
reusable synchronous double-checked execution policy around an atomic or
equivalently synchronized gate. It covers the basic executor and the
lifecycle executor as two independent workflows.

## Purpose and Audience

Use Qubit DCL when a task should run only if a gate still says that work is
needed, while several callers may race to make that decision. The crate
coordinates the two checks and the lock boundary; your application still owns
the gate, protected data, selected lock mode, and task side effects.

This guide does not turn an arbitrary boolean into a synchronization
protocol. The gate must be atomic or equivalently synchronized, and conflicting
updates must use the same underlying lock as the executor calls.

## Conceptual Model

A basic DclExecutor stores a predicate and applies this sequence for each call:

```text
initial predicate
  false ──> ExecutionOutcome::ConditionNotMet
  true  ──> acquire this call's Lock
               └─> second predicate
                    false ──> ConditionNotMet
                    true  ──> run task under that guard
```

The first check is lock-free. The second check and the task share one RAII
guard from the qubit_lock::Lock supplied to that invocation. The executor
owns neither the lock nor its protected data.

The predicate must perform an atomic or equivalently synchronized read. With
qubit_atomic::ArcAtomic, load() uses Acquire by default and store() uses
Release by default. An Acquire load paired with Release stores is the usual
starting contract. The predicate must not acquire the same underlying lock and
must not block.

A LifecycleDclExecutor adds one invocation-local token:

```text
initial predicate -> prepare token -> lock -> second predicate -> task
                                      -> release guard -> commit or rollback
```

prepare runs before lock acquisition. The task may mutate the token while the
guard is held. Commit or rollback consumes that token only after the guard is
released.

## Installation and Minimal Configuration

The examples use three Qubit crates:

- qubit-dcl provides the executors and structured outcomes.
- qubit-atomic provides ArcAtomic<bool> as a shared gate wrapper.
- qubit-lock provides the backend-neutral Lock and ReadWriteLock capabilities.

For standard-library locks:

```toml
[dependencies]
qubit-dcl = "0.12"
qubit-atomic = "0.16"
qubit-lock = { version = "0.13", default-features = false }
```

For parking_lot, enable the matching optional feature:

```toml
[dependencies]
qubit-dcl = { version = "0.12", features = ["parking-lot"] }
qubit-atomic = "0.16"
qubit-lock = "0.13"
parking_lot = "0.12"
```

The qubit-atomic and qubit-lock versions above are application dependency
choices. Qubit DCL itself requires qubit-lock at runtime but does not require
qubit-atomic; a caller can supply another synchronized gate.

## Scenario 1: Initialize an Expensive Resource

Several request handlers may discover that a resource still needs
initialization. The success criterion is that one exclusive task initializes
it, closes the gate, and later calls return without acquiring the lock.

### Build the executor

Use ArcAtomic<bool> for the gate and a standard-library read-write lock for
the coordination object. ReadWriteLock::write_lock() produces the exclusive
Lock adapter expected by DclExecutor::run.

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
```

The executor is reusable. Each call supplies the lock mode, so a later call
could use another compatible mode from the same underlying read-write lock.

### Run the task

The task changes the gate only after it has entered the exclusive mode:

```rust
let outcome = executor.run(&write_mode, {
    let gate = gate.clone();
    move || {
        // Initialize the resource here.
        gate.store(false);
        Ok::<usize, std::io::Error>(42)
    }
});

assert!(matches!(outcome, ExecutionOutcome::Success(42)));
```

A later call fails at the first check and does not call the lock:

```rust
assert!(matches!(
    executor.run(&write_mode, || Ok::<(), std::io::Error>(())),
    ExecutionOutcome::ConditionNotMet
));
```

If the task returns an error, Qubit DCL preserves it unchanged:

```rust
let failed = executor.run(&write_mode, || {
    Err::<(), _>(std::io::Error::other("resource unavailable"))
});

assert!(matches!(failed, ExecutionOutcome::TaskFailed(error)
    if error.kind() == std::io::ErrorKind::Other));
```

ExecutionOutcome::into_result() is useful when a caller wants a
Result<Option<R>, E> pipeline:

```rust
let result = executor
    .run(&write_mode, || Ok::<usize, std::io::Error>(7))
    .into_result();

assert_eq!(result.ok().flatten(), None);
```

The last snippet returns Ok(None) because the gate is already closed. A
successful task would return Ok(Some(value)), while a task error would be
Err(error).

### What this scenario proves

- The initial check avoids lock acquisition after the gate closes.
- The second check closes the race between the initial observation and lock
  acquisition.
- The task and second check share the same guard.
- write_lock() expresses the exclusive mode without making the executor own
  the read-write lock.
- TaskFailed(E) is an application error, not a library-wide error wrapper.

## Scenario 2: Stage a Database Transaction

A database-backed service may need to prepare a transaction object before it
enters the coordination lock, stage writes under the guard, and publish those
writes only after successful execution. The following complete synchronous
adapter is intentionally application-owned. It models transaction ownership
without adding a database-driver dependency.

### Define the transaction adapter

Database stores committed statements. DatabaseTransaction owns pending
statements and a shared database handle. commit appends pending statements;
rollback drops them.

```rust
use std::{
    io,
    sync::{Arc, Mutex},
};

#[derive(Clone, Default)]
struct Database {
    committed: Arc<Mutex<Vec<String>>>,
}

impl Database {
    fn begin_transaction(&self) -> DatabaseTransaction {
        DatabaseTransaction {
            database: self.clone(),
            pending: Vec::new(),
        }
    }

    fn committed(&self) -> Vec<String> {
        self.committed
            .lock()
            .expect("database state should not be poisoned")
            .clone()
    }
}

struct DatabaseTransaction {
    database: Database,
    pending: Vec<String>,
}

impl DatabaseTransaction {
    fn execute(&mut self, statement: impl Into<String>) {
        self.pending.push(statement.into());
    }

    fn commit(self) -> Result<(), io::Error> {
        self.database
            .committed
            .lock()
            .map_err(|_| io::Error::other("database state is poisoned"))?
            .extend(self.pending);
        Ok(())
    }

    fn rollback(self) -> Result<(), io::Error> {
        Ok(())
    }
}
```

This adapter is synchronous and deliberately small. A real database driver
would replace these application-owned methods while preserving the lifecycle
boundary: the token is prepared per invocation, the task stages work, and
commit or rollback consumes the token afterward.

### Configure lifecycle callbacks

The predicate, coordination lock, and builder use the same gate and
write_lock() pattern as Scenario 1:

```rust
use qubit_atomic::ArcAtomic;
use qubit_dcl::{
    FinalizationOutcome,
    LifecycleDclExecutor,
    LifecycleOutcome,
    RollbackCause,
};
use qubit_lock::ReadWriteLock;

let gate = ArcAtomic::new(true);
let rw_lock = std::sync::RwLock::new(());
let write_mode = rw_lock.write_lock();
let database = Database::default();

let executor = LifecycleDclExecutor::builder()
    .when({
        let gate = gate.clone();
        move || gate.load()
    })
    .prepare({
        let database = database.clone();
        move || Ok::<DatabaseTransaction, io::Error>(
            database.begin_transaction(),
        )
    })
    .commit(|transaction| transaction.commit())
    .rollback(|transaction, cause| {
        if let RollbackCause::TaskFailed(error) = cause {
            eprintln!("database task failed: {error}");
        }
        transaction.rollback()
    })
    .build();
```

The typestate builder exposes these valid lifecycle choices:

- prepare -> commit -> rollback -> build
- prepare -> commit -> no_rollback -> build
- prepare -> no_commit -> rollback -> build

There is intentionally no no_commit + no_rollback path. A prepared token
must have a defined successful or unsuccessful finalization policy.

### Run with the token

Use run_with_token when the task must mutate the transaction token:

```rust
let outcome = executor.run_with_token(&write_mode, |transaction| {
    transaction.execute("INSERT INTO audit_log VALUES ('ready')");
    gate.store(false);
    Ok::<usize, io::Error>(1)
});

assert!(matches!(
    outcome,
    LifecycleOutcome::TaskSucceeded {
        value: 1,
        commit: FinalizationOutcome::Succeeded,
    }
));
assert_eq!(database.committed(), ["INSERT INTO audit_log VALUES ('ready')"]);
```

The task runs while the exclusive guard is held. commit runs after that guard
is released, consumes the token, and makes the staged statement visible.

Use run when the task does not need the token. A second condition failure
after prepare still has a token to roll back, so it produces
LifecycleOutcome::SecondConditionNotMet { rollback }. A task error produces
LifecycleOutcome::TaskFailed { error, rollback }. A prepare error produces
LifecycleOutcome::PrepareFailed(error) and no token exists to roll back.

## Choosing Lock Modes

`Lock` represents an acquisition mode, not necessarily exclusivity.
qubit_lock::Lock is the capability used by the executor.
write_lock() adapts the exclusive mode of a ReadWriteLock; read_lock()
adapts its shared mode.

Use an exclusive mode when the task changes the gate or protected protocol,
consumes work, initializes a resource, or requires serialized execution. A
shared mode is valid only when all of these conditions hold:

1. The task is read-only with respect to the protected protocol.
2. Every conflicting writer uses the paired write mode of the same underlying
   lock.
3. The caller does not require at-most-once task execution.

Several shared calls can pass the second check and run concurrently. This is
intentional. A shared read mode is not a uniqueness mechanism, and separate
method names cannot make Rust verify whether a closure is actually read-only.

The same executor may receive paired modes from one read-write lock while the
tasks capture different data. Correctness depends on the shared gate protocol
and the same underlying lock for conflicting updates, not on the tasks
accessing the same object.

## Outcomes and Diagnostics

### Basic outcomes

DclExecutor::run returns ExecutionOutcome<R, E>:

| Variant | Meaning |
| --- | --- |
| Success(R) | The task ran under the selected guard and returned a value. |
| ConditionNotMet | The initial or second condition check returned false. |
| TaskFailed(E) | The task returned its original error. |

ExecutionOutcome::into_result() maps these to Ok(Some(value)),
Ok(None), and Err(error).

DclExecutor::run_catching returns
Result<ExecutionOutcome<R, E>, PanicInfo>. It captures panics from both
predicate calls, lock acquisition, task execution, and lock guard release.
Without run_catching, those panics propagate normally.

### Lifecycle outcomes

LifecycleOutcome<R, E, C> represents every non-captured lifecycle terminal
state:

| Variant | Meaning |
| --- | --- |
| InitialConditionNotMet | The first check rejected the call before prepare. |
| PrepareFailed(C) | prepare returned an error and no token was created. |
| TaskSucceeded { value, commit } | The task succeeded and the token followed commit or no-commit finalization. |
| SecondConditionNotMet { rollback } | The second check rejected a prepared token. |
| TaskFailed { error, rollback } | The task returned its original error and the token followed rollback. |

FinalizationOutcome<C> is NotRequired, Succeeded, or Failed(C).
RollbackCause tells the rollback callback whether the cause was
ConditionNotMet, TaskFailed(error), or Panicked(panic).

### Captured lifecycle outcomes

LifecycleDclExecutor::run_catching and
run_with_token_catching return CapturedLifecycleOutcome<R, E, C>:

- InitialConditionNotMet and InitialConditionCheckPanicked(PanicInfo)
  describe the first check.
- PrepareFailed(C) and PreparePanicked(PanicInfo) describe preparation.
- TaskSucceeded { value, commit } describes successful execution.
- SecondConditionNotMet { rollback } describes a rejected prepared token.
- TaskFailed { error, rollback } preserves the task error.
- ExecutionPanicked { panic, rollback } preserves the captured execution panic
  and the rollback result.

CapturedFinalizationOutcome<C> distinguishes NotRequired, Succeeded,
Failed(C), and Panicked(PanicInfo). A finalizer panic is reported instead
of replacing the structured result.

PanicInfo retains the panic phase, an optional string message(), and the
original payload for inspection or resume_unwind(). PanicPhase identifies
one of these exact phases:

- InitialConditionCheck
- Prepare
- LockAcquisition
- SecondConditionCheck
- Task
- LockRelease
- Commit
- Rollback

Panic capture requires an unwinding panic strategy. With panic = "abort", the
process terminates before an outcome can be returned or unwind-based rollback
can run. Capturing a panic does not undo side effects or prove that application
invariants were restored.

## Advanced Usage

### Select a lifecycle shape

The typestate builder intentionally prevents incomplete callback policies at
compile time. Choose commit and rollback according to ownership:

- Use commit and rollback when the token represents a staged external change.
- Use commit and no_rollback when an unsuccessful path can safely drop the
  token.
- Use no_commit and rollback when only unsuccessful cleanup is meaningful.

### Treat callbacks as concurrent

The executor stores callbacks behind shared ownership and can invoke them from
concurrent calls. Any mutable state captured by when, prepare, commit, or
rollback needs its own synchronization. The token itself is invocation-local;
it is not shared executor state.

### Preserve lock boundaries

External code that changes the gate or protected data must use the same
underlying lock as conflicting executor calls. An atomic gate supplies
visibility; it does not replace lock-based coordination. ptr::read_volatile is
for volatile memory such as MMIO, not synchronization.

## Errors and Diagnostics

Inspect the returned outcome from the phase in which it was produced:

| Symptom | First check |
| --- | --- |
| A task runs more than once. | Use write_lock() or an explicit compare-and-exchange uniqueness protocol; read_lock() permits overlap. |
| A task runs after an external state change. | Coordinate that change with the same underlying lock and pair Release stores with the predicate's Acquire load. |
| PrepareFailed appears. | Inspect the preparation error; no token exists and rollback cannot run. |
| A rollback result is Failed(C). | Preserve both the original RollbackCause and the finalization error. |
| A standard mutex is poisoned. | Check whether panic crossed a standard-library guard; poisoning is intentionally preserved. |
| A captured panic is unexpected. | Inspect PanicInfo::phase() and PanicInfo::message(), then verify application state before reuse. |

## Troubleshooting

Use the observable outcome to narrow the problem before changing the gate or
lock protocol:

1. If the task runs more than once, confirm that every call requiring
   uniqueness uses write_lock() or another exclusive/atomic uniqueness
   protocol.
2. If a task runs after an external state change, confirm the update uses the
   same underlying lock and a Release store paired with the predicate's
   Acquire load.
3. If execution deadlocks, confirm the predicate and callbacks do not acquire
   the executor's underlying lock again.
4. If finalization fails, retain both the original RollbackCause or task error
   and the FinalizationOutcome that describes the callback failure.
5. If panic capture returns an unexpected phase, inspect PanicInfo before
   reusing application state; capture does not restore external side effects.

## Limitations and Best Practices

- Qubit DCL is synchronous and accepts synchronous qubit_lock::Lock
  capabilities.
- It does not discover or own locks, expose protected data, cache task
  results, or choose the application's memory-ordering protocol.
- It does not make a non-atomic predicate safe and does not turn a shared mode
  into exclusive execution.
- Keep predicates short, non-blocking, and free of acquisition of the same
  lock.
- Prefer an exclusive mode for gate-changing tasks and a shared mode only for
  genuinely read-only work without at-most-once requirements.
- Treat lifecycle rollback as an observable compensation step, not a guarantee
  that every external side effect has been undone.
- Keep callback-captured mutable state synchronized because callbacks may run
  concurrently.

## Related Qubit Crates and Further Reading

- [rs-atomic](https://github.com/qubit-ltd/rs-atomic) — atomic values and
  ArcAtomic shared-owner wrappers.
- [rs-lock](https://github.com/qubit-ltd/rs-lock) — backend-neutral lock
  capabilities and read/write adapters.
- [rs-dcl](https://github.com/qubit-ltd/rs-dcl) — the executor documented here.
- Return to the [English README](../README.md) or
  [中文 README](../README.zh_CN.md).
- Browse the [API reference](https://docs.rs/qubit-dcl).
