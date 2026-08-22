# RFC 0041：嵌入方全局读取

- 状态：已采纳
- 依赖：RFC 0002

`Context::get_global(name)` 返回 `Option<Value>`，供宿主读取宿主注入或脚本产生的全局绑定；未知名称返回 `None`。结果是公开 `Value` 的克隆，绝不暴露环境、调用帧或引用计数实现，且不触发脚本执行。该 API 与 `set_global`、`add_native`、`eval`、`run` 组成最小且直接的嵌入表面。

测试验证宿主数组/映射的读取与缺失名称；VM 的 fuel、验证与词法作用域语义不变。
