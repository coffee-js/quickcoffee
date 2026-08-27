# RFC 0149：事务性 Context 保留托管内存限制

- 状态：已采纳
- 日期：2026-08-27
- 依赖：RFC 0118、RFC 0146、RFC 0147、RFC 0148

## 契约

`ResourceLimits` 增加 `max_retained_managed_objects` 与 `max_retained_managed_bytes`。默认均为 `u64::MAX`，表示该可选策略关闭；零是有效边界。超限使用不可捕获的 `ResourceLimit::RetainedManagedObjects` 或 `RetainedManagedBytes`。

这两个值使用 RFC 0147 的 Context writable-global-root logical 对象与 bytes 单位。它们独立于 RFC 0146 累计分配遥测与 RFC 0148 的宿主采样高水位，也不是 allocator/capacity/RSS、逐指令 live peak 或全进程预算。

启用策略的顶层 `eval`、`run`、`run_program` 先普查既有 Context；若已超限，则不执行字节码且保留上一条 `last_execution`。否则运行完成后在提交前再次普查。若最终 retained graph 超限，执行统计仍记录本轮已尝试指令，但 API 返回 Resource error 并恢复运行前状态。

事务快照涵盖从 Context root 可达的 lexical Environment、Class static fields 与 Instance fields。快照持有 Array 的 Rc，因而数组原地 append 走既有 copy-on-write；Map 是不可变值。超限回滚恢复这些可变单元，保证 global、closure、class 与 instance 对宿主和后续执行保持运行前状态。普通 Runtime error 在未触发 retained 限额时保留既有非事务语义。

模块子 Context 继承宿主当前策略。模块返回值在宿主未存回 global 前仍不属于宿主 root；子模块自身执行若将其私有 root 推过边界则失败。`set_global` 维持既有无失败 API：宿主可先存入大值，但下一次受限顶层执行会在 preflight 拒绝。

## 非目标

本 RFC 不限制临时栈/VM scratch/宿主回调/编译/验证/Program 缓存，或在同一轮执行中创建后丢弃的值。它也不提供逐指令强制、GC、跨 Context 总量、线程隔离或 hard RSS 限制；这些需要独立的执行期资源模型。

## 验收

debug/release 测试覆盖对象和字节预检、global/closure/class/instance 回滚、成功后的恢复、模块继承、关闭策略时的旧行为与资源错误不可捕获。qbench 证明默认关闭时不创建事务或执行 retained census；README、双语语法索引与 RFC 0118 说明提交期边界。
