# RFC 0122：VM 执行热点计数

- 状态：已采纳
- 依赖：RFC 0066、RFC 0118

`ExecutionStats` 在既有指令、fuel 与调用深度之外，增加 `name_loads`、`name_stores`、`calls`、`container_ops`、`iterator_ops` 与 `exception_ops`。这些是实际尝试执行的字节码类别计数，成功和运行时失败都会保留；它们不声称是耗时或堆分配字节数。

`qcoffee --stats` 将这些字段写入 stderr，保持程序 stdout 不变。计数为后续局部槽位、容器优化与紧凑字节码提供可复现的热点证据；fuel 规则、字节码验证与语言语义不变。
