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

已删除的 `ArcMutex`、`ArcRwLock` 等包装器不会在 `qubit-dcl` 中提供替代品。
直接使用 `qubit-lock` 支持的 `std`、`parking_lot` 或 Tokio 原生锁。读写锁必须
通过 `read_lock()` 或 `write_lock()` 明确选择模式，所得适配器才能作为 `Lock`
传入。

predicate 仍是线程安全的零参数 callback，并且必须读取 atomic 或具有等价同步
语义的 gate。需要与 task 互斥的外部状态转换，必须获取传给 `run` 的同一个底层锁。

