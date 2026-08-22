# RFC 0018：映射标识符简写

- 状态：已采纳
- 依赖：RFC 0001、RFC 0002、RFC 0006

映射字面量中的标识符可省略重复值：`{name}` 等价于 `{name: name}`，`{name, answer}` 等价于 `{name: name, answer: answer}`。它只适用于标识符键；字符串键仍必须显式写成 `{'name': value}`，从而保持解析无歧义。

简写在编译期仍是普通 `Load` 加 `MakeMap`，不会引入 JavaScript 的对象展开、原型、getter 或隐式接收者。

## 验收

测试须覆盖单键、多键、显式键值并存，以及无值字符串键的语法错误。
