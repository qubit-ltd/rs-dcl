# rs-dcl 第一性原理重设计

日期：2026-07-18

状态：已批准并实现；完整发布验证等待 `qubit-lock 0.10.0` 发布

目标版本：0.10.0

目标仓库：`rs-dcl`

参考实现：

- `java-common` 中的
  `ltd.qubit.commons.concurrent.DoubleCheckedLockExecutor`
- `java-common` 中的
  `ltd.qubit.commons.service.impl.TaskExecutionServiceImpl`

## 1. 决策摘要

本次重设计允许完全推翻当前实现，不保留源代码或行为兼容层。

rs-dcl 重新定义为一个双重检查锁设计模式的模板执行器：

1. predicate 观察一个可无锁读取的、具备线程间同步语义的 gate。
2. 第一次 predicate 在锁外执行；失败时不产生任何锁开销。
3. 第一次 predicate 成功后才获取锁，并在锁内调用同一个 predicate 复查。
4. 第二次 predicate 仍成功时，在锁内执行任意 task。
5. task 已处于 executor 的锁内，因此可以直接修改其捕获的 gate，也可以按业务需要保持 gate 不变。
6. task 不以读取或修改 Lock<T> 中的 T 为目标，T 对公共 task API 不可见。
7. 生命周期执行器在第一次检查和加锁之间执行 prepare，在解锁后执行 commit 或 rollback。
8. prepare 为每次调用产生独立令牌 P；P 被且仅被一次 commit、rollback 或显式无 finalizer 路径消费。
9. task 结果和生命周期 finalization 结果使用两个正交维度表示，任何一方的错误都不得覆盖另一方。
10. 内置日志、异常字符串化、ExecutionContext、one-shot 包装和旧 builder 全部删除。

## 2. 问题本质

当前 Rust 设计把 predicate 检查的状态和 Lock<T> 保护的 T 混为一谈，试图让 predicate 接收 &T。这样第一次检查必须先获取读锁，失去了 DCL 通过快速失败减少锁开销的核心价值。

真正的 DCL 场景中存在两个不同角色：

- gate：可通过 atomic/等价同步机制无锁读取的状态，例如服务是否 STARTED。
- lock：序列化受 gate 约束的 task，以及发生在 task 之外的相关 gate 状态转换。

典型场景为：

- lifecycle_state 是 AtomicU8、AtomicBool、AtomicRef 或其他原子状态。
- task 操作线程池、任务映射、数据库、文件或其他业务资源。
- gate 在锁内复查为允许状态后，task 在 executor 的锁内执行，并可在该临界区内直接修改 gate。
- task 也可以不修改 gate；若 task 返回后或系统其他路径需要修改 gate，该修改必须使用与 executor 相同的底层锁。

## 3. 第一性原理与并发不变量

### 3.1 无锁快速检查

predicate 类型为零参数、只读、可并发调用的函数：

    Fn() -> bool + Send + Sync + 'static

executor 对同一个 predicate 调用两次：

    if !predicate() {
        return ConditionNotMet;
    }

    lock.with_write(|_| {
        if !predicate() {
            return ConditionNotMet;
        }
        task()
    })

第一次调用不得获取 executor 所使用的同一底层锁，也不应执行阻塞操作。

### 3.2 Rust 内存模型

Java volatile 的 Rust 对应物不是 ptr::read_volatile。read_volatile 主要面向 MMIO，不提供线程同步协议。

predicate 应读取以下来源之一：

- std::sync::atomic 下的原子类型；
- qubit-atomic 提供的 Atomic、AtomicRef 或相应 Arc 包装；
- 具有等价 acquire/release 保证的其他同步数据源。

推荐协议：

- predicate 使用 Acquire load；
- gate 写入使用 Release store；
- 需要更直观的全序行为时使用 SeqCst；
- qubit-atomic 的 AtomicOps::load/store 已默认采用 Acquire/Release。

executor 不自行插入 fence。脱离正确 atomic load/store 的 fence 无法把普通非原子共享变量变成线程安全状态。

### 3.3 gate 修改与同一底层锁约束

仅有 atomic 可见性不足以让任意 gate 修改自动与锁内 task 互斥。约束取决于 gate 修改发生的位置：

- 第二次 predicate 在锁内判定为 true 后，task 已经持有 executor 的锁。task 可以直接修改其捕获的 gate，无需也不得为此重复获取同一把非重入锁；是否修改由具体业务决定。
- 如果 task 不修改 gate，而是在 task 返回、executor 释放锁之后再修改，或者由系统其他路径修改 gate，那么修改路径必须使用与 executor 相同的底层锁。

task 在 executor 临界区内修改 gate：

    let outcome = executor.run(|| {
        perform_business_action()?;
        lifecycle_state.store(STOPPED, Ordering::Release);
        Ok::<_, TaskError>(())
    });

task 之外的路径修改 gate：

    transition_lock.with_write(|_| {
        lifecycle_state.store(STOPPED, Ordering::Release);
    });

其中 transition_lock 必须与 executor 保存的 lock 指向同一同步对象；它可以是该锁句柄的 clone。

这样才能保证：

- 第一次检查失败：调用在该 atomic load 处线性化，不获取锁。
- 第一次检查成功、等待锁期间 gate 被同锁保护的其他路径改变：第二次检查发现改变，不执行 task。
- 第二次检查成功：task 与其内部的 gate 修改均处于 executor 临界区内。
- task 不修改 gate 时，task 之后或其他路径的 gate 修改只能在获得同一底层锁后发生，因而无法穿过第二次检查与 task 之间的临界区。

### 3.4 类型系统边界

Send + Sync 可以阻止安全 Rust 闭包直接共享 Cell、RefCell 等非线程安全可变状态，但无法证明闭包一定无锁，也无法证明闭包没有捕获同一 mutex。

本设计不引入 unsafe marker trait：

- 错误 predicate 会导致逻辑竞态或死锁，但不会直接破坏 Rust 内存安全。
- unsafe 不应被用于表达一般业务正确性约束。
- 一个看似安全的 marker trait 也无法验证用户实现是否真正 lock-free。

该约束通过 API 命名、Rustdoc、README 示例和并发测试明确表达。

## 4. 目标与非目标

### 4.1 目标

1. 锁外第一次检查在失败路径上不获取任何锁。
2. 使用同一个同步 predicate 完成锁外检查和锁内复查。
3. 支持所有实现 qubit_lock::Lock<T> 的同步锁。
4. task 为任意零参数 FnOnce，而不是对 T 的操作器。
5. 提供独立的基础 executor 和生命周期 executor。
6. 生命周期支持并发 prepare、每调用独立 P、成功 commit、失败 rollback。
7. rollback 获得结构化失败原因和实际 task error 的只读视图。
8. task outcome 与 preparation outcome 独立保留。
9. panic 捕获边界位于整个锁操作之外，尊重具体锁的 poisoning 语义。
10. API 可存入普通结构体字段，不要求用户命名闭包的匿名类型。
11. 明确区分 task 内 gate 修改与 task 外 gate 修改：前者已受 executor 锁保护，后者必须遵守同一底层锁协议。

### 4.2 非目标

1. 不提供旧 API 兼容层或 deprecated 转发。
2. 不把 Lock<T> 中的 T 暴露给 task。
3. 不承诺事务性撤销 task 的任意业务副作用。
4. 不加入 async executor。
5. 不加入共享读锁执行模式；0.10 统一使用 with_write。
6. 不在 rs-dcl 内提供新的 atomic 容器。
7. 不内置日志、指标或 tracing。
8. 不允许 commit 失败后自动 rollback 已成功的 task。
9. 不为 parking_lot 模拟 poisoning。

## 5. 公共架构

公共执行器分为两种：

    DoubleCheckedLockExecutor<L, T>
        - 无生命周期
        - 返回 ExecutionOutcome<R, E>

    LifecycleDoubleCheckedLockExecutor<L, T, P, C>
        - 包含 prepare / commit / rollback
        - 返回 ExecutionReport<R, E, C>

二者共享私有 DclCore<L, T>：

    struct DclCore<L, T: ?Sized> {
        lock: L,
        predicate: Arc<dyn Fn() -> bool + Send + Sync + 'static>,
        catch_panics: bool,
        marker: PhantomData<fn(&T)>,
    }

predicate 使用 Arc trait object 的原因：

- executor 可以作为稳定、可命名的字段类型保存。
- builder 接受任意捕获闭包。
- 每次调用没有分配，只产生一次间接调用。
- 不使用 Mutex<FnMut>，不会引入隐藏串行或回调重入死锁。
- 省去把匿名闭包类型传播到整个公共 executor 类型。

生命周期回调同样使用无内部 mutex 的 Arc<dyn Fn... + Send + Sync>。这些回调可能被多个并发执行共享，因此必须实现 Fn，而不是 FnMut。

当 L: Clone 时，两个 executor 均可条件性实现 Clone。clone 后共享 predicate 和生命周期回调，但每次 run 的 P 独立存在于调用栈中。

## 6. 基础执行器 API

### 6.1 构建

    let executor = DoubleCheckedLockExecutor::builder(lock)
        .when(move || {
            state.load(Ordering::Acquire) == STARTED
        })
        .catch_panics(true)
        .build();

builder 阶段：

1. builder(lock)：持有锁，尚不能 build。
2. when(predicate)：进入 ready 状态。
3. catch_panics(bool)：仅在 ready 状态提供，默认 false。
4. build()：产生 executor。

when 是必选项，缺少 predicate 的 builder 不提供 build。

### 6.2 执行

    pub fn run<R, E, F>(
        &self,
        task: F,
    ) -> ExecutionOutcome<R, E>
    where
        F: FnOnce() -> Result<R, E>;

示例：

    let outcome = executor.run(|| {
        update_business_state();
        Ok::<_, TaskError>(())
    });

    match outcome {
        ExecutionOutcome::Success(()) => {}
        ExecutionOutcome::ConditionNotMet => {}
        ExecutionOutcome::TaskFailed(error) => handle(error),
        ExecutionOutcome::Panicked(panic) => handle_panic(panic),
        ExecutionOutcome::NotExecuted => unreachable!(),
    }

基础 executor 不产生 NotExecuted；该变体仅用于共享结果模型中的 prepare 失败。

## 7. 生命周期执行器 API

### 7.1 回调类型

概念签名：

    prepare: Fn() -> Result<P, C>

    commit: Fn(P) -> Result<(), C>

    rollback:
        for<'a> Fn(P, RollbackCause<'a>) -> Result<(), C>

其中：

- P 是每次 prepare 产生的独立令牌。
- C 是 prepare、commit、rollback 共用的生命周期错误类型。
- prepare、commit、rollback 必须为 Fn + Send + Sync + 'static。
- P 被 commit、rollback 或明确 no-finalizer 路径消费一次。

### 7.2 builder 合法组合

完整生命周期：

    let executor =
        LifecycleDoubleCheckedLockExecutor::builder(lock)
            .when(predicate)
            .catch_panics(true)
            .prepare(prepare)
            .commit(commit)
            .rollback(rollback)
            .build();

成功需要 commit、失败不需要 rollback：

    .prepare(prepare)
    .commit(commit)
    .no_rollback()
    .build()

成功不需要 commit、失败需要 rollback：

    .prepare(prepare)
    .no_commit()
    .rollback(rollback)
    .build()

typestate 保证：

- when 和 prepare 必须存在。
- commit 与 no_commit 二选一。
- rollback 与 no_rollback 二选一。
- 选择 no_commit 后只提供 rollback，不提供 no_rollback。
- 不存在 no_commit + no_rollback。
- commit/rollback 不能在 prepare 之前配置。
- 不完整状态没有 build。
- catch_panics 只在 prepare 前的配置阶段提供一次。

no_commit/no_rollback 表示该路径无需 finalizer。对应路径中的 P 在 executor 内自然结束生命周期，但该实现细节不进入方法命名。

### 7.3 执行入口

task 不读取 P：

    pub fn run<R, E, F>(
        &self,
        task: F,
    ) -> ExecutionReport<R, E, C>
    where
        E: Error + Send + Sync + 'static,
        F: FnOnce() -> Result<R, E>;

task 需要读取或更新 P：

    pub fn run_with_token<R, E, F>(
        &self,
        task: F,
    ) -> ExecutionReport<R, E, C>
    where
        E: Error + Send + Sync + 'static,
        F: FnOnce(&mut P) -> Result<R, E>;

run 内部委托给同一个私有执行过程，只是不把 P 暴露给 task。

没有实际令牌数据时，prepare 返回 P = ()，调用方继续使用 run：

    .prepare(|| {
        perform_preflight()?;
        Ok::<(), LifecycleError>(())
    })

## 8. rollback 原因

rollback 不应通过读取外部共享状态猜测失败发生在哪个阶段。它接收结构化原因：

    pub enum RollbackCause<'a> {
        ConditionNotMet,

        TaskFailed(
            &'a (dyn Error + Send + Sync + 'static),
        ),

        Panicked(&'a PanicInfo),
    }

TaskFailed 提供对原始 E 的 trait-object 视图：

- E 本身仍由 ExecutionOutcome::TaskFailed(E) 持有。
- rollback 只能在调用期间借用错误，不能保存该引用。
- 调用方可通过 Error::downcast_ref 检查具体错误类型。
- rollback 完成后，原始 E 原封不动返回给调用方。

Panicked 中的 PanicInfo 包含具体阶段，因此不再需要为每个 panic 阶段增加 RollbackCause 变体。

prepare 失败时尚未产生 P，因此不调用 rollback。

commit 失败时 task 已成功，且业务副作用可能已被其他线程观察，因此不自动调用 rollback。

## 9. 结果模型

### 9.1 ExecutionOutcome

    #[must_use]
    pub enum ExecutionOutcome<R, E> {
        Success(R),
        ConditionNotMet,
        NotExecuted,
        TaskFailed(E),
        Panicked(PanicInfo),
    }

语义：

- Success：锁内第二次检查通过，task 返回 Ok。
- ConditionNotMet：第一次或第二次检查不满足。
- NotExecuted：仅生命周期 executor 使用；prepare 未成功完成。
- TaskFailed：task 返回 Err，保留原始 E。
- Panicked：启用 panic capture 后，predicate、锁或 task panic。

### 9.2 PreparationOutcome

    #[must_use]
    pub enum PreparationOutcome<C> {
        NotStarted,

        PrepareFailed(C),
        PreparePanicked(PanicInfo),

        CommitNotRequired,
        Committed,
        CommitFailed(C),
        CommitPanicked(PanicInfo),

        RollbackNotRequired,
        RolledBack,
        RollbackFailed(C),
        RollbackPanicked(PanicInfo),
    }

### 9.3 ExecutionReport

    #[must_use]
    pub struct ExecutionReport<R, E, C> {
        execution: ExecutionOutcome<R, E>,
        preparation: PreparationOutcome<C>,
    }

只提供不会丢失信息的访问方式：

    pub fn execution(&self) -> &ExecutionOutcome<R, E>;
    pub fn preparation(&self) -> &PreparationOutcome<C>;
    pub fn into_parts(
        self,
    ) -> (
        ExecutionOutcome<R, E>,
        PreparationOutcome<C>,
    );

不提供把所有失败压缩成 bool 的 finish，不提供吞掉错误的默认转换。

### 9.4 组合示例

task 成功且 commit 成功：

    Success(R) + Committed

task 成功但 commit 失败：

    Success(R) + CommitFailed(C)

task 失败且 rollback 成功：

    TaskFailed(E) + RolledBack

task 失败且 rollback 失败：

    TaskFailed(E) + RollbackFailed(C)

第二次条件不满足并 rollback：

    ConditionNotMet + RolledBack

第一次条件不满足：

    ConditionNotMet + NotStarted

prepare 失败：

    NotExecuted + PrepareFailed(C)

任何 finalization 错误都不得覆盖 task outcome。

## 10. 精确执行状态机

### 10.1 基础 executor

1. 调用 predicate。
2. false：返回 ConditionNotMet，不获取锁。
3. true：调用 lock.with_write。
4. 写锁内再次调用 predicate。
5. false：释放锁，返回 ConditionNotMet。
6. true：锁内执行 task。
7. task Ok：释放锁，返回 Success。
8. task Err：释放锁，返回 TaskFailed。
9. panic：遵循第 11 节策略。

### 10.2 生命周期 executor

1. 锁外调用 predicate。
2. false：返回 ConditionNotMet + NotStarted。
3. true：锁外调用 prepare。
4. prepare Err：返回 NotExecuted + PrepareFailed。
5. prepare Ok(P)：进入 locked phase。
6. 获取写锁并第二次调用 predicate。
7. 第二次 false：释放锁，调用 rollback(P, ConditionNotMet) 或 no_rollback。
8. 第二次 true：锁内执行 task。
9. task Ok(R)：释放锁，调用 commit(P) 或 no_commit。
10. task Err(E)：释放锁，调用 rollback(P, TaskFailed(&E)) 或 no_rollback。
11. locked phase panic：锁的 guard 先观察 unwind 并释放；随后调用 rollback(P, Panicked) 或 no_rollback。
12. 将 execution 和 preparation 两个结果组合返回。

prepare 始终在锁外，可以被多个竞争调用并发执行。每个调用持有自己的 P。

commit 和 rollback 始终在锁外执行，不延长数据锁临界区。

## 11. panic 策略

### 11.1 配置

    .catch_panics(false)  // 默认
    .catch_panics(true)

配置作用于：

- 第一次 predicate；
- prepare；
- lock.with_write；
- 第二次 predicate；
- task；
- commit；
- rollback。

### 11.2 PanicInfo

    pub struct PanicInfo {
        phase: PanicPhase,
        message: Option<String>,
        payload: Box<dyn Any + Send + 'static>,
    }

    pub enum PanicPhase {
        InitialConditionCheck,
        Prepare,
        LockAcquisition,
        SecondConditionCheck,
        Task,
        Commit,
        Rollback,
    }

PanicInfo 提供：

- phase()；
- message()；
- payload()；
- into_payload()。

Debug 实现不尝试格式化未知 payload。

### 11.3 catch_panics = true

- panic 被转换为对应的 Panicked outcome。
- prepare 之后的 locked-phase panic 会先释放锁，再执行 rollback。
- task/lock/predicate panic 与 rollback panic 可以同时保留在双轴 report 中。
- commit panic 不改变 Success(R)。

### 11.4 catch_panics = false

- 第一次 predicate、prepare、commit 的 panic 直接传播。
- prepare 成功后的 locked phase 仍在 with_write 外临时 catch，以便释放锁并执行 rollback。
- rollback 完成后通过 resume_unwind 继续传播原始 panic。
- 标准库锁可以正常观察 task unwind 并进入 poisoned 状态。
- parking_lot 锁按其自身语义不 poisoning。
- 若原始 panic 传播期间 rollback 返回 Err 或发生第二个 panic，原始 panic 优先；secondary rollback failure 无法通过普通返回值报告。需要完整双轴诊断的调用方应启用 catch_panics。

executor 不承诺撤销 task panic 前已经发生的副作用。

## 12. 生命周期令牌 P

P 的职责是把一次 prepare 与该次调用的 commit/rollback 精确关联。

示例：

    struct PrepareToken {
        transaction: DatabaseTransaction,
        attempt_id: u64,
    }

并发调用 A、B：

- A prepare 得到 P_A。
- B prepare 得到 P_B。
- A 获得锁、通过复查并成功，commit(P_A)。
- B 随后获得锁、复查失败，rollback(P_B, ConditionNotMet)。

不使用 P 时，调用方需要借助共享 Mutex<Option<_>> 或 HashMap 保存 prepare 资源，会引入串行、覆盖或调用关联错误。

run_with_token 允许 task 把 undo 信息写入 P：

    executor.run_with_token(|token| {
        let operation = perform_business_action()?;
        token.record(operation);
        Ok(result)
    });

rollback 可以根据 P 和 RollbackCause 补偿外部资源。它不自动获得 executor 锁，也不自动访问 Lock<T> 中的 T。

## 13. 线性化与可观察性

### 13.1 第一次检查失败

调用在第一次 atomic predicate load 处线性化。之后 gate 立即改变不影响该次调用；调用方需要再次调用才能重试。

### 13.2 第二次检查失败

调用在锁内第二次 predicate load 处确定不执行 task。生命周期 prepare 已发生，因此解锁后进入 rollback。

### 13.3 task 成功

task 的业务效果在锁内发生。task 可在该临界区内修改 gate，也可以保持 gate 不变；若由后续或其他路径修改 gate，则该路径必须获取 executor 的同一底层锁。锁释放后，其他线程可能在 commit 完成前观察到 task 效果。

因此：

    Success(R) + CommitFailed(C)

表示 task 已执行，调用方不得把它当成未执行并无条件重试。

### 13.4 task 失败

TaskFailed(E) 仅表示 task 返回 Err，不保证 task 在返回前没有产生部分副作用。

rollback 是领域补偿器，不是通用事务系统。

## 14. 日志与可观测性

rs-dcl 不再直接依赖 log，也不主动记录日志。

理由：

- outcome 已完整返回调用方。
- 库内日志容易与调用方日志重复。
- 库无法决定业务错误的严重级别和消息。
- panic hook 在 panic 被 catch_unwind 捕获前仍按 Rust 标准行为执行。

调用方通过 match ExecutionOutcome/PreparationOutcome 自行记录日志、指标或 tracing span。

## 15. 依赖调整

运行时依赖：

- 保留 qubit-lock。
- 删除 qubit-function。
- 删除 log。
- 不强制依赖 qubit-atomic；文档同时演示 std atomics 和 qubit-atomic 的兼容使用方式。

开发依赖：

- 增加 loom 0.7，用于 atomic gate 与锁交错的模型测试。

## 16. 公共 API 清理

删除：

- DoubleCheckedLock
- DoubleCheckedLockBuilder
- DoubleCheckedLockReadyBuilder
- 旧 DoubleCheckedLockExecutor 实现
- ExecutionContext
- ExecutionResult
- ExecutorError
- CallbackError
- ExecutionLogger
- ExecutorBuilder
- ExecutorLockBuilder
- ExecutorReadyBuilder
- 所有 call/execute/call_with/execute_with 组合
- 所有 one-shot 入口
- 所有内置日志配置方法
- qubit-function 的 ArcTester/ArcRunnable 使用
- 根模块对 ArcMutex 和 Lock 的再导出

新增并根导出：

- DoubleCheckedLockExecutor
- LifecycleDoubleCheckedLockExecutor
- ExecutionOutcome
- ExecutionReport
- PreparationOutcome
- RollbackCause
- PanicInfo
- PanicPhase

builder stage 类型保持 public 以满足 Rust 公共签名可达性，但标记为 doc(hidden)，不在 crate 根重导出，用户正常使用时无需命名。

## 17. 建议源码布局

    src/
      lib.rs
      double_checked/
        mod.rs
        double_checked_lock_executor.rs
        double_checked_lock_executor_builder.rs
        double_checked_lock_executor_ready_builder.rs
        lifecycle_double_checked_lock_executor.rs
        lifecycle_double_checked_lock_executor_builder.rs
        lifecycle_predicate_builder.rs
        lifecycle_prepare_builder.rs
        lifecycle_commit_builder.rs
        lifecycle_rollback_builder.rs
        lifecycle_ready_builder.rs
        execution_outcome.rs
        execution_report.rs
        preparation_outcome.rs
        rollback_cause.rs
        panic_info.rs
        panic_phase.rs
        internal/
          mod.rs
          dcl_core.rs
          locked_execution.rs
          panic_capture.rs

每个公共类型单独一个源文件。生产源码不包含 cfg(test) 测试模块。

测试目录按源文件映射：

    tests/
      double_checked/
        double_checked_lock_executor_tests.rs
        lifecycle_double_checked_lock_executor_tests.rs
        execution_outcome_tests.rs
        execution_report_tests.rs
        preparation_outcome_tests.rs
        rollback_cause_tests.rs
        panic_info_tests.rs
        builder_typestate_tests.rs
        concurrency_tests.rs
        loom_model_tests.rs
      docs/
        readme_tests.rs
    benches/
      dcl_bench.rs

## 18. 测试设计

### 18.1 基础流程

- 第一次 false 时不调用 with_write。
- 第一次 true、第二次 false 时不调用 task。
- 两次 true 时 task 恰好调用一次。
- task Ok 保留 R。
- task Err 保留原始 E。
- executor 可由多个线程并发调用。

使用测试 Lock 记录 with_write 次数，证明快速失败路径为零锁调用。

### 18.2 atomic gate

- std AtomicBool/AtomicU8 使用 Acquire/Release。
- qubit-atomic AtomicOps 使用默认 Acquire/Release。
- task 在 executor 锁内修改 gate，不需要重复获取锁；竞争调用的第二次检查阻止过期 task。
- task 返回后或其他路径修改 gate 时，修改方持有同一底层锁，第二次检查阻止过期 task。
- task 外的 gate transition 不受同一锁保护的示例只作为 compile 文档反例，不宣称 executor 能修复错误协议。

### 18.3 生命周期

- 第一次 false：不 prepare、不 commit、不 rollback。
- prepare Err：无 P，不 rollback。
- 多线程 prepare 可并发执行。
- 第二次 false：对应 P 进入 rollback，cause 为 ConditionNotMet。
- task success：对应 P 进入 commit。
- task error：rollback 得到同一个 E 的 borrowed Error 视图，report 保留 owned E。
- run_with_token 对 P 的修改可被 commit/rollback 观察。
- P 在每条终止路径上仅消费一次。
- no_commit/no_rollback 返回对应 outcome。
- commit failure 不覆盖 Success。
- rollback failure不覆盖 ConditionNotMet、TaskFailed 或 Panicked。

### 18.4 panic 与锁语义

- 初次 predicate panic。
- prepare panic。
- 标准 mutex task panic 后 poisoned。
- parking_lot mutex task panic后不 poisoning。
- 锁获取 panic。
- 第二次 predicate panic。
- task panic 后先解锁再 rollback。
- commit panic。
- rollback panic。
- catch_panics true 返回完整双轴 report。
- catch_panics false 在 rollback 后恢复原始 unwind。
- task 在 panic 前的副作用不会被虚假宣称已撤销。

### 18.5 typestate

通过 compile_fail doctest 验证：

- 缺少 when 不能 build。
- 缺少 prepare 不能 build。
- prepare 后未选择 commit/no_commit 不能 build。
- commit 后未选择 rollback/no_rollback 不能 build。
- no_commit 后没有 no_rollback。
- commit/rollback 不能出现在 prepare 前。

### 18.6 loom

使用 loom 0.7 的原子量和自定义测试 Lock 建模：

- 外层 false 时不获取锁。
- 两个线程都看到第一次 true，但只有获得锁后第二次仍为 true 的线程执行 task。
- task 在 executor 锁内直接以 Release store 改变 gate，且不重复获取锁；竞争者第二次 Acquire load 观察到新状态。
- task 外的线程获取同一底层锁后改变 gate，等待中的调用在随后第二次检查时观察到新状态。
- prepare token 不跨调用串线。

### 18.7 性能基准

dcl_bench.rs 对比：

- 单独执行一次 raw atomic predicate；
- executor 第一次 predicate 为 false 的快速退出；
- executor 条件成立时的无竞争写锁路径；
- 等价的手写 DCL 流程。

快速退出 benchmark 必须证明没有锁调用或每次调用分配。基准报告单独展示 predicate trait-object 间接调用的成本，防止后续改动侵蚀 DCL 通过跳过锁获取获得的收益。

## 19. 文档设计

README 和 README.zh_CN 必须首先说明三条并发契约：

1. predicate 必须读取 atomic/等价同步状态。
2. predicate 不得获取 executor 同一底层锁。
3. task 可以在 executor 锁内直接修改 gate；task 返回后或其他路径修改 gate 时，必须使用 executor 的同一底层锁。

README 示例不再以修改 Lock<T> 中的 T 为中心，而使用：

- AtomicU8 生命周期状态；
- ArcRwLock<()> 或其他同步锁；
- 任意业务 task；
- 带令牌的 prepare/commit/rollback。

Rustdoc 为 run、run_with_token、when、prepare、commit、rollback 明确列出：

- Parameters；
- Returns；
- Errors；
- Panics；
- Synchronization；
- Locking；
- 并发调用与重入限制。

README 最后四节按项目规范排列：

- Testing / 测试
- License / 许可证
- Contributing / 贡献
- Author / 作者

## 20. 验收标准

设计实现完成必须同时满足：

1. 第一次 predicate false 的测试证明未调用任何 Lock 方法。
2. predicate 为零参数同步 gate，不接收 &T。
3. task 为零参数或仅接收 &mut P，不接收 &T/&mut T。
4. 测试覆盖 task 在 executor 锁内直接修改 gate，且不会为此重复获取锁。
5. 测试覆盖 task 返回后或其他路径仅在持有 executor 同一底层锁时修改 gate。
6. 基础和生命周期 executor 均有 builder API。
7. 三种生命周期组合由 typestate 编译期约束。
8. task E 和 lifecycle C 在所有双失败组合中均可恢复。
9. panic catch 位于 lock.with_write 外层。
10. prepare/commit/rollback 无共享 FnMut mutex。
11. 当前旧公共类型和兼容入口全部移除。
12. README、Rustdoc 和 examples 与新语义一致。
13. align-ci.sh、ci-check.sh、coverage.sh json 按顺序通过。
14. 不执行 git add、commit 或 push，除非用户另行明确授权。

## 21. 明确排除的误解

- Send + Sync 不等于 lock-free；predicate 的无锁约束仍是调用方协议。
- Acquire load 不能替代相关 gate 写入时的 Release store。
- atomic 可见性不能让锁外 gate 写入自动与锁内 task 互斥；gate 要么由 task 在 executor 临界区内修改，要么由持有同一底层锁的后续或其他路径修改。
- rollback 不表示任意 task 都具备事务性。
- commit failure 不表示 task 没有执行。
- parking_lot 不 poisoning 是其正常语义。
- 第一次检查允许在线性化点之后立即变旧；第二次锁内检查负责关闭获得锁前的竞争窗口。
