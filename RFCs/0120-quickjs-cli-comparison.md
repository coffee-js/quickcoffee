# RFC 0120：QuickJS CLI 对比基准

- 状态：已采纳
- 依赖：RFC 0081、RFC 0103、RFC 0105

`qbench --compare-qjs PATH` 运行双方都能表达并校验最终值的版本化负载集合。初始的 `scalar-loop` 与 `function-loop` 之外，#79 加入 `array-build-index-iterate`、`map-own-lookup`、`map-functional-update`、`unicode-scalar-iterate` 与 `unicode-scalar-index`。它从 `qbench` 同目录启动 `qcoffee`，并以 `PATH -e` 启动 QuickJS；故读数是每轮 CLI 启动、解析、编译和执行的端到端中位数，不可与 `qbench.v1` 的进程内 `Program` 执行字段混用。

新增负载保持以下等价边界：

- 数组负载在两端各构造一份 `0..999` 数组，以相同的显式下标循环读取元素，期望值为 `1498500`。
- 映射读取只访问字面量创建的固定自身键，不依赖 JavaScript 原型；期望值为 `100000`。
- 映射更新在两端都使用 spread 创建新值并覆盖 `beta`，不测原地可变对象；期望值为 `507502`。
- Unicode 遍历在两端都按 Unicode 标量枚举 `a☕中🙂z`，期望值为 `15000`。
- QuickCoffee 字符串索引按 Unicode 标量定义，JavaScript 下标则按 UTF-16 code unit 定义。因此索引负载由 QuickJS 在计时函数内先以 `Array.from` 建立标量数组，再进行重复索引；期望值为 `22000`。这是为匹配结果语义而显式记录的适配，不是底层操作同构或 JavaScript 兼容性声明。

负载名称、双方源码与期望值共同构成该 schema 的可复现实验集合；改变它们时必须同步本 RFC、性能报告和语义护栏。每次 CLI 执行及预编译热执行都核对期望值，任一端漂移即停止计时并以非零退出。

比较输出使用独立 schema `quickcoffee.qcompare.v1`，每个负载包含 `name`、`iterations`、`repeat`、`expected`、`quickcoffee_cli_ns` 与 `quickjs_cli_ns`。RFC 0121 额外定义同名的 `*_mad_ns` 离散度字段。`--compare-iterations` 控制每个采样内的 CLI 次数，默认一；`--repeat` 仍控制样本数并取上侧中位数。建议构建全部 CLI 后以 `--compare-iterations 1 --repeat 11 --json` 运行。

该模式不下载、安装或选择 QuickJS；PATH 由调用方明确提供。若 `qcoffee` 不与 `qbench` 一同安装，或 QuickJS 无法运行/结果不符，命令以非零退出。它不改变现有 `qbench.v1` 字段或 CI 门禁。

验收覆盖参数错误、模式冲突、稳定 schema、名称与期望值同记录配对，以及对官方 QuickJS 的 11+ 样本手动复现实验；五种手册与性能报告说明其端到端边界。
