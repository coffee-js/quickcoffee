# RFC 0151：确定性模块图指纹 / Deterministic module-graph fingerprints

- 状态：已采纳 / Status: Adopted
- 日期：2026-08-27 / Date: 2026-08-27
- 依赖：RFC 0080、RFC 0082、RFC 0119、RFC 0133、RFC 0141 / Dependencies: RFC 0080, RFC 0082, RFC 0119, RFC 0133, RFC 0141

## 中文

### 契约

`Engine::fingerprint_module_graph(entry, loader)` 从一个已经验证的入口 `Module` 出发，只通过调用者提供的 `ModuleLoader` 加载并验证完整静态依赖图，返回确定性的 `u64` 指纹。该过程不创建 `Context`、不执行任何模块，也不调用脚本或宿主 native function。缺失模块/导出、解析/验证失败和循环依赖返回既有结构化 Error，不为部分图产生指纹。

图指纹使用独立于 `Chunk::fingerprint()` / `Program::fingerprint()` 的 canonical FNV-1a 64 位编码域。公开常量 `MODULE_GRAPH_FINGERPRINT_VERSION` 固定当前编码版本为 `1`。v1 依次编码 domain、版本、入口规范名，以及按规范模块名词法排序的节点记录。每个节点记录包含规范名、原始 UTF-8 source 的独立 domain/version 指纹、已验证 Program 指纹、imports 和 public exports。

每条 import 按源码顺序编码原始 specifier、loader 返回的依赖规范名和按源码顺序排列的 `public -> local` 绑定，因此 import 执行顺序、别名和依赖边都属于缓存身份。exports 按 `(public, local)` 词法排序后编码，使 public surface 不依赖内部容器迭代。模块节点始终按规范名排序，因此 DFS 发现顺序和宿主 map 插入/迭代顺序不影响结果。

原始 source 指纹使注释、空白或 literate prose 等纯源码变化也会使图缓存失效；Program 指纹同时锁定编译后的可执行内容。入口规范名、任一节点规范名、source、Program、import/export surface 或规范依赖边的变化都改变输入编码。若同一次构图中同一规范名解析为不一致的 source/Program/import/export 定义，则返回 Runtime error，而不采用“先发现者获胜”的顺序相关结果。

`Module::fingerprint()` 继续只表示单模块可执行 body，现有 bytecode 指纹值和 `--fingerprint FILE` 格式均不改变。任何图编码语义变更必须递增 `MODULE_GRAPH_FINGERPRINT_VERSION`、更新 RFC 和 golden test；现有字节码编码仍按自己的兼容域演进。

CLI 复用显式权限边界：`qcoffee --fingerprint --module-root ROOT ENTRY`（flag 顺序可交换）通过 `RestrictedFileModuleLoader` 输出 16 位小写十六进制图指纹且不执行模块。它与 `--json`、`--stats`、普通源码、check、反汇编和交互模式互斥；普通 `--fingerprint FILE` 不取得模块根权限。

### 非目标

本 RFC 不增加 package manifest、序列化 bytecode cache、dynamic import、side-effect-only import、cycle 初始化语义、隐式文件/网络权限或加密哈希保证。`u64` 碰撞边界与既有 bytecode 指纹相同，嵌入方仍应把版本和业务环境纳入自己的缓存策略。

### 验收

Memory loader 覆盖重复稳定性、宿主插入顺序、source-only 变化、入口/别名/export/规范边变化、缺失模块、循环和同名不一致来源；restricted-file loader 覆盖规范相对路径、依赖变化和 `.coffee` / `.litcoffee` 权限边界。CLI 覆盖稳定格式、不执行、flag 顺序和互斥规则。固定 v1 golden、防回归 API 示例、双语 CLI/API 文档以及完整 package/tooling 门禁必须通过。

---

## English

### Contract

`Engine::fingerprint_module_graph(entry, loader)` starts from an already verified entry `Module`, loads and verifies the complete static dependency graph solely through the caller-provided `ModuleLoader`, and returns a deterministic `u64` fingerprint. It creates no `Context`, executes no module, and invokes no script or host native function. Missing modules/exports, parse/verification failures, and cycles return the existing structured Errors; no partial graph receives a fingerprint.

The graph fingerprint uses a canonical FNV-1a 64-bit encoding domain separate from `Chunk::fingerprint()` and `Program::fingerprint()`. The public `MODULE_GRAPH_FINGERPRINT_VERSION` constant fixes the current encoding version at `1`. Version 1 encodes the domain, version, canonical entry name, and node records sorted lexically by canonical module name. Each node record contains its canonical name, a separately domain/version-tagged fingerprint of the raw UTF-8 source, its verified Program fingerprint, imports, and public exports.

Each import encodes, in source order, the literal specifier, the loader-returned canonical dependency name, and its source-ordered `public -> local` bindings. Import execution order, aliases, and dependency edges therefore belong to cache identity. Exports are sorted lexically by `(public, local)` before encoding so the public surface does not depend on internal container iteration. Module nodes are always sorted by canonical name, making DFS discovery order and host map insertion/iteration order irrelevant.

The raw-source fingerprint makes comments, whitespace, and literate prose changes invalidate a graph cache even when bytecode is unchanged; the Program fingerprint independently locks the compiled executable content. Changing the canonical entry name, any node name, source, Program, import/export surface, or canonical dependency edge changes the encoded input. If one graph build resolves the same canonical name to inconsistent source/Program/import/export definitions, it returns a Runtime error instead of accepting a discovery-order-dependent first definition.

`Module::fingerprint()` continues to describe only one module's executable body. Existing bytecode fingerprint values and the `--fingerprint FILE` format remain unchanged. Any semantic graph-encoding change must increment `MODULE_GRAPH_FINGERPRINT_VERSION` and update this RFC and the golden test; bytecode encoding continues to evolve in its own compatibility domain.

The CLI reuses the explicit authority boundary: `qcoffee --fingerprint --module-root ROOT ENTRY` (in either flag order) prints the 16-digit lowercase hexadecimal graph fingerprint through `RestrictedFileModuleLoader` without executing modules. It is mutually exclusive with `--json`, `--stats`, ordinary source input, checking, disassembly, and interactive mode. Ordinary `--fingerprint FILE` gains no module-root authority.

### Non-goals

This RFC adds no package manifest, serialized bytecode cache, dynamic import, side-effect-only import, cycle-initialization semantics, implicit file/network authority, or cryptographic hash guarantee. The `u64` collision boundary is the same as for existing bytecode fingerprints, and embedders should still include version and business environment in their cache policy.

### Acceptance

Memory-loader tests cover repeat stability, host insertion order, source-only changes, entry/alias/export/canonical-edge changes, missing modules, cycles, and inconsistent same-name sources. Restricted-file tests cover canonical relative paths, dependency changes, and `.coffee` / `.litcoffee` authority boundaries. CLI tests cover stable formatting, non-execution, flag order, and conflicts. A fixed v1 golden, regression-safe API example, bilingual CLI/API documentation, and the complete package/tooling gates must pass.
