# RFC 0103：qbench 输出 schema 版本

- 状态：已采纳
- 依赖：RFC 0081、RFC 0087

`qbench --json` 每条记录新增稳定的 `schema` 与 `version` 字段：`schema` 固定为 `quickcoffee.qbench.v1`，`version` 为当前 crate 版本。已有 `name`、`iterations`、`expected`、`compile_ns`、`verify_ns`、`execute_ns` 字段保持不变；issue #102 以可选加法字段加入 `prepare_ns`，单独记录 `Engine::compile_program` 的端到端准备。所有 `*_ns` 字段仍表示本进程内指定迭代次数的纳秒总耗时，而非单次平均值。

默认文本输出也带有同样的 `schema=` 与 `version=` 前缀。机器采集方应先检查 schema，再按字段读取；未来不兼容变更必须使用新的 schema 标识，不得静默改变 v1 字段含义。

## 验收

集成测试必须验证每条 JSON 记录含 schema、当前 crate 版本及原有字段；`--iterations`、语义护栏和逐行输出契约保持不变。
