# Migrating qubit-dcl from 0.9 to 0.10

Version 0.10 intentionally removes the 0.9 API instead of providing deprecated
forwarders. The redesign restores the essential DCL fast path: the first
condition check does not acquire the executor lock.

## Dependency changes

Declare both crates when lock types are used:

```toml
[dependencies]
qubit-dcl = "0.10"
qubit-lock = "0.10"
```

`qubit-dcl` no longer re-exports `ArcMutex` or `Lock`. It also no longer depends
on `qubit-function` or `log`.

## API mapping

| 0.9 concept | 0.10 replacement |
| --- | --- |
| `tester(&T)` or a condition tied to protected data | `.when(|| atomic_gate.load(Ordering::Acquire))` |
| `call` / `execute` | `DoubleCheckedLockExecutor::run` |
| `call_with` / `execute_with` | Redesign the task as an arbitrary zero-argument closure; `T` is not exposed |
| one-shot `DoubleCheckedLock` | Build and retain `DoubleCheckedLockExecutor` |
| shared mutable prepare state | `prepare` returns one token `P` per invocation |
| task needs prepared data | `run_with_token(|token| ...)` |
| `rollback_prepare` | `.rollback(|token, cause| ...)` |
| `commit_prepare` | `.commit(|token| ...)` |
| `ExecutionContext` / `ExecutionResult` | `ExecutionOutcome` or `ExecutionReport` |
| `ExecutionLogger` configuration | Match structured outcomes in the caller and log there |

No compatibility feature is available.

## Gate synchronization

The predicate must perform an atomic or equivalently synchronized read and must
not acquire the executor's lock. A task runs inside the lock after the second
check and may update the gate directly. When the task leaves the gate unchanged,
any later or external gate update that must exclude the task has to acquire the
same underlying lock.

Use Acquire loads and Release stores unless the application requires a stronger
ordering. `ptr::read_volatile` is not a thread synchronization primitive.

## Lifecycle combinations

The typestate builder accepts only these combinations:

```text
prepare -> commit -> rollback -> build
prepare -> commit -> no_rollback -> build
prepare -> no_commit -> rollback -> build
```

The token is consumed exactly once. `RollbackCause` distinguishes a failed
second check, a task error, and a captured panic. The report retains task and
finalizer failures independently.

## Panic migration

Panic capture is disabled by default. Configure `.catch_panics(true)` before
`prepare` to receive `PanicInfo` and `PanicPhase`. Capture occurs outside the
lock call, preserving the poisoning semantics of the selected `Lock<T>`
implementation.
