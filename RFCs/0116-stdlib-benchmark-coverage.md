# RFC 0116：数值标准库性能负载覆盖

- 状态：已采纳
- 依赖：RFC 0045、RFC 0096、RFC 0109、RFC 0113

## 动机

RFC 0113 新增了严格数值标准库函数，但既有 benchmark 只覆盖语言运算和容器路径。若不把标准库调用纳入同一编译、验证、执行计时口径，性能报告无法发现其回归，也无法比较宿主回调与 VM 内建函数的实际成本。

## 契约

1. `qbench` 和 `cargo bench --bench core` 都必须包含同名的 `stdlib-abs`、`stdlib-sum`、`stdlib-min-max`、`stdlib-range-sum` 负载。
2. 每个负载在编译、验证和执行阶段都检查 RFC0113 的最终值；qbench JSON schema、默认完整集合和 `--only` 选择语义不变。
3. 标准 benchmark 继续记录重复迭代吞吐，不设置跨机器的硬时间阈值；性能报告必须说明样本口径和环境。

## 验收

`tests/cli_tools.rs` 必须枚举并验证四个机器可读负载名；`make qbench-check` 和 `make bench` 必须执行全套；`PERFORMANCE.md` 必须列出新增负载及复现实验命令。
