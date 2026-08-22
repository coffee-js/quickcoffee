# RFC 0026：严格解构形参

- 状态：已采纳

## 决议

函数与无原型工厂类的普通形参可使用 RFC 0023 的数组/映射模式：`([left, right], {factor}) -> expression`、`class Point([x, y]) -> expression`；数组模式最后一项可按 RFC 0071 写作 `tail...`。调用时每个实参与对应模式严格匹配；数组长度不足、映射缺键或容器类型不符会使调用失败。

形参模式的绑定只写入新调用帧的词法环境，永不修改调用者。`_` 可在任何深度忽略值。默认值仅可用于命名形参，如 `factor = 2`；普通参数 rest 仍必须为最终的单个名称 `tail...`，数组模式 rest 见 RFC 0071。模式默认、映射 rest 与函数参数解构中的 computed/string 键不支持。

## 字节码与 API

函数常量保存 `Vec<Pattern>` 而非单纯名称，`FunctionKind::Bytecode` 在创建调用帧前以同一递归模式验证器收集绑定。`Pattern` 是公开字节码 API 的一部分，因此嵌入方构造或审查 `Constant::Function` 时可见完整形状；VM 验证、fuel 和闭包语义不变。

## 验证

`tests/rfc_core.rs` 覆盖函数与工厂类、映射嵌套数组、数组模式 rest、忽略位、形状失败、被拒绝的模式默认、函数常量的 Pattern 元数据与 chunk 验证。
