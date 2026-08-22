# RFC 0005：可执行用户手册管线

- 状态：已采纳
- 依赖：RFC 0003、RFC 0004

## 输入与产物

权威手册源位于 `manuals/manual.<locale>.qc`。它们是可运行 QuickCoffee 文本：`##` 注释行是叙述，非注释行是被展示且被执行的示例。`qdocco` 产生 `docs/manual.<locale>.html`，并在写入前完成解析、编译和运行。

当前 locale 为 `zh-CN`、`classical-zh`、`en`、`latin` 与 `devanagari-sa`。最后一项是将用户所称“天成文”暂按天城文（Devanagari）处理的可替换约定，不构成对该名称的语言学断言。

## 验证

每份源的最终值必须为 `true`，以便既可由 `qdocco --check` 验证，也可由 `qtest` 作为演示运行。HTML 是派生产物；源文本是唯一应人工编辑的权威内容。修改引擎语义或示例后，必须重新生成 HTML 并运行全套测试。CI 先运行 `make docs && make check`，再用 `git diff --exit-code -- docs` 拒绝未提交的派生产物差异。
