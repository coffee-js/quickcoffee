# RFC 0157：选择性 checkpointed live 托管内存观测 / Opt-in checkpointed live managed-memory observation

- 状态：已采纳 / Status: Adopted
- 日期：2026-08-28 / Date: 2026-08-28
- 依赖：RFC 0146、RFC 0147、RFC 0148、RFC 0149、RFC 0153、RFC 0156 / Dependencies: RFC 0146, RFC 0147, RFC 0148, RFC 0149, RFC 0153, RFC 0156

## 中文

### 目的与术语

RFC 0146 的 managed allocation 是一轮执行的累计创建量；RFC 0147/0148 的 retained snapshot 与 high water 只从 Context writable global root 开始。它们都不包含正在执行的 value stack、call frame 或 iterator，因此不能回答“在一个定义的 VM 安全点，QuickCoffee 自己可见的值图有多大”。

本 RFC 定义可选的 **checkpointed live managed-memory observation**。这里的 *live* 只指在一个已命名 checkpoint、由本 RFC 列出的 QuickCoffee VM root 可达的逻辑托管图；它不是任意机器指令之间的峰值。报告的 object/byte 单位完全复用 RFC 0146/0147，且仍不等于 Rust allocator 调用、capacity、RSS、进程总量或宿主堆。

### 选择性 API 与报告

后续实现新增默认关闭的 `LiveMemoryObservation` 配置，唯一初始启用模式为 `Checkpointed`；它由 `ContextBuilder` 或同等显式 Context 配置选择。默认 `Off` 不分配观测工作区、不改变 `ExecutionStats`、不增加 VM dispatch 分支，也不改变现有 retained/transient policy。

启用时，每次顶层 `eval`、`run`、`run_program` 或模块图执行完成一个独立 `LiveMemoryReport`。报告使用独立的 `LiveManagedMemory { objects, bytes }`，不得复用或重新解释 `RetainedMemory`；它至少包含最后一个 checkpoint snapshot、object/byte 各自的 checkpoint high water 及 checkpoint kind、按 kind 的计数与总 sample 数，以及成功/普通错误/资源错误/取消的结束类别。

报告只由宿主读取；不进入 script globals、序列化、qcoffee `--stats`、qbench v1 schema 或 `ResourceLimit`。它不会产生资源错误、停止执行、改变 rollback 或给 callback 授予 capability。存储的是固定大小汇总而非无界 sample 历史；逐样本诊断需要另一个明确、有界的 API。

### 根、单位与 checkpoint

每个 checkpoint 以 RFC 0147 的 Rc identity 去重和 RFC 0146 的 logical object/payload-byte 单位，普查以下 VM 可见 root：

1. Context owned writable global environment（不含共享 builtin parent）；
2. 当前 operand/value stack、pending call/return/error value，以及 receiver-bound call 的 receiver；
3. 所有活跃 frame 的 lexical environment、fast-local values、默认参数暂存值与方法 receiver；
4. 活跃 iterator 中的 `Value` backing/entry values，以及异常传播期间仍由 VM 持有的 Error/throw value；
5. 模块 child Context 的同类根；静态模块图在一次顶层调用中合并为同一个 report，而不是各模块独立 high water。

checkpoint 是有限的既有 VM 控制流边界：顶层进入与退出、call frame 入栈/出栈、iterator 建立/结束、handler 建立/解除与异常转移、以及 contextual native 调用前后。实现可以在这些边界中加入更细的已命名 checkpoint，但不得把未命名的每条 instruction、每次 global store 或 callback 内部当作已观测。因此 report 的 high water 是 **checkpoint peak**，不是任意 instruction 间完整 live peak。

每个启用 checkpoint 允许做一次 O(reachable managed graph) cycle-safe census，并可为 identity set 分配临时 Rust 工作空间；这部分工作空间不属于 logical measurement。默认关闭路径不得建立 census、identity set 或 transaction。更高频观测或 hard live limit 必须另行 RFC，给出增量所有权账本、原子性、成本上限和独立性能证据。

### 排除、错误与隔离

编译 Program/Chunk、source/debug/source-map/binding plan、VM scratch buffer、iterator 的 Rust vector capacity 或复制的非 `Value` metadata、allocator headers、共享 builtin internals、Runtime compile caches、host state/capabilities、native callback closure、未协作 native host allocation、OS RSS 和 cycle collection 全部排除。Rc cycle 会在一次 census 内去重并终止；本 RFC 不回收 cycle，也不声称已限制跨 Context 或进程生命周期。

callback 前 checkpoint 包含 VM 持有的参数与 receiver，callback 后 checkpoint 包含 VM 已接受的结果或错误；callback 内部及未报告的 host heap 不可见。普通错误、资源错误和取消都在 VM 清理/事务回滚后记录 run-exit checkpoint；已经观察到的较大 checkpoint high water 不会回滚。模块 child Context 继承这项显式配置，但独立 Context 没有共享 report 或 high water。

### 验收

- API 默认关闭，release qbench 证明关闭时不引入新 dispatch/census 开销；启用成本以单独 benchmark 报告。
- golden tests 精确覆盖 stack temporary、nested/default-parameter call、receiver、iterator、throw/catch/finally、rollback、alias/cycle、module child Context 与 host-provided values 的纳入或排除。
- 测试证明 component-wise high water、checkpoint kind、错误/取消退出和 Context 隔离稳定；RFC 0146–0156 的计数、限额、错误和回滚语义不变。
- README、双语语法索引和 embedding docs 明确使用“checkpointed logical live managed memory”，不使用 RSS、total heap、GC 或 hard live limit 的说法。

---

## English

### Purpose and terminology

RFC 0146's managed allocation is cumulative creation during one run; RFCs 0147/0148's retained snapshot and high water start only at the Context writable-global root. Neither includes the active value stack, call frames, or iterators, so neither answers how large the QuickCoffee-visible value graph is at a defined VM safe point.

This RFC defines opt-in **checkpointed live managed-memory observation**. Here *live* means the logical managed graph reachable from the QuickCoffee VM roots listed here at a named checkpoint only; it is not a peak between arbitrary machine instructions. Object/byte units reuse RFCs 0146/0147 and remain distinct from Rust allocator calls, capacity, RSS, process totals, and host heap.

### Opt-in API and report

A later implementation adds a default-off `LiveMemoryObservation` configuration whose sole initial enabled mode is `Checkpointed`, selected through `ContextBuilder` or an equivalent explicit Context configuration. Default `Off` allocates no observation workspace, changes neither `ExecutionStats` nor existing retained/transient policy, and adds no VM-dispatch branch.

When enabled, each top-level `eval`, `run`, `run_program`, or module-graph execution produces an independent `LiveMemoryReport`. It uses a distinct `LiveManagedMemory { objects, bytes }` rather than reusing or reinterpreting `RetainedMemory`, and includes the final checkpoint snapshot, component-wise checkpoint high water and checkpoint kind, counts by kind and total samples, and a success/ordinary-error/resource-error/cancellation outcome.

The report is host-readable only. It does not enter script globals, serialization, qcoffee `--stats`, qbench v1 schema, or `ResourceLimit`. It creates no resource error, execution stop, rollback change, or callback capability. It stores fixed-size aggregates rather than an unbounded sample history; per-sample diagnostics require another explicit, bounded API.

### Roots, units, and checkpoints

Each checkpoint uses RFC 0147 Rc-identity deduplication and RFC 0146 logical object/payload-byte units to census these VM-visible roots:

1. the Context-owned writable global environment, excluding the shared builtin parent;
2. the active operand/value stack, pending call/return/error value, and a receiver-bound call receiver;
3. every active frame's lexical environment, fast-local values, default-argument temporaries, and method receiver;
4. `Value` backings/entry values held by active iterators and Error/throw values still held during exception propagation; and
5. equivalent roots in module child Contexts. A static module graph merges into the one top-level report instead of creating independent module high waters.

Checkpoints are finite existing VM control-flow boundaries: top-level entry and exit, call-frame push/pop, iterator creation/end, handler setup/removal and exception transfer, and immediately before/after a contextual native call. An implementation may add finer named checkpoints within those boundaries, but may not present every unnamed instruction, global store, or callback interior as observed. A report high water is therefore a **checkpoint peak**, not a complete live peak between arbitrary instructions.

An enabled checkpoint may perform one O(reachable managed graph), cycle-safe census and allocate temporary Rust workspace for its identity set; that workspace is outside the logical measurement. The default-off path must not create a census, identity set, or transaction. Higher-frequency observation or a hard live limit requires a separate RFC defining incremental ownership accounting, atomicity, bounded cost, and independent performance evidence.

### Exclusions, errors, and isolation

Compiled Program/Chunk, source/debug/source-map/binding plan, VM scratch buffers, iterator Rust-vector capacity or copied non-`Value` metadata, allocator headers, shared builtin internals, Runtime compile caches, host state/capabilities, native callback closures, unreported native host allocation, OS RSS, and cycle collection are excluded. An Rc cycle is deduplicated and terminates within a census; this RFC neither collects cycles nor claims cross-Context or process-lifetime limits.

The pre-callback checkpoint includes VM-held arguments and receiver; the post-callback checkpoint includes the result or error accepted by the VM. Callback internals and unreported host heap are invisible. Ordinary errors, resource errors, and cancellation record the run-exit checkpoint after VM cleanup and transactional rollback; an already observed larger high water is not rolled back. Module child Contexts inherit this explicit configuration, while independent Contexts share neither report nor high water.

### Acceptance

- The API is off by default, and release qbench demonstrates no new dispatch/census cost while off; enabled cost is reported in a separate benchmark.
- Exact golden tests cover stack temporaries, nested/default-argument calls, receivers, iterators, throw/catch/finally, rollback, aliases/cycles, module child Contexts, and inclusion/exclusion of host-provided values.
- Tests prove stable component-wise high water, checkpoint kind, error/cancellation exit, and Context isolation; RFCs 0146–0156 counters, limits, errors, and rollback semantics remain unchanged.
- README, bilingual syntax index, and embedding docs say “checkpointed logical live managed memory” and make no RSS, total-heap, GC, or hard-live-limit claim.
