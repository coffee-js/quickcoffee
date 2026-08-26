# RFC 0068：`qtest --stats` 测试执行统计

- 状态：已接受
- 依赖：RFC 0003、RFC 0066

## 摘要

`qtest` 增加可选的 `--stats` 开关，为每个成功或失败的 `.coffee` 或 `.litcoffee` 文件将 RFC0066 的执行统计写到标准错误。默认不启用时，既有 stdout、stderr 和退出码保持不变。

## 契约

```text
qtest --stats tests/scripts/arithmetic.coffee
ok tests/scripts/arithmetic.coffee
qtest stats: tests/scripts/arithmetic.coffee instructions=N fuel_remaining=M
```

统计行格式固定为：

```text
qtest stats: PATH instructions=N fuel_remaining=M
```

`PATH` 是测试文件显示路径，`N` 与 `M` 来自该文件独立 `Context` 的 `last_execution()`。成功、运行时错误和 fuel 耗尽都会给出统计；文件已读入但编译失败且尚未执行时，统计为初始值 `0/0`；读取文件失败没有可执行上下文，因而不产生统计行。错误报告仍按原有格式输出。

每个文件继续使用独立上下文与 fuel 预算，目录遍历顺序、最终值必须为 `true` 的规则和失败退出码不变。`--stats` 只影响 stderr，便于保留机器可读的 `ok` 输出。

## 验收

- 默认 `qtest` 输出与此前完全一致。
- 成功、运行时错误和 fuel 耗尽测试均覆盖统计行。
- 统计不改变测试执行结果、遍历顺序或退出码。
- `make check` 与 `make docs` 通过。
