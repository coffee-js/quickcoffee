# RFC 0127：lexer/parser 精确源码 span

- 状态：已采纳
- 日期：2026-08-24
- 依赖：RFC 0047、RFC 0126

## 动机

RFC 0126 建立结构化诊断类型，但既有 lexer 仍只给每个 token 保存行号，parser 因而无法指出具体列与范围。直接把预处理后的字符位置当作原源码位置会产生错误诊断：heredoc、多行普通字符串和缩进映射都可能重写 lexer 输入。

## 决策

1. lexer 为能够一一对应原始物理行的真实 token 保存 `SourceSpan`。起点为含端点，终点为不含端点；列从 1 开始按 Unicode scalar value 计数。
2. 列追踪随字符游标线性推进。不得为取得列号而从行首重复扫描，也不得使用 UTF-8 字节偏移冒充列。token 流使用不含来源字符串的紧凑内部 span，只在产生公开 `Error` 时扩展为 `SourceSpan`，避免按 token 重复来源存储。
3. `Indent`、`Dedent`、自动 `Semi`、`Eof` 等布局合成 token 保持行级 span。显式源码分号属于真实 token，可有精确范围。
4. 只有 heredoc 与缩进映射预处理保持全文不变、且逻辑行与对应物理行相同时，才承诺该行列号精确。被多行字符串拼接或其他 lowering 改写的 token 保持 `column: None`、`end: None`。
5. parser 错误引用当前未消费 token；若错误是在取出 token 后才确认，则引用刚消费的 token。parser 不再把这种错误错误地指向后续 token。
6. lexer 的非法字符错误在来源未改写时附加该单个 Unicode 标量的精确 span。尚未迁移的复合词法错误继续保留行级位置，后续切片逐项收敛。

本 RFC 不给 AST、bytecode instruction 或运行期值附加 span，不改变字节码编码与 `Program::fingerprint()`。来源名称、模块图传播、verify/runtime 诊断属于 #74 后续工作。

## 验收

测试必须覆盖 ASCII parser token、Unicode 标量列、已消费 token 的错误位置、非法字符的半开范围，以及预处理改写后不虚构列。原有行号、Display、JSON、确定性编译压力语料和字节码指纹保持不变。
