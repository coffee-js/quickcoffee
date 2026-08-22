# RFC 0038：后缀存在性测试

- 状态：已采纳
- 依赖：RFC 0001、RFC 0002、RFC 0015

## 语法与语义

`value?` 产生布尔值：仅当 `value` 为 `nil` 时为 `false`，所有其他 QuickCoffee 值（包括 `false`、零和空数组）均为 `true`。名称仍须已绑定或由宿主注册；本语言没有 JavaScript `undefined`，故不存在未声明名称的隐式存在性检查。

该后缀仅在 `?` 后不是表达式开头时识别，因此与 nil 回退 `left ? right` 及 nil 安全调用、索引、成员和切片后缀不冲突。它可位于逻辑与比较运算之前，例如 `value? and enabled`。

## 编译与验证

解析器产生 `Expr::Exists`；编译为一元 `Exists` 指令。验证器要求一个栈值并保持栈深，VM 只做 `nil` 判定，不做真值转换或宿主访问。

## 验收

测试覆盖 nil、false、数值、逻辑组合、与 nil 回退的不歧义行为，以及已验证的反汇编 chunk。
