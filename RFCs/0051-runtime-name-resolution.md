# RFC 0051：执行期名称解析与宿主绑定

- 状态：已采纳
- 依赖：RFC 0001、RFC 0002、RFC 0041

编译器把普通名称读取编译为 `Load(name)`，不在没有宿主环境的 `Engine::compile` 阶段假定完整的全局集合。执行时先查当前函数的词法环境，再沿父环境查找，最后查 `Context` 的全局环境；`Context::set_global` 与 `add_native` 注入的名称因此可以被已编译程序使用。

若所有环境都没有该名称，VM 返回 `ErrorKind::Runtime`，详情包含未知名称；这不是 JavaScript 的 `undefined`，也不会静默产生 `nil`。`value?` 仍会先严格读取名称并报告该错误；只有 `name ?= value` 使用 `LoadOrNil`，把未绑定名称视为 `nil` 并按 RFC 0039 的短路规则处理。

该策略保持共享 `Program` 可在多个 Context 中复用，同时允许每个 Context 拥有不同的宿主全局。验收沿用未知名称错误、宿主注入读取、闭包词法捕获和共享程序重复执行测试；字节码验证不接受额外的动态指令或隐藏对象模型。
