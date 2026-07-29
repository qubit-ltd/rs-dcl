# Qubit DCL

[![Rust CI](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-dcl/coverage-badge.json)](https://qubit-ltd.github.io/rs-dcl/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-dcl.svg?color=blue)](https://crates.io/crates/qubit-dcl)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

`qubit-dcl` 将双重检查锁（Double-Checked Locking）设计模式封装为可复用
executor。无锁 predicate 首先排除不需要执行的任务；条件满足后，executor
获取本次调用传入的通用 `qubit_lock::Lock`，再次检查同一个 predicate，并在锁内
执行任意 task。

0.11 是一次有意进行的破坏性重设计。executor 只持有 predicate 和 callback，
每次 `run` 单独传入锁获取模式。锁在这里是协调机制，而不是数据所有权，因此
executor 不会永久绑定某个锁，也不会绑定 task 使用的数据。

## 并发契约

正确使用 DCL 必须同时遵守以下三条规则：

1. predicate 读取 atomic 或具有等价同步语义的 gate。常用协议是 Acquire load
   配对 Release store。
2. predicate 不得获取 executor 的同一底层锁，也不应执行阻塞操作。
3. 锁模式必须匹配 task 的实际语义。`Lock` 表示获取模式，并不必然表示排他锁。
   executor 会让第二次检查和 task 共用该模式产生的同一个 guard。

当 task 只读取协议保护的状态、所有冲突写入都使用同一底层锁配套的 write mode，
并且调用方不要求 task 至多执行一次时，共享 read mode 是正确选择。此时多个调用
可以同时通过第二次检查并并发执行。

一个常见设计是由读线程和写线程调用同一个 executor：读线程传入某个 RWLock 的
read mode 并执行只读 action；写线程传入同一底层锁配套的 write mode 并执行写
action。两个 action 可以操作不同的捕获数据，只需读取同一个 atomic 状态变量。
这正是每次调用才传入锁，而不是把锁或受保护数据固定在 executor 内的原因。

如果 task 会修改 gate 或受保护状态、消费任务、执行一次性初始化，或者要求串行化，
则必须使用 `ExclusiveLock` mode，例如 mutex 或 write-mode adapter。也可以使用独立
的 compare-and-exchange 协议来选出唯一执行者。

atomic gate 提供可见性；所选锁模式提供对应的共享或排他协调。
`ptr::read_volatile` 面向 MMIO 等 volatile memory，不能替代 atomic 同步。

## 基础 executor

锁类型应直接从其所属 crate 导入。下面的例子有意使用 mutex，因此 task 运行在
排他获取模式中，可以在该 guard 内直接关闭 gate：

```rust
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use qubit_dcl::{DoubleCheckedLockExecutor, ExecutionOutcome};
use parking_lot::Mutex;

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
        // 第二次检查已经成功，task 此时位于 executor 锁内，可以直接关闭
        // gate，无需重新获取同一把锁。
        gate.store(false, Ordering::Release);
        Ok::<usize, io::Error>(42)
    }
});

assert!(matches!(outcome, ExecutionOutcome::Success(42)));
```

第一次检查返回 `false` 时，executor 不调用任何锁方法，直接返回
`ExecutionOutcome::ConditionNotMet`。task 返回的错误会原样保存在
`ExecutionOutcome::TaskFailed(E)` 中。

同一个 executor 可以使用同一 RWLock 配套的 read mode 和 write mode。下面两个
action 有意操作不同的捕获数据，只共享协调协议：

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
let read_outcome = executor.run(&read_mode, {
    let read_value = Arc::clone(&read_value);
    move || Ok::<usize, std::io::Error>(read_value.load(Ordering::Acquire))
});
assert!(matches!(read_outcome, ExecutionOutcome::Success(42)));

let write_mode = lock.write_lock();
let write_outcome = executor.run(&write_mode, {
    let gate = Arc::clone(&gate);
    let write_value = Arc::clone(&write_value);
    move || {
        write_value.store(43, Ordering::Release);
        gate.store(false, Ordering::Release);
        Ok::<(), std::io::Error>(())
    }
});
assert!(matches!(write_outcome, ExecutionOutcome::Success(())));
assert_eq!(write_value.load(Ordering::Acquire), 43);
```

如果由 task 外的路径修改 gate，该路径必须使用同一个锁对象：

```rust
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use qubit_dcl::DoubleCheckedLockExecutor;
use qubit_lock::Lock;

let lock = Arc::new(parking_lot::Mutex::new(()));
let gate = Arc::new(AtomicBool::new(true));
let executor = DoubleCheckedLockExecutor::builder()
    .when({
        let gate = Arc::clone(&gate);
        move || gate.load(Ordering::Acquire)
    })
    .build();

let guard = Lock::lock(&lock);
gate.store(false, Ordering::Release);
drop(guard);

let outcome = executor.run(&lock, || Ok::<(), std::io::Error>(()));
assert!(matches!(
    outcome,
    qubit_dcl::ExecutionOutcome::ConditionNotMet
));
```

## 生命周期 executor

`LifecycleDoubleCheckedLockExecutor` 在第一次检查之后、加锁之前执行 prepare。
每次调用都有独立令牌 `P`。task 和解锁完成后，该令牌由 commit 或 rollback
消费。

```rust
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
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
    .catch_panics(true)
    .prepare(|| Ok::<Vec<&'static str>, io::Error>(vec!["prepared"]))
    .commit(|token| {
        assert_eq!(token, ["prepared", "task"]);
        Ok::<(), io::Error>(())
    })
    .rollback(|token, cause| {
        assert!(!token.is_empty());
        match cause {
            RollbackCause::ConditionNotMet => {}
            RollbackCause::TaskFailed(error) => eprintln!("task failed: {error}"),
            RollbackCause::Panicked(panic) => {
                eprintln!("panic in {:?}", panic.phase());
            }
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

typestate builder 只允许三种生命周期组合：

- `prepare -> commit -> rollback -> build`
- `prepare -> commit -> no_rollback -> build`
- `prepare -> no_commit -> rollback -> build`

不存在 `no_commit + no_rollback` 组合。没有实际令牌数据时，prepare 可以返回
`()`，调用方继续使用 `run`，无需改用 `run_with_token`。

## 结果与 panic 语义

基础 executor 返回 `ExecutionOutcome<R, E>`。生命周期 executor 返回一个穷尽的
`LifecycleOutcome<R, E, C>`，由 variant 直接表示最终控制流状态。执行 commit 或
rollback 的 variant 包含 `FinalizationOutcome<C>`，其状态为 `NotRequired`、
`Succeeded`、`Failed(C)` 或 `Panicked(PanicInfo)`。

commit 失败不会覆盖 task 成功；rollback 失败不会覆盖触发它的 task error 或
panic。`RollbackCause::TaskFailed` 在 rollback 调用期间借用原始 error，而
`LifecycleOutcome` 仍然保留其所有权。

默认不捕获 panic。启用 `.catch_panics(true)` 后，panic 信息包含准确的
`PanicPhase` 和原始 payload。捕获边界位于 RAII guard 的作用域外，因此标准库锁
会正常观察 unwind 并进入 poisoned 状态；parking-lot 锁则保持其不 poisoning 的
正常语义。正常完成锁内工作后，显式释放 guard 时发生的 panic 会分类为
`PanicPhase::LockRelease`。

## 安装

```toml
[dependencies]
qubit-dcl = "0.11"
qubit-lock = "0.12"
parking_lot = "0.12"
```

`qubit-dcl` 不重导出 `Lock` 或其他 crate 拥有的锁原语。调用方必须直接声明
`qubit-lock` 和所选锁后端依赖。默认 `parking-lot` feature 会通过
`qubit-lock` 启用 `parking_lot` 锁实现；只使用标准库锁的调用方可以关闭它：

```toml
qubit-dcl = { version = "0.11", default-features = false }
```

## 从 0.10 迁移

0.11 将锁从 builder 状态移到每次 executor 调用，并采用与数据无关的
`qubit_lock::Lock` trait。完整映射参见
[0.11 迁移指南](doc/user_guide_migration_0_11.zh_CN.md)。

## 测试

```bash
# 使用默认 feature 集运行测试
cargo test

# 使用项目声明的全部 feature 运行测试
cargo test --all-features

# 运行项目 CI 检查
./ci-check.sh

# 检查代码覆盖率
./coverage.sh
```

## 许可证

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

本项目基于 Apache License 2.0 授权。完整许可证文本请参阅
[LICENSE](LICENSE)。

## 贡献

欢迎贡献。请遵循 Rust API 指南，及时更新公共 API 文档与测试，并在提交
Pull Request 前运行 `./align-ci.sh`格式化代码，运行`./ci-check.sh`对齐CI要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[https://github.com/qubit-ltd/rs-dcl](https://github.com/qubit-ltd/rs-dcl)
