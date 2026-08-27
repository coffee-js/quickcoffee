# RFC 0005：可执行用户手册管线

- 状态：已采纳
- 依赖：RFC 0003、RFC 0004

## 输入与产物

权威手册源位于 `manuals/manual.<locale>.litcoffee`。它们遵循 RFC 0145：未缩进的 Markdown 是叙述，缩进代码块是被展示且执行的示例。`make docs` 通过 `qdocco` 产生被跟踪的 `docs/manual.<locale>.md` GitHub 可读投影，并在写入前完成解析、编译和运行。显式的 `make docs-html` 可在 `target/manuals/` 生成本地单页 HTML 预览；HTML 是被忽略的构建产物，不进入版本库。

当前 locale 为 `zh-CN`、`classical-zh`、`en`、`latin` 与 `devanagari-sa`。最后一项是将用户所称“天成文”暂按天城文（Devanagari）处理的可替换约定，不构成对该名称的语言学断言。

## 验证

每份源的最终值必须为 `true`，以便既可由 `qdocco --check` 验证，也可由 `qtest` 作为演示运行。Markdown 与 HTML 都是派生产物；源文本是唯一应人工编辑的权威内容。修改引擎语义或示例后，必须运行 `make docs` 和全套测试；需要浏览器预览时再运行 `make docs-html`。CI 先运行 `make docs && make check`，再要求完整工作区无已跟踪差异或未跟踪文件，拒绝未提交的派生产物差异。
