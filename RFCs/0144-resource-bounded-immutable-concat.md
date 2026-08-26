# RFC 0144：资源有界的不可变连接

- 状态：已采纳
- 日期：2026-08-26
- 依赖：RFC 0001、RFC 0118、RFC 0139、RFC 0140

## 动机

QuickCoffee 的推导、展开和 `join` 已能表达部分集合与文本构造，但把两个运行期 Array 或 String 连接为新值仍然冗长，或必须经过不等价的文本转换。业务脚本需要一个严格、确定、无 callback 且不暴露可变容器的最小操作；RFC 0118 的通用 String/Array 上限已经为输出增长建立了 Context 策略。

## 契约

1. 标准库增加普通纯函数 `concat(left, right)`，且只接受恰好两个实参。String + String 返回按原 UTF-8 内容连接的新 String；Array + Array 返回保持左右顺序的新 Array。空值合法，输入与结果不共享可变容器状态。
2. 两个实参必须同为 String 或同为 Array。混合类型、Map、其他值或错误参数数目产生可捕获的 Runtime error；不做字符串化、数值转换、prototype dispatch、locale 查询或脚本 callback。
3. 实现先用 checked arithmetic 计算结果 UTF-8 byte 数或 Array item 数，再进行任何输出 buffer 分配。整数溢出及超过 `max_string_bytes` / `max_array_items` 分别产生不可捕获的 `StringBytes` / `ArrayItems` 资源错误。
4. Array 连接还在复制元素前，以左右输入项数总和检查 `max_collection_operation_items`；超限为不可捕获的 `CollectionOperationItems`。输入项依既有不可变 `Value` 语义浅克隆，嵌套 String/Array/Map 和精确数值仍由当前 Context 的通用值策略复核。
5. 资源失败不修改任一输入、不返回部分结果，也不能由脚本 `catch` 吞掉。成功 String 记录一个托管值分配事件；成功 Array 按既有集合 builtin 口径记录结果 Array 及其每个输出项。

## 非目标

本 RFC 不增加可变 `push`、Map 更新、可变 buffer、prototype 方法、任意参数连接或 callback 集合操作。Map 的动态不可变更新、字符串 replace 及 callback continuation 继续由 issue #124 的后续切片设计。

## 验收

核心测试覆盖 String/Array、空值、Unicode、顺序、输入不变、严格类型/参数和分配统计；嵌入测试覆盖 String/Array/collection-operation 三种边界、源码标签、不可捕获性、失败原子性及恢复。英文与中文可执行手册、语法索引、qbench 和 cargo bench 均包含该函数。
