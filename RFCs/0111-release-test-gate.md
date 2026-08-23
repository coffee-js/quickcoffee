# RFC 0111：release profile 完整测试门禁

- 状态：已采纳
- 依赖：RFC 0045、RFC 0081、RFC 0107、RFC 0110

## 约束

`make check` 必须在 debug 与 release 两种 profile 执行完整 `cargo test --locked`。release 测试覆盖库单元测试、所有 CLI 集成测试、公开嵌入 API、五份文学手册、RFC 索引和鲁棒性语料；它不能只依赖 qbench 的少量计时负载。`cargo test --locked --examples` 仍以 debug profile 单独检查示例编译。

release qbench 继续负责优化 VM 的全套 34 个语义负载与 compile/verify/execute 计时；本 RFC 的目标是让非 benchmark 的 CLI、嵌入和错误路径也经过优化构建验证。门禁不设置机器相关的时间阈值。

## 验收

`make check` 必须包含 `cargo test --locked --release`，并在 Rust 1.85 与 stable 工具链上均成功。任何 profile 的测试失败都阻止 PR 进入 `CLEAN` 状态。
