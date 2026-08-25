# RFC 0131：Verifier 站点与调用上下文诊断

> RFC 0136 取代脚本可观察的 catch 字符串；可信 source labels 与调用上下文仍不暴露给脚本。

- 状态：已采纳
- 日期：2026-08-24
- 依赖：RFC 0126、RFC 0128、RFC 0129、RFC 0130

## 动机

RFC 0130 让正常 `Program` 执行在失败 instruction 处获得 primary label，但编译器在 source map 已存在时执行的 bytecode verifier 仍会丢失其失败 instruction 位置；函数内错误也只显示最内层失败处，调用者无法得知 QuickCoffee 调用链。公开 `Chunk` 仍允许宿主手工构造，因此不能为没有编译来源的 bytecode 伪造位置。

## 决策

1. verifier 错误私有地记录失败的 `Chunk` identity 与 instruction index；这些临时元数据不进入 `Error` 公共 API、`Chunk`、指令编码、指纹或反汇编。
2. `Engine::compile_program`、命名 `Program`、模块和 CLI 的 mapped 编译路径只在 verifier 失败时，以 RFC 0130 的 sidecar 将该站点转换为 primary label。顶层、嵌套函数与 destructuring pattern default chunk 都使用各自的 map。
3. `Chunk::verify()`、`Engine::compile()`、`Context::run(Chunk)` 与 `Program::from(Chunk)` 仍不附加来源；空 chunk 等没有精确 instruction 站点的错误也不附加 label。
4. 运行期或 resource 错误保留 RFC 0130 primary label，并在错误穿过 QuickCoffee frames 时追加 `Secondary` label，局部 message 为 `called from here`。顺序为 primary、最近调用点至最外层调用点；来源名按各 frame 的定义/调用 `Program` 保持不透明。
5. 被脚本 `try/catch` 消费的错误继续只转换为既有错误文本，不产生新的脚本可观察值。`Error::Display` 与 CLI JSON 的既有 primary `line`/`source` 字段保持不变。
6. instruction-to-span 查找与 label 分配只在 compile verification failure 或 VM error/resource slow path 发生；成功 verifier/VM 执行不扫描 source map。

## 验收

测试必须覆盖 mapped 顶层、嵌套函数及 pattern default verifier 错误、裸 bytecode 无标签、嵌套调用链、跨 eval 来源名、Unicode/改写输入既有精度规则，以及 legacy display、字节码指纹与反汇编兼容。debug/release、qbench、Clippy、rustdoc、手册、MSRV 与 crate package 门禁必须通过。
