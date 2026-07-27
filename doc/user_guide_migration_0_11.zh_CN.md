# qubit-dcl 0.10 到 0.11 迁移指南

0.11 将可复用的 DCL 策略与锁所有权分离。executor 持有 predicate 和生命周期
callback；调用方在每次执行时传入与数据无关的 `qubit_lock::Lock`。

迁移前：

```rust,ignore
let executor = DoubleCheckedLockExecutor::builder(lock)
    .when(predicate)
    .build();
let outcome = executor.run(task);
```

迁移后：

```rust,ignore
let executor = DoubleCheckedLockExecutor::builder()
    .when(predicate)
    .build();
let outcome = executor.run(&lock, task);
```

生命周期 executor 同样修改：

- `LifecycleDoubleCheckedLockExecutor::builder(lock)` 改为 `builder()`。
- `run(task)` 改为 `run(&lock, task)`。
- `run_with_token(task)` 改为 `run_with_token(&lock, task)`。

生命周期结果模型也被直接替换，不提供兼容 alias：

- 删除 `ExecutionReport<R, E, C>` 和 `PreparationOutcome<C>`。
- `LifecycleDoubleCheckedLockExecutor::run` 与 `run_with_token` 现在返回一个
  穷尽的 `LifecycleOutcome<R, E, C>`。
- commit 与 rollback 状态由 `FinalizationOutcome<C>` 表示。
- 基础 executor 仍返回 `ExecutionOutcome<R, E>`，但删除了仅服务于生命周期的
  `NotExecuted` variant。

主要状态映射如下：

| 旧 report 状态 | 新生命周期结果 |
| --- | --- |
| 第一次条件不满足 | `InitialConditionNotMet` |
| 第一次条件检查 panic | `InitialConditionCheckPanicked` |
| `NotExecuted` + prepare 失败/panic | `PrepareFailed` / `PreparePanicked` |
| task 成功 + commit 状态 | `TaskSucceeded { value, commit }` |
| 第二次条件不满足 + rollback 状态 | `SecondConditionNotMet { rollback }` |
| task 失败 + rollback 状态 | `TaskFailed { error, rollback }` |
| 锁内执行 panic + rollback 状态 | `ExecutionPanicked { panic, rollback }` |

在嵌套的 commit 或 rollback 字段内，旧的 `*NotRequired`、
`Committed`/`RolledBack`、`*Failed` 与 `*Panicked` variant 分别映射到
`FinalizationOutcome::{NotRequired, Succeeded, Failed, Panicked}`。

已删除的 `ArcMutex`、`ArcRwLock` 等包装器不会在 `qubit-dcl` 中提供替代品。
直接使用 `qubit-lock` 支持的 `std`、`parking_lot` 或 Tokio 原生锁。读写锁必须
通过 `read_lock()` 或 `write_lock()` 明确选择模式，所得适配器才能作为 `Lock`
传入。

predicate 仍是线程安全的零参数 callback，并且必须读取 atomic 或具有等价同步
语义的 gate。需要与 task 互斥的外部状态转换，必须获取传给 `run` 的同一个底层锁。

每次调用单独传锁是有意设计。同一个 executor 可以在一个线程中接收 read mode，
在另一个线程中接收配套的 write mode，但两种模式必须来自同一个 RWLock。两个
task 可以使用不同的捕获数据，只需读取同一个 atomic 状态变量；锁提供的是协调，
而不是数据所有权。

启用 panic 捕获后，如果锁内工作正常结束，但释放 guard 时发生 panic，将报告为
`PanicPhase::LockRelease`。
