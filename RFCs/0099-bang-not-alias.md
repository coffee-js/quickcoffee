# RFC 0099：前缀 `!` 的严格否定别名

- 状态：已采纳
- 依赖：RFC 0024、RFC 0057

## 动机

CoffeeScript 将文字 `not` 定义为前缀 `!` 的可读别名。QuickCoffee 已有严格 Bool 的 `not` 运算，但单字符形式被词法器拒绝；这使同一语言契约不完整。

## 契约

单独的 `!expression` 与 `not expression` 完全等价，均要求操作数为 Bool，并产生 Bool 的相反值：

```coffee
[!true, !false, not true, not false]
# => [false, true, false, true]
```

`!=` 仍是严格不等比较，不被拆成 `!` 与 `=`。QuickCoffee 不采用 JavaScript 的真值转换，因此 `!0`、`!''` 和 `!1` 都是运行时类型错误；`!!value` 只适用于 Bool。别名不改变字节码、fuel、嵌入 API 或原型无关值模型。

## 验收

词法测试覆盖 `!` 与 `!=` 的区分；核心测试覆盖真/假、双重否定、非 Bool 错误及已验证 `Not` 字节码。五语手册和中英文语法索引同步说明该别名。
