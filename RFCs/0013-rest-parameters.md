# RFC 0013：函数 rest/splat 参数

- 状态：已采纳
- 依赖：RFC 0001、RFC 0002

函数参数表中的最后一个 `name...` 是 rest 参数，例如 `(head, tail...) -> expression`。调用至少必须传入固定参数个数；多余实参按顺序作为新的 QuickCoffee 数组绑定到 rest 名称。固定参数函数仍要求严格的相等元数。

该特性在函数模板中保存可选 rest 名称，并在创建调用帧时构造数组；没有 JavaScript `arguments` 对象、隐式 this 或动态调用器访问。
