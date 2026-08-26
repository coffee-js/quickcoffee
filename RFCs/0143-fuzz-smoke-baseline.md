# RFC 0143：可复现 fuzz smoke 基线

- 状态：已采纳
- 日期：2026-08-26
- 依赖：RFC 0034、RFC 0048、RFC 0087、RFC 0110

## 动机

常规 Rust 测试覆盖已知语义与回归，但 lexer/parser 恢复和公开 `Chunk::verify` 必须能承受未知字节输入。fuzzing 需要与稳定发布工具链隔离，且不能把无限时间的随机探索伪装成每个 PR 的阻断门禁。

## 决策

1. `fuzz/` 是独立 cargo-fuzz package，不纳入发布 crate 或 Rust 1.85/stable workspace 检查；它只使用固定的 `nightly-2025-03-28`。
2. `parser` target 将任意 bytes 作为 lossy UTF-8 交给 `Engine::check_program`；`verifier` target 将 bytes 映射为公开指令的 Chunk，再调用 `Chunk::verify`。正常语言错误允许，panic/abort 均是失败。
3. `make fuzz-smoke` 以固定 nightly、`-runs=1024` 和 `-seed=1` 运行两个 target。`seed_corpus/` 是可审阅起点；不断增长的 `corpus/` 和 `artifacts/` 不纳入版本控制。
4. Linux workflow 仅定时或手动运行 smoke，上传失败 artifacts；既有 PR 的稳定、发布和性能门禁不改变。确认 crash 必须最小化并转为普通确定性 Rust 回归测试。

## 验收

仓库可列出并运行两个 target；种子包含 parser recovery 和 invalid verifier 输入。README、fuzz README、Makefile、CI 与 issue #81 说明固定工具链、可复现命令、artifact 保留和阻断边界。正常 cargo tests、docs、package 与 qbench 门禁不引入 nightly 依赖。
