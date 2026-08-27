# RFC 0147：Context 保留托管内存普查

- 状态：已采纳
- 日期：2026-08-27
- 依赖：RFC 0118、RFC 0123、RFC 0146

## 动机

RFC 0146 的 `managed_objects_allocated` 与 `managed_bytes_allocated` 是一次执行中的累计分配 delta；它们故意不表示当前仍可从 Context 访问的值。把累计分配直接用作 retained-memory 限制会把已丢弃的临时值、回滚路径和宿主保留值混为一谈。

在任何 peak 或 hard limit 之前，嵌入方需要一个小而平台无关的、可重复读取的 Context 保留图快照。它必须能处理共享 `Rc` backing 与闭包/环境/class 的环，且不能把进程共享 builtin 或宿主 callback 内存归入某个 Context。

## API 与根

`Context::retained_memory()` 返回 `RetainedMemory { objects, bytes }`。它是读取型快照，不执行脚本、不改变 `ExecutionStats`、fuel、错误、字节码或 Context 全局状态。

根是 Context 自己的可写 global environment。进程共享的标准库 parent environment 不进入图；因此每个空 Context 只保留自己的一个逻辑 lexical environment。模块执行返回的 `ModuleExports` 属于调用宿主；只有宿主随后把导出值存入 Context global 时，它才成为该 Context 的保留图一部分。宿主手中但未存入 Context 的 `Value` 同样不计入。

## 普查模型

`objects` 与 `bytes` 使用 RFC 0146 的规范化逻辑单位：Integer coefficient magnitude bytes、Decimal magnitude 加四字节 scale、String UTF-8 bytes、Array 每元素八字节、Map/字段每项十六字节加 key bytes、Error code/message 加两个引用槽、Function 八字节、Class/Instance 的引用槽和已存在 method/field payload。每个 lexical environment 是一个 0-byte logical object。

遍历的边包括 Array/Map child values、Error data/cause、Class superclass/constructor/method/static-field values、Instance class/field values、Bytecode Function capture environment、receiver-bound and bound-method receiver/function/class links，以及 Environment 已初始化 slots 与其 parent。普通 Number、Bool 和 nil 不产生对象。原生 callback 的语言 Function wrapper 可以计入，但 callback 闭包内部、allocator headers、capacity、RSS、Program/Chunk/debug/source-map/binding-plan、VM scratch buffer 与共享 builtin internals 均排除。

每种 Rc-backed logical node 以其 allocation identity 去重；首次遇到才累计单位并继续遍历。这样别名只计一次，函数捕获 Environment、Environment 又绑定函数的循环能终止，Class/Instance/field 互相引用也不会重复。

## 边界与后续

该快照不代表 allocator 调用、当前进程 RSS、宿主对象、弱引用、collectable garbage 或跨 Context 的总量。它也不保存历史 peak，不会在执行过程中强制停止脚本。下一阶段可以在这一稳定根/身份/单位模型上定义采样点、per-run/per-Context peak 与 hard retained-memory limits；这些限额仍须独立规定原子性、失败后的 host-visible state 和生命周期策略。

## 验收

嵌入测试必须覆盖空 Context、共享 alias、替换 root 后移除、闭包/Environment cycle、Class/Instance retained graph、Context 隔离和模块导出在宿主显式保留前后的边界。debug/release 重复读取必须一致；既有 RFC 0123/0146 profile 字段保持不变。README、双语语法索引、嵌入示例与 RFC 0118 说明该快照与累计分配遥测、资源限制之间的区别。
