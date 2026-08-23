# RFC 0118：嵌入执行资源治理

- 状态：已采纳
- 依赖：RFC 0002、RFC 0066、RFC 0084、RFC 0089、RFC 0114

## 动机

fuel 能限制无限循环，却不能明确区分资源耗尽与普通运行时错误，也不能约束递归函数。嵌入宿主还需要从另一线程或请求生命周期中取消尚未结束的脚本。因此执行上下文必须提供小而稳定的资源治理接口，同时不向脚本暴露 VM 帧、线程、对象地址或 JavaScript 运行时模型。

## 契约

1. `ErrorKind` 增加 `Resource`；`Error::resource_limit()` 返回 `Some(ResourceLimit::{Fuel, CallDepth, Cancellation})`，其他错误返回 `None`。原生回调继续以 `Error::runtime` 创建 `Runtime` 错误。
2. fuel 耗尽是 `ResourceLimit::Fuel`，消息仍包含 `execution fuel exhausted`。资源错误不进入 QuickCoffee 的 `try`/`catch`，因此脚本不能吞掉取消、fuel 或深度限制后继续执行。
3. `Context` 默认允许最多 1,024 层嵌套 QuickCoffee 字节码函数调用；顶层程序不计入层数。`with_max_call_depth`、`set_max_call_depth` 与 `max_call_depth` 配置每次后续运行；零只允许顶层代码，拒绝任何字节码函数调用。原生 Rust 回调不新增 QuickCoffee 调用帧。
4. `CancellationToken` 是可克隆、一次性的宿主取消信号。`Context::with_cancellation_token` 或 `set_cancellation_token` 配置它，`clear_cancellation_token` 移除它。VM 在每条指令之前检查取消；已开始执行的同步原生回调不能被强行中断，宿主回调应自行遵守自己的取消策略。
5. `ExecutionStats` 增加 `call_depth_peak`，记录本次运行达到的最大嵌套 QuickCoffee 函数深度。编译/验证失败仍保留上一条统计；资源错误和普通运行时错误均写入本次统计。

## 验收

`tests/embedding_api.rs` 必须覆盖 fuel、递归深度、预取消 token、替换 token、资源错误不可被 `catch` 吞掉及深度峰值；CLI JSON 必须输出 `kind:"resource"` 的 fuel 错误；嵌入示例、中文语法索引与中文可执行手册说明该 API。完整 `make check` 必须通过。
