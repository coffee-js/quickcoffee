# RFC 0067：`qcoffee --stats` 执行统计

- 状态：已接受
- 依赖：RFC 0002、RFC 0019、RFC 0066

## 摘要

`qcoffee` 增加可选的 `--stats` 开关，将本次执行的公开统计写到标准错误；程序值仍只写标准输出，故可安全用于管道。

## 契约

```text
qcoffee --stats -e '1 + 2'
3
qcoffee stats: instructions=N fuel_remaining=M
```

统计行格式固定为：

```text
qcoffee stats: instructions=N fuel_remaining=M
```

其中 `N` 与 `M` 直接来自 RFC 0066 的 `Context::last_execution()`。成功、运行时错误和 fuel 耗尽均输出统计；错误消息仍按原有 CLI 契约输出，退出码不变。`instructions` 是已尝试的指令数，`fuel_remaining` 是停止时剩余 fuel。

`--stats` 只适用于实际执行模式（`-e SOURCE`、FILE 或 `-`）。与 `--check`、`--dump-bytecode` 同时使用是参数错误，退出码为 `2`；这两种模式不执行字节码，因而没有本次执行统计。

## 验收

- 普通程序 stdout 与未传 `--stats` 时完全相同。
- 统计只写 stderr，且成功和失败路径均覆盖。
- fuel 耗尽时统计中的 `instructions` 不超过传入 fuel，剩余 fuel 为零。
- CLI 测试覆盖成功、错误、fuel 耗尽及互斥参数；`make check` 与 `make docs` 通过。
