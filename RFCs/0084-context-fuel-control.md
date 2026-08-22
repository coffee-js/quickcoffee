# RFC 0084：嵌入上下文的可变 fuel 预算

- 状态：已采纳
- 依赖：RFC 0002、RFC 0066

## 动机

嵌入宿主通常复用一个 `Context` 以保留全局值和原生函数，但不同请求需要不同执行预算。只有构造器式 `with_fuel` 会迫使宿主重建上下文，容易误丢状态。

## API 契约

`Context::set_fuel(u64)` 修改下一次及后续 `eval`、`run`、`run_program` 使用的每轮指令预算；`Context::fuel()` 返回当前配置值。`Context::with_fuel` 保留并委托相同语义，便于链式构造。调整预算不清空全局环境、原生函数、共享 `Program` 或 `last_execution()`；一次运行仍从完整配置值开始扣减，耗尽仍返回 Runtime 错误。

## 验收

嵌入测试必须验证低预算运行失败、随后提高预算可以成功、此前设置的全局值仍可读取，并验证 getter 与 builder 语义一致。API 不暴露 VM 帧或内部环境。
