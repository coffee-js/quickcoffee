# RFC 0143：可复现 fuzz smoke 基线

- 状态：已采纳
- 日期：2026-08-26
- 依赖：RFC 0034、RFC 0048、RFC 0087、RFC 0110

## 动机

常规 Rust 测试覆盖已知语义与回归，但 lexer/parser 恢复、公开 `Chunk::verify` 和成功编译后的 VM 执行都必须能承受未知字节输入。fuzzing 需要与稳定发布工具链隔离，且不能把无限时间的随机探索伪装成每个 PR 的阻断门禁。

## 决策

1. `fuzz/` 是独立 cargo-fuzz package，不纳入发布 crate 或 Rust 1.85/stable workspace 检查；它只使用固定的 `nightly-2026-08-20`。旧 `nightly-2025-03-28` 无法再从 cargo-fuzz 0.13.2 的发布锁构建其 Rust 1.91+ 依赖，因此固定工具链与固定 cargo-fuzz 必须作为一组验证和升级。
2. `parser` target 将任意 bytes 作为 lossy UTF-8 交给 `Engine::check_program`；`verifier` target 将 bytes 映射为公开指令的 Chunk，再调用 `Chunk::verify`。`vm` target 只接受严格 UTF-8，并在输入 16 KiB、fuel、调用深度、一般值、JSON、数值与 retained-state 边界内执行成功编译的源码。正常语言、资源或编译错误允许，panic/abort 均是失败；三个 target 都不获得宿主 capability。
3. `make fuzz-smoke` 以固定 nightly、`-runs=1024`、`-seed=1` 和 `-max_len=16384` 运行三个 target。每个运行把忽略的可写 `corpus/TARGET` 作为首 corpus、把受版本控制的 `seed_corpus/TARGET` 作为只读起点，避免 smoke 修改权威 seed；不断增长的 `corpus/` 和 `artifacts/` 不纳入版本控制。
4. Linux workflow 仅定时或手动运行 smoke，上传失败 artifacts；同一 workflow 使用固定 nightly 的 Miri component，在保留 isolation 的前提下解释执行适用的 library tests。VM 的 `Rc` 循环和 thread-local builtin 环境有意允许进程期保留，因此 Miri 使用 `-Zmiri-ignore-leaks`；专门验证大规模 JSON 嵌套/尺寸上界的 stress 由普通测试覆盖，不进入有界 Miri smoke。其他 library tests 不整体跳过。
5. 独立 dependency-audit workflow 使用固定 `cargo-audit 0.22.2` 审计根与 fuzz 两个 lockfile，在 manifest/lockfile 变更、每周计划和手动触发时运行。安全 advisory 阻断，仓库不配置静默 ignore；该 workflow 不自动修改依赖。
6. 既有 PR 的 Rust 1.85/stable、发布和性能门禁不引入 nightly。确认 fuzz crash 或 Miri 缺陷必须最小化并转为普通确定性 Rust 回归测试。

## 验收

仓库可列出并运行 parser、verifier、vm 三个 target；种子包含 parser recovery、invalid verifier，以及函数/闭包、迭代/容器、class/error 和 JSON 执行输入。根与 fuzz lockfile 通过 RustSec 审计，适用 library tests 通过 pinned-nightly Miri。README、fuzz README、Makefile、CI 与 issue #81 说明固定工具链、可复现命令、artifact 保留和阻断边界。正常 cargo tests、docs、package 与 qbench 门禁不引入 nightly 依赖。
