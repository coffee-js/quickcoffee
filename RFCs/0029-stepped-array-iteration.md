# RFC 0029：数组 `for … by step` 步长迭代

- 状态：已采纳
- 依赖：RFC 0002、RFC 0004

## 语法与语义

数组循环可在可迭代表达式后、可选 `when` 前写 `by step`：`for item in items by 2 then body`。`step` 在建立迭代器时恰好求值一次，必须产生正的有限整数；否则为运行时错误。省略时步长为 `1`。该特性适用于数组及 `range` 结果；映射循环 `for own key, value of map` 不接受 `by`，是解析错误。

步长按数组下标前进：`[1..9] by 3` 依次绑定 `1`、`4`、`7`。过滤、`break`、`continue`、函数 return、fuel 与嵌套循环沿用 RFC 0004 和 RFC 0028 的规则。

## 字节码与验证

`IterStartEnumerable` 消耗栈顶的步长和其下的数组或字符串，建立帧私有迭代器；数组按步长前进，字符串仅接受步长 1（字符串语义详见 RFC 0070）。`IterStartMap` 仍只消耗映射。验证器相应要求 enumerable 开始指令有两个栈值，并继续追踪迭代器在 `IterNext`、`IterEnd` 与 `Return` 的平衡。VM 以饱和加法更新数组位置，防止极大合法步长造成整数回绕。

## 验收

测试覆盖普通步长、一次求值、零/小数/非数字拒绝、映射拒绝和已编译 chunk 验证。
