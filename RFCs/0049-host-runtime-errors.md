# RFC 0049：宿主回调运行时错误

> RFC 0136 取代 catch 字符串，并增加 `Error::domain`；`Error::runtime` 与未捕获通用 Display 继续兼容。

- 状态：已采纳
- 依赖：RFC 0002、RFC 0011、RFC 0043

`Context::add_native` 注册的 Rust 回调返回 `Result<Value, Error>`。为使嵌入方能够直接报告自己的失败，`Error::runtime(message)` 作为公开构造器创建 `ErrorKind::Runtime`，保留结构化 `kind()` 与不带前缀的 `message()`。

宿主错误与 VM 原生错误走同一传播路径：未捕获时从 `Context::eval`/`run_program` 返回；在 QuickCoffee `try` 保护区内，`catch` 名称得到稳定的 `runtime error: ...` 字符串；`finally` 仍按 RFC 0011 执行。该 API 不暴露 VM 帧、环境或 JavaScript 错误对象。

验收覆盖宿主回调直接返回错误、错误类别与详情访问器，以及脚本捕获宿主错误后的稳定字符串。
