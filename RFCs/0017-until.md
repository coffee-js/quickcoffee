# RFC 0017：`until` 循环

- 状态：已采纳
- 依赖：RFC 0001、RFC 0002、RFC 0009

`until condition then body` 是 `while not condition then body` 的语法形式。条件仍必须产出 Bool，循环值仍为 `nil`，`break`、`continue`、fuel 与缩进块语义均与 `while` 相同。

解析器将其降为带 `Not` 的既有 `While` AST，故编译器与 VM 不需要新的循环指令或隐含控制状态。

## 验收

测试须覆盖单行 `until` 递增到停止条件，并间接确认它遵守既有循环的 fuel 和 Bool 条件规则。
