# RFC 0154：宿主 capability 与 embedding 稳定性 / Host capabilities and embedding stability

- 状态：已采纳 / Status: Adopted
- 日期：2026-08-28 / Date: 2026-08-28
- 依赖：RFC 0118、RFC 0146、RFC 0152、RFC 0153 / Dependencies: RFC 0118, RFC 0146, RFC 0152, RFC 0153

## 中文

### Capability allowlist

QuickCoffee 不提供 clock、random、logging、file 或 network 的系统实现，也不从进程环境取得 ambient authority。宿主通过 `CapabilityKey<T>` 定义一个静态命名、带 `CapabilityKind` 的 typed slot，通过 `HostCapabilities`、`ContextBuilder::capability` 或 `Context::set_capability` 显式安装其自行实现的 `'static` 值。slot 由类别与名称识别；以错误 `T` 查询同一 slot 返回 `None`。`CapabilityKind` 为 non-exhaustive，未来版本可增加类别而不要求 embedding 对枚举穷尽匹配。

capability handle 使用同线程 `Rc<T>`，只可由 contextual native 通过 `NativeCallContext::capability` 或由宿主通过 Context 查询。它不成为脚本 `Value`、global、JSON、公开 bytecode、Runtime 编译缓存或 managed retained-memory census 的一部分。模块子 Context 继承发起 Context 的同一 opaque handles；独立 Context 默认拥有空表，只有宿主显式复制 allowlist 才会共享 handle。替换、移除或清空表使用 Context-owned copy-on-write snapshot，不修改先前传入的 `HostCapabilities` 值。

任意 capability 的工作量与宿主堆大小不能由 VM 自动推断。callback 必须使用 RFC 0153 的 `check_cancelled`、`consume_fuel` 与 `record_managed_allocation` 明确声明可审计成本；表本身不绕过 Value/JSON/resource policy，也不构成完整 sandbox。缺失 capability 返回 `None`，由 callback 决定稳定的宿主错误，不触发隐式 fallback。

### Panic 与线程边界

`NativeFunction` 与 `ContextualNativeFunction` 在调用 `Context` 的同一 OS 线程中同步执行。VM 不捕获 Rust panic，也不把 panic 转换为 QuickCoffee `Error`；unwind 可能发生在脚本 global、closure、class/instance 或宿主内部状态已经改变之后，因此 panic 路径没有事务回滚保证。宿主必须让 callback 保持 panic-free，并把预期失败返回为 `Error`。QuickCoffee 不改变进程的 panic strategy。

`Runtime`、`Context`、`Program` 的执行相关对象、`Value` 与 capability handle 使用 `Rc`/`RefCell`，明确不是 `Send` 或 `Sync`，也不能在 Context 执行中并发重入。同一 `Runtime` 的多个 Context 是状态隔离而非线程并行原语。`CancellationToken` 使用原子共享状态，是唯一明确可跨线程克隆、用于请求后续 VM/callback 协作停止的控制对象；它不能强制中断已经运行且不轮询的同步 callback。

### 0.x 稳定性与迁移

在 `0.1.x` 内，已采纳 RFC、公开 rustdoc、错误/resource category、`.coffee`/`.litcoffee` 识别和测试锁定的行为属于 patch-compatible contract，不应被有意破坏。新增方法、non-exhaustive enum variant、可选限制或新诊断细节可以在 patch 版本增加。未文档化内部布局、性能数字、错误展示文本、缓存淘汰的非语义实现细节和 host object 大小不构成兼容承诺。

升级到新的 `0.y` minor 可以包含 breaking public-API 或语义变更，但必须先有 adopted RFC、双语迁移说明、旧行为测试的显式更新和 release changelog。`Program`/`Module` 是进程内已验证对象；项目没有稳定 bytecode serialization ABI，跨版本持久化必须重新从 source 编译。宿主应锁定 crate 版本，并在升级时运行自己的语义、资源、capability-denial 与 panic-boundary 集成测试。

---

## English

### Capability allowlist

QuickCoffee supplies no system clock, random, logging, file, or network implementation and acquires no ambient authority from the process environment. A host defines a statically named typed slot with `CapabilityKey<T>` and `CapabilityKind`, then explicitly installs its own `'static` value through `HostCapabilities`, `ContextBuilder::capability`, or `Context::set_capability`. Category plus name identify a slot; querying that slot with the wrong `T` returns `None`. `CapabilityKind` is non-exhaustive so future releases can add categories without requiring embedders to exhaustively match the enum.

A capability handle is a same-thread `Rc<T>` available only to contextual natives through `NativeCallContext::capability` or directly to the host through Context inspection. It never becomes a script `Value`, global, JSON value, public bytecode, Runtime compilation-cache entry, or managed retained-memory census object. Module child Contexts inherit the initiating Context's same opaque handles. Independent Contexts start with empty tables and share handles only when the host explicitly copies an allowlist. Replacement, removal, and clearing use a Context-owned copy-on-write snapshot and do not mutate a previously supplied `HostCapabilities` value.

The VM cannot infer arbitrary capability work or host-heap size. A callback explicitly declares auditable cost through RFC 0153's `check_cancelled`, `consume_fuel`, and `record_managed_allocation` methods. The table bypasses no Value, JSON, or resource policy and is not a complete sandbox. A missing capability returns `None`; the callback chooses a stable host error rather than receiving an implicit fallback.

### Panic and thread boundary

`NativeFunction` and `ContextualNativeFunction` run synchronously on the OS thread that calls the `Context`. The VM does not catch a Rust panic or convert one into a QuickCoffee `Error`. Unwinding may occur after script globals, closures, class/instance state, or host state changed, so panic paths have no transactional rollback guarantee. Hosts keep callbacks panic-free and return expected failures as `Error`. QuickCoffee does not alter the process panic strategy.

`Runtime`, `Context`, execution-related `Program` objects, `Value`, and capability handles use `Rc`/`RefCell`; they are deliberately neither `Send` nor `Sync`, and a Context cannot be re-entered concurrently. Multiple Contexts attached to one Runtime provide state isolation, not a thread-parallel primitive. `CancellationToken` uses atomically shared state and is the sole explicitly cross-thread control object: a clone can request a later VM or cooperative callback stop, but cannot forcibly interrupt a synchronous callback that does not poll.

### 0.x stability and migration

Within `0.1.x`, adopted RFCs, public rustdoc, error/resource categories, `.coffee`/`.litcoffee` recognition, and behavior locked by tests form the patch-compatible contract and are not intentionally broken. New methods, non-exhaustive enum variants, optional limits, or diagnostic detail may be added in a patch. Undocumented internal layout, performance numbers, rendered error text, non-semantic cache-eviction implementation detail, and host-object size are not compatibility promises.

Moving to a new `0.y` minor may include breaking public-API or semantic changes, but requires an adopted RFC, bilingual migration notes, explicit updates to old-behavior tests, and a release changelog first. `Program` and `Module` are verified in-process objects; there is no stable bytecode-serialization ABI, so cross-version persistence recompiles from source. Hosts pin the crate version and run their own semantic, resource, capability-denial, and panic-boundary integration tests when upgrading.
