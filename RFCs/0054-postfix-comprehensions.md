# RFC 0054：后置列表推导

- 状态：已采纳
- 依赖：RFC 0004、RFC 0029、RFC 0042、RFC 0044、RFC 0052

## 语法

已有的前置形式
`for pattern in items [by step] [when condition] then expression` 保持不变；
现在表达式也可置于循环体前，写成
`expression for pattern in items [by step] [when condition]`。
映射形式同样支持 `expression for own key_pattern, value_pattern of map`。

例如：

```coffee
double = n * 2 for n in [1..3]
evens = [n for n in [1..5] when n % 2 == 0]
labels = key + value for own key, value of {a: 1, b: 2}
```

方括号包住一个完整后置推导时只是 CoffeeScript 风格的列表推导界标，
不会再增加一层数组；不允许在该界标中追加逗号项。

## 语义与实现

后置形式与相应前置 `for` 完全相同：每轮体值收集到新数组，严格递归模式逐轮原子绑定，
`when`、`by`、`break`、`continue` 和数组实际下标规则不变。它在 AST 中直接降低为既有
`Expr::For`，因此继续使用 `IterNext`、验证器和字节码收集路径，不引入新的 VM 状态。
`for` 后缀只在完整表达式层启用；成员、索引、解构及映射的既有严格限制仍然有效。

验收覆盖数组、方括号界标、步进下标、过滤、映射、递归模式、字节码验证以及混合逗号的拒绝。
