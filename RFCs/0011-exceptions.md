# RFC 0011：`try` / `catch` / `finally` 与 `throw`

- 状态：已采纳
- 依赖：RFC 0002、RFC 0009

## 语法与语义

`try body catch error body [finally body]` 捕获其受保护体及其调用函数中产生的 QuickCoffee 运行时错误。`throw expression` 显式产生运行时错误。catch 名称绑定稳定的错误字符串（例如 `runtime error: thrown: bad`）；它不是 JavaScript `Error` 对象。

正常路径与捕获路径都会执行可选 `finally`。finally 本身的错误取代先前结果或错误并向外传播。函数内 `return` 穿过受保护体或 catch 时也会执行 finally；finally 中的 return 覆盖待返回值，详见 RFC 0028。catch 体的错误不会被同一个 catch 再次捕获，但可以由外层 try 捕获。

## VM 与验证

`Try { catch, name }` 记录当前值栈和迭代器栈深度；错误展开到最近处理器时恢复这两个深度、绑定 catch 名称并跳到 catch 目标。`EndTry` 仅在正常路径移除处理器，`Throw` 终止当前普通控制流。字节码验证器同时追踪值栈、迭代器栈和处理器栈，拒绝处理器下溢或 Return 时的处理器泄漏。

## 非目标

没有 JavaScript Error 类型、堆栈对象、`finally` 对取消的特殊语义、`rethrow` 关键字或跨宿主边界的 panic 捕获。原生函数返回的 `Err` 使用同一捕获机制。
