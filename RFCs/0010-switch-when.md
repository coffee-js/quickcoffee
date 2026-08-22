# RFC 0010：`switch` / `when` 分支表达式

- 状态：已采纳
- 依赖：RFC 0001、RFC 0002、RFC 0009

`switch subject` 后接缩进的一个或多个 `when pattern`（同一 `when` 可用逗号列出多个模式），可带唯一的 `else`。每个分支体可为单行或缩进块；整体值为所选分支的最后值。模式以 QuickCoffee 严格相等 `==` 与判别式比较。不存在贯穿、隐式类型转换或 JavaScript 对象语义。

编译器只求值一次 `subject`，再以 `Dup`、`Eq` 和跳转依序选择分支。无匹配且没有 `else` 时结果为 `nil`。
