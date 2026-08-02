# Qubit DCL

[![Rust CI](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-dcl/coverage-badge.json)](https://qubit-ltd.github.io/rs-dcl/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-dcl.svg?color=blue)](https://crates.io/crates/qubit-dcl)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

Qubit DCL 为使用 atomic 或等价同步 gate 的 Rust 应用提供可复用的同步双重检查锁
executor。它把重复出现的 check-lock-check-task 流程固定下来，同时把锁所有权、
受保护数据和 memory ordering 的决定权保留给应用。

典型场景是懒初始化：多个请求处理器都可能观察到昂贵资源仍需初始化，但只有在排他锁
下通过第二次检查的处理器才应真正执行初始化。

## 安装

Qubit DCL 要求 Rust 1.94 或更高版本。标准库路径使用后端无关的
qubit-lock capability 和 qubit-atomic gate 封装：

```toml
[dependencies]
qubit-dcl = "0.12"
qubit-atomic = "0.16"
qubit-lock = { version = "0.13", default-features = false }
```

parking-lot 后端是可选的。应用使用 parking_lot 锁时，同时在 qubit-dcl 和
qubit-lock 中启用匹配 feature：

```toml
[dependencies]
qubit-dcl = { version = "0.12", features = ["parking-lot"] }
qubit-atomic = "0.16"
qubit-lock = "0.13"
parking_lot = "0.12"
```

qubit-atomic 是应用为 gate 选择的库，并不是 qubit-dcl 的强制运行时依赖。
qubit-lock 提供 executor 使用的锁 capability，具体锁后端由应用直接声明。

## 快速开始

下面示例使用 qubit-atomic 的 ArcAtomic<bool>，以及 qubit-lock 提供的排他
write_lock() adapter：

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

let outcome = executor.run(&write_mode, {
    let gate = gate.clone();
    move || {
        gate.store(false);
        Ok::<usize, std::io::Error>(42)
    }
});

assert!(matches!(outcome, ExecutionOutcome::Success(42)));
assert!(matches!(
    executor.run(&write_mode, || Ok::<(), std::io::Error>(())),
    ExecutionOutcome::ConditionNotMet
));
```

第一次检查不获取锁。第二次检查和 task 在 write_lock() 选出的同一个 guard 内执行。
task 关闭 gate 后，后续调用会在请求 guard 前返回 ExecutionOutcome::ConditionNotMet。
task 的错误会原样保留在 ExecutionOutcome::TaskFailed(error) 中。

## 两类执行模型

DclExecutor 是小而通用的路径。使用 DclExecutor::new(predicate) 构造，
每次 run 调用传入一个 qubit_lock::Lock 模式；需要捕获 panic 时使用 run_catching，
并读取 PanicInfo。

LifecycleDclExecutor 适用于每次调用都需要独有 token 的流程：token 在获取锁前
prepare，并在 guard 释放后完成终结。typestate builder 要求先配置 predicate、
prepare callback，再选择有效的 commit/rollback 组合。task 需要修改 token 时使用
run_with_token，否则使用 run。

公开结果模型包括：

- ExecutionOutcome：基础成功、条件拒绝和 task error。
- LifecycleOutcome 与 FinalizationOutcome：生命周期成功和失败路径。
- RollbackCause：传给 rollback callback 的失败原因。
- CapturedLifecycleOutcome、CapturedFinalizationOutcome、PanicInfo 和 PanicPhase：
  panic-aware 生命周期执行的结果与诊断信息。

完整工作流和结果表请阅读[中文用户手册](doc/user_guide.zh_CN.md)与
[English User Guide](doc/user_guide.md)。

## 为什么需要这个项目

双重检查锁容易描述，也容易被重复实现错：fast path 可能无谓地获取锁，第二次检查
可能被漏掉，共享 read lock 也可能被误当成至多执行一次机制。Qubit DCL 把执行顺序
明确固定为：

```text
initial predicate -> lock selected by this call -> second predicate -> task
```

executor 不拥有锁，也不拥有锁保护的数据。因此，同一个 executor 可以复用同一读写锁
的兼容模式，而每个 task 要捕获哪些数据仍由应用决定。

## 契约与边界

- predicate 必须读取 atomic 或具有等价同步语义的 gate。常见配对是 Acquire load
  和 Release store；ArcAtomic 提供这样的默认值。
- predicate 不得获取同一个底层锁，也不应阻塞。
- 当 task 修改 gate 或受保护协议、消费工作或要求串行执行时，必须使用 write_lock()
  或其他排他模式。
- read_lock() 只适用于相对于受保护协议只读的 task，且冲突 writer 使用同一底层锁的
  配套 write mode。共享调用可以重叠，不提供至多执行一次保证。
- 外部修改 gate 或受保护状态的代码，必须通过与冲突 executor 调用相同的底层锁协调。
- Qubit DCL 不拥有锁、不暴露受保护数据、不缓存 task 结果、不替应用选择 memory
  ordering，也不会让非 atomic predicate 自动变成同步操作。

## 相关 Qubit 库

- [rs-atomic](https://github.com/qubit-ltd/rs-atomic)：提供易用的 atomic value 和
  ArcAtomic<bool> 等 shared-owner gate 封装。
- [rs-lock](https://github.com/qubit-ltd/rs-lock)：提供后端无关的同步锁 capability，
  包括 ReadWriteLock、read_lock() 和 write_lock()。
- [rs-dcl](https://github.com/qubit-ltd/rs-dcl)：把同步 gate、调用方选择的锁模式和
  双重检查执行策略组合起来。

## 延伸阅读

- 阅读完整[中文用户手册](doc/user_guide.zh_CN.md)。
- Read the full [English User Guide](doc/user_guide.md).
- 浏览 [API 文档](https://docs.rs/qubit-dcl)。
- Read the [English README](README.md)。
- 查看相关的 [rs-atomic](https://github.com/qubit-ltd/rs-atomic) 和
  [rs-lock](https://github.com/qubit-ltd/rs-lock) 仓库。

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
Pull Request 前运行 ./align-ci.sh 格式化代码，运行 ./ci-check.sh 对齐 CI 要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[https://github.com/qubit-ltd/rs-dcl](https://github.com/qubit-ltd/rs-dcl)
