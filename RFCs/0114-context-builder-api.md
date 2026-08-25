# RFC 0114：链式嵌入上下文配置

- 状态：已采纳
- 依赖：RFC 0041、RFC 0043、RFC 0084、RFC 0085、RFC 0089

## 动机

宿主可以用 `Context::set_global` 与 `Context::add_native` 配置全局值和回调，但每次配置都需要可变借用。嵌入示例和小型宿主程序更适合声明式、可链式的初始化，同时不能破坏已有的可变 API。

## 契约

1. `Context::with_global(name, value)` 消费并返回上下文，语义等同于先调用 `set_global`。
2. `Context::with_native(name, callback)` 消费并返回上下文，语义等同于先调用 `add_native`；回调仍返回结构化 `Result<Value, Error>`。
3. 两个 builder 方法可与 `with_fuel`、RFC 0118 的 `with_max_call_depth`、`with_resource_limits` 和 `with_cancellation_token` 任意顺序链式组合；全局值、原生函数和资源边界的后续执行语义保持明确且彼此独立。
4. 既有 `set_global`、`add_native`、`fuel` 和 `run_program` API 保持兼容；不暴露环境、原型链或 JavaScript 对象。

## 验收

`tests/embedding_api.rs` 必须以链式配置执行共享 Program 并读取宿主全局；`examples/embed.rs` 使用链式 API；`make check` 必须继续通过完整 debug/release、文档和打包门禁。
