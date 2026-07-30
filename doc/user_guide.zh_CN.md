# Qubit DCL 用户手册

[English](user_guide.md) · [README](../README.zh_CN.md) ·
[API 文档](https://docs.rs/qubit-dcl) ·
[0.11 迁移指南](user_guide_migration_0_11.zh_CN.md)

本手册说明如何使用 `qubit-dcl` 0.11，在保持同步协议正确性的同时避免不必要的串行
工作。它面向已经拥有 atomic 或等价同步 gate，并需要复用双重检查执行策略的 Rust
开发者。

## 概念模型

`DoubleCheckedLockExecutor` 保存一个 predicate。每次 `run` 都遵循下列流程：

```text
读取同步 gate
  false ──> ExecutionOutcome::ConditionNotMet
  true  ──> 获取本次调用传入的 Lock
               └─> 再次读取同一个 gate
                     false ──> ConditionNotMet
                     true  ──> 在该 guard 内运行 task
```

第一次检查不获取锁。第二次检查与 task 共用本次调用传入的
`qubit_lock::Lock` 所产生的同一个 guard。executor 不拥有锁或锁保护的数据，因此
同一个 executor 可以在不同调用中复用，并接收兼容的锁模式。

predicate 必须读取 atomic 或具有等价同步语义的 gate。常用协议是 Acquire load 配对
Release store。predicate 不得获取同一个底层锁，也不应阻塞。

## 贯穿场景：只初始化一次昂贵状态

设想多个请求处理器都可能懒初始化一个昂贵资源。第一个观察到 gate 打开的处理器完成
初始化并关闭 gate；后续处理器应在不获取 mutex 的情况下直接返回。

添加 crate 与锁后端：

```toml
[dependencies]
qubit-dcl = { version = "0.11", features = ["parking-lot"] }
qubit-lock = "0.13"
parking_lot = "0.12"
```

使用该后端时，请显式启用可选的 `parking-lot` feature，以获得匹配的 `qubit-lock`
支持。只使用标准库锁时不需要启用 DCL feature：

```toml
qubit-dcl = "0.11"
qubit-lock = { version = "0.13", default-features = false }
```

构造一个 executor，并在每次调用时传入 mutex：

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
        // task 已持有本次调用选择的 mutex。
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

第一次成功调用在 mutex 内执行 task。第二次调用在无锁的第一次检查中看到 gate 已关闭，
不会调用锁实现。`TaskFailed(E)` 会原样保留 task error，而不会转换为库的统一错误类型。

## 选择锁模式

`Lock` 表示获取模式，并不必然表示排他性。

`run` 有意接受 `Lock` 而不是 `ExclusiveLock`，对共享模式和排他模式采用相同的控制
流程，因此调用方可以传入同一 RWLock 的 read mode 或 write mode。Rust 无法证明 task
闭包只读，所以传入共享模式时，必须由调用方保证只读契约。

当 task 修改 gate 或受保护状态、消费工作、只允许一次初始化，或必须串行执行时，使用
mutex 或 write-mode adapter 等排他模式。能够选出唯一执行者的独立
compare-and-exchange 协议也同样有效。

只有同时满足下列条件时，才可以使用共享 read mode：

1. task 相对于受保护协议是只读的。
2. 所有冲突 writer 都使用同一底层锁配套的 write mode。
3. 调用方不要求 task 至多执行一次。

多个共享调用可以同时通过第二次检查并执行 task。这是有意的语义，而不是重复执行缺陷。

同一个 executor 可以接收同一读写锁产生的配套模式。两个 action 可以捕获不同的数据，
只要共享 gate 协议的冲突更新使用同一个底层锁：

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

task 外部修改 gate 的代码必须获取与冲突 `run` 调用相同的底层锁。atomic gate 提供
可见性，不能替代锁协调。尤其是，`ptr::read_volatile` 用于 MMIO 等 volatile memory，
不是同步机制。

## 生命周期执行

当每次调用都需要在第一次检查后创建 token，并在锁内执行结束后完成收尾时，使用
`LifecycleDoubleCheckedLockExecutor`。其 typestate builder 只暴露以下有效组合：

- `prepare -> commit -> rollback -> build`
- `prepare -> commit -> no_rollback -> build`
- `prepare -> no_commit -> rollback -> build`

有意不存在 `no_commit + no_rollback` 组合。`prepare` 在加锁前运行。task 在
executor guard 内修改本次调用独有的 token。guard 释放后，`commit` 消费成功 token，
`rollback` 消费失败路径的 token。

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

task 不需要直接访问 token 时使用 `run`；只有 task 需要修改 token 时才使用
`run_with_token`。callback 对象会被不同调用共享并可能并发运行，因此 prepare、commit
或 rollback 捕获的可变状态需要自己的同步机制。

## 结果与 panic 处理

基础 executor 返回 `ExecutionOutcome<R, E>`：

| Variant | 含义 |
| --- | --- |
| `Success(R)` | task 在选定 guard 内运行并返回值。 |
| `ConditionNotMet` | 任一次条件检查返回 false。 |
| `TaskFailed(E)` | task 返回未被改变的 error。 |
| `Panicked(PanicInfo)` | 配置的 panic 边界捕获了 panic。 |

生命周期 executor 返回一个穷尽的 `LifecycleOutcome<R, E, C>`，区分第一次检查、
prepare、第二次检查、task failure、task success 与 panic 路径。收尾字段使用
`FinalizationOutcome<C>`：`NotRequired`、`Succeeded`、`Failed(C)` 或
`Panicked(PanicInfo)`。commit failure 不会覆盖 task success；rollback failure 不会
覆盖原始 task error 或 panic。

默认不捕获 panic，predicate、锁、callback 和 task 的 panic 会正常传播。启用
`catch_panics(true)` 后会得到包含 `PanicPhase` 的 `PanicInfo`。捕获边界位于 RAII
guard 作用域外：标准库锁保持正常 poisoning 语义，parking-lot 保持其正常的不 poisoning
语义。正常完成锁内工作后，显式释放 guard 时发生的 panic 会归类为
`PanicPhase::LockRelease`。

只有使用 unwind panic 策略时才能捕获 panic。使用 `panic = "abort"` 时，进程会在返回
outcome 或执行基于 unwind 的 rollback 之前终止。`PanicInfo` 只负责分类和传递 payload，
并不提供恢复能力或事务边界：panic 前已经完成的副作用仍会保留，生命周期 rollback
本身也可能失败或 panic。捕获到 outcome 不代表应用不变量已经恢复；再次使用相关状态前，
调用方必须验证或重建这些不变量。

## 排障

| 症状 | 检查项 |
| --- | --- |
| task 执行多次。 | 使用排他模式或显式 atomic 唯一性协议；共享模式允许重叠。 |
| 外部状态转换后 task 仍执行。 | 确认转换使用同一底层锁，并使用与 predicate 的 Acquire load 配对的 Release store。 |
| 出现死锁。 | 确认 predicate 不获取 executor 锁，callback 也没有意外重获该锁。 |
| rollback 无法解释失败。 | 匹配 `RollbackCause`，并从 `LifecycleOutcome` 保留原始 error/panic。 |
| 标准 mutex 被 poisoning。 | 检查 panic 是否跨越标准锁 guard；该 poisoning 语义会被有意保留。 |

## 限制与最佳实践

- 本 crate 是同步的，只接收同步 `qubit_lock::Lock`。
- 它不发现锁、不拥有受保护数据、不缓存 task 结果，也不替应用选择 memory ordering 协议。
- 它不会让非 atomic predicate 自动安全，也不会把共享锁模式变成排他执行。
- predicate 应短小、无阻塞，并且不得获取同一把锁；把 executor callback 当作可并发复用的函数。
- 在确实需要 prepare/commit/rollback 的所有权语义之前，优先使用基础 executor。

## 延伸阅读

- 返回 [English README](../README.md) 或[中文 README](../README.zh_CN.md)。
- 阅读 [API 文档](https://docs.rs/qubit-dcl)。
- 从 0.10 升级时，阅读 [迁移指南](user_guide_migration_0_11.zh_CN.md)。
