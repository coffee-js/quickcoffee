# RFC 0109：qbench 核心负载全套护栏

- 状态：已采纳
- 依赖：RFC 0045、RFC 0081、RFC 0105、RFC 0107

## 约束

`cargo bench --bench core` 中的每一个命名负载都必须同时出现在 `qbench --json` 的内建负载集合中。两者使用相同的源程序和最终值期望；core benchmark 可以使用更大的迭代次数，而 qbench 负责较小、可在 CI 中运行的编译、验证和执行语义护栏。

qbench 输出必须为每个负载产生恰好一条记录，记录包含 schema、版本、负载名、迭代数、重复数、期望值和三个阶段的纳秒计时。每轮计时前后仍需经过 RFC 0045 的最终值检查；任何负载编译、验证或执行失败都使命令失败。新增 core benchmark 负载时，必须同步加入 qbench 并更新 CLI 集成断言。

## 性能口径

`make check` 使用 `--iterations 1 --repeat 3` 覆盖全套负载，保证优化后的 VM 路径不会绕过 release 门禁；正式性能数据仍由 `cargo bench --bench core` 和 RFC 0096 的重复采样方法产生。该门禁不设机器相关的时间阈值，只验证语义和输出结构。

## 验收

CLI 集成测试必须断言全套负载名称各出现一次，并检查每条 JSON 记录的 schema、版本、重复数、期望值及 compile/verify/execute 字段。
