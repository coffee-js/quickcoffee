# RFC 0083：`qdocco` Markdown 产物

- 状态：已采纳
- 依赖：RFC 0003、RFC 0005

## 动机

文学编程源同时需要可读说明、可复制代码和可比较的最终值。`qdocco` 已能生成单页 HTML，但 Markdown 产物有利于源码审阅、静态站点和无浏览器环境；两者必须共享同一验证和执行路径。

## CLI 契约

`qdocco --markdown FILE [-o OUTPUT]` 先按普通 `qdocco` 路径读取、编译、验证并执行文件，再生成 Markdown。默认输出为输入文件同名的 `.md`；`-o` 指定其他路径。文档包含 `## Notes` 说明栏、`quickcoffee` 代码栏和 `## Final value` 最终值栏。代码围栏至少使用四个反引号，并按源码中最长的连续反引号序列自适应加长，以保留源文本而不让其中的 Markdown 标记执行。

输出路径经规范化后不得与输入源码相同（包括指向输入的符号链接），避免 `-o` 意外破坏文学源文件；此时命令以用法错误退出。

`--markdown` 与 `--check` 互斥；冲突返回退出码 2。读取、解析、验证或执行错误沿用现有非零退出语义。`qdocco --check` 仍只验证而不写任何产物，HTML 默认行为不变。

Markdown 产物以 `quickcoffee.qdocco.markdown.v1` HTML 注释标识模板版本。模板布局或机器可读契约发生不兼容变化时必须递增版本；普通说明与代码内容变化不递增版本。

`--incremental` 仍完成读取、编译、验证、执行和渲染，只在最终 UTF-8 字节与已有目标完全相同时保留目标并报告 `unchanged PATH`。目标缺失或字节变化时报告 `wrote PATH`，并沿用 RFC 0101 的原子替换。它不使用源码摘要跳过执行，因而不是执行结果缓存。`--check --incremental` 与 `--check --markdown` 同样以退出码 2 拒绝。

## 验收

集成测试必须验证 Markdown 的模板版本、说明、代码和最终值栏，源文本在围栏中保留，默认扩展名与 `-o` 路径正确，增量生成区分 `wrote` / `unchanged`，以及冲突参数被拒绝。五语手册源仍须通过 `qdocco --check` 与 `make docs`。
