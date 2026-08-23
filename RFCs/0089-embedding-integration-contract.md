# RFC 0089：外部嵌入 API 集成验收

- 状态：已采纳
- 依赖：RFC 0002、RFC 0043、RFC 0046、RFC 0069、RFC 0084、RFC 0085

## 动机

QuickCoffee 的嵌入 API 面向其他 Rust crate，而不是只面向自身模块。仅在 `tests/rfc_core.rs`
中验证实现内部细节，不能证明公开导出、类型访问器和错误边界能以真实外部依赖方式组合。

## 契约

`tests/embedding_api.rs` 必须只通过 `quickcoffee` crate 的公开导出测试以下宿主流程：

1. `Engine::compile_program` 产生的 `Program` 可克隆，并由 `Context::run_program` 在宿主注入的
   全局与 native 函数下重复执行；
2. `Value::string`、`Value::array`、`Value::map` 和 `as_*` 访问器保持结构化、不可变边界；
3. native 错误通过 `ErrorKind::Runtime` 与 `message()` 传回，不要求宿主解析展示文本；
4. `with_fuel` 与 `last_execution` 能限制不受信脚本并报告确定的执行统计。

测试不得引用 `src` 私有模块、内部环境或 VM 帧；公开 API 改动若破坏该契约，必须在同一 PR
更新设计文档与示例。

## 验收

`cargo test --locked`（包含 integration test）、`cargo clippy --locked --all-targets -- -D warnings`
与 `cargo doc --locked --no-deps` 均应通过。现有 `examples/embed.rs` 继续作为最小可运行示例，
本 RFC 则提供跨模块、可回归的公开表面证据。
