# RFC 0081：`qbench` 可复现基准命令

- 状态：已采纳
- 依赖：RFC 0002、RFC 0045、RFC 0069

## 动机

`cargo bench --bench core` 适合开发者查看完整基准，但其文本输出不便于 CI 保存、比较或绘图。项目需要一个不依赖外部运行时的轻量基准入口，同时保留每个负载的语义护栏。

## CLI 契约

`qbench [--iterations N] [--json]` 对内建负载逐一测量：源码编译、含源码映射和私有执行 sidecar 的共享 `Program` 准备、已验证 `Program` 的验证调用、以及每轮新 `Context` 的执行。默认 `N=100`，必须为正整数；每个负载在计时循环内都验证最终值与预期值一致，语义错误直接使命令失败。

每个负载输出一条记录，核心字段为 `name`、`iterations`、`expected`、`compile_ns`、`prepare_ns`、`verify_ns`、`execute_ns`。`compile_ns` 测 `Engine::compile`，`prepare_ns` 测 `Engine::compile_program` 的端到端准备；后者包含重复的解析、lowering、验证、源码映射和 sidecar 构建，不用两字段相减来声称隔离的 sidecar 时间。`--json` 输出一行一个 JSON 对象，适合 CI 逐行采集；默认文本输出为同一字段的空格分隔记录。计时值是本次进程的纳秒总耗时，不作为跨机器性能结论；正式报告仍须按 RFC 0002 记录硬件、工具链、样本和中位数。

## 验收

集成测试必须验证 JSON 每个负载一行且字段完整、迭代次数可配置、默认文本模式不输出 JSON、非法迭代参数返回退出码 2，以及所有内建负载的语义护栏通过。
