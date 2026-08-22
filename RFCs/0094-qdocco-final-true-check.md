# RFC 0094：qdocco 文学源最终值门禁

- 状态：已采纳
- 依赖：RFC 0003、RFC 0005、RFC 0083

## 动机

`qdocco --check` 是文学编程源和用户手册的可执行验收入口。RFC 0005 规定每份手册最后
必须得到布尔 `true`，但旧实现只判断源程序没有读取、解析或运行错误，导致返回数字、字符串
或 `nil` 的文档错误地通过检查。

## 契约

`qdocco --check FILE` 在读取、编译、验证和执行成功后，还必须要求最终值严格为
`Value::Bool(true)`。其他值以非零退出码（1）失败，并在标准错误说明实际值与期望 `true`。
普通 HTML/Markdown 生成模式仍可展示任意最终值，便于文档工具调试；该门禁只属于显式
`--check`。qdocco 不执行 Markdown、HTML 或 JavaScript 内容。

## 验收

CLI 集成测试覆盖最终值 `true` 的通过、数字最终值的拒绝及既有 HTML/Markdown 输出。五份
文学手册继续由 `qdocco --check` 验证，`make check` 与生成物一致性必须通过。
