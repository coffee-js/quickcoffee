# RFC 0036：后置 `while` / `until` 循环

- 状态：已采纳
- 依赖：RFC 0001、RFC 0004、RFC 0008、RFC 0012

## 语法与语义

在语句位置，`body while condition` 等价于 `while condition then body`；`body until condition` 等价于 `until condition then body`。条件在每次体执行前按既有 while 规则求值，结果为 `nil`，并沿用 `break`、`continue`、fuel 和缩进规则。

后置循环特别保留赋值体：`n = n + 1 while n < 3` 重复执行整个赋值，而不是先求一次右值后将循环结果 `nil` 写入 `n`。严格数组/映射解构同样可作体，例如 `[a, b] = [a + 1, b + 1] while a < 2`。为保持单遍解析清晰，本版本只在语句位置接受后置循环，不能嵌入任意子表达式。

## 编译与验证

解析器将普通表达式、名称赋值或解构赋值统一 lowering 为既有 `Expr::While`，并以 `Expr::Destructure` 表示需要重复的解构体。编译器为该表达式发出既有 `Store` / `Destructure` 和 while 跳转；VM 与验证器无需新指令或状态。

## 验收

测试覆盖后置 while、until、严格解构、已验证 chunk；性能基准单独覆盖重复赋值路径。
