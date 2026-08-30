# RFC 0161：场景化执行策略 / Scenario-oriented execution policy

- 状态：已采纳 / Status: Adopted
- 日期：2026-08-30 / Date: 2026-08-30
- 依赖：RFC 0118、RFC 0152、RFC 0155–0157 / Dependencies: RFC 0118, RFC 0152, RFC 0155–0157

## 中文

### 动机

QuickCoffee 已分别公开 `CompileLimits`、fuel、最大调用深度、18 项 `ResourceLimits` 与 logical live-memory observation。它们能表达确定性边界，但定价、JSON 规范化和多文件策略包三个真实 Rust host 都需要复制较长的 builder 链，也可能让 Runtime 编译边界与新 Context 的执行边界发生漂移。

### API 与继承

`ExecutionPolicy` 是可复制、可比较的公开值，集中保存 `CompileLimits`、初始 fuel、最大调用深度、`ResourceLimits` 和 `LiveMemoryObservation`。所有字段通过 getter 审计，并以 `with_*` 返回修改后的策略；不公开可变字段。

`RuntimeBuilder::execution_policy(policy)` 把编译部分交给 Runtime 的 `Engine`，并将完整策略保存为新 Context 的默认值。`Runtime::execution_policy()` 返回该快照。`Runtime::context_builder()` 和 `new_context()` 继承其 fuel、调用深度、资源与观测设置；现有 `ContextBuilder::fuel`、`max_call_depth`、`resource_limits`、`live_memory_observation` 仍可覆盖单次请求。builder 调用按顺序生效，因此最后一次 `compile_limits` 或 `execution_policy` 设置对应编译部分。

`ExecutionPolicy::default()` 必须逐项保持此前默认：`CompileLimits::default()`、1,000,000 fuel、1,024 调用深度、`ResourceLimits::default()` 和关闭 live-memory observation。仅创建 `Runtime::new()` 的既有宿主不会被收紧。

### `isolated_request` 预设

`ExecutionPolicy::isolated_request()` 面向每请求或有限批次创建新 Context、复用 verified Program/ModulePackage 的部署方式。数值由 #232 的三个旗舰工作流与 10/100/1000 规模基准共同校准：

- source 128,000 bytes、递归 bytecode 40,000 instructions、8 个唯一模块、整图 source 256,000 bytes；
- 250,000 fuel、64 层调用、默认关闭 checkpointed observation；
- JSON 输入/输出 256,000 bytes、单字符串 64,000 bytes、单容器 1,024 项、单次 16,000 values、32 层嵌套；
- Integer/Decimal coefficient 256 bits、Decimal scale 8；单次集合操作 4,096 项、文本扫描 64,000 bytes；
- 一般 String 256,000 bytes、Array 1,024 项、Map 128 entries；
- retained 4,096 objects / 512,000 bytes；单轮 transient 50,000 objects / 8,000,000 bytes。

这些值是可审计的初始安全配置，不是普遍业务 SLO。宿主可从该预设复制并显式覆盖，性能基准也可为大规模输入单独提高 fuel，但不得静默绕过其他边界。

### 生命周期、取消与隔离

策略不保存 `CancellationToken`、globals、host state、capabilities 或 native callbacks。取消令牌具有一次性状态，必须由每个请求显式创建和注入，避免已取消 token 泄漏到后续请求。短生命周期是宿主部署约定，不是 VM 强行销毁 Context 的机制；长生命周期 state 和 batch 需要宿主审查 retained/cycle 与资源成本。

预设只提供确定性进程内纵深防御。logical bytes 不等于 RSS，未协作 native callback 不受 VM 强制终止，真正不可信脚本仍需进程隔离、OS 限额与外部超时。本 RFC 不改变 Runtime、Context、Program 或 ModulePackage 的 `Send` / `Sync` 契约。

### 验收

外部集成测试逐项锁定默认与预设值、Runtime introspection、Context 继承和单项覆盖。定价、JSON 规范化与策略包的 example、integration test 和 benchmark 共用预设；取消、capability 拒绝、fuel、depth、retained/transient 与模块图越界仍由 focused tests 验证。

---

## English

### Motivation

QuickCoffee already exposes `CompileLimits`, fuel, maximum call depth, 18 `ResourceLimits` fields, and logical live-memory observation. These controls express deterministic boundaries, but each real pricing, JSON-normalization, and multi-file-policy Rust host repeats a long builder chain and can accidentally diverge Runtime compilation boundaries from new-Context execution boundaries.

### API and inheritance

`ExecutionPolicy` is a copyable and comparable public value containing `CompileLimits`, initial fuel, maximum call depth, `ResourceLimits`, and `LiveMemoryObservation`. Getters make every component auditable, and `with_*` methods return a modified policy without exposing mutable fields.

`RuntimeBuilder::execution_policy(policy)` installs the compile portion in the Runtime `Engine` and retains the whole policy as defaults for new Contexts. `Runtime::execution_policy()` returns that snapshot. `Runtime::context_builder()` and `new_context()` inherit fuel, call depth, resource, and observation settings. Existing `ContextBuilder::fuel`, `max_call_depth`, `resource_limits`, and `live_memory_observation` methods remain request-specific overrides. Builder calls apply in order, so the last `compile_limits` or `execution_policy` call controls the compile portion.

`ExecutionPolicy::default()` exactly preserves the earlier defaults: `CompileLimits::default()`, 1,000,000 fuel, 1,024 call depth, `ResourceLimits::default()`, and disabled live-memory observation. Existing hosts that only construct `Runtime::new()` are not tightened.

### The `isolated_request` preset

`ExecutionPolicy::isolated_request()` targets a new Context per request or bounded request batch while reusing verified Programs or ModulePackages. Its values are calibrated by the three #232 flagship workflows and their 10/100/1000 scale benchmarks:

- 128,000 source bytes, 40,000 recursive bytecode instructions, 8 unique modules, and 256,000 whole-graph source bytes;
- 250,000 fuel, 64 call depth, and checkpointed observation disabled by default;
- 256,000-byte JSON input/output, 64,000-byte JSON strings, 1,024 items per JSON container, 16,000 values per operation, and 32 nesting levels;
- 256-bit Integer/Decimal coefficients, Decimal scale 8, 4,096 items per collection operation, and 64,000 text-operation bytes;
- 256,000 bytes per general String, 1,024 Array items, and 128 Map entries;
- 4,096 retained objects / 512,000 retained bytes and 50,000 transient objects / 8,000,000 transient bytes per run.

These values are an auditable initial safety configuration, not a universal business SLO. Hosts may copy and explicitly override the preset. Performance benchmarks may independently raise fuel for larger inputs but must not silently bypass other boundaries.

### Lifecycle, cancellation, and isolation

A policy stores no `CancellationToken`, globals, host state, capabilities, or native callbacks. Cancellation tokens have one-way state and must be created and installed per request so a cancelled token cannot leak into later work. Short lifetime is a host deployment convention, not forced Context destruction; hosts using long-lived state or batches must review retained/cycle behavior and resource costs.

The preset is deterministic in-process defense in depth only. Logical bytes are not RSS, uncooperative native callbacks cannot be forcibly terminated by the VM, and genuinely untrusted scripts still require process isolation, OS limits, and external deadlines. This RFC changes no `Send` / `Sync` contract for Runtime, Context, Program, or ModulePackage.

### Acceptance

External integration tests lock every default and preset value, Runtime introspection, Context inheritance, and individual overrides. Pricing, JSON normalization, and policy-package examples, integration tests, and benchmarks share the preset. Focused tests retain cancellation, capability-denial, fuel, depth, retained/transient, and module-graph failure coverage.
