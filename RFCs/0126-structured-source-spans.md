# RFC 0126：结构化源码 span 契约

- 状态：已采纳
- 日期：2026-08-23
- 依赖：RFC 0002、RFC 0043、RFC 0047、RFC 0125

## 动机

RFC 0047 只公开从一开始计数的源码行。编辑器、CLI、模块宿主与批量诊断还需要列、范围、来源名称，以及主错误与关联位置的区别；如果把这些信息拼入 `Display` 文本，下游仍必须解析不稳定的人类文本。另一方面，现有词法器尚未保存列号，不能用虚构的第 1 列冒充精确位置。

## 数据契约

1. `SourcePosition` 包含从一开始计数的 `line` 和可选 `column`。列按 Unicode scalar value 计数，不是 UTF-8 字节、UTF-16 code unit、grapheme cluster 或终端显示宽度。`None` 表示当前阶段只可靠地知道行。
2. `SourceSpan` 包含可选 `source_name`、含端点 `start` 与可选的不含端点 `end`。有 `end` 的范围采用半开区间 `[start, end)`；无 `end` 表示行级位置，不得推断长度或列。
3. `source_name` 是由 CLI、模块加载器或嵌入宿主提供的不透明 UTF-8 标识，可为文件路径、规范模块名或虚拟文档名。引擎不规范化路径、不访问文件或网络，也不把名称解释为权限。
4. `DiagnosticLabel` 由 `kind`、`span` 与可选局部 `message` 组成。`Primary` 指示直接错误位置，`Secondary` 指示定义处、前一声明或配对分隔符等关联上下文。标签顺序稳定；一个错误至多有一个 primary，secondary 按源码发现顺序排列。
5. `Error::labels()` 返回结构化标签；`Error::kind()`、`message()` 与 `resource_limit()` 保持既有职责。`Error::position()` 是兼容访问器，返回 primary span 的 start；其既有 `line` 不变，新 `column` 在未精确保存时为 `None`。

字段和访问器是展示无关契约。`Display` 与既有 `qcoffee --json` 的 `line` 字段在本 RFC 中保持兼容；未来加入列、end、source 或多标签的机器输出必须以向后兼容字段或版本化 schema 完成。

## 分阶段实施

本 RFC 先建立公开数据类型，并把现有行级解析错误表示为一个 primary、无 end、列未知的标签；它不声称当前错误已经精确到列。后续实现按以下顺序推进：

1. lexer 为真实 token 和合成布局 token 生成可靠 span；
2. AST、lowering、bytecode 与模块图保留来源和 primary/secondary 关系；
3. parse、verify、runtime 与 resource 错误在来源已知时附加 span；
4. CLI 和嵌入 API 增加版本化的结构化输出，同时保持旧 `line` 消费方可用。

RFC 0127 完成第一步中可与原始物理行一一对应的 lexer token 与 parser 错误范围；被预处理改写的 token 仍保持列未知，AST、bytecode 与运行期传播尚未完成。

RFC 0128 把嵌入宿主、命名模块与 CLI 文件入口已知的不透明来源名附到现有 label，并为 `qcoffee --json` 增加可选 `source` 字段；它不为无位置错误合成 label，也不提前声称运行期 span 已完成。

字节码指纹在 span 未进入规范编码前不得改变。任何无法可靠定位的错误宁可没有标签，也不得伪造列或来源名。

## 验收

公开 API 测试覆盖 primary 标签、可选来源/end/列的缺省状态、`position()` 行号兼容，以及无来源 runtime error 的空标签。RFC 索引、Clippy 和 missing-docs 检查必须通过。
