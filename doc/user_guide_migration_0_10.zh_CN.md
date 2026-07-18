# qubit-dcl 从 0.9 迁移至 0.10

0.10 有意删除 0.9 API，不提供 deprecated 转发。此次重设计恢复 DCL 的核心
快速路径：第一次条件检查不获取 executor 锁。

## 依赖变化

使用锁类型时需要直接声明两个 crate：

```toml
[dependencies]
qubit-dcl = "0.10"
qubit-lock = "0.10"
```

`qubit-dcl` 不再重导出 `ArcMutex` 或 `Lock`，也不再依赖 `qubit-function` 和
`log`。

## API 映射

| 0.9 概念 | 0.10 替代方式 |
| --- | --- |
| `tester(&T)` 或绑定受保护数据的条件 | `.when(|| atomic_gate.load(Ordering::Acquire))` |
| `call` / `execute` | `DoubleCheckedLockExecutor::run` |
| `call_with` / `execute_with` | 将 task 重构为任意零参数 closure；不再暴露 `T` |
| one-shot `DoubleCheckedLock` | 构建并保存 `DoubleCheckedLockExecutor` |
| 共享可变 prepare 状态 | `prepare` 为每次调用返回独立令牌 `P` |
| task 需要 prepare 数据 | `run_with_token(|token| ...)` |
| `rollback_prepare` | `.rollback(|token, cause| ...)` |
| `commit_prepare` | `.commit(|token| ...)` |
| `ExecutionContext` / `ExecutionResult` | `ExecutionOutcome` 或 `ExecutionReport` |
| `ExecutionLogger` 配置 | 调用方 match 结构化结果后自行记录日志 |

不存在兼容 feature。

## gate 同步协议

predicate 必须执行 atomic 或等价同步读取，而且不得获取 executor 的锁。第二次
检查成功后 task 位于锁内，可以直接修改 gate。如果 task 保持 gate 不变，则
task 返回后的修改或系统其他路径中需要与 task 互斥的修改，必须获取同一底层锁。

除非应用需要更强排序，一般使用 Acquire load 和 Release store。
`ptr::read_volatile` 不是线程同步原语。

## 生命周期组合

typestate builder 只接受以下组合：

```text
prepare -> commit -> rollback -> build
prepare -> commit -> no_rollback -> build
prepare -> no_commit -> rollback -> build
```

令牌只会被消费一次。`RollbackCause` 区分第二次检查失败、task error 和捕获的
panic。report 分别保留 task 与 finalizer 的失败。

## panic 迁移

默认不捕获 panic。在 `prepare` 之前配置 `.catch_panics(true)`，即可获得
`PanicInfo` 与 `PanicPhase`。捕获边界位于 lock 调用外部，从而保留所选
`Lock<T>` 实现的 poisoning 语义。
