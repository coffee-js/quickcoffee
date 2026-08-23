# RFC 0130：Program source map 与运行期归因

- 状态：已采纳
- 日期：2026-08-24
- 依赖：RFC 0117、RFC 0119、RFC 0126、RFC 0128、RFC 0129

## 动机

RFC 0129 只让 lowering 自身拒绝的 AST 节点保留 span。正常表达式一旦编译为 bytecode，运行期 `unknown name`、严格类型错误、容器错误、宿主回调错误和资源中断仍无法关联回源码。直接给公开 `Chunk` 增加私有字段会破坏外部 struct literal，给 `Instruction` 增加调试指令又会改变 program counter、jump offset、disassembly 与指纹，因此 source map 必须是执行语义之外的 sidecar。

## 决策

1. parser 为每个成功表达式保存其起始 token span；assignment 与 destructuring statement 另存 statement 起点。lowering 期间，表达式产生的 instruction 继承当前表达式 span，`Store`/`Destructure` 继承 statement span，丢弃结果的 comprehension 继承其外层表达式 span。
2. compiler 生成私有 source-map sidecar。每条 instruction 只保存一个 32-bit span ID，`0` 表示未知；完整紧凑 `TokenSpan` 按表达式保存在 chunk 本地表中。source map 覆盖顶层、嵌套 bytecode 函数和 pattern 默认值 chunk。
3. sidecar 只存入私有 `Program` 数据。公开 `Chunk { constants, code }`、`Constant` 与 `Instruction` 不增加字段或 variant；`Chunk::fingerprint()`、`Program::fingerprint()`、disassembly、verification 规则及 jump offset 不读取 source map。
4. `Engine::compile_program`、`compile_program_named`、`Context::eval`、`eval_named`、命名模块和 CLI 使用带 sidecar 的编译路径。`Engine::compile` 仍返回原始 `Chunk`；`Context::run` 与 `Program::from(Chunk)` 对宿主提供的裸 bytecode 不虚构来源。
5. 运行期错误在尚无 label 时使用失败 instruction 的 span；函数 arity、call-depth 与 native callback 错误使用 caller 的 call expression span。嵌套函数、跨 eval 保留的函数和跨模块导入函数使用其定义 Program 的 source map。fuel/cancellation 在停止前使用下一条 instruction 的 span。
6. source-map 查询和 `SourceSpan`/来源字符串构造只发生在错误慢路径。成功 VM instruction 热路径不查询 map；VM frame 直接共享其定义 `Program` 的调试信息，使跨 eval 与跨模块函数无需扫描或注册 source-map 表。
7. 结构化 `labels()` 与 `qcoffee --json` 的既有 `line`/可选 `source` 字段获得新数据；legacy `Display` 对 runtime、resource 与 verify 错误继续保持无位置文本，parse 错误保持既有行号文本。已有 label 的宿主错误不被覆盖。
8. 本 RFC 不给宿主伪造 bytecode 的 verifier 错误补来源，也不增加 stack trace 或 secondary call-site label；这些属于 #74 后续工作。

## 验收

测试必须覆盖顶层表达式、嵌套函数、默认参数、destructuring、native callback、fuel、跨 eval 保留函数、依赖模块函数、匿名 Program、CLI 文件 JSON 与无 source map 的裸 `Chunk`。相同源码的 `Chunk` 与 `Program` 指纹/disassembly 必须一致；debug/release、qbench、Clippy、rustdoc、手册、MSRV 与 crate package 门禁必须通过。
