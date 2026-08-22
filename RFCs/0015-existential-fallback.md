# RFC 0015：nil 专属回退运算符

- 状态：已采纳
- 依赖：RFC 0001、RFC 0002

QuickCoffee 以 `nil` 为唯一空值，因此采用 CoffeeScript 风格的二元 `?` 作为空值回退：`left ? right` 先且只先求值 `left`；若它为 `nil`，再求值并返回 `right`，否则直接返回 `left`。`false`、`0`、空字符串和空容器都不是空值，不触发回退。

本 RFC 初版只采纳二元形式；后缀存在性测试 `value?`、安全导航与 `?=` 分别由 RFC 0038、RFC 0022 与 RFC 0039 后续定义，JavaScript 的 `undefined` 仍不在范围内。编译器发出 `JumpIfNil`，使非空左值跳过右表达式，而不依赖真值转换、原型机制或 JavaScript 空值语义。

## 验收

测试须验证 `nil` 的回退、`false` 与 `0` 的保留、右操作数短路，以及右操作数表达式的正常优先级。
