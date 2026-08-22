# RFC 0022：nil 安全访问与调用

- 状态：已采纳

## 决议

QuickCoffee 增加 CoffeeScript 风格的 soak 后缀：`value?.name`、`value?[key]` 与 `function?(arguments)`。它们只保护接收者为 `nil` 的情形：此时结果为 `nil`，索引表达式或调用实参完全不求值。接收者非 `nil` 时，操作与普通 `.name`、`[key]` 或 `()` 相同。

这不是 JavaScript 的 `undefined`、可选原型查找或隐式容错。譬如非空映射的缺失成员、错误索引、非函数调用仍为运行时错误；`record?.missing` 因此与 `record.missing` 一样报错。若需为 `nil` 结果提供值，可与 `left ? right` 组合。

## 字节码

编译器先求值接收者并 `Dup`，以 `JumpIfNil` 跳过后续成员、索引或实参求值；nil 分支弹出副本并保留原始 `nil`，非 nil 分支弹出副本后发出已有的 `Member`、`Index`、`Call` 或 `CallSpread`。无需新增 VM 指令，故保持已验证字节码和显式调用帧模型。

## 验证

`tests/rfc_core.rs` 覆盖 nil 与非 nil 成员/索引、splat 调用、接收者 nil 时宿主回调不被求值、非 nil 严格错误，以及生成 chunk 的 `JumpIfNil` 与验证器检查。
