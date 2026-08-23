# RFC 0047：源码行诊断

- 状态：已采纳
- 依赖：RFC 0002、RFC 0043

`SourcePosition` 是公开的、从一开始计数的源码行号。`Error::position()` 在词法或解析器能确定来源行时返回 `Some(SourcePosition)`；验证和运行期错误、没有可靠来源行的预处理错误返回 `None`。列号暂不承诺，避免把 UTF-8 字节偏移误称为用户可见列。

`Display` 在存在位置时输出 `parse error (line N): ...`，没有位置时保持原有 `kind error: ...` 格式。嵌入方应使用 `kind()`、`message()` 与 `position()`，不应解析展示文本。CLI 直接使用同一 Display，因此命令行与 Rust API 的类别、详情和行号一致。

验收覆盖 parser 语法错误、lexer 非法字符错误、公开位置访问器、显示文本和无位置的运行期错误；现有 catch 错误字符串只对运行期错误保持不变。

RFC 0126 在不改变本 RFC 行号兼容性的前提下增加可选列、半开 span、来源名称与主/次标签；尚未保存精确列的既有错误明确返回 `column: None`。
