# RFC 0092：公开 Rust API 文档门禁

- 状态：已采纳
- 依赖：RFC 0002、RFC 0043、RFC 0046、RFC 0089

## 动机

QuickCoffee 的 crate 既是 CLI 的实现，也是宿主系统的嵌入 API。若公开的 `Value`、`Context`、
`Program`、`Chunk` 或错误类型没有 rustdoc，docs.rs 不能提供可靠的集成入口；普通编译通过
并不能证明发布 API 可发现、可维护。

## 契约

所有公开类型、字段、枚举变体、方法和顶层函数必须有简短 rustdoc。低层 `Instruction`、
`Pattern`、`Constant` 的变体允许使用统一的枚举级说明，因为它们是已验证字节码的机械标签，
但公开 `Chunk` 字段、宿主值访问器、错误分类和 `Engine`/`Context`/`Program` 操作必须说明
所有权、验证与执行语义。文档不得承诺原型链、JavaScript `undefined` 或隐藏的可变状态。

## 验收

Makefile 的 `api-doc` 使用 `RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc --locked
--no-deps`；CI 的 `make check` 因而把缺失文档视为失败。外部 `tests/embedding_api.rs` 继续
证明文档所描述的公开入口可从 crate 外部调用，其他运行时行为不变。
