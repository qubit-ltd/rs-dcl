# Qubit DCL

[![Rust CI](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-dcl/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-dcl/coverage-badge.json)](https://qubit-ltd.github.io/rs-dcl/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-dcl.svg?color=blue)](https://crates.io/crates/qubit-dcl)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

Qubit DCL 为“多数时候无需执行、一旦需要则必须串行执行”的工作提供可复用的同步
双重检查执行流程。它统一处理 check-lock-check-task 顺序，同时仍由应用拥有 gate、
协调锁、业务数据和 task 副作用。

## 安装

Qubit DCL 要求 Rust 1.94 或更高版本。使用标准库 gate 和 mutex 时，应用只需添加：

```toml
[dependencies]
qubit-dcl = "0.12"
```

这条最小路径不需要 atomic 辅助库，也不要求应用直接依赖 `qubit-lock`。

## 快速开始

假设配置发生变化后，应用把服务路由缓存标记为失效。多个请求线程可能同时发现该
标记，但配置只应加载一次。下面的示例只使用 Qubit DCL 和标准库同步原语：

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

一个 worker 刷新缓存；另一个 worker 要么在 fast path 直接看到已经关闭的 gate，要么
在等待 mutex 后被第二次检查拦下。两种情况下都不会重复执行昂贵加载。

## 工作原理

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

第一次检查让无需工作的调用避免获取锁；第二次检查关闭首次观察与成功获取锁之间的
竞争窗口。从任一锁内分支返回时，guard 都会按 RAII 自动释放。

## 选择执行器

- `DclExecutor` 在两次 predicate 都通过时，使用调用方传入的锁执行 task。它返回
  `ExecutionOutcome`，并原样保留 task error。
- `LifecycleDclExecutor` 为每次调用增加独有 token：获取锁前 prepare，task 中使用或
  修改 token，释放 guard 后再 commit 或 rollback。typestate builder 会阻止不完整的
  终结策略。

调用方需要结构化的 `PanicInfo` 而不是正常传播 panic 时，使用 `run_catching` 或
`run_with_token_catching`。

## 契约与边界

- predicate 必须读取 atomic 或具有等价同步语义的 gate。Acquire load 配对 Release
  store 是一种常见起点。
- predicate 不得获取同一个底层锁，也不应阻塞。
- task 会修改 gate、消费工作、改变受保护协议或要求至多执行一次时，使用 `Mutex`、
  `write_lock()` 或其他排他模式。
- 共享 `read_lock()` 允许 task 重叠执行，不提供至多一次保证。
- executor 外部修改 gate 或冲突业务状态的代码，必须通过同一个底层锁协调。
- Qubit DCL 不拥有锁或业务数据、不缓存 task 结果、不自动重试、不替应用选择 memory
  ordering，也不会让缺少同步的 predicate 自动变安全。

## 可选集成

上面的最小示例不需要以下任何附加依赖：

- 只有应用需要 `ArcAtomic<bool>` 等 atomic value 或 shared-owner wrapper 时，才添加
  [`qubit-atomic`](https://github.com/qubit-ltd/rs-atomic)。
- 只有应用代码要直接调用 `ReadWriteLock::read_lock()`、`write_lock()` 等 capability
  时，才直接添加 [`qubit-lock`](https://github.com/qubit-ltd/rs-lock)。Qubit DCL 已在
  内部使用它。
- 需要直接传入 `parking_lot` 锁时，启用 Qubit DCL 的匹配 feature，并添加所选后端：

  ```toml
  [dependencies]
  qubit-dcl = { version = "0.12", features = ["parking-lot"] }
  parking_lot = "0.12"
  ```

## 延伸阅读

- 阅读完整[中文用户手册](doc/user_guide.zh_CN.md)或
  [English User Guide](doc/user_guide.md)。
- 浏览 [API 文档](https://docs.rs/qubit-dcl)。
- Read the [English README](README.md)。

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
