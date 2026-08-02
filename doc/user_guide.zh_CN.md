# Qubit DCL 用户手册

[English](user_guide.md) · [中文 README](../README.zh_CN.md) ·
[API 文档](https://docs.rs/qubit-dcl)

本手册适用于 qubit-dcl 0.12，面向需要在同步 Rust 应用中复用双重检查执行策略的
开发者。手册分别用两个独立场景讲解基础 executor 和生命周期 executor。读者应已经
拥有一个 atomic 或等价同步 gate。

## 手册目标与读者

当多个调用方可能同时判断某项工作是否仍需执行，而 task 只有在 gate 仍然满足时才应
运行，可以使用 Qubit DCL。crate 负责协调两次检查和锁边界；gate、受保护数据、所选
锁模式和 task 的副作用仍由应用拥有。

本手册不会把任意 boolean 自动变成同步协议。gate 必须是 atomic 或具有等价同步语义，
冲突更新必须使用与 executor 调用相同的底层锁。

## 概念模型

基础 DclExecutor 为每次调用执行下列流程：

```text
initial predicate
  false ──> ExecutionOutcome::ConditionNotMet
  true  ──> acquire this call's Lock
               └─> second predicate
                    false ──> ConditionNotMet
                    true  ──> run task under that guard
```

第一次检查不获取锁。第二次检查和 task 共用本次调用传入
qubit_lock::Lock 产生的 RAII guard。executor 不拥有锁或锁保护的数据。

predicate 必须执行 atomic 或等价同步读取。使用 qubit_atomic::ArcAtomic 时，
load() 默认使用 Acquire，store() 默认使用 Release。Acquire load 配对 Release store
是常见起点。predicate 不得获取同一个底层锁，也不得阻塞。

LifecycleDclExecutor 额外为每次调用创建独有 token：

```text
initial predicate -> prepare token -> lock -> second predicate -> task
                                      -> release guard -> commit or rollback
```

prepare 在获取锁前运行。task 可以在 guard 持有期间修改 token；guard 释放后，commit
或 rollback 才会消费该 token。

## 安装与最小配置

本手册使用三个 Qubit 库：

- qubit-dcl：提供 executor 和结构化结果。
- qubit-atomic：提供 ArcAtomic<bool> 这样的共享 gate 封装。
- qubit-lock：提供后端无关的 Lock 与 ReadWriteLock capability。

使用标准库锁：

```toml
[dependencies]
qubit-dcl = "0.12"
qubit-atomic = "0.16"
qubit-lock = { version = "0.13", default-features = false }
```

使用 parking_lot 时启用对应的可选 feature：

```toml
[dependencies]
qubit-dcl = { version = "0.12", features = ["parking-lot"] }
qubit-atomic = "0.16"
qubit-lock = "0.13"
parking_lot = "0.12"
```

上面的 qubit-atomic 和 qubit-lock 版本是应用的依赖选择。Qubit DCL 运行时确实需要
qubit-lock，但不强制要求 qubit-atomic；调用方也可以提供其他同步 gate。

## 场景一：初始化昂贵资源

多个请求处理器都可能发现某个资源仍需初始化。成功标准是一个排他 task 完成初始化
并关闭 gate，后续调用无需获取锁就返回。

### 构造 executor

使用 ArcAtomic<bool> 表示 gate，使用标准库读写锁作为协调对象。ReadWriteLock 的
write_lock() 会生成 DclExecutor::run 所需的排他 Lock adapter。

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

executor 可以复用。每次调用传入锁模式，因此后续调用也可以使用同一底层读写锁产生的
其他兼容模式。

### 执行 task

task 只有进入排他模式后才修改 gate：

```rust
let outcome = executor.run(&write_mode, {
    let gate = gate.clone();
    move || {
        // 在这里初始化资源。
        gate.store(false);
        Ok::<usize, std::io::Error>(42)
    }
});

assert!(matches!(outcome, ExecutionOutcome::Success(42)));
```

后续调用在第一次检查处失败，不会调用锁：

```rust
assert!(matches!(
    executor.run(&write_mode, || Ok::<(), std::io::Error>(())),
    ExecutionOutcome::ConditionNotMet
));
```

task error 会被 Qubit DCL 原样保留：

```rust
let failed = executor.run(&write_mode, || {
    Err::<(), _>(std::io::Error::other("resource unavailable"))
});

assert!(matches!(failed, ExecutionOutcome::TaskFailed(error)
    if error.kind() == std::io::ErrorKind::Other));
```

当调用方希望使用 Result<Option<R>, E> 流程时，可以调用
ExecutionOutcome::into_result()：

```rust
let result = executor
    .run(&write_mode, || Ok::<usize, std::io::Error>(7))
    .into_result();

assert_eq!(result.ok().flatten(), None);
```

因为 gate 已关闭，上例返回 Ok(None)。成功 task 会返回 Ok(Some(value))，task error
则返回 Err(error)。

### 该场景说明了什么

- gate 关闭后，第一次检查可以避免获取锁。
- 第二次检查消除了第一次观察和获取锁之间的竞态。
- 第二次检查和 task 共用同一个 guard。
- write_lock() 表达排他模式，但 executor 不拥有读写锁。
- TaskFailed(E) 是应用错误，不是库统一包装的错误。

## 场景二：暂存数据库事务

数据库服务可能需要在进入协调锁前准备事务对象，在 guard 内暂存写入，并在执行成功后
才发布这些写入。下面定义一个完整、同步且由应用拥有的 adapter；它用于说明事务
所有权，不添加具体数据库驱动依赖。

### 定义事务 adapter

Database 保存已提交的 statement。DatabaseTransaction 拥有待提交 statement 和共享
数据库 handle。commit 追加待提交内容，rollback 丢弃待提交内容。

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

这个 adapter 是同步且刻意保持很小的。实际数据库驱动可以替换这些应用方法，但仍应
保留相同生命周期边界：每次调用准备 token，task 暂存工作，之后由 commit 或 rollback
消费 token。

### 配置生命周期 callback

predicate、协调锁和 builder 使用与场景一相同的 gate 与 write_lock()：

```rust
use std::io;

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

typestate builder 暴露以下有效组合：

- prepare -> commit -> rollback -> build
- prepare -> commit -> no_rollback -> build
- prepare -> no_commit -> rollback -> build

有意不存在 no_commit + no_rollback。准备出的 token 必须有明确的成功或失败终结策略。

### 使用 token 执行

当 task 需要修改事务 token 时，使用 run_with_token：

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
assert_eq!(
    database.committed(),
    vec!["INSERT INTO audit_log VALUES ('ready')".to_owned()]
);
```

task 在排他 guard 持有期间执行。guard 释放后才运行 commit；commit 消费 token，并使
暂存 statement 可见。

task 不需要 token 时使用 run。prepare 后第二次检查失败时仍然有 token 可 rollback，
因此结果是 LifecycleOutcome::SecondConditionNotMet { rollback }。task error 产生
LifecycleOutcome::TaskFailed { error, rollback }。prepare error 产生
LifecycleOutcome::PrepareFailed(error)，此时不存在可 rollback 的 token。

## 选择锁模式

`Lock` 表示获取模式，并不必然表示排他性。write_lock() adapter 表示 ReadWriteLock
的排他模式，read_lock() adapter 表示共享模式。

当 task 修改 gate 或受保护协议、消费工作、初始化资源或要求串行执行时，使用排他模式。
只有同时满足以下条件时才可以使用共享模式：

1. task 相对于受保护协议是只读的。
2. 所有冲突 writer 使用同一个底层锁配套的 write mode。
3. 调用方不要求 task 至多执行一次。

多个共享调用可以通过第二次检查并并发执行。这是有意的语义。共享 read mode 不是唯一
性机制；不同方法名称也不能让 Rust 验证闭包实际上是否只读。

同一个 executor 可以接收同一读写锁产生的配套模式，而 task 捕获不同的数据。正确性
依赖共享 gate 协议，以及冲突更新使用同一个底层锁，而不是依赖 task 是否访问同一个
对象。

## 结果与诊断

### 基础结果

DclExecutor::run 返回 ExecutionOutcome<R, E>：

| Variant | 含义 |
| --- | --- |
| Success(R) | task 在选定 guard 内运行并返回值。 |
| ConditionNotMet | 第一次或第二次条件检查返回 false。 |
| TaskFailed(E) | task 返回原始 error。 |

ExecutionOutcome::into_result() 将它们分别映射为 Ok(Some(value))、Ok(None) 和
Err(error)。

DclExecutor::run_catching 返回 Result<ExecutionOutcome<R, E>, PanicInfo>。它捕获
两次 predicate、获取锁、task 执行和释放锁 guard 时的 panic。不使用 run_catching
时，这些 panic 会正常传播。

### 生命周期结果

LifecycleOutcome<R, E, C> 表示未捕获 panic 的全部生命周期终态：

| Variant | 含义 |
| --- | --- |
| InitialConditionNotMet | 第一次检查在 prepare 前拒绝调用。 |
| PrepareFailed(C) | prepare 返回 error，未创建 token。 |
| TaskSucceeded { value, commit } | task 成功，token 进入 commit 或 no-commit 终结。 |
| SecondConditionNotMet { rollback } | 第二次检查拒绝已准备的 token。 |
| TaskFailed { error, rollback } | task 返回原始 error，token 进入 rollback。 |

FinalizationOutcome<C> 可以是 NotRequired、Succeeded 或 Failed(C)。
RollbackCause 会告诉 rollback callback 原因是 ConditionNotMet、TaskFailed(error)
还是 Panicked(panic)。

### 捕获生命周期结果

LifecycleDclExecutor::run_catching 和 run_with_token_catching 返回
CapturedLifecycleOutcome<R, E, C>：

- InitialConditionNotMet 与 InitialConditionCheckPanicked(PanicInfo) 表示第一次检查。
- PrepareFailed(C) 与 PreparePanicked(PanicInfo) 表示 prepare 阶段。
- TaskSucceeded { value, commit } 表示成功执行。
- SecondConditionNotMet { rollback } 表示已准备 token 被第二次检查拒绝。
- TaskFailed { error, rollback } 保留 task error。
- ExecutionPanicked { panic, rollback } 保留执行阶段捕获的 panic 与 rollback 结果。

CapturedFinalizationOutcome<C> 区分 NotRequired、Succeeded、Failed(C) 和
Panicked(PanicInfo)。终结 callback 的 panic 会进入结构化结果，不替换原结果。

PanicInfo 保留 panic phase、可选的字符串 message() 和原始 payload，可供检查或调用
resume_unwind()。PanicPhase 精确区分以下阶段：

- InitialConditionCheck
- Prepare
- LockAcquisition
- SecondConditionCheck
- Task
- LockRelease
- Commit
- Rollback

捕获 panic 依赖 unwind 策略。使用 panic = "abort" 时，进程会在返回 outcome 或执行
基于 unwind 的 rollback 前终止。捕获 panic 不会撤销已经发生的副作用，也不能证明应用
不变量已经恢复。

## 进阶用法

### 选择生命周期形状

typestate builder 会在编译期阻止不完整的 callback 策略。根据所有权选择 commit 和
rollback：

- token 表示暂存外部变更时，使用 commit 与 rollback。
- 失败路径只需安全丢弃 token 时，使用 commit 与 no_rollback。
- 只有失败清理有意义时，使用 no_commit 与 rollback。

### 把 callback 当作可并发调用

executor 通过共享所有权保存 callback，并可能在并发调用中执行它们。when、prepare、
commit 或 rollback 捕获的任何可变状态都需要独立同步。token 属于单次调用，不是共享
executor 状态。

### 保留锁边界

外部修改 gate 或受保护数据的代码必须使用与冲突 executor 调用相同的底层锁。atomic
gate 提供可见性，但不能替代锁协调。ptr::read_volatile 用于 MMIO 等 volatile memory，
不是同步机制。

## 错误与诊断

从结果产生的阶段开始检查：

| 症状 | 首要检查 |
| --- | --- |
| task 执行多次。 | 使用 write_lock() 或显式 compare-and-exchange 唯一性协议；read_lock() 允许重叠。 |
| 外部状态变化后 task 仍执行。 | 用同一个底层锁协调该变化，并让 Release store 与 predicate 的 Acquire load 配对。 |
| 出现 PrepareFailed。 | 检查 prepare error；此时没有 token，rollback 不能运行。 |
| rollback 结果为 Failed(C)。 | 同时保留原始 RollbackCause 和终结 error。 |
| 标准 mutex 被 poisoning。 | 检查 panic 是否跨越标准库 guard；该 poisoning 会被有意保留。 |
| 捕获到意外 panic。 | 检查 PanicInfo::phase() 和 PanicInfo::message()，再次使用状态前验证应用不变量。 |

## 排障

先根据可观察结果缩小范围，再调整 gate 或锁协议：

1. 如果 task 执行多次，确认所有要求唯一性的调用都使用 write_lock() 或其他排他/atomic
   唯一性协议。
2. 如果外部状态变化后 task 仍执行，确认该变化使用同一个底层锁，并让 Release store
   与 predicate 的 Acquire load 配对。
3. 如果出现死锁，确认 predicate 和 callback 没有再次获取 executor 的底层锁。
4. 如果终结失败，同时保留原始 RollbackCause 或 task error，以及描述 callback 失败的
   FinalizationOutcome。
5. 如果捕获到意外的 panic phase，先检查 PanicInfo，再复用应用状态；捕获 panic 不会
   恢复外部副作用。

## 限制与最佳实践

- Qubit DCL 是同步库，接收同步 qubit_lock::Lock capability。
- 它不发现或拥有锁，不暴露受保护数据，不缓存 task 结果，也不替应用选择
  memory-ordering 协议。
- 它不会让非 atomic predicate 自动安全，也不会把共享模式变成排他执行。
- predicate 应短小、无阻塞，并且不得获取同一把锁。
- 修改 gate 的 task 应优先使用排他模式；共享模式只适用于确实只读且不要求至多一次的工作。
- 把生命周期 rollback 当作可观察的补偿阶段，不要把它当成所有外部副作用都已撤销的保证。
- callback 可能并发运行，因此 callback 捕获的可变状态必须同步。

## 相关 Qubit 库与延伸阅读

- [rs-atomic](https://github.com/qubit-ltd/rs-atomic)：atomic value 与 ArcAtomic
  shared-owner 封装。
- [rs-lock](https://github.com/qubit-ltd/rs-lock)：后端无关的锁 capability 与读写 adapter。
- [rs-dcl](https://github.com/qubit-ltd/rs-dcl)：本手册介绍的 executor。
- 返回 [中文 README](../README.zh_CN.md) 或 [English README](../README.md)。
- 浏览 [API 文档](https://docs.rs/qubit-dcl)。
