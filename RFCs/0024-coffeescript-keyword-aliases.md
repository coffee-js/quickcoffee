# RFC 0024：CoffeeScript 布尔与相等别名

## 决议

`yes` 与 `on` 是 `true` 的词法别名；`no` 与 `off` 是 `false` 的词法别名。`is` 与 `isnt` 分别是 `==` 与 `!=` 的词法别名。它们是保留字，不能用作变量名。

别名在词法阶段归一化为现有 token，随后使用既有 `Bool` 常量和 `Eq`/`Ne` 字节码。因而 `is` 仍是 QuickCoffee 的同型严格相等：`42 isnt '42'` 为真，绝不发生 JavaScript 式类型转换。

## 验证

`tests/rfc_core.rs` 覆盖每个布尔别名、`is`/`isnt`、控制流使用、保留字赋值错误，以及由别名编译所得 chunk 的验证与反汇编。
