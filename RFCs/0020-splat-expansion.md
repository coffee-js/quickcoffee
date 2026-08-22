# RFC 0020：数组与调用 splat 展开

- 状态：已采纳
- 依赖：RFC 0001、RFC 0002、RFC 0013

数组项和调用实参尾部的 `...` 展开一个数组：`[first, values..., last]` 将数组元素拼入新数组，`function(prefix, values...)` 将数组元素作为逐个实参传入。它与函数定义中最后一个 `name...` rest 参数互补；二者都使用后缀标记，但一个展开、一个收集。

展开目标必须为 Array；其他值为运行时错误。无 splat 的数组与调用保留原有 `MakeArray`/`Call` 快路径；含 splat 时编译器把普通项包装为一项数组段，发出 `MergeArrays`，调用再发出 `CallSpread`。没有 JavaScript `apply`、可变参数对象、原型方法或隐式转换。

## 验收

测试须覆盖混合数组展开、向闭包调用展开、多段展开、非数组错误，以及已编译 Chunk 中的 `MergeArrays` 和 `CallSpread`。
