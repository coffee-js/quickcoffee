# RFC 0030：否定成员关系 `not in` / `not of`

- 状态：已采纳
- 依赖：RFC 0001、RFC 0016

## 语法与语义

`value not in array` 是 `not (value in array)` 的中缀写法；`key not of map` 是 `not (key of map)` 的中缀写法。二者优先级与 `in` / `of` 相同，返回 Bool，不进行 JavaScript 式类型转换。

`not in` 的右值必须是数组，比较沿用 QuickCoffee 的严格同型相等。`not of` 的左值必须为字符串、右值必须为映射，只查映射自身键。映射没有原型链，因此没有继承键、`constructor` 等隐式例外。`not value in array` 不作为此 RFC 的替代拼写；需要清晰的否定成员关系时使用中缀 `value not in array`。

## 编译与验证

编译器重用 `Contains` / `HasKey` 后接 `Not`，不新增 VM 值类型或隐藏对象行为。验证器已有这三个指令的栈规则，故编译结果仍可由同一控制流验证器检查。

## 验收

测试覆盖数组和映射的真/假结果、错误右操作数、以及已验证 chunk 中的成员与取反指令。
