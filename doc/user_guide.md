# Qubit DCL User Guide

[中文](user_guide.zh_CN.md) · [README](../README.md) ·
[API reference](https://docs.rs/qubit-dcl)

This guide applies to qubit-dcl 0.13 and Rust 1.94 or later. It is for Rust
developers who need a reusable synchronous policy for conditional work under
contention, including work with a prepare/commit/rollback lifecycle.

## Purpose and Audience

Use Qubit DCL when most calls should return immediately, but several callers
can race after some event makes work necessary. The crate standardizes two
checks around a caller-supplied lock. Your application still defines what
“work is needed” means and owns every resource involved.

Qubit DCL does not make an arbitrary boolean thread-safe. Before using it, you
must have a synchronized gate and a lock protocol shared by every conflicting
update.

## Conceptual Model

Five pieces have different responsibilities:

| Piece | Responsibility |
| --- | --- |
| Gate | Answers whether work is currently needed; it must support synchronized reads and writes. |
| Predicate | Reads the gate quickly without acquiring the coordination lock. |
| Coordination lock | Serializes the second decision and any conflicting task. |
| Business data | Remains application-owned; the task captures and updates it. |
| Task | Runs under the guard only when both checks pass. |

An Acquire load paired with Release store is a common starting protocol for an
atomic gate. The predicate must not acquire the coordination lock and should
not block.

`DclExecutor` performs the following steps:

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

The first check is the fast path. The second check is the correctness step that
closes the race between observing the gate and acquiring the lock. The executor
owns neither the lock nor the business data. Returning from either locked
branch releases the guard through RAII.

`LifecycleDclExecutor` adds one invocation-local token:

```text
if the initial predicate is false:
    return InitialConditionNotMet

prepare an invocation-local token
if preparation fails:
    return PrepareFailed

acquire the lock supplied by the caller

if the second predicate is false while holding the lock:
    release the lock guard
    finalize the token through the rollback path
    return SecondConditionNotMet with the rollback outcome

run the task while holding the same lock guard
release the lock guard

if the task succeeds:
    finalize the token through the commit path
    return TaskSucceeded with the commit outcome
otherwise:
    finalize the token through the rollback path
    return TaskFailed with the task error and rollback outcome
```

Preparation happens before lock acquisition. The configured commit or rollback
callback, when present, consumes the token after the guard has been released.

## Installation and Optional Integrations

The minimum application dependency is:

```toml
[dependencies]
qubit-dcl = "0.13"
```

This is enough for standard-library `AtomicBool` and `Mutex`. Add another crate
only when the application uses its API:

- Add `qubit-atomic = "0.16"` when conveniences such as `ArcAtomic<bool>` are
  useful. Qubit DCL does not require that gate wrapper.
- Add `qubit-lock = { version = "0.14", default-features = false }` when
  application code directly calls `ReadWriteLock::read_lock()`, `write_lock()`,
  or another `qubit-lock` capability.
- Add a `parking_lot` backend only when using its lock types directly:

  ```toml
  [dependencies]
  qubit-dcl = { version = "0.13", features = ["parking-lot"] }
  parking_lot = "0.12"
  ```

## Scenario 1: Refresh a Service-Routing Cache

A service keeps an in-memory routing table. A configuration event marks the
table stale. Request threads may notice the stale flag concurrently, but the
configuration loader must run once and later requests should avoid the
coordination mutex entirely.

The success criteria are observable: exactly one caller loads the routes, the
cache contains the new endpoint, and the other caller reports that the
condition is no longer met.

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

The synchronization objects deliberately have separate roles. `refresh_lock`
coordinates refresh decisions; the cache's `RwLock` protects access to the
route vector; `refresh_needed` lets the common path skip `refresh_lock` after a
successful refresh. The task closes the gate only after publishing the new
routes.

A task error is preserved rather than retried or wrapped:

```rust
use std::{io, sync::Mutex};
use qubit_dcl::{DclExecutor, ExecutionOutcome};

let executor = DclExecutor::new(|| true);
let outcome = executor.run(&Mutex::new(()), || {
    Err::<(), _>(io::Error::other("configuration source unavailable"))
});

assert!(matches!(outcome, ExecutionOutcome::TaskFailed(error)
    if error.kind() == io::ErrorKind::Other));
```

The application decides whether a failed load leaves the gate open for a later
call, closes it, or applies another retry policy. Qubit DCL does not decide that
policy.

## Why the Second Check Matters

Consider two request threads that both observe `refresh_needed == true`:

| Time | Worker A | Worker B |
| --- | --- | --- |
| 1 | Initial check returns true. | Initial check returns true. |
| 2 | Acquires `refresh_lock`. | Waits for `refresh_lock`. |
| 3 | Second check returns true; refreshes routes and closes the gate. | Still waiting. |
| 4 | Releases the lock. | Acquires the lock. |
| 5 | Returns `Success`. | Second check now returns false; skips the loader. |

Without the second check, Worker B would repeat a refresh based on an
observation that became stale while it waited. Checking twice without using the
same exclusive coordination lock for the second check and task does not close
that race.

## Scenario 2: Finalize a Database Transaction

Some work owns a resource that must be prepared before entering the
coordination lock and finalized after leaving it. A database transaction is a
concrete example: create a transaction, stage statements under the guard, then
commit or roll back after the guard is released.

This small synchronous adapter models ownership without adding a database
driver dependency:

The two code blocks below form one example. Place the first block's imports and
type definitions at module scope, then put the second block's statements inside
`main`.

```rust
use std::{
    io,
    sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}},
};

#[derive(Clone, Default)]
struct Database {
    committed: Arc<Mutex<Vec<String>>>,
    fail_next_commit: Arc<AtomicBool>,
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
        if self.database.fail_next_commit.swap(false, Ordering::AcqRel) {
            return Err(io::Error::other("commit failed before writing"));
        }
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

Configure the lifecycle and run a task that needs mutable access to the token:

```rust
use qubit_dcl::{
    FinalizationOutcome, LifecycleDclExecutor, LifecycleOutcome, RollbackCause,
};

let work_available = Arc::new(AtomicBool::new(true));
let transaction_lock = Mutex::new(());
let database = Database {
    committed: Arc::default(),
    fail_next_commit: Arc::new(AtomicBool::new(true)),
};

let executor = LifecycleDclExecutor::builder()
    .when({
        let work_available = Arc::clone(&work_available);
        move || work_available.load(Ordering::Acquire)
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

let outcome = executor.run_with_token(&transaction_lock, |transaction| {
    transaction.execute("INSERT INTO audit_log VALUES ('routes refreshed')");
    work_available.store(false, Ordering::Release);
    Ok::<usize, io::Error>(1)
});

match outcome {
    LifecycleOutcome::TaskSucceeded {
        value: 1,
        commit: FinalizationOutcome::Failed(error),
    } => assert_eq!(error.to_string(), "commit failed before writing"),
    other => panic!("unexpected first outcome: {other:?}"),
}
assert!(database.committed().is_empty());
{
    let _guard = transaction_lock.lock().expect("coordination lock should not be poisoned");
    work_available.store(true, Ordering::Release);
}
let outcome = executor.run_with_token(&transaction_lock, |transaction| {
    transaction.execute("INSERT INTO audit_log VALUES ('routes refreshed')");
    work_available.store(false, Ordering::Release);
    Ok::<usize, io::Error>(1)
});
match outcome {
    LifecycleOutcome::TaskSucceeded {
        value: 1,
        commit: FinalizationOutcome::Succeeded,
    } => {}
    other => panic!("unexpected second outcome: {other:?}"),
}
assert_eq!(
    database.committed(),
    ["INSERT INTO audit_log VALUES ('routes refreshed')"],
);
```

The token is invocation-local. `prepare` runs before `transaction_lock` is
acquired; the task closes `work_available` under the guard to claim the work;
`commit` consumes the token after the guard has been released. The gate means
the work has been claimed, not that it has been persisted. Only
`TaskSucceeded` with commit `Succeeded` means this example committed. The first
commit fails before writing, so the caller reacquires the same coordination
mutex and reopens the gate; the second call commits exactly one record. Use
`run` instead of `run_with_token` when the task does not need access to the
token.

If the task returns an error and rollback completes, reopen the gate under the
same coordination mutex when the work is safe to retry. This mock adapter knows
the first commit failed before writing. A real database can report an error
after the commit took effect; in that uncertain case, query the transaction
result or use an idempotency key before retrying. Do not blindly reopen the gate.

The builder exposes exactly these finalization shapes:

- `prepare -> commit -> rollback -> build`
- `prepare -> commit -> no_rollback -> build`
- `prepare -> no_commit -> rollback -> build`

A prepared token must have at least one defined finalization path, so there is
no `no_commit + no_rollback` combination.

## Choosing Lock Modes

`qubit_lock::Lock` represents an acquisition mode, not necessarily an exclusive
lock. A standard `Mutex` and the `write_lock()` adapter are exclusive. The
`read_lock()` adapter is shared.

Use an exclusive mode when the task changes the gate or protected protocol,
consumes work, initializes or refreshes a resource, or otherwise requires
serialized execution. Use a shared mode only when all three statements hold:

1. The task is read-only with respect to the protected protocol.
2. Every conflicting writer uses the paired write mode of the same underlying
   lock.
3. The caller does not require at-most-once task execution.

Several shared calls can pass the second check and run concurrently. Method
names cannot make Rust prove that a closure is read-only.

## Outcomes and Diagnostics

`DclExecutor::run` returns `ExecutionOutcome<R, E>`:

| Variant | Meaning |
| --- | --- |
| `Success(R)` | The task ran under the guard and returned a value. |
| `ConditionNotMet` | The initial or second predicate returned false. |
| `TaskFailed(E)` | The task returned its original error. |

`into_result()` maps these variants to `Ok(Some(value))`, `Ok(None)`, and
`Err(error)`.

`LifecycleOutcome<R, E, C>` distinguishes initial rejection, prepare failure,
task success plus commit result, second-check rejection plus rollback result,
and task failure plus rollback result. `FinalizationOutcome<C>` is
`NotRequired`, `Succeeded`, or `Failed(C)`. `RollbackCause` tells the rollback
callback whether execution was rejected, returned an error, or panicked.

`DclExecutor::run_catching` retains panic information in `PanicInfo`.
`LifecycleDclExecutor::run_catching` and `run_with_token_catching` return
`CapturedLifecycleOutcome`; its commit and rollback fields use
`CapturedFinalizationOutcome`. `PanicPhase` identifies the initial check,
prepare, lock acquisition, second check, task, lock release, commit, rollback,
or `TaskCleanup` phase. `TaskCleanup` means an unstarted task was dropped after
a condition rejection, outside the lock. On initial rejection no token exists, so a cleanup panic is
reported as `TaskCleanupPanicked` without rollback. On second-condition
rejection a token has already been prepared; a cleanup panic is reported as
`ExecutionPanicked` and the token follows rollback after the guard is released.
Existing failures such as
a predicate panic, prepare error, or prepare panic keep priority if dropping
the unstarted task also panics. Callers matching these public enums
exhaustively must handle the new variants. Captured finalization separately
reports not-required, success, error, or panic.

Panic capture requires unwinding. With `panic = "abort"`, the process exits
before an outcome or unwind-based rollback can be produced. Capturing a panic
does not undo task side effects or prove that application invariants were
restored.

## Advanced Usage

- Choose commit and rollback when the token represents a staged external
  change; commit and `no_rollback` when dropping is sufficient on failure; or
  `no_commit` and rollback when only unsuccessful cleanup matters.
- Callbacks are shared by cloned executors and can run concurrently. Mutable
  state captured by the predicate, prepare, commit, or rollback callback needs
  its own synchronization.
- External code that changes the gate or conflicting business state must use
  the same underlying lock as executor calls. An atomic gate supplies
  visibility; it does not replace lock-based coordination.

## Errors and Troubleshooting

Start from the observable outcome:

| Symptom | Check |
| --- | --- |
| A task runs more than once. | Confirm every uniqueness-sensitive call uses an exclusive lock; shared modes permit overlap. |
| A task runs after an external update. | Confirm that update uses the same underlying lock and a store ordered consistently with the predicate load. |
| Execution deadlocks. | Confirm the predicate and callbacks do not acquire the coordination lock recursively. |
| `PrepareFailed` appears. | Inspect the preparation error; no token exists, so rollback cannot run. |
| Rollback fails. | Preserve both the original `RollbackCause` or task error and the finalization failure. |
| A standard lock is poisoned. | Check whether a panic crossed its guard; Qubit DCL preserves the backend's poisoning behavior. |
| A captured panic has an unexpected phase. | Inspect `PanicInfo::phase()` and `message()`, then validate application state before reuse. |

## Limitations and Best Practices

- Qubit DCL is synchronous and accepts synchronous `qubit_lock::Lock`
  capabilities.
- Keep predicates short, non-blocking, and free of acquisition of the same
  underlying lock.
- Publish protected state before closing the gate, using a memory-ordering
  protocol appropriate for the application. Acquire/Release is a common
  starting point, not a universal replacement for protocol analysis.
- Prefer an exclusive mode for gate-changing work. A shared mode is for truly
  read-only tasks without uniqueness requirements.
- Inspect every structured outcome. A skipped call, task error, rollback error,
  and captured panic have different operational meanings.
- Treat rollback as observable compensation, not a guarantee that every
  external side effect was reversed.
- Qubit DCL does not own locks or data, cache results, retry failures, provide an
  asynchronous executor, or repair an unsynchronized predicate.

## Related Qubit Crates and Further Reading

- [`rs-atomic`](https://github.com/qubit-ltd/rs-atomic) provides optional atomic
  convenience wrappers.
- [`rs-lock`](https://github.com/qubit-ltd/rs-lock) provides optional direct
  access to backend-neutral lock capabilities and read/write adapters.
- Return to the [English README](../README.md) or
  [中文 README](../README.zh_CN.md).
- Browse the [API reference](https://docs.rs/qubit-dcl).
