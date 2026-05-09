# Qubit DCL

[![CircleCI](https://circleci.com/gh/qubit-ltd/rs-dcl.svg?style=shield)](https://circleci.com/gh/qubit-ltd/rs-dcl)
[![Coverage Status](https://coveralls.io/repos/github/qubit-ltd/rs-dcl/badge.svg?branch=main)](https://coveralls.io/github/qubit-ltd/rs-dcl?branch=main)
[![Crates.io](https://img.shields.io/crates/v/qubit-dcl.svg?color=blue)](https://crates.io/crates/qubit-dcl)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Doc](https://img.shields.io/badge/docs-English-blue.svg)](README.md)

**双重检查锁（double-checked locking）** 的独立 crate：在实现 [`Lock<T>`](https://docs.rs/qubit-dcl) 的锁句柄上，将「锁外先判断 → 可选 prepare → 加写锁 → 再判断 → 执行业务」固定为可复用的 `DoubleCheckedLockExecutor`，并支持可选的 **prepare / 回滚 / 提交** 与结构化执行结果。

本 crate 会再导出 `qubit-lock` 的 [`ArcMutex`](https://crates.io/crates/qubit-lock) 与 `Lock` trait，常见用法下只需在 `Cargo.toml` 中依赖 `qubit-dcl` 即可。

## 特性

- **`DoubleCheckedLockExecutor`**：通过 builder 一次配置、多次调用；与 [`qubit-function`](https://crates.io/crates/qubit-function) 的 `Tester`、可运行任务 trait 配合使用。
- **`DoubleCheckedLock`**：无需先保存 executor，支持 `on(...).when(...).call*` 风格的一次性快捷执行链。
- **完整双重检查流程**：锁外条件判断 → 可选加锁前 prepare → 写锁 → 锁内再判断 → 任务；释锁后可选对 prepare 做 **提交** 或 **回滚**。
- **多种执行入口**：无受保护数据参数的 `call` / `execute`，以及带 `&mut T` 的 `call_with` / `execute_with`。
- **明确的结果类型**：`ExecutionContext` 与 `ExecutionResult` 区分成功、条件未满足、任务失败以及 prepare 收尾阶段失败（`ExecutorError`）。
- **可配置日志**：基于 `log` 与 `ExecutionLogger`，在 builder 上为「条件未满足」与 prepare 各阶段配置日志，也可按事件关闭日志。

## 工作方式

1. **条件测试会执行两次**（加锁前一次，持写锁后再一次）。第一次判断所依据的状态，必须在不持有本执行器所关联锁的情况下仍可安全访问（例如配合合适内存序的 atomics）。
2. 若第一次通过，可配置在加锁前执行 **prepare**；持锁后再次检测条件并执行任务。
3. 若已执行过 prepare 且需收尾：任务整体成功时可选 **commit_prepare**；内层检查或任务未成功时可选 **rollback_prepare**（均在释放写锁之后执行）。

executor 不捕获 tester、prepare 回调或任务中的 panic。若任务在 prepare 成功后 panic，panic 会继续向外传播，且不会执行 prepare rollback。克隆后的 executor 并发执行时，可能有多个调用先完成 prepare，再由其中一个调用在锁内二次检查中胜出；锁内二次检查失败的调用会在配置了 rollback 时执行 prepare rollback。

## 安装

```toml
[dependencies]
qubit-dcl = "0.2.5"
```

`qubit-dcl` 已依赖并再导出 `qubit-lock` 的部分类型；仅当你需要本 crate 未再导出的其它类型时，才额外直接依赖 `qubit-lock`。

## 快速开始

```rust
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use qubit_dcl::{DoubleCheckedLockExecutor, ArcMutex, Lock, ExecutionResult};

fn main() {
    let data = ArcMutex::new(10);
    let skip = Arc::new(AtomicBool::new(false));

    let executor = DoubleCheckedLockExecutor::builder()
        .on(data.clone())
        .when({
            let skip = skip.clone();
            move || !skip.load(Ordering::Acquire)
        })
        .build();

    let updated = executor
        .call_with(|value: &mut i32| {
            *value += 5;
            Ok::<i32, std::io::Error>(*value)
        })
        .get_result();

    assert!(matches!(updated, ExecutionResult::Success(15)));
    assert_eq!(data.read(|value| *value), 15);
}
```

### 仅副作用场景（`finish`）

对 `execute` / 无返回值的 `call`，若错误类型为 `E` 且成功时值为 `()`，可在 `ExecutionContext<(), E>` 上调用 `finish()` 得到是否成功：

```rust
use qubit_dcl::{DoubleCheckedLockExecutor, ArcMutex};

let data = ArcMutex::new(());
let ok = DoubleCheckedLockExecutor::builder()
    .on(data)
    .when(|| true)
    .build()
    .execute(|| Ok::<(), std::io::Error>(()))
    .finish();
assert!(ok);
```

### 一次性快捷模式（`DoubleCheckedLock`）

当你不需要复用 executor 实例时，可以直接使用 `DoubleCheckedLock`：

```rust
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use qubit_dcl::{DoubleCheckedLock, ArcMutex, ExecutionResult, Lock};

let data = ArcMutex::new(10);
let skip = Arc::new(AtomicBool::new(false));

let updated = DoubleCheckedLock::on(data.clone())
    .when({
        let skip = skip.clone();
        move || !skip.load(Ordering::Acquire)
    })
    .call_with(|value: &mut i32| {
        *value += 5;
        Ok::<i32, std::io::Error>(*value)
    })
    .get_result();

assert!(matches!(updated, ExecutionResult::Success(15)));
assert_eq!(data.read(|value| *value), 15);
```

### 示例程序

`examples/double_checked_lock_executor_demo.rs` 提供可运行示例：

```bash
cargo run --example double_checked_lock_executor_demo
```

## Builder API（概要）

- 入口：`DoubleCheckedLockExecutor::builder()`。
- 绑定锁：`.on(lock)`，要求 `L: Lock<T>`。
- 设置双重条件：`.when(tester)`。
- 可选：`.prepare`、`.rollback_prepare`、`.commit_prepare`。
- 诊断日志：`.log_unmet_condition`、`.log_prepare_failure`、`.log_prepare_commit_failure`、`.log_prepare_rollback_failure`；对应的 `.disable_*_logging` 方法可关闭某类日志。
- 结束：`.build()` 得到可复用执行器。

## 项目结构

- `src/double_checked`：执行器、各类 builder、执行上下文与结果、错误与日志。
- `src/lock`：与执行器配合使用的锁相关工具。
- `tests/double_checked` 与 `tests/docs`：行为测试与 README 一致性测试。

## 质量检查

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
./align-ci.sh
./ci-check.sh
./coverage.sh json
```

## 许可证

Apache-2.0
