# RFC 0146：确定性的托管分配遥测

- 状态：已采纳
- 日期：2026-08-27
- 依赖：RFC 0067、RFC 0122、RFC 0123、RFC 0135、RFC 0136、RFC 0137

## 动机

RFC 0123 的 `value_allocations` 与 `environment_allocations` 是为了比较优化前后的稳定事件计数，并非完整的语言值模型：它不覆盖精确 Integer/Decimal 算术，也不能表达对象 payload 大小。直接把这两个旧字段解释成内存占用会破坏历史 qbench 可比性，并受分配器、指针宽度和容量策略影响。

本 RFC 新增两个独立、可累加的 `ExecutionStats` 字段：

- `managed_objects_allocated`：一次执行中创建的逻辑托管对象数；
- `managed_bytes_allocated`：这些对象及其后续字段增长所分配的规范化逻辑 payload bytes。

旧字段及其所有既有计数点保持原义和原值。新字段不表示 Rust allocator 调用、RSS、capacity、当前存活量、峰值存活量或可用于强制终止的硬限制。

## 规范化成本模型

成本只依赖语言值，不依赖宿主平台；累计使用饱和 `u64`。对象计数与 payload bytes 分开，因此不为对象头、`Rc` 头或 allocator 元数据臆测字节数。

| 逻辑对象 | objects | logical payload bytes |
|---|---:|---:|
| Integer | 1 | 系数 magnitude 的 `ceil(bits / 8)` |
| Decimal | 1 | 系数 magnitude bytes + 4-byte scale |
| String | 1 | UTF-8 byte 长度 |
| Array | 1 | 每个元素 8 bytes |
| Map | 1 | 每项 16 bytes，加 key 的 UTF-8 bytes |
| Error | 1 | code + message 的 UTF-8 bytes，加 data/cause 两个 8-byte 引用槽 |
| Function | 1 | 一个 8-byte 捕获/dispatch 引用槽 |
| Class | 1 | class name、superclass/constructor 两个 8-byte 引用槽，以及每个方法 16 bytes 加方法名 bytes |
| Instance | 1 | 一个 8-byte class 引用槽，以及构造时已有的每个字段 16 bytes 加字段名 bytes |
| lexical environment | 1 | 0 bytes；首个切片只稳定记录逻辑帧，不暴露优化相关槽位布局 |
| Nil / Bool / Number | 0 | 0 |

Instance 或 Class 首次写入一个字段时追加 `16 + field-name UTF-8 bytes`，覆盖既有字段不重复追加。这里记录的是语言层 payload 增长，不是 BTreeMap 的实际节点分配。

## 浅层、深层与共享

计量对象必须确实由当前执行创建。普通容器构造、拼接、排序、切片和写时复制只计新容器本身；其元素若只是克隆已有 `Rc`，不得重复计量。`keys`、`split`、字符串迭代、Integer range 与 `parse_json` 会物化完整的新值图，因此递归计入新建的子值。Map key 是 Map payload 的内嵌字符串，不另计 String 对象。

精确算术的每个新 Integer/Decimal 结果均计量；返回既有引用的转换或查询不计量。标准库的 typed allocation profile 可以同时保持 RFC 0123 的旧事件数并提交本 RFC 的对象/byte delta。宿主通过 `add_native` 返回的 opaque 值继续排除，因为 VM 无法观察回调内部哪些引用是新建、缓存或共享的。

函数调用无论使用物理 `Environment`、快帧私有槽位还是完全复用捕获环境，都稳定计一个 logical lexical environment；这延续 RFC 0122/0123 的优化无关口径。编译期常量、共享 `Program`、debug/source map、binding plan、builtin/global 环境、VM scratch buffer、Rust 临时集合和宿主回调内部对象均排除。

## 错误、嵌套执行与模块

成功、可捕获 Runtime error、未捕获 Runtime error 与 Resource error 都保留错误发生前已提交的 delta。把非 Error 值 `throw` 为结构化 ScriptError，以及 catch 物化的 Error 值，分别计量；回滚名称环境不会回滚已经发生的托管分配。

默认参数的嵌套 VM 执行继续累计到外层 run。`Context::last_execution()` 每次运行重新开始计数，Context 复用不得泄漏上次 delta。静态模块图像既有 profile 字段一样逐模块累计两个新字段。

## CLI 与 benchmark

`qcoffee --stats` 追加 `managed_objects_allocated` 与 `managed_bytes_allocated`。`qbench` 的 `quickcoffee.qbench.v1` JSON/text 记录追加 `profile_managed_objects_allocated` 与 `profile_managed_bytes_allocated`；它们仍来自一次不计时 profile 执行，不乘以 `iterations` 或 `repeat`。这是 v1 的 additive extension，既有字段名和含义不变。

## 验收边界

测试覆盖容器、精确数值、builtins、Error、Class/Instance、字段增长、函数与词法帧、默认参数嵌套执行、模式回滚、失败执行、Context 复用、宿主回调排除、模块累计，以及 qcoffee/qbench 两种输出。固定 workload 的旧 RFC 0123 counters 必须保持不变。

保留量、峰值和硬内存上限需要 cycle-safe graph census 和明确的生命周期采样点，属于后续 RFC，不得由本遥测字段推断。
