# RFC 0086：crate 发布元数据

- 状态：已采纳
- 依赖：RFC 0002、RFC 0085

## 动机

QuickCoffee 既可作为 CLI 使用，也可嵌入其他 Rust 系统。Cargo 发布包若缺少仓库、文档、主页、README 和分类元数据，宿主开发者难以发现 API，且 `cargo package` 会产生质量警告。

## 契约

`Cargo.toml` 必须声明 README、仓库、主页、docs.rs 文档地址、与项目用途相符的关键词和分类，同时保持现有许可证与默认二进制。元数据不参与 QuickCoffee 字节码、运行时或指纹语义；示例、RFC、手册和 CI 仍是包内容的一部分。

## 验收

`cargo metadata --no-deps --format-version 1` 应报告这些字段；`cargo package --allow-dirty --no-verify` 在网络可用时不得因缺失元数据产生警告。核心 `make check` 与嵌入示例验收保持不变。
