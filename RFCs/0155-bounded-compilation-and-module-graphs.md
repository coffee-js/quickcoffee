# RFC 0155：有界编译与模块图 / Bounded compilation and module graphs

- 状态：已采纳 / Status: Adopted
- 日期：2026-08-28 / Date: 2026-08-28
- 依赖：RFC 0118、RFC 0119、RFC 0133、RFC 0145、RFC 0151、RFC 0152 / Dependencies: RFC 0118, RFC 0119, RFC 0133, RFC 0145, RFC 0151, RFC 0152

## 中文

### 独立编译策略

`CompileLimits` 与执行期 `ResourceLimits` 分离，默认把单份原始 UTF-8 source 限制为 1,000,000 bytes、单个 Program/Module 的递归 bytecode 限制为 1,000,000 instructions、一个静态模块图限制为 1,024 个唯一 canonical modules 和 16,000,000 cumulative raw source bytes。零是有效边界；宿主可通过 `Engine::with_compile_limits` 或 `RuntimeBuilder::compile_limits` 显式降低或提高任一项，`Runtime::compile_limits` 与 `Context::compile_limits` 返回实际策略。

source bytes 在 `.litcoffee` 提取、lexer/parser/lowering 与 Runtime cache-key 复制之前按原始输入的 `str::len()` 计费，因此 `.coffee` 与 GitHub-compatible `.litcoffee` 使用同一 UTF-8 规则。`ResourceLimit::SourceBytes` 保留命名 source 的第 1 行归因。失败输入不查询或污染 Runtime cache，也不增加 cache miss。核心只能限制 `ModuleLoader::load` 返回 `String` 之后的处理；自定义 loader 仍负责约束其自身 I/O 与返回前分配。

lowering 从已发射的 top-level 与 nested source-map instruction 向量汇总递归 instruction 总数，并在 verifier 和 `ProgramExecutionPlan` 构造前检查；不记录 source map 的普通 `Chunk` 编译使用无需 identity set 的编译器生成树遍历。公开 raw `Chunk` 则通过按 identity 去重的递归遍历得到同一计数。越界为 `ResourceLimit::BytecodeInstructions`。`Program::instruction_count` 公开同一确定性计数。`Context::run_program` 在执行前再次按所属 Runtime 的策略检查，因此由更宽松 Engine 编译或从公开 `Chunk` 包装的 Program 不能绕过当前 Context 边界。

### 静态模块图预检

模块执行与 `Engine::fingerprint_module_graph` 使用相同的 `ModuleGraphBudget`。入口与依赖均按 canonical name 去重；同名同 source alias 不重复计数，同名不同 source 继续产生 inconsistent-source 错误。唯一模块越界为 `ResourceLimit::ModuleGraphModules`，累计原始 source bytes 越界或算术溢出为 `ResourceLimit::ModuleGraphSourceBytes`。单模块仍独立受 `SourceBytes` 与 `BytecodeInstructions` 限制。

`Context::run_module` 先完成整个静态图的 load、source/bytecode 检查、cycle 检查、import/export 验证和依赖边解析，再执行任何模块。预算、缺失 export、cycle 或编译失败因此不会留下 dependency 执行副作用；执行阶段复用预解析 edge，不二次调用可能有状态的 loader。图 fingerprint 同样不执行脚本，并在相同图上给出相同预算结论。

`RestrictedFileModuleLoader` 默认使用同一 1,000,000-byte 单文件边界，并允许 `with_max_source_bytes` 与 Engine/Runtime 策略对齐。它先检查 metadata，再通过 `Read::take(limit + 1)` 防止读取期间文件增长绕过边界。`qcoffee` 的 `--max-source-bytes`、`--max-bytecode-instructions`、`--max-module-graph-modules` 与 `--max-module-graph-source-bytes` 把同一显式策略用于文件/stdin 读取、普通编译、模块执行与图指纹。

### 边界

这些限制约束编译输入、保留的 bytecode 规模与静态图准备，不声称测量 allocator capacity、RSS、parser/lowering 瞬时峰值或宿主 loader 在返回前的内存。它们也不替代执行期 transient/live managed-memory、单条 instruction 或 native callback host-heap 增长模型；后者继续由 #76 跟踪。所有四类失败都是宿主可见、脚本不可捕获的 `ErrorKind::Resource`。`ResourceLimit` 现明确为 non-exhaustive；这与 RFC 0154 允许 patch 版本增加资源类别的契约一致，宿主应保留通配分支。新类别在既有 0–18 类别之后从 20 开始，19 保留给 `Error` 的私有“无资源类别”槽位；这样既保持旧判别值，也避免扩展公开类别时改变 VM 资源停止慢路径的布局。

---

## English

### Independent compilation policy

`CompileLimits` is separate from execution-time `ResourceLimits`. Its defaults bound one raw UTF-8 source at 1,000,000 bytes, one Program or Module at 1,000,000 recursive bytecode instructions, one static module graph at 1,024 unique canonical modules, and cumulative raw graph source at 16,000,000 bytes. Zero is a valid boundary. A host explicitly lowers or raises any field through `Engine::with_compile_limits` or `RuntimeBuilder::compile_limits`; `Runtime::compile_limits` and `Context::compile_limits` expose the effective policy.

Source bytes use raw `str::len()` before `.litcoffee` extraction, lexing/parsing/lowering, and Runtime cache-key copying, so `.coffee` and GitHub-compatible `.litcoffee` follow the same UTF-8 rule. `ResourceLimit::SourceBytes` attributes named source to line 1. Failed input neither queries nor contaminates a Runtime cache and does not increment cache misses. Core enforcement begins after `ModuleLoader::load` returns its `String`; a custom loader remains responsible for its own I/O and pre-return allocation.

Lowering totals recursive instructions from the emitted top-level and nested source-map instruction vectors, then checks the result before verification or `ProgramExecutionPlan` construction. Ordinary `Chunk` compilation without source maps traverses the compiler-produced tree without an identity set. Public raw `Chunk` values use an identity-deduplicated recursive traversal to obtain the same count. Overflow uses `ResourceLimit::BytecodeInstructions`, and `Program::instruction_count` exposes the same deterministic count. `Context::run_program` reapplies its owning Runtime policy before execution, preventing a Program compiled by a more permissive Engine or wrapped from public `Chunk` from bypassing the current Context boundary.

### Static module-graph preflight

Module execution and `Engine::fingerprint_module_graph` share `ModuleGraphBudget`. Entry and dependency modules are deduplicated by canonical name; same-name/same-source aliases are charged once, while same-name/different-source resolution retains the inconsistent-source error. Unique-module overflow uses `ResourceLimit::ModuleGraphModules`; cumulative raw source overflow, including arithmetic overflow, uses `ResourceLimit::ModuleGraphSourceBytes`. Every module independently remains subject to `SourceBytes` and `BytecodeInstructions`.

`Context::run_module` completes loading, source/bytecode checks, cycle checks, import/export validation, and dependency-edge resolution for the entire static graph before executing any module. A budget, missing-export, cycle, or compilation failure therefore leaves no dependency execution side effect. Execution reuses prepared edges and does not call a potentially stateful loader twice. Graph fingerprinting likewise executes no script and reaches the same budget decision for the same graph.

`RestrictedFileModuleLoader` defaults to the same 1,000,000-byte per-file boundary and exposes `with_max_source_bytes` to match a raised or lowered Engine/Runtime policy. It checks metadata first and then uses `Read::take(limit + 1)` so file growth during reading cannot bypass the boundary. `qcoffee` options `--max-source-bytes`, `--max-bytecode-instructions`, `--max-module-graph-modules`, and `--max-module-graph-source-bytes` apply one explicit policy to file/stdin reads, ordinary compilation, module execution, and graph fingerprinting.

### Boundary

These limits bound compilation input, retained bytecode scale, and static graph preparation. They do not claim to measure allocator capacity, RSS, parser/lowering transient peaks, or host-loader memory before return. They also do not replace an execution-time transient/live managed-memory model or bound host-heap growth from one instruction or native callback; #76 continues to track those concerns. All four failures are host-visible, script-uncatchable `ErrorKind::Resource` values. `ResourceLimit` is now explicitly non-exhaustive, matching RFC 0154's contract that patch releases may add resource categories; hosts must retain a wildcard match arm. New categories start at 20 after legacy categories 0–18, while 19 remains reserved for `Error`'s private “no resource category” slot. This preserves old discriminants and prevents future public-category additions from changing VM resource-stop slow-path layout.
