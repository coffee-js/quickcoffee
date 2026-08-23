# RFC 0088：统一 CLI 版本查询

- 状态：已采纳
- 依赖：RFC 0019、RFC 0062、RFC 0081、RFC 0083

## 动机

QuickCoffee 由 `qcoffee` 主解释器和 `qtest`、`qdocco`、`qbench` 三个辅助工具组成。
发行包、脚本和诊断报告需要一种不执行用户代码、不读取输入文件的方式确认工具版本；只有
主命令提供版本查询会使工具链探测不一致。

## 契约

四个可执行文件都接受 `--version`，在标准输出打印一行 `<工具名> <Cargo 包版本>`，退出码
为 0，且不产生标准错误或副作用。版本值来自同一个 `CARGO_PKG_VERSION`，因此不会在各
工具之间漂移。`--help` 的用法文本同时列出版本查询形式；`--version` 在参数解析阶段提前
返回，不能与脚本、测试目录或 qdocco 输出操作组合。

## 验收

`qcoffee --version`、`qtest --version`、`qdocco --version` 与 `qbench --version` 均应输出
对应工具名和相同版本，并通过 `tests/cli_tools.rs` 的黑盒测试。其他 CLI 行为与运行时语义
保持不变。
