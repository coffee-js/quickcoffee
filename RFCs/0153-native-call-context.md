# RFC 0153：Native-call context 与类型化宿主状态 / Native-call context and typed host state

- 状态：已采纳 / Status: Adopted
- 日期：2026-08-28 / Date: 2026-08-28
- 依赖：RFC 0118、RFC 0146、RFC 0152 / Dependencies: RFC 0118, RFC 0146, RFC 0152

## 中文

既有 `NativeFunction`、`add_native`、`with_native` 与 builder `native` 保持兼容和简单调用路径。显式注册的 contextual native 接收可变 `NativeCallContext` 与 `&[Value]`；调用上下文包含本轮 `ResourceLimits`、fuel remaining、可选 `CancellationToken` 与可选 `HostState` handle，不暴露 VM frame、environment 或 Runtime cache。

回调可调用 `check_cancelled()` 协作停止长任务；同步 Rust 回调仍不能被 VM 强制中断。`consume_fuel(amount)` 从本轮 fuel 确定性扣减，不足时清零并产生不可捕获 `Fuel` 资源错误。已经扣减的 fuel 在成功或失败后都进入 `last_execution()`。`record_managed_allocation(objects, bytes)` 饱和累加 contextual host work 的 logical object/byte 遥测，即使回调随后失败；它不改变兼容字段 `value_allocations`，也不是 transient-memory 硬限制。返回 Value 仍由 VM 按 Context policy 递归复核。

`HostState` 以 `Rc<dyn Any>` 保存一个同线程 `'static` 值。Context builder 与 set/with/clear/get API 明确其所有权；native call 通过类型匹配取得克隆 `Rc<T>`，不匹配返回 `None`。模块子 Context 克隆同一 handle，独立 Context 不自动共享。state 与 callback internals 不进入脚本 Value、globals、JSON、retained-memory census 或 Runtime cache；需要可变状态时宿主显式选择 `Cell`、`RefCell` 等内部可变类型。

本 RFC 不捕获 native panic，不增加 async、跨线程 Context、命名 state table、opaque script handle 或 clock/random/logging/file/network capability。测试覆盖取消、fuel、成功/失败遥测、policy 读取、类型不匹配、替换/清空、Context 隔离、模块继承、源码标签与资源不可捕获；示例、五种手册、中英文索引和完整门禁同步更新。

---

## English

Existing `NativeFunction`, `add_native`, `with_native`, and builder `native` APIs preserve compatibility and their simple call path. An explicitly registered contextual native receives a mutable `NativeCallContext` plus `&[Value]`. The call context contains current `ResourceLimits`, remaining fuel, an optional `CancellationToken`, and an optional `HostState` handle without exposing VM frames, environments, or Runtime caches.

A callback can call `check_cancelled()` to cooperatively stop long work; the VM still cannot forcibly interrupt a synchronous Rust callback. `consume_fuel(amount)` deterministically charges the current run, sets the remainder to zero on insufficiency, and returns an uncatchable `Fuel` resource error. Fuel already charged is retained in `last_execution()` after success or failure. `record_managed_allocation(objects, bytes)` saturating-adds logical object/byte telemetry for contextual host work even when the callback later fails. It does not alter the compatibility `value_allocations` field and is not a transient-memory hard limit. Returned Values remain recursively checked against the Context policy by the VM.

`HostState` stores one same-thread `'static` value as `Rc<dyn Any>`. Context builder and set/with/clear/get APIs make ownership explicit. A native call obtains a cloned `Rc<T>` on a type match and `None` on mismatch. Module child Contexts clone the same handle; independent Contexts do not share automatically. State and callback internals never enter script Values, globals, JSON, retained-memory census, or Runtime caches. Hosts explicitly choose interior-mutability types such as `Cell` or `RefCell` when mutation is required.

This RFC does not catch native panics and adds no async execution, cross-thread Context, named state table, opaque script handle, or clock/random/logging/file/network capability. Tests cover cancellation, fuel, success/error telemetry, policy reads, type mismatch, replacement/clearing, Context isolation, module inheritance, source labels, and uncatchable resources; the example, five manuals, English/Chinese indexes, and complete gates are updated together.
