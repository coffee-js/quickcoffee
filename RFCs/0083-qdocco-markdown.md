# RFC 0083：`qdocco` Markdown 产物

- 状态：已采纳
- 依赖：RFC 0003、RFC 0005

## 动机

文学编程源同时需要可读说明、可复制代码和可比较的最终值。`qdocco` 已能生成单页 HTML，但 Markdown 产物有利于源码审阅、静态站点和无浏览器环境；两者必须共享同一验证和执行路径。

## CLI 契约

`qdocco --markdown FILE [-o OUTPUT]` 先按普通 `qdocco` 路径读取、编译、验证并执行文件，再生成 Markdown。默认输出为输入文件同名的 `.md`；`-o` 指定其他路径。文档包含 `## Notes` 说明栏、四反引号 `quickcoffee` 代码栏和 `## Final value` 最终值栏。代码放在四反引号围栏内，以保留源文本而不让其中的 Markdown 标记执行。

`--markdown` 与 `--check` 互斥；冲突返回退出码 2。读取、解析、验证或执行错误沿用现有非零退出语义。`qdocco --check` 仍只验证而不写任何产物，HTML 默认行为不变。

## 验收

集成测试必须验证 Markdown 的说明、代码和最终值栏，源文本在围栏中保留，默认扩展名与 `-o` 路径正确，以及 `--check --markdown` 冲突被拒绝。五语手册源仍须通过 `qdocco --check` 与 `make docs`。
