# RFC 0043：嵌入方结构化错误

> RFC 0136 将可捕获 Runtime 错误升级为结构化 Error 值；本 RFC 的宿主 ErrorKind/message 与通用 Display 契约继续有效。

- 状态：已采纳
- 依赖：RFC 0002、RFC 0041

`ErrorKind` 是公开、可比较的枚举，包含 `Parse`、`Verify` 与 `Runtime`。每个 `Error` 公开 `kind()` 与 `message()`：前者供宿主稳定分流，后者提供不含类别前缀的人类可读详情。`Display` 继续输出既有的 `"{kind} error: {message}"` 形式，因而 CLI 与 QuickCoffee 中 catch 到的错误文本保持兼容。

解析、对嵌入方 `Chunk` 的验证、以及 VM/原生函数运行失败分别产生相应类别；宿主不必解析展示文本，也不获得 VM 内部状态。验收测试覆盖三类错误、详情访问器与既有验证入口。
