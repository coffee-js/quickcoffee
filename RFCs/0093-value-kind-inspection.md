# RFC 0093：宿主值类型标签

- 状态：已采纳
- 依赖：RFC 0002、RFC 0041、RFC 0089、RFC 0092

## 动机

嵌入方经常需要在调用 `Value` 访问器前判断值类型。逐个尝试 `as_number`、`as_array` 等
方法既冗长，也会诱使宿主直接匹配公开 enum 的内部 `Rc` 容器。需要一个不泄漏存储实现、
可在 match 中使用的稳定类型标签。

## 契约

公开 `ValueKind::{Nil, Bool, Number, String, Array, Map, Function}`，并提供：

- `Value::kind()`：只读返回对应标签，不执行脚本、不克隆容器；
- `Value::is_nil()`：仅对 `nil` 返回 true，false/0/空容器均返回 false。

标签与 QuickCoffee 运行时类型一一对应；新增类型必须显式扩展 `ValueKind`。现有 `as_*`
访问器、构造器、Display 语义和字节码指纹不变。

## 验收

`tests/embedding_api.rs` 从 crate 外部检查全部相关标签与 `nil` 行为；严格 rustdoc 门禁必须
包含新类型和方法。五语嵌入说明可继续使用 `Value::kind()` 做宿主分流，`make check` 保持
通过。
