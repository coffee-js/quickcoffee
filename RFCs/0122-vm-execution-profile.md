# RFC 0122：VM 执行热点计数

- 状态：已采纳
- 依赖：RFC 0066、RFC 0118

`ExecutionStats` 在既有指令、fuel 与调用深度之外，增加 `name_loads`、`name_stores`、`calls`、`container_ops`、`iterator_ops` 与 `exception_ops`。这些是实际尝试执行的字节码类别计数，成功和运行时失败都会保留；它们不声称是耗时或堆分配字节数。

`qcoffee --stats` 将这些字段写入 stderr，保持程序 stdout 不变。计数为后续局部槽位、容器优化与紧凑字节码提供可复现的热点证据；fuel 规则、字节码验证与语言语义不变。

issue #95 的当前环境槽位提示不改变分类口径：无论名称通过缓存槽位、当前环境索引还是父环境查找完成，每条实际尝试的 `Load` / `LoadOrNil` / `Store` 仍只计一次。缓存失配与回退不是额外字节码步骤，也不额外消耗 fuel。

issue #100 的编译器解析叶函数槽位沿用同一口径。槽位化的参数与局部读写仍按原字节码计入 `name_loads` / `name_stores`；为保持跨版本可比性，逻辑函数帧仍计一次 `environment_allocations`，即使该隔离帧的局部值不再写入名称环境。该字段因此是稳定的 VM 分配事件模型，不等同于底层分配器调用次数。
