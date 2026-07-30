# Qubit DCL

[![Rust CI](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-dcl/coverage-badge.json)](https://qubit-ltd.github.io/rs-dcl/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-dcl.svg?color=blue)](https://crates.io/crates/qubit-dcl)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

Qubit DCL 为 Rust 提供可复用的双重检查锁 executor。它帮助并发应用跳过已经不需要的
工作，而不必在业务代码中重复拼接容易出错的同步流程。调用方提供 atomic 或具有等价
同步语义的 gate，并在每次执行时选择锁模式；executor 先无锁检查，再在该模式下复查，
只有两次都满足条件时才运行 task。

## 安装

```toml
[dependencies]
qubit-dcl = { version = "0.11", features = ["parking-lot"] }
qubit-lock = "0.13"
parking_lot = "0.12"
```

Qubit DCL 要求 Rust 1.94 或更高版本。使用该后端时，请显式启用可选的
`parking-lot` feature。只使用标准库锁时不需要启用 DCL feature：

```toml
qubit-dcl = "0.11"
qubit-lock = { version = "0.13", default-features = false }
```

Qubit DCL 不重导出 `qubit_lock::Lock` 或其他 crate 所拥有的锁原语。请直接声明
`qubit-lock` 和所选锁后端依赖。

## 快速开始

假设多个请求处理器都可能初始化同一个昂贵资源。第一个处理器在 mutex 内关闭 atomic
gate；后续处理器看到 gate 已关闭后，不尝试获取锁便直接返回。

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

第一次 predicate 返回 `false` 时，executor 不调用任何锁方法，直接返回
`ExecutionOutcome::ConditionNotMet`。task error 会原样保存在
`ExecutionOutcome::TaskFailed(E)` 中。

## 为什么需要这个 crate

双重检查锁很容易描述，却很容易实现得不一致：fast path 可能意外获取锁，第二次检查
可能被遗漏，或者共享 read lock 被误当成“至多执行一次”机制。Qubit DCL 明确固定执行
顺序，同时把锁所有权和受保护数据保留在调用方。

传给 `run` 的锁是协调模式，而不是 executor 持有的状态。因此，只要 task 遵守同一个
gate 协议，读者与 writer 可以使用同一个 executor，并传入同一 RWLock 产生的配套模式。

## 它提供什么

- `DoubleCheckedLockExecutor`：复用 predicate、调用方选择
  `qubit_lock::Lock`，并获得结构化 `ExecutionOutcome`。
- `LifecycleDoubleCheckedLockExecutor`：适用于先准备每次调用独有的 token，再在
  锁内执行后 commit 或 rollback 的工作流。
- `LifecycleOutcome`、`FinalizationOutcome`、`RollbackCause`、`PanicInfo` 和
  `PanicPhase`：用于检查全部生命周期终态。
- 可选 `catch_panics(true)`：调用方需要结构化 panic 元数据而不是传播时使用。

它不拥有锁、不暴露受保护数据、不缓存 task 结果、不替应用选择 memory ordering，也不会
把共享锁模式变成排他执行。修改 gate 或受保护状态，或要求 task 至多执行一次时，必须
使用排他锁模式或独立唯一性协议。

## 延伸阅读

- 阅读完整[用户手册](doc/user_guide.zh_CN.md)，了解同步契约、读写协调、生命周期
  token、panic 处理、排障与限制。
- Read the full [English User Guide](doc/user_guide.md).
- 浏览 [API 文档](https://docs.rs/qubit-dcl)。
- Read the [English README](README.md).

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
