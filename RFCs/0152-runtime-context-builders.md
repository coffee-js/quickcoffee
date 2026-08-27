# RFC 0152：Runtime/Context 构建器与有界共享编译缓存 / Runtime/Context builders and bounded shared compilation caches

- 状态：已采纳 / Status: Adopted
- 日期：2026-08-28 / Date: 2026-08-28
- 依赖：RFC 0046、RFC 0069、RFC 0118、RFC 0119、RFC 0151 / Dependencies: RFC 0046, RFC 0069, RFC 0118, RFC 0119, RFC 0151

## 中文

### Ownership

`Runtime` 是可克隆的同线程编译产物所有者；所有 clone 共享一个无状态 `Engine`、Program 缓存与 Module 缓存。`RuntimeBuilder` 分别以最大条目数配置两个缓存，默认各 64 项，容量为零表示禁用。`Runtime::context_builder()` 返回 `ContextBuilder`，用于声明式配置 fuel、最大 QuickCoffee 调用深度、`ResourceLimits`、`CancellationToken`、宿主 globals 与 native callbacks。

每次 `ContextBuilder::build()` 都创建独立可写 global environment。Runtime 不保存或共享 globals、native replacements、已求值模块 exports、fuel、取消令牌、执行统计、VM frame、迭代器或 retained-memory census/high-water。模块仍在每次 `Context::run_module` 中重新求值；单次模块图内部继续按 RFC 0119 复用同名 dependency exports。Runtime 缓存不属于 `Context::retained_memory()` 的 root，也不受 Context retained-state 提交限制；source/bytecode byte 总量边界继续由 #76 规划。

现有 `Engine` 保持无状态、无缓存的显式编译器。`Context::new()`、`Context::with_*`、`set_global`、`add_native` 与手工共享 `Program` 保持兼容；为避免给既有 Context 构造和预编译执行基准增加 Runtime 分配，`Context::new()` 继续使用无缓存轻量路径。需要跨 Context 复用时，宿主必须显式保留同一个 Runtime 并从其 builder 创建 Context；`Context::builder()` 是从新默认 Runtime 开始配置的便利入口。

### Cache identity and behavior

Program 缓存键是 `(optional source name, raw UTF-8 source)`；Module 缓存键是 `(canonical module name, raw UTF-8 source)`。身份使用完整字符串而非散列，因此名称、空白、注释、literate prose 或可执行文本任一变化都会 miss，不存在仅凭 `u64` 碰撞误命中的路径。`.litcoffee` 的预处理由名称决定，故名称必须属于键。失败的 prepare、parse、lowering 或 verify 会增加 miss 统计，但绝不写入缓存。

命中会把条目标记为最近使用；容量满时确定性淘汰最久未使用条目。缓存仅改变编译工作量，不改变验证、字节码、诊断或执行语义。`RuntimeCacheStats` 公开当前 entries 与累计 hits/misses/evictions；`clear_compile_caches()` 只移除缓存句柄而保留累计计数，宿主已持有的 `Program`/`Module` 继续有效。RFC 0151 的版本化模块图指纹仍是宿主构建整图/持久缓存键；本 RFC 的进程内 Module 缓存只保存逐模块编译产物，不缓存 loader 结果或整图求值。

### Thread and authority boundary

当前 Program、Value 与 VM environment 使用 `Rc` / `RefCell`，因此 Runtime、Context 和相关句柄刻意不实现 `Send`/`Sync`。宿主可在控制线程持有 `CancellationToken` 的线程安全 clone，但脚本执行与 Runtime cache 访问仍发生在 Context 所在线程。本 RFC 不捕获宿主 native panic，也不新增 clock、random、logging、file、network、host state carrier 或任何 ambient authority。

### 验收

集成测试覆盖跨 Context hit、精确 source/name 失效、LRU 淘汰、零容量、失败不插入、显式清空、builder policies、global/native/fuel/cancellation/statistics/retained-memory 隔离，以及模块编译共享但 exports 重新求值。嵌入示例、五种可执行手册、中英文语法索引、公开 rustdoc、MSRV、package 与完整门禁必须通过。

---

## English

### Ownership

`Runtime` is a cloneable same-thread owner of compilation artifacts. All clones share one stateless `Engine`, a Program cache, and a Module cache. `RuntimeBuilder` configures independent maximum entry counts, with 64 entries per cache by default and zero disabling a cache. `Runtime::context_builder()` returns a `ContextBuilder` for declarative fuel, maximum QuickCoffee call depth, `ResourceLimits`, `CancellationToken`, host globals, and native callbacks.

Every `ContextBuilder::build()` creates an independent writable global environment. Runtime stores or shares no globals, native replacements, evaluated module exports, fuel, cancellation token, execution statistics, VM frames, iterators, or retained-memory census/high-water state. Modules are evaluated again by every `Context::run_module`; dependency exports remain shared only inside one module-graph run under RFC 0119. Runtime caches are outside the roots counted by `Context::retained_memory()` and outside Context retained-state commit limits. Aggregate source/bytecode byte boundaries remain planned by #76.

The existing `Engine` remains an explicit stateless, uncached compiler. `Context::new()`, `Context::with_*`, `set_global`, `add_native`, and manually shared Programs remain compatible. To avoid adding Runtime allocation to existing Context construction and precompiled-execution benchmarks, `Context::new()` keeps its lightweight uncached path. A host must explicitly retain one Runtime and build Contexts from it for cross-Context reuse; `Context::builder()` is a convenience that starts with a fresh default Runtime.

### Cache identity and behavior

The Program cache key is `(optional source name, raw UTF-8 source)` and the Module cache key is `(canonical module name, raw UTF-8 source)`. Identity uses complete strings instead of a digest, so any name, whitespace, comment, literate prose, or executable-text change misses without a `u64` collision path. The name participates because it selects `.litcoffee` preprocessing. A failed prepare, parse, lowering, or verification increments miss statistics but is never inserted.

A hit marks an entry most recently used; a full cache deterministically evicts the least-recently-used entry. Caching changes compilation work only, never verification, bytecode, diagnostics, or execution semantics. `RuntimeCacheStats` exposes current entries and cumulative hits, misses, and evictions. `clear_compile_caches()` drops only cached handles and preserves cumulative counters; Programs and Modules already held by the host remain valid. RFC 0151's versioned module-graph fingerprint remains the host key for whole-graph or persistent caches. This RFC's in-process Module cache stores only per-module compilation artifacts, never loader results or graph evaluation.

### Thread and authority boundary

Programs, Values, and VM environments currently use `Rc` / `RefCell`, so Runtime, Context, and related handles deliberately do not implement `Send` or `Sync`. A host may retain a thread-safe CancellationToken clone on a control thread, but script execution and Runtime cache access stay on the Context thread. This RFC does not catch host-native panics and adds no clock, random, logging, file, network, host-state carrier, or ambient authority.

### Acceptance

Integration tests cover cross-Context hits, exact source/name invalidation, LRU eviction, zero capacity, failed-compilation exclusion, explicit clearing, builder policies, global/native/fuel/cancellation/statistics/retained-memory isolation, and shared module compilation with fresh export evaluation. The embedding example, five executable manuals, English/Chinese syntax indexes, public rustdoc, MSRV, package, and complete gates must pass.
