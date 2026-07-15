# rs-dcl 设计与实现评审

评审日期：2026-07-11

评审版本：`qubit-dcl 0.9.3`

## 1. 评审结论

`rs-dcl` 对“检查条件、加锁、再次检查、执行任务”这一并发惯用法做了大量工程化包装，代码、测试和错误分支实现较完整；但它当前的核心 API 没有让第二次条件检查直接观察已经持锁的 `T`，反而使最自然的用法存在自锁死锁风险。这是设计层面的结构问题，不是补充几条文档即可完全解决的问题。

此外，prepare/commit/rollback、panic capture、logging、一次性 builder、可复用 executor 和多层结果类型集中在一个很小的同步原语之上，形成了明显的能力过载。工作区内没有其他 crate 直接依赖 `qubit-dcl`，因此这些复杂性尚未得到真实下游验证，也没有兼容包袱迫使当前设计继续固化。

建议暂缓将当前 API 作为稳定基础设施推广，优先进行一次允许破坏兼容性的收缩式重设计：让锁内 predicate 显式接收 `&T`，把 fast-path predicate 与 locked predicate 分开，核心只保留 condition + mutation + typed outcome；prepare 生命周期、日志和 panic 转换应降为可选的上层能力。

综合评分：**4.5/10**。实现完整度约 **7.5/10**，核心并发语义约 **4/10**，API 聚焦度约 **4/10**，下游验证约 **2/10**。

## 2. 评审依据

### 2.1 代码与验证

当前 crate 约有 3,300 行库代码、3,300 行测试代码、12 个公开定义和约 125 个公开方法。公开定义数量不大，但多个 builder 阶段重复暴露配置方法，使方法面明显膨胀。

本轮评审执行过以下验证：

- `cargo +1.94.0 test --all-features --quiet`：127 项通过。
- nightly clippy all-target/all-feature 并启用 `-D warnings`：通过。
- rustdoc 并启用 `-D warnings`：通过。

因此，下述结论不是“现有测试失败”，而是测试主要证明了实现符合当前设计，没有覆盖或消除当前设计本身制造的误用路径。

### 2.2 下游实际使用

在整个 `rust-common` 工作区以及已检查的相邻 Qubit Rust 仓库中，没有发现其他 crate 的 `Cargo.toml` 依赖 `qubit-dcl`。当前调用主要来自自身测试、README 和示例。

这有两个含义：

1. 无法用生产调用证明 builder、prepare pipeline、panic capture 和结果层级是必要能力；
2. 现在正是修正根本 API 的低成本窗口，不应为了尚不存在的下游兼容性继续维护有问题的边界。

## 3. 值得保留的设计

### 3.1 把 double-check 流程集中起来的动机合理

手写“先查、加写锁、再查、执行”容易漏掉第二次检查或把任务放在错误的锁范围。把流程集中在一个 helper 中有价值，尤其适合 lazy initialization、去重提交和只执行一次的状态转换。

### 3.2 typed outcome 比布尔值更有诊断价值

区分成功、条件不满足、任务失败和 executor 自身失败是合理方向。调用方不应被迫把“没有执行”和“执行失败”都理解为 `false`。

### 3.3 builder 的 typestate 顺序能阻止部分配置错误

`.on(lock).when(condition).build()` 通过不同 builder 阶段限制了必要参数缺失。这个思想可以保留，但应减少每个阶段重复出现的配置方法，并让核心类型数量与实际状态数量匹配。

### 3.4 锁外 prepare、锁内 task、锁外 finalize 的范围意识是正确的

耗时准备工作和最终处理不应无条件占用写锁。当前实现有意识地把这些步骤移出临界区，这个原则值得保留。不过 prepare/rollback/commit 的业务契约必须重新命名和约束，不能暗示数据库事务式保证。

### 3.5 panic 默认传播是正确默认值

当前默认不捕获 panic，只有显式 `.catch_panics()` 才转换为 executor error。对 Rust 库而言，默认保留 unwind 语义比默认吞掉 panic 更安全。

## 4. 主要问题

### 4.1 阻断级：锁内第二次检查无法直接访问已锁定的数据

核心执行流程大致是：

```rust
let first_check = self.tester.test();
if !first_check {
    return ExecutionResult::unmet();
}

self.lock.with_write(|data| {
    let second_check = self.tester.test();
    if !second_check {
        return ExecutionResult::unmet();
    }
    task(data)
})
```

`task` 能获得 `&mut T`，但第二次 `tester` 仍是零参数 `Tester`。它不知道当前已经由 executor 持有的 `data`。如果条件就是“受该锁保护的数据是否尚未初始化”，调用方最自然的写法只能捕获同一个 `ArcMutex`，再在 tester 中调用 `read`：

```rust
let data = ArcMutex::new(None::<i32>);
let condition_data = data.clone();

let executor = DoubleCheckedLockExecutor::builder()
    .on(data)
    .when(move || condition_data.with_read(Option::is_none))
    .build();
```

第一次检查能够完成；第二次检查发生在 `write` 临界区内，再次读取同一把非可重入锁，于是死锁。本轮评审已用该自然写法复现超时。

README 通过要求 tester 使用独立原子变量规避这一问题，但这把 API 限制为“外部 fast flag 的 double check”，并不能安全表达一般的“检查被锁对象本身”。crate 的名字和 `when` 方法没有暴露这个限制。

建议从签名上修复，而不是只写警告：

- 将第一次无锁检查建模为独立的 `fast_check: Fn() -> bool`，并明确它只能读取原子或其他无需该锁的数据；
- 将第二次检查建模为 `locked_check: Fn(&T) -> bool`，直接使用 executor 已持有的引用；
- 如果不需要 fast path，提供只执行“加锁、检查 `&T`、修改 `&mut T`”的简化入口；
- 禁止通过同一个零参数 closure 同时承担锁外和锁内两种语义。

核心问题位于 `src/double_checked/double_checked_lock_executor.rs` 的 `execute_with_write_lock`。

### 4.2 高优先级：`catch_panics` 改变 poison 语义并保留部分写入

任务在获得 `&mut T` 后由 `try_run` 内部的 `catch_unwind` 捕获。panic 在离开 `lock.write` closure 之前已经被转换为 `ExecutionResult`，因此：

- 对标准库 mutex，guard 看不到 unwind，锁不会按通常语义进入 poisoned 状态；
- 任务在 panic 前对 `T` 的修改仍然保留；
- 返回值是结构化 `ExecutorError::Panic`，容易让调用方误以为状态也被安全回滚；
- panic hook 默认仍会输出 panic 信息，“捕获”不等于静默。

本轮评审已验证：任务先修改受保护 `Vec` 再 panic，调用返回 panic failure，但修改保留，标准库锁也没有 poisoned。

这对通用库是危险契约，因为 panic 往往表示某个不变量可能在中途被破坏。建议：

1. 核心 API 保持 panic 传播，不在持锁 closure 内转换 panic。
2. 若保留捕获能力，应在 `lock.with_write(...)` 外层执行 `catch_unwind`，让 guard 先按底层锁语义完成 unwind/poison。
3. 明确说明任何 panic capture 都不提供受保护数据的回滚。
4. 对需要回滚的场景，应要求用户任务先计算新值再一次性提交，或使用显式 snapshot/transaction abstraction，而不是依赖通用 `catch_panics`。

### 4.3 高优先级：prepare/commit/rollback 名称暗示了实际不存在的事务保证

prepare 在加锁前执行；多个并发调用可以都完成 prepare，只有一个调用通过第二次检查，其余调用再执行 rollback。commit/rollback 又在释放锁后运行。这套流程可以表达“预留外部资源并在结果后补偿”，但它不具备数据库事务或内存事务的原子性：

- prepare 必须允许并发和重复；
- rollback 必须可补偿且通常需要幂等；
- task 对 `T` 的部分修改不会因 task error 或 panic 自动回滚；
- commit 失败时，受保护数据可能已经成功修改；
- rollback 失败时，原任务结果还会与 finalize error 发生优先级选择。

类型系统没有表达这些前置条件，名称却容易制造强保证的预期。建议把该能力从 DCL core 移到独立 coordinator/lifecycle 层，并使用 `before_lock`、`on_applied`、`on_skipped_or_failed` 等不暗示原子事务的名称。若保留 prepare 术语，文档必须明确“补偿回调，不是状态回滚”。

### 4.4 高优先级：一个小型同步惯用法承载了过多职责

当前 crate 同时负责：

- double-check 控制流；
- lock abstraction；
- `Tester`、`Callable`、`Runnable` callback abstraction；
- 一次性 fluent API；
- 可复用 executor；
- prepare/commit/rollback 生命周期；
- panic 捕获和 payload 转换；
- logging policy；
- 多层 result/context/error 转换。

这导致核心私有函数 `execute_with_write_lock` 具有较高分支复杂度，公开 builder 上同一组开关也在多个 typestate 阶段重复出现。对没有生产下游的 crate，这属于超前抽象，而不是由使用反馈形成的复杂度。

建议把最小核心收缩为：

```text
fast check（可选）
  -> 获取写锁
  -> locked check(&T)
  -> task(&mut T)
  -> Applied / Skipped / Failed
```

日志、panic 转换和补偿生命周期由调用方或可选 adapter 处理。

### 4.5 中优先级：`ExecutionContext`、`ExecutionResult` 与 `ExecutorError` 层级偏重

调用一次任务后先得到 `ExecutionContext<R, E>`，再通过 `get_result`、`finish`、`try_finish` 等方法转换到不同信息量的结果。`finish` 会把条件不满足和执行失败都压成 `false`，而 `try_finish` 又保留部分错误。这套 API 给简单调用增加了选择成本。

建议用一个直接、穷尽的公开结果表达核心状态，例如：

```rust
enum DclOutcome<R, E> {
    Applied(R),
    Skipped,
    Failed(E),
}
```

如果 executor 自身还可能失败，再使用 `Result<DclOutcome<R, E>, DclError>`。便利方法可以作为少量显式转换存在，不需要额外 context wrapper 承载整组近义方法。

### 4.6 中优先级：一次性入口与可复用 executor 重复了大部分 API

`DoubleCheckedLock` fluent chain 和 `DoubleCheckedLockExecutor` builder 最终进入同一执行模型，但各自又暴露 panic、logging、prepare 等配置。它们降低了少量调用代码，却扩大了文档、测试和兼容面。

建议只保留一个核心配置对象：

- 如果配置通常复用，就保留 executor builder，并提供一个简单自由函数处理一次性调用；
- 如果调用通常一次性，就保留函数式入口，可复用场景由用户保存配置对象；
- 不要为“少写一个局部变量”复制完整 builder surface。

### 4.7 低优先级：重导出依赖类型模糊 crate 边界

crate root 重导出 `qubit_lock::ArcMutex` 和 `Lock`，让示例只依赖 `qubit-dcl`，但也让调用方难以判断 lock 类型真正由哪个 crate 定义。若未来 `qubit-lock` API 或版本变化，`rs-dcl` 也要承担路径兼容责任。

建议核心签名继续依赖 `qubit_lock::Lock`，但文档鼓励应用对自己直接使用的 lock 类型声明直接依赖。只有 `rs-dcl` 自己定义的类型从其 root 导出。

## 5. 建议的重设计方向

### 5.1 最小核心签名

推荐首先验证如下级别的 API，而不是立即恢复全部 builder 能力：

```rust,ignore
execute_double_checked(
    &lock,
    || fast_flag.load(Ordering::Acquire),
    |state: &T| locked_condition(state),
    |state: &mut T| apply(state),
)
```

语义要求：

- `fast_check` 不得获取同一把锁；
- `locked_check` 一定在持有写锁后执行，并直接观察 `&T`；
- `task` 只在两个 check 都通过时执行；
- panic 默认传播；
- 返回值直接区分 applied、skipped 和 task error。

同时提供无 fast path 的简化版本：

```rust,ignore
execute_if_locked(&lock, |state| condition(state), |state| apply(state))
```

### 5.2 可选能力分层

在最小核心获得真实下游后，再按证据增加：

1. 可复用 executor，仅保存 lock 和两个 predicate；
2. observer/log hook，只观察事件，不改变控制流；
3. compensating lifecycle adapter，明确无事务保证；
4. panic-to-error adapter，放在锁调用外，并明确 poison 与部分写入语义。

### 5.3 迁移顺序

1. 先新增能够接收 `&T` 的 locked predicate，并为自然 DCL 用法建立并发测试。
2. 将旧零参数 tester 标记为仅适用于独立 atomic fast flag，并进入弃用流程。
3. 从核心执行路径移出锁内 panic 捕获。
4. 收缩结果类型和重复 builder。
5. 找到至少一个真实生产下游后，再决定 prepare lifecycle 是否值得保留。

## 6. 最终意见

`rs-dcl` 的问题不是代码写得粗糙，而是实现得很完整的 API 建立在一个不够自然的核心 predicate 边界上。继续补测试、日志开关或 builder 方法会进一步固化错误抽象。

由于目前没有生产下游，建议利用低迁移成本窗口先重做核心：**锁外检查显式独立，锁内检查直接接收受保护数据，panic 不伪装成回滚，外围生命周期从核心剥离**。完成这一点后，crate 才适合继续推广。
