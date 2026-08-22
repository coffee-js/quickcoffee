# RFC 0016：集合与映射成员表达式

- 状态：已采纳
- 依赖：RFC 0001、RFC 0002、RFC 0006

`value in array` 以 QuickCoffee 的同型值相等检查数组成员，并返回 Bool。`key of map` 只检查映射自身的字符串键，并返回 Bool。二者都是表达式；右操作数先由左到右求值，类型不匹配为运行时错误。

`of` 不查询 JavaScript 原型链，因为 QuickCoffee 没有原型链。它只接受字符串键和 `Map`，因此 `constructor of map` 一类 JavaScript 特殊键没有隐式含义。编译器分别发出 `Contains` 与 `HasKey` 指令，不依赖可覆盖的标准库函数。

## 验收

测试须覆盖数组成员命中/未命中、映射自身键命中/未命中，以及右操作数或键类型错误。
