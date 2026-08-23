# RFC 0066：嵌入执行统计

- 状态：Accepted
- 依赖：RFC 0002、RFC 0045、RFC 0046

## 摘要

嵌入者可通过 `Context::last_execution()` 读取最近一次字节码执行的公开统计：

```rust
let mut context = quickcoffee::Context::new().with_fuel(100);
context.eval("1 + 2")?;
let stats = context.last_execution();
assert!(stats.instructions > 0);
```

`ExecutionStats` 包含 `instructions`（尝试执行的 VM 指令数）、`fuel_remaining`（停止时剩余 fuel）和 `call_depth_peak`（本次达到的嵌套 QuickCoffee 函数调用深度）。成功返回、运行时错误和 RFC 0118 的资源错误都会更新记录；编译或字节码验证错误不会伪造一次执行记录，保留上一条统计。

## 资源与封装边界

计数发生在 VM 指令循环入口，fuel 检查先于计数：fuel 为零时不会虚增指令数。统计只暴露两个整数，不暴露调用帧、环境、指令地址、原型或宿主对象。每次 `run_program`、`run` 或 `eval` 的成功执行/运行时失败都会覆盖上一条记录。

## 验收

核心测试覆盖成功、未知名称运行时错误、fuel 耗尽、编译错误保留旧记录、共享 `Program` 执行与 API 文档；完整基准继续检查语义护栏和统计计数不影响字节码验证。
