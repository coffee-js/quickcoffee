# RFC 0110：Rust 最低版本契约

- 状态：已采纳
- 依赖：RFC 0000、RFC 0086、RFC 0087

## 契约

QuickCoffee 使用 Edition 2024，并将 Rust `1.85.0` 声明为 crate 的最低支持版本（MSRV）。`Cargo.toml` 的 `rust-version` 字段是发布元数据的一部分；新增依赖或语言特性不得无意中提高该版本。

## 验收

CI 必须对 Rust `1.85.0` 和当前 stable 各运行一次 `make docs && make check`。两套工具链都必须通过格式、全部测试、示例、crate 打包、release qbench、Clippy、rustdoc 及五份手册门禁。集成测试还必须确认 manifest 暴露 `rust-version = "1.85"`，避免文档与发布元数据分离。

MSRV 是编译器兼容性下限，不承诺旧平台的运行时性能；性能数据仍按 RFC 0096 与 RFC 0109 的 release 基准口径解释。
