# RFC 0105：qbench 重复采样与中位数

- 状态：已采纳
- 依赖：RFC 0081、RFC 0096、RFC 0103

`qbench` 增加 `--repeat N`（默认 `1`），对每个内建负载完整执行 N 次编译、`Program` 准备、验证和执行计时。每一轮都运行语义护栏；输出的 `compile_ns`、`prepare_ns`、`verify_ns`、`execute_ns` 是各阶段样本的中位数，仍表示单轮内 `--iterations` 次迭代的纳秒总耗时。偶数样本取排序后的上侧中位数（索引 `N / 2`）。

JSON 与文本输出新增 `repeat` 字段/键；`--repeat 0` 或非整数返回退出码 2。默认 `--repeat 1` 保持既有单次采样行为，schema 仍为 `quickcoffee.qbench.v1`。

## 验收

集成测试必须验证默认和显式 repeat 值、JSON 的中位数记录字段、非法 repeat 参数及每轮语义护栏；性能报告可直接使用 `--repeat 3` 取得三次中位数。
