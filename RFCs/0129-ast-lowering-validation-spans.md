# RFC 0129：AST lowering 验证 span

- 状态：已采纳
- 日期：2026-08-24
- 依赖：RFC 0119、RFC 0126、RFC 0127、RFC 0128

## 动机

parser 能为 token 产生精确范围，但有些错误必须等到 AST lowering 才能判断：`break`/`continue` 是否位于循环内、`return` 是否位于函数内、模块指令是否误送入单文件编译，以及同一模块是否重复导出名称。此前这些错误在 AST 中丢失了触发 token 的位置，因此即使命名入口已经知道来源，也只能返回无 label 的错误。

## 决策

1. AST 的 `break`、`continue`、`return` 节点保存对应关键字的紧凑内部 `TokenSpan`。lowering 发现其上下文非法时，以该范围产生 primary label。
2. AST 的 `import`、赋值式 `export` 与列表式 `export` statement 保存指令关键字范围。单文件编译拒绝模块指令时指向该关键字；模块解析在拆分 directives 与可执行 body 时继续携带 export span。
3. 重复模块导出指向后出现的 `export` directive，并保留 `Engine::compile_module` 的不透明模块名。第一处定义的 secondary label 留待多标签 AST 数据流完成后添加。
4. AST 只保存 lexer 已确认可靠的内部 span。Unicode 列仍按 scalar value 计数；缩进 map 等预处理改写输入保持 `column: None`、`end: None`，lowering 不反推或伪造列。
5. 本切片不向公开 `Chunk`、`Instruction` 或常量编码加入 span，不改变 disassembly 与 `Program::fingerprint()`。RFC 0130 已通过私有 `Program` sidecar 完成成功表达式/statement 的运行期归因；裸 bytecode verification 与多标签调用栈仍属于 #74 后续工作。

## 验收

测试必须覆盖命名来源中的 `break`、`continue`、`return` 精确关键字范围，嵌套 Unicode 源码列，预处理改写后的行级降级，单文件模块指令，以及重复 export 的后出现位置。debug/release、指纹、Clippy、rustdoc、手册和 MSRV 门禁必须保持通过。
