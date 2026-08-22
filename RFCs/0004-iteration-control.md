# RFC 0004：数组迭代及循环控制

- 状态：已采纳
- 依赖：RFC 0001、RFC 0002

## 语法与语义

`for pattern[, index_pattern] in iterable then expression` 依序遍历数组的每个值，并在每轮以严格模式绑定值；若写第二个模式，还绑定从零开始的数组下标，例如 `for value, index in items then value + index`。`range(a, b)` 返回数组，因而可直接作为迭代对象。`for [own] key_pattern, value_pattern of map then expression` 依序遍历映射的键和值；`own` 为 CoffeeScript 可读性标记，在没有原型链的 QuickCoffee 中不改变结果。任选的 `when condition` 在绑定后、循环体前求值；仅 Bool `true` 执行循环体，`false` 直接进入下一项。`for` 收集各次体值为新数组；过滤项不收集，break 返回已收集前缀（RFC 0042）。模式规则与逐轮原子性见 RFC 0044。当前版本不遍历字符串，字符串作为迭代对象为运行时错误。

`break` 终止最内层循环，`continue` 跳至最内层循环的下一轮。二者只能在循环体的词法范围内出现；否则是编译错误。循环体仍为一个表达式，可使用 `if … then … else …` 组合控制流。

## 字节码

`IterStartArray` 或 `IterStartMap` 将迭代对象建立为帧私有状态；`IterNext { patterns, end }` 先原子匹配并写入下一值（或键和值），或跳至 `end`；`IterEnd` 用于 `break` 清理最内层迭代状态。数组 `by step` 由 `IterStartArray` 同时接收数组和已求值步长（RFC 0029）。VM 调用帧各自持有迭代器栈，所以递归和嵌套循环不共享状态。

## 验收

测试须覆盖数组、映射和 `range` 迭代、数组与映射的 `when` 过滤、嵌套或条件中的 `break`/`continue`、非数组迭代错误、以及循环外控制语句的编译错误。
