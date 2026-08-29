# Changelog / 更新日志

QuickCoffee 的用户可见变化记录在此。版本标题与 `Cargo.toml` 以及 `vX.Y.Z` release tag 保持一致。

User-visible QuickCoffee changes are recorded here. Version headings stay aligned with `Cargo.toml` and `vX.Y.Z` release tags.

## [0.1.0]

### 中文

- 初始实验版本：CoffeeScript 风格的严格、无原型字节码语言与可嵌入 Rust API。
- 提供 `qcoffee`、`qtest`、`qdocco` 与 `qbench`，支持规范 `.coffee` 和 GitHub-compatible `.litcoffee` 源码。
- `qtest --module-root ROOT ENTRY...` 可预检并隔离运行导出 `test = true` 的静态模块用例。
- 包含精确数值、不可变集合、受限 class、静态模块图、结构化诊断与显式资源治理。

### English

- Initial experimental release of a strict, prototype-free CoffeeScript-inspired bytecode language and embeddable Rust API.
- Ships `qcoffee`, `qtest`, `qdocco`, and `qbench` with canonical `.coffee` and GitHub-compatible `.litcoffee` sources.
- `qtest --module-root ROOT ENTRY...` preflights and runs isolated static-module cases that export `test = true`.
- Includes exact numeric values, immutable collections, restricted classes, static module graphs, structured diagnostics, and explicit resource governance.
