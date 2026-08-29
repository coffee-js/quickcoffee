# RFC 0158：确定性内存模块包 manifest / Deterministic in-memory module-package manifests

- 状态：已采纳 / Status: Adopted
- 日期：2026-08-29 / Date: 2026-08-29
- 依赖：RFC 0119、RFC 0151、RFC 0152、RFC 0155、RFC 0156、RFC 0157 / Dependencies: RFC 0119, RFC 0151, RFC 0152, RFC 0155, RFC 0156, RFC 0157

## 中文

### 契约

`Engine::prepare_module_package(entry, loader)` 与 `Runtime::prepare_module_package(entry, loader)` 在调用者显式提供的 `ModuleLoader` 边界内，完整加载、编译、验证并预检静态模块图，返回不可变的 `ModulePackage`。构建期间沿用既有 canonical name、一致来源、缺失 export、cycle、单模块 source/bytecode 与整图预算诊断；失败不返回部分 package，也不执行任何脚本。

package 保存已验证 `Module`、解析后的静态依赖边、canonical entry、模块数和 RFC 0151 相同编码域的版本化图 fingerprint。它不保存 loader、原始 source、全局环境、native handle、求值 exports、fuel、取消令牌、统计或任何 Context 资源状态。因此构建完成后，`Context::run_module_package` 不再调用 loader，也不会获得任何文件、网络或其他 capability。

每一次 `run_module_package` 均新建私有 module globals 与 exports，并复用既有的 graph-wide fuel、取消、transient allocation、retained/live-memory observation 和结构化错误契约。package 只复用已验证的编译与预检结果；相同 package 的跨 Context 运行不能共享脚本值、模块副作用、execution stats 或 resource accounting。当前 Context 会在任意模块执行前重新检查单模块及模块图 `CompileLimits`，因此宽策略下构建的 package 不能绕过窄策略 Context。

package identity 是其不可变快照身份，不是对可能变化 loader 的实时声明。宿主负责将自身 loader/version/configuration 纳入缓存失效策略，并在需要新 source 时显式重新构建 package。`Runtime` 构建可复用其精确 module compilation cache；`Engine` 保持无缓存。两者都不持久化 package，也不把 package 放入 Runtime cache。

### 非目标

本 RFC 不定义磁盘或网络 package 格式、bytecode 序列化、签名、注册表、动态 import、side-effect-only import、cycle 初始化、共享 exports 或 CLI package discovery。它不改变单文件 CLI、`Context::run_module`、图 fingerprint 的 v1 编码，也不测量 loader 返回前的宿主 I/O/heap。

### 验收

- Memory 与 restricted-file loader 均可构建 package；同一图的 package fingerprint 与 RFC 0151 图 fingerprint 相同。
- 构建后的执行不再调用 loader；多 Context 每次都重新求值并保持 globals、exports、取消、统计及资源隔离。
- 预检失败无脚本副作用、无部分 package；cycle、缺失 export、canonical inconsistency 和全部 compile graph limits 保持既有 Error。
- 宽策略构建、窄策略运行会在任意脚本前被拒绝；完整模块、文档、MSRV、Clippy、package 与性能门禁通过。

---

## English

### Contract

`Engine::prepare_module_package(entry, loader)` and `Runtime::prepare_module_package(entry, loader)` load, compile, verify, and preflight the complete static graph inside the caller-supplied explicit `ModuleLoader` boundary, returning an immutable `ModulePackage`. Construction preserves existing canonical-name, inconsistent-source, missing-export, cycle, per-module source/bytecode, and graph-budget diagnostics; failure returns no partial package and executes no script.

A package retains verified `Module`s, resolved static dependency edges, its canonical entry, module count, and a versioned graph fingerprint in the RFC 0151 encoding domain. It retains no loader, raw source, global environment, native handle, evaluated exports, fuel, cancellation token, statistics, or Context resource state. Once construction finishes, `Context::run_module_package` never calls a loader and gains no file, network, or other capability.

Every `run_module_package` creates fresh private module globals and exports while retaining the existing graph-wide fuel, cancellation, transient-allocation, retained/live-memory observation, and structured-error contracts. A package reuses only verified compilation and preflight artifacts; cross-Context runs of one package cannot share script values, module side effects, execution statistics, or resource accounting. Before any module executes, the current Context rechecks per-module and graph `CompileLimits`, so a package made under a wider policy cannot bypass a narrower Context.

Package identity identifies its immutable snapshot; it is not a live assertion about a loader that may change. Hosts include their loader/version/configuration in cache invalidation and explicitly rebuild when they need new source. `Runtime` construction may reuse its exact module-compilation cache, while `Engine` remains uncached. Neither persists packages nor places a package in the Runtime cache.

### Non-goals

This RFC defines no disk or network package format, bytecode serialization, signing, registry, dynamic import, side-effect-only import, cycle initialization, shared exports, or CLI package discovery. It changes neither single-file CLI behavior, `Context::run_module`, nor v1 graph-fingerprint encoding, and does not measure host I/O/heap before a loader returns.

### Acceptance

- Memory and restricted-file loaders can construct packages; a package fingerprint equals the RFC 0151 graph fingerprint for the same graph.
- Execution after construction never calls the loader; every Context reevaluates while globals, exports, cancellation, statistics, and resources remain isolated.
- Preflight failure has no script effect and returns no partial package; cycles, missing exports, canonical inconsistency, and every compile-graph limit retain their existing Errors.
- A package built under wide limits is rejected before scripts run under narrower limits; complete module, documentation, MSRV, Clippy, package, and performance gates pass.
