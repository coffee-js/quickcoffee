# RFC 0121：基准离散度报告

- 状态：已采纳
- 依赖：RFC 0081、RFC 0103、RFC 0105、RFC 0120

仅有中位数不能说明一次基准测量是否稳定。`qbench` 与 `qbench --compare-qjs` 必须保留每个阶段的重复样本，并在现有中位数之外输出 median absolute deviation（MAD）：每个样本到上侧中位数的绝对差再取上侧中位数。`--repeat 1` 的 MAD 恒为零；正式报告应使用至少 11 个样本。

`quickcoffee.qbench.v1` 保持原有字段和含义，新增可选的 `compile_mad_ns`、`verify_mad_ns`、`execute_mad_ns`。issue #102 的可选 `prepare_ns` 同样配有 `prepare_mad_ns`；它们与其他阶段使用相同的每轮 `iterations` 口径。`quickcoffee.qcompare.v1` 同样保持原字段，新增 `quickcoffee_cli_mad_ns` 与 `quickjs_cli_mad_ns`。因此既有消费者可继续读取原有字段；需要判断噪声的消费者可读取新增字段。

MAD 只描述同一命令、同一机器、同一阶段的离散度，不把 CLI 端到端读数伪装成预编译热执行，也不构成跨机器或跨引擎的性能结论。`qbench` 的默认重复数和 CI 语义门禁不变，避免将机器噪声带入通过/失败判定。

验收包括默认和多次重复输出的所有 MAD 字段；文档必须说明 11 次样本建议及其与中位数的关系。
