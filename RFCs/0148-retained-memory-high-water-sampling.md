# RFC 0148：显式保留托管图高水位采样

- 状态：已采纳
- 日期：2026-08-27
- 依赖：RFC 0118、RFC 0146、RFC 0147

## 动机

RFC 0147 能读取某一时刻由 Context global 可达的 retained managed-memory 图，但不会保存历史。直接在 VM 每条指令、每次 Store 或每次执行结束时执行完整图普查，既会把 O(graph) 工作引入调度热路径，也仍不能准确表示所有栈上临时值的 live-memory peak。

嵌入方仍需要一个可重复、低侵入的观测高水位：例如在一次业务请求、顶层脚本执行或明确 root 更新之后记录 retained graph。这个记录必须明确是 host-selected samples，而不是自动的 RSS、逐指令峰值或可执行的硬限制。

## API 与采样点

`Context::sample_retained_memory()` 是唯一的非创建采样点。它返回与 `retained_memory()` 相同的当前快照，并更新 `Context::retained_memory_high_water()`。新建 Context 将其空 writable global environment 作为第一个样本，因此初始 high water 是 `{ objects: 1, bytes: 0 }`。

high water 分别记录 objects 和 bytes 的 Context-lifetime 最大已采样值；两个字段可以来自不同样本，因而它不是某一单一时刻的 `RetainedMemory` 图。该 Context 没有 reset API，重建 Context 才会开始新的观测生命周期。

设置 global、执行成功或失败、模块执行、VM 指令与宿主 callback 不会自动采样。宿主应在符合其业务语义的稳定边界显式调用该方法；例如在 `eval`/`run_program` 返回后，或用 `set_global` 安装/替换其可达根之后。

## 边界

每个样本仍遵循 RFC 0147：从 Context owned writable global root 出发，以 Rc identity 去重，跳过共享 builtin parent、宿主 callback 内部、host-only Value 与未存回 Context 的 module exports。RFC 0146 的累计 allocation delta 仍是独立指标。

该 API 不代表 allocator 调用、capacity、RSS、全进程总量、跨 Context 总量、逐指令 live-memory peak 或强制内存限额。显式样本之间创建又丢弃的对象，或在同一顶层执行中短暂可达后移除的图，不会自动出现在 high water。

未来 hard limit 必须另行规定执行期间的检查点、pre-mutation atomicity、资源错误、回滚、Context/module 生命周期与 host-visible state；不得把该稀疏观测记录当作限制器。

## 验收

嵌入测试覆盖初始样本、host global 根、显式样本、root 变小后的单调记录、未采样的临时图、成功与失败执行后的选择性观测，以及 alias/cycle 的 RFC 0147 单位继承。debug/release 重复读取一致；qbench 保持不增加 VM dispatch/instruction census。README、双语语法索引、嵌入示例与 RFC 0118 说明 sample/high-water 与 retained snapshot、allocation telemetry、live peak 和 hard limit 的区别。
