# Changelog / 更新日志

QuickCoffee 的用户可见变化记录在此。版本标题与 `Cargo.toml` 以及 `vX.Y.Z` release tag 保持一致。

User-visible QuickCoffee changes are recorded here. Version headings stay aligned with `Cargo.toml` and `vX.Y.Z` release tags.

## [0.1.0]

### 中文

- 初始实验版本：CoffeeScript 风格的严格、无原型字节码语言与可嵌入 Rust API。
- 提供 `qcoffee`、`qtest`、`qdocco`、`qbench` 与 `qcson`，支持规范 `.coffee`、GitHub-compatible `.litcoffee` 源码和纯数据 `.cson`。
- 公开资源有界的 JSON/CSON 纯内存 Rust API；`qcson` 在不执行输入的前提下双向转换 canonical CSON/JSON，并提供版本化机器诊断。
- Decimal 定价场景可由人工维护的 `.cson` 配置驱动；Rust 宿主和发布归档中的 `qcson -> qcoffee` 链路产生相同结果，脚本不获得隐式文件权限。
- `qtest --module-root ROOT ENTRY...` 可预检并隔离运行导出 `test = true` 的静态模块用例。
- 提供共享 `.litcoffee` 规则的 Decimal 定价与确定性 JSON 规范化业务工作流。
- 多文件嵌入式策略包验证隔离 Context、typed host state、显式 capability、取消与资源策略。
- 新增由三个业务工作流校准的 `ExecutionPolicy::isolated_request()`，统一 Runtime 编译边界与新 Context 的默认执行边界。
- `qcoffee` / `qtest` 人类错误显示完整 ranges、源码片段与可操作 hints；`qcoffee --json` 错误附带 version 1 完整 labels。
- 包含精确数值、不可变集合、受限 class、静态模块图、结构化诊断与显式资源治理。
- 为 Linux x86_64、macOS Intel、macOS Apple silicon 与 Windows x86_64 生成可校验归档；归档携带 Decimal `.litcoffee`/`.coffee` 场景，并从干净解包目录验证全部用户工作流。

### English

- Initial experimental release of a strict, prototype-free CoffeeScript-inspired bytecode language and embeddable Rust API.
- Ships `qcoffee`, `qtest`, `qdocco`, `qbench`, and `qcson` with canonical `.coffee`, GitHub-compatible `.litcoffee`, and data-only `.cson` support.
- Exposes resource-bounded, in-memory JSON/CSON Rust APIs. `qcson` converts canonical CSON/JSON bidirectionally without executing input and provides versioned machine diagnostics.
- The Decimal pricing scenario accepts a human-maintained `.cson` configuration. The Rust host and archived `qcson -> qcoffee` chain agree without granting ambient file access to the script.
- `qtest --module-root ROOT ENTRY...` preflights and runs isolated static-module cases that export `test = true`.
- Includes Decimal pricing and deterministic JSON-normalization workflows backed by shared `.litcoffee` rules.
- A multi-file embedded policy package validates isolated Contexts, typed host state, explicit capabilities, cancellation, and resource policy.
- Adds `ExecutionPolicy::isolated_request()`, calibrated by all three business workflows, to align Runtime compilation bounds with defaults inherited by new Contexts.
- Human `qcoffee` / `qtest` errors render complete ranges, source excerpts, and actionable hints; `qcoffee --json` errors include complete version 1 labels.
- Includes exact numeric values, immutable collections, restricted classes, static module graphs, structured diagnostics, and explicit resource governance.
- Produces verified archives for Linux x86_64, macOS Intel, macOS Apple silicon, and Windows x86_64. Each archive carries the Decimal `.litcoffee`/`.coffee` workflow and exercises the complete user path from a clean extracted directory.
