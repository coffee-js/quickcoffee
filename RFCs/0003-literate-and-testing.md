# RFC 0003：文学编程与脚本测试

- 状态：已采纳
- 依赖：RFC 0000、RFC 0001、RFC 0002

## `qdocco`

`qdocco FILE [-o OUTPUT]` 对 QuickCoffee 源文件执行语法、编译和运行时验证，并产出无需网络资源的单页 HTML。以 `##` 开头的注释行归入相邻的说明栏，其余行归入代码栏。页面在页末记录程序最终值；标准输出仍由宿主进程输出，故不作为可比较的文档产物。

`qdocco --check FILE` 只执行验证，不写 HTML。任一读取、解析、编译或执行错误以非零状态退出。该工具绝不执行 Markdown、HTML、JavaScript 或非 QuickCoffee 文件内容。

## `qtest`

`qtest [--fuel N] FILE_OR_DIRECTORY...` 是一个最小内建测试运行器。目录会递归发现并按路径排序执行 `.qc` 文件；每个文件必须成功执行且最终值严格为 `true`。任何其他值、错误或读取失败均为失败。`--fuel N` 为每个文件设置独立执行预算，默认 1,000,000。`# test: 描述` 仅供人阅读并不改变语义。该约定使测试既是可直接执行的 QuickCoffee 案例，也是完整的 API/CLI 演示。

## 验收

集成测试必须覆盖成功的 HTML 生成、`--check`、通过与失败的 `qtest` 文件，以及 HTML 转义。生成 HTML 的正文须保留输入源的字面文本（经 HTML 转义）。
