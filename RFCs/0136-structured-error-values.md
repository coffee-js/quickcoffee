# RFC 0136：结构化 Error 值

- 状态：已采纳
- 日期：2026-08-25
- 依赖：RFC 0011、RFC 0043、RFC 0049、RFC 0118、RFC 0131
- 跟踪：issue #126、issue #133

## 动机与边界

业务脚本不能依赖解析 `runtime error: ...` 文本来区分校验、领域或宿主错误。QuickCoffee 因此提供密封的 `Error` 值，但不采用 JavaScript Error 对象、可变原型、公开堆栈或可伪造源码位置。fuel、调用深度、取消及未来内存限制继续是不可捕获的 Resource 错误。

## 值与构造

`error(code, message[, data[, cause]])` 产生 Error。code 是非空、最多 64 字节的 ASCII 小写标识，由字母开头，后续可含数字、点、下划线与连字符；message 必须是 String。data 默认为 nil，可由 nil、Bool、有限 Number、Integer、String、Array、Map 递归组成，不接受 Function 或 Error。cause 默认为 nil，否则必须是 Error。

Error 是不可变密封值。`type(value)` 返回 `"error"`；成员读取只允许 `.code`、`.message`、`.data`、`.cause`，未知成员报错。相等比较递归比较四个公开字段。普通显示为 `error(code): message`；CLI 值 JSON 使用带 `$quickcoffee: "error"` 的规范标签，并递归编码 data/cause。

## 传播、捕获与宿主

`catch` 名称现在总是绑定 Error，而非 RFC 0011 的兼容字符串。普通 VM 或 `Error::runtime` 失败映射为 code `runtime`、原始 message、nil data/cause。`throw` Error 保留其字段；throw 其他值保持既有未捕获文本 `thrown: value`，捕获时成为 code `throw`、message 为同一 `thrown: value` 文本、data 为原值。Rust 宿主可用 `Error::domain(code, message, data)` 创建同一领域错误，并用 `script_error()` 读取公开字段。

源码 primary/secondary labels 与调用上下文保存在不可见的可信元数据中。catch 不暴露它们；重新 throw 同一 Error 时继续携带原始可信 labels，脚本构造的 Error 没有可信位置。通用 Runtime 的未捕获 Display 保持 `runtime error: message`；领域错误稳定显示为 `runtime error [code]: message`。

RFC 0011、0043、0049、0131 中“catch 得到字符串”的部分由本 RFC 取代。finally、处理器展开、字节码、fuel、统计和资源错误规则不变。普通脚本 JSON 解析/编码的 Error 适配仍由 issue #125 定义。
