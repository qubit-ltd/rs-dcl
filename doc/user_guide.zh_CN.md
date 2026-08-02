# Qubit DCL 用户手册

[English](user_guide.md) · [中文 README](../README.zh_CN.md) ·
[API 文档](https://docs.rs/qubit-dcl)

本手册适用于 qubit-dcl 0.12 和 Rust 1.94 或更高版本，面向需要在并发竞争下复用
同步条件执行策略的 Rust 开发者，也涵盖带 prepare、commit 和 rollback 生命周期的
工作。

## 手册目标与读者

当多数调用应立即返回，但某个事件发生后多个调用方可能同时发现工作待执行时，可以
使用 Qubit DCL。crate 负责围绕调用方传入的锁执行两次检查；“何时需要工作”的定义和
涉及的全部资源仍由应用负责。

Qubit DCL 不会让任意 boolean 自动具备线程安全性。使用前，应用必须定义同步 gate，
并让所有冲突更新遵守同一套锁协议。

## 概念模型

五个组成部分各自承担不同职责：

| 组成部分 | 职责 |
| --- | --- |
| Gate | 回答当前是否需要工作；必须支持同步读写。 |
| Predicate | 快速读取 gate，不获取协调锁。 |
| 协调锁 | 串行化第二次决策以及与之冲突的 task。 |
| 业务数据 | 始终由应用拥有，由 task 捕获和更新。 |
| Task | 仅在两次检查都通过后，才在 guard 下执行。 |

对于 atomic gate，Acquire load 配对 Release store 是常见起点。predicate 不得获取
协调锁，也不应阻塞。

`DclExecutor` 按以下步骤执行：

```text
如果第一次条件检查不通过：
    直接返回 ConditionNotMet，不获取锁

获取调用方传入的锁

如果锁内的第二次条件检查不通过：
    返回 ConditionNotMet，不执行 task

在同一个锁保护范围内执行 task

如果 task 成功：
    返回 Success
否则：
    返回 TaskFailed
```

第一次检查是 fast path；第二次检查负责关闭“观察 gate”与“成功获取锁”之间的竞争
窗口。executor 不拥有锁或业务数据。从任一锁内分支返回时，guard 都会按 RAII 自动
释放。

`LifecycleDclExecutor` 为每次调用增加一个独有 token：

```text
如果第一次条件检查不通过：
    返回 InitialConditionNotMet

为本次调用准备独立的 token
如果准备失败：
    返回 PrepareFailed

获取调用方传入的锁

如果锁内的第二次条件检查不通过：
    释放 lock guard
    通过 rollback 终结路径处理 token
    返回 SecondConditionNotMet，并携带 rollback 结果

在同一个锁保护范围内执行 task
释放 lock guard

如果 task 成功：
    通过 commit 终结路径处理 token
    返回 TaskSucceeded，并携带 commit 结果
否则：
    通过 rollback 终结路径处理 token
    返回 TaskFailed，并携带 task error 和 rollback 结果
```

prepare 在获取锁前执行；已配置的 commit 或 rollback callback（若存在）会在 guard
释放后消费 token。

## 安装与可选集成

应用的最低依赖是：

```toml
[dependencies]
qubit-dcl = "0.12"
```

这已经足够配合标准库 `AtomicBool` 和 `Mutex` 使用。只有应用实际调用额外库的 API 时，
才添加相应依赖：

- 需要 `ArcAtomic<bool>` 等便利封装时添加 `qubit-atomic = "0.16"`。Qubit DCL 不要求
  使用该 gate wrapper。
- 应用代码直接调用 `ReadWriteLock::read_lock()`、`write_lock()` 或其他
  `qubit-lock` capability 时，添加
  `qubit-lock = { version = "0.13", default-features = false }`。
- 直接使用 `parking_lot` 锁类型时，才添加后端并启用匹配 feature：

  ```toml
  [dependencies]
  qubit-dcl = { version = "0.12", features = ["parking-lot"] }
  parking_lot = "0.12"
  ```

## 场景一：刷新服务路由缓存

某服务在内存中维护路由表。配置事件会把路由表标记为失效；多个请求线程可能同时发现
该标记，但配置加载器必须只执行一次，后续请求还应完全绕过协调 mutex。

成功标准可以直接观察：恰好一个调用方加载路由，缓存出现新的 endpoint，另一个调用方
报告条件已不满足。

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

这些同步对象有意承担不同职责：`refresh_lock` 协调刷新决策；缓存自己的 `RwLock`
保护路由 vector；成功刷新后，`refresh_needed` 让常见路径跳过 `refresh_lock`。task 先
发布新路由，再关闭 gate。

task error 会被原样保留，不会被自动重试或包装：

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

加载失败后是保持 gate 开启以供后续调用重试、关闭 gate，还是采用其他重试策略，由
应用决定；Qubit DCL 不替应用选择。

## 为什么必须进行第二次检查

假设两个请求线程都观察到 `refresh_needed == true`：

| 时刻 | Worker A | Worker B |
| --- | --- | --- |
| 1 | 第一次检查返回 true。 | 第一次检查返回 true。 |
| 2 | 获取 `refresh_lock`。 | 等待 `refresh_lock`。 |
| 3 | 第二次检查返回 true；刷新路由并关闭 gate。 | 仍在等待。 |
| 4 | 释放锁。 | 获取锁。 |
| 5 | 返回 `Success`。 | 第二次检查发现 gate 已关闭，跳过加载器。 |

如果没有第二次检查，Worker B 会根据等待期间已经过期的观察结果重复刷新。只执行两次
检查，但没有让第二次检查和 task 共用同一个排他协调锁，同样不能关闭该竞争窗口。

## 场景二：终结数据库事务

有些工作拥有必须在进入协调锁前准备、离开协调锁后终结的资源。数据库事务就是具体
场景：创建事务，在 guard 下暂存语句，释放 guard 后再 commit 或 rollback。

下面的同步适配器模拟所有权，不引入数据库驱动依赖：

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

配置生命周期，并执行需要可变访问 token 的 task：

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use qubit_dcl::{
    FinalizationOutcome, LifecycleDclExecutor, LifecycleOutcome, RollbackCause,
};

let refresh_needed = Arc::new(AtomicBool::new(true));
let transaction_lock = Mutex::new(());
let database = Database::default();

let executor = LifecycleDclExecutor::builder()
    .when({
        let refresh_needed = Arc::clone(&refresh_needed);
        move || refresh_needed.load(Ordering::Acquire)
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
    refresh_needed.store(false, Ordering::Release);
    Ok::<usize, io::Error>(1)
});

assert!(matches!(
    outcome,
    LifecycleOutcome::TaskSucceeded {
        value: 1,
        commit: FinalizationOutcome::Succeeded,
    }
));
assert_eq!(
    database.committed(),
    ["INSERT INTO audit_log VALUES ('routes refreshed')"],
);
```

token 属于单次调用。`prepare` 在获取 `transaction_lock` 前执行；task 在 guard 下修改
事务；`commit` 在 guard 释放后消费事务。task 不需要访问 token 时使用 `run`，否则
使用 `run_with_token`。

builder 只暴露以下三种终结结构：

- `prepare -> commit -> rollback -> build`
- `prepare -> commit -> no_rollback -> build`
- `prepare -> no_commit -> rollback -> build`

准备出的 token 至少要有一条明确的终结路径，因此不存在
`no_commit + no_rollback` 组合。

## 选择锁模式

`qubit_lock::Lock` 表示获取模式，不一定表示排他锁。标准库 `Mutex` 和
`write_lock()` adapter 是排他的，`read_lock()` adapter 是共享的。

task 修改 gate 或受保护协议、消费工作、初始化或刷新资源，或者有其他串行执行要求
时，使用排他模式。只有同时满足以下三个条件时才使用共享模式：

1. task 对受保护协议只读；
2. 每个冲突 writer 都使用同一个底层锁的配套 write mode；
3. 调用方不要求 task 至多执行一次。

多个共享调用可能通过第二次检查并并发执行。方法名无法让 Rust 证明 closure 真正只读。

## 结果与诊断

`DclExecutor::run` 返回 `ExecutionOutcome<R, E>`：

| Variant | 含义 |
| --- | --- |
| `Success(R)` | task 在 guard 下执行并返回结果。 |
| `ConditionNotMet` | 第一次或第二次 predicate 返回 false。 |
| `TaskFailed(E)` | task 返回原始 error。 |

`into_result()` 分别把它们映射为 `Ok(Some(value))`、`Ok(None)` 和 `Err(error)`。

`LifecycleOutcome<R, E, C>` 区分首次条件拒绝、prepare failure、task success 及其
commit 结果、第二次条件拒绝及其 rollback 结果，以及 task failure 及其 rollback
结果。`FinalizationOutcome<C>` 为 `NotRequired`、`Succeeded` 或 `Failed(C)`。
`RollbackCause` 告诉 rollback callback 执行是因条件拒绝、error 还是 panic 而失败。

`DclExecutor::run_catching` 在 `PanicInfo` 中保留 panic 信息。
`LifecycleDclExecutor::run_catching` 和 `run_with_token_catching` 返回
`CapturedLifecycleOutcome`，其中 commit 与 rollback 字段使用
`CapturedFinalizationOutcome`。`PanicPhase` 可定位首次检查、prepare、获取锁、第二次
检查、task、释放锁、commit 或 rollback 阶段。捕获模式还会分别报告终结步骤无需
执行、成功、error 或 panic。

panic 捕获依赖 unwinding。使用 `panic = "abort"` 时，进程会在返回 outcome 或执行
基于 unwind 的 rollback 前退出。捕获 panic 不会撤销 task 副作用，也不能证明应用
不变量已经恢复。

## 进阶用法

- token 表示暂存的外部变更时选择 commit 和 rollback；失败时直接 drop 已足够时选择
  commit 和 `no_rollback`；只有失败清理有意义时选择 `no_commit` 和 rollback。
- 克隆后的 executor 共享 callback，各 callback 可以并发运行。predicate、prepare、
  commit 或 rollback 捕获的可变状态必须自行同步。
- executor 外部修改 gate 或冲突业务状态的代码必须使用同一个底层锁。atomic gate
  提供可见性，但不能替代锁协调。

## 错误与排障

先从可观察结果开始定位：

| 症状 | 检查项 |
| --- | --- |
| task 执行多次。 | 确认所有要求唯一性的调用都使用排他锁；共享模式允许重叠。 |
| 外部更新后 task 仍然执行。 | 确认该更新使用同一个底层锁，且 store ordering 与 predicate load 协议一致。 |
| 执行死锁。 | 确认 predicate 和 callback 没有递归获取协调锁。 |
| 出现 `PrepareFailed`。 | 检查 prepare error；此时没有 token，无法执行 rollback。 |
| rollback 失败。 | 同时保留原始 `RollbackCause` 或 task error，以及终结 failure。 |
| 标准库锁 poisoned。 | 检查 panic 是否跨越 guard；Qubit DCL 保留锁后端的 poisoning 行为。 |
| 捕获到的 panic phase 不符合预期。 | 检查 `PanicInfo::phase()` 和 `message()`，验证应用状态后再复用。 |

## 限制与最佳实践

- Qubit DCL 是同步库，接受同步 `qubit_lock::Lock` capability。
- predicate 应短小、不阻塞，也不得获取同一个底层锁。
- 先发布受保护状态，再关闭 gate；memory ordering 应符合应用协议。Acquire/Release 是
  常见起点，但不能代替具体协议分析。
- 修改 gate 的工作优先使用排他模式。共享模式只适用于不要求唯一性的真正只读 task。
- 检查每一个结构化 outcome。跳过调用、task error、rollback error 和捕获到的 panic
  具有不同的运维含义。
- rollback 是可观察的补偿步骤，不保证撤销所有外部副作用。
- Qubit DCL 不拥有锁或数据、不缓存结果、不自动重试、不提供异步 executor，也不会
  修复缺少同步的 predicate。

## 相关 Qubit 库与延伸阅读

- [`rs-atomic`](https://github.com/qubit-ltd/rs-atomic) 提供可选的 atomic 便利封装。
- [`rs-lock`](https://github.com/qubit-ltd/rs-lock) 提供可选的后端无关锁 capability 和
  read/write adapter。
- 返回[中文 README](../README.zh_CN.md)或
  [English README](../README.md)。
- 浏览 [API 文档](https://docs.rs/qubit-dcl)。
