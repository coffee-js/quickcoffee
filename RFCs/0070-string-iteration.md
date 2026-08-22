# RFC 0070：字符串 `for` 迭代

- 状态：已接受
- 依赖：RFC 0004、RFC 0042、RFC 0044

## 摘要

`for value in string then expression` 按 Unicode 标量值顺序遍历字符串。每个绑定值都是只含一个 Unicode 标量的 QuickCoffee 字符串；字符串中的 UTF-8 多字节序列不得被拆成宿主字节。

## 语法与语义

字符串迭代复用数组 `for` 的收集、`when`、`break`、`continue`、严格递归模式和可选第二绑定规则：

```coffee
for character, index in 'a☕中' then index
# => [0, 1, 2]
```

第二绑定是从零开始的 Unicode 标量下标，而不是 UTF-8 字节偏移。每轮模式匹配成功后才写入绑定；字符串为空时产生空数组。后置推导和语句位置的丢弃循环同样适用。

`by` 只属于数组迭代。由于迭代对象可在运行时求值，`for value in dynamic by step` 在运行时遇到字符串时报告错误，而不是静默改变步长语义。映射仍使用 `of`，不受本 RFC 影响。

非数组、非字符串的 `in` 迭代对象仍是运行时错误；字符串的 `of` 迭代仍是映射类型错误。该功能不暴露 JavaScript 的 UTF-16 code unit、迭代器对象或原型链。

## 字节码与验收

编译器发出统一的 `IterStartEnumerable`，消耗迭代对象与步长；VM 根据运行时值选择数组或 Unicode 字符串迭代。验证器按原数组迭代路径检查两个栈值与一个迭代器状态，`IterNext` 的模式数量和控制流规则不变。

验收至少包括 ASCII 与非 ASCII 标量、动态字符串、可选下标、过滤、空字符串、`by` 错误、非字符串/数组错误、嵌套控制流，以及生成字节码的验证。
