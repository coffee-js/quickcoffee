# RFC 0120：QuickJS CLI 对比基准

- 状态：已采纳
- 依赖：RFC 0081、RFC 0103、RFC 0105

`qbench --compare-qjs PATH` 运行双方都能表达并校验最终值的 `scalar-loop` 与 `function-loop`。它从 `qbench` 同目录启动 `qcoffee`，并以 `PATH -e` 启动 QuickJS；故读数是每轮 CLI 启动、解析、编译和执行的端到端中位数，不可与 `qbench.v1` 的进程内 `Program` 执行字段混用。

比较输出使用独立 schema `quickcoffee.qcompare.v1`，每个负载包含 `name`、`iterations`、`repeat`、`expected`、`quickcoffee_cli_ns` 与 `quickjs_cli_ns`。RFC 0121 额外定义同名的 `*_mad_ns` 离散度字段。`--compare-iterations` 控制每个采样内的 CLI 次数，默认一；`--repeat` 仍控制样本数并取上侧中位数。建议构建全部 CLI 后以 `--compare-iterations 1 --repeat 11 --json` 运行。

该模式不下载、安装或选择 QuickJS；PATH 由调用方明确提供。若 `qcoffee` 不与 `qbench` 一同安装，或 QuickJS 无法运行/结果不符，命令以非零退出。它不改变现有 `qbench.v1` 字段或 CI 门禁。

验收覆盖参数错误、模式冲突、稳定 schema 和对官方 QuickJS 的一次手动复现实验；五种手册与性能报告说明其端到端边界。
