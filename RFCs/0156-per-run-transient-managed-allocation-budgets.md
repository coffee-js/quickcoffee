# RFC 0156：单轮瞬时托管分配预算 / Per-run transient managed allocation budgets

- 状态：已采纳 / Status: Adopted
- 日期：2026-08-28 / Date: 2026-08-28
- 依赖：RFC 0118、RFC 0146、RFC 0149、RFC 0153、RFC 0154 / Dependencies: RFC 0118, RFC 0146, RFC 0149, RFC 0153, RFC 0154

## 中文

### 独立的累计瞬时预算

`ResourceLimits` 增加 `max_transient_managed_objects` 与 `max_transient_managed_bytes`。默认均为 `u64::MAX`，表示该可选策略关闭；零是有效边界。超限分别产生 `ResourceLimit::TransientManagedObjects` 与 `TransientManagedBytes`，其稳定判别值为 24 和 25，继续避开 `Error` 私有槽位 19。

两个边界复用 RFC 0146 的平台无关 logical managed object/payload byte 成本模型，并在每次顶层 `eval`、`run` 或 `run_program` 开始时归零。默认参数等嵌套 VM 执行继续累计到外层账本。计数使用饱和 `u64`；到达边界允许继续，首次越过边界的完整 delta 仍写入 `ExecutionStats`，随后立即返回宿主可见、脚本不可捕获的 Resource error。

该策略是保守的**累计 transient allocation budget**：即使值在同一轮中创建后丢弃，其创建成本仍消耗预算。它不是某一时刻的 live set、allocator capacity、RSS、进程总量或 RFC 0147 的 Context retained graph。提高上限只允许更多累计工作，不证明同时存活内存的安全上界。

### 强制点、callback 与原子性

VM、精确数值、容器、字符串、函数/环境、class/instance、结构化 Error 与已建模标准库在 RFC 0146 的既有分配记账点更新统计并检查预算；不增加逐指令 retained census。默认关闭时不创建事务，也不扫描 Context 图。

`NativeCallContext::record_managed_allocation` 保持无失败的协作记账 API，使 callback 可在执行任意宿主工作后报告完整 delta。VM 在 callback 返回后、处理其返回值或错误前强制检查；因此若 callback 同时报告越界并返回普通 Runtime error，资源越界优先，且统计保留。callback 的宿主副作用与未报告堆分配不能回滚或验证；旧式 opaque native 仍排除在本预算之外。

启用任一 transient 边界时，Context 在运行前复用 RFC 0149 的脚本状态事务快照。因本 RFC 边界失败会恢复 global、closure environment、class static field 与 instance field；已发生的统计及宿主 callback 副作用不回滚。其他 Runtime/Fuel/取消错误维持各自既有提交语义。

静态模块图共享一个累计账本。依赖按执行顺序消耗 objects/bytes，后续模块只得到图总额度的剩余部分；默认关闭的 `u64::MAX` 在模块间保持关闭而不被递减。`Context::last_execution()` 保存包括失败模块在内的聚合统计。

### 验收与后续边界

测试覆盖 object/byte 精确边界、创建后丢弃、跨运行重置、不可捕获及命名源码归因、脚本状态回滚、callback 成功/失败优先级、失败统计与模块图累计。完整 debug/release、Clippy、rustdoc、examples、manual、publish dry-run 与 paired qbench 门禁必须通过。

本 RFC 只解决累计瞬时分配压力。逐时刻 live managed-memory、cycle 回收、单条 instruction 的未建模 Rust 临时堆，以及 native callback 未协作报告的宿主堆增长继续由 #76 跟踪；不可信脚本仍需宿主进程隔离。

---

## English

### Independent cumulative transient budgets

`ResourceLimits` adds `max_transient_managed_objects` and `max_transient_managed_bytes`. Both default to `u64::MAX`, disabling the optional policy; zero is a valid boundary. Exceeding them produces `ResourceLimit::TransientManagedObjects` or `TransientManagedBytes`, with stable discriminants 24 and 25 while continuing to avoid `Error`'s private slot 19.

Both bounds reuse RFC 0146's platform-independent logical managed object/payload-byte cost model and restart at zero for every top-level `eval`, `run`, or `run_program`. Nested VM execution such as default parameters continues to accumulate into the outer ledger. Counters saturate at `u64::MAX`; reaching a boundary is allowed, while the complete delta that first crosses it remains visible in `ExecutionStats` before an immediate host-visible, script-uncatchable Resource error.

This policy is a conservative **cumulative transient allocation budget**: creating a value consumes budget even when that value is discarded during the same run. It is not an instantaneous live set, allocator capacity, RSS, a process-wide total, or RFC 0147's Context-retained graph. Raising it permits more cumulative work and does not prove a safe simultaneous-live-memory bound.

### Enforcement points, callbacks, and atomicity

The VM, exact numerics, containers, strings, functions/environments, classes/instances, structured Errors, and modeled standard-library operations update statistics and check the budget at their existing RFC 0146 allocation accounting points. No per-instruction retained census is added. Disabled defaults create no transaction and scan no Context graph.

`NativeCallContext::record_managed_allocation` remains an infallible cooperative accounting API so a callback can report its complete delta after arbitrary host work. The VM enforces the result after the callback returns and before processing its value or error. A reported budget breach therefore takes precedence over an ordinary Runtime error from the same callback, while preserving statistics. Host side effects and unreported callback heap allocations cannot be rolled back or verified; legacy opaque natives remain outside this budget.

When either transient bound is active, Context reuses RFC 0149's script-state transaction snapshot before execution. A failure from this RFC restores globals, closure environments, class static fields, and instance fields. Statistics and host callback side effects already performed are not rolled back. Other Runtime, Fuel, and cancellation failures retain their existing commit semantics.

A static module graph shares one cumulative ledger. Dependencies consume objects/bytes in execution order, and later modules receive only the graph-wide budget remainder. Disabled `u64::MAX` limits stay disabled rather than being decremented between modules. `Context::last_execution()` retains aggregate statistics including the failing module.

### Acceptance and remaining boundary

Tests cover exact object/byte boundaries, create-and-discard work, reset across runs, uncatchability and named-source attribution, script-state rollback, callback success/error precedence, failure statistics, and module-graph accumulation. Full debug/release, Clippy, rustdoc, examples, manuals, publish dry-run, and paired qbench gates must pass.

This RFC addresses cumulative transient allocation pressure only. Instantaneous live managed memory, cycle collection, unmodeled Rust temporary heap within one instruction, and host heap growth not cooperatively reported by a native callback remain tracked by #76; untrusted scripts still require host process isolation.
