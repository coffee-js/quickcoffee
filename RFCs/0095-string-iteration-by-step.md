# RFC 0095：字符串 Unicode 标量步进迭代

- 状态：已采纳
- 依赖：RFC 0070、RFC 0042、RFC 0044

## 动机

RFC 0070 已定义字符串按 Unicode 标量迭代，但 `by` 仍被拒绝。数组与字符串共享 `for` 收集、过滤、模式绑定和可选下标语义；拒绝字符串步长使同一语法在运行时类型切换时不一致，也迫使文档保留一个不必要的例外。

## 契约

`for value in string by step then body` 按 Unicode 标量位置 `0, step, 2*step, ...` 取值。每个值是单一 Unicode 标量组成的 QuickCoffee 字符串，不暴露 UTF-8 字节或 JavaScript UTF-16 code unit。第二绑定得到实际的标量下标，而非迭代轮数：

```coffee
for character, index in 'a☕中x' by 2 then [character, index]
# => [[a, 0], [中, 2]]
```

步长表达式只求值一次，必须是非零的有限整数；正步长从首标量开始，负步长从末标量开始（RFC 0100）。数组和字符串共享该检查。动态 `in` 迭代对象在运行时决定采用数组或字符串的步进路径；`of` 映射迭代不接受 `by`。空字符串、过滤、后置推导、`break`、`continue` 与严格递归模式保持 RFC 0070 语义。

## 实现与验收

`IterationKind::String` 保存步长，并以饱和加法推进 Unicode 标量位置；字节码指令格式与验证规则不变。验收覆盖 ASCII 与多字节 Unicode、动态字符串、实际标量下标、动态步长、空输入、过滤、嵌套控制流及既有非法步长/映射错误。
