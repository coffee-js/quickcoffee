# QuickCoffee 用户手册（简体中文）

QuickCoffee 是一个 Rust 字节码引擎，不是 JavaScript 运行时。源码被解析、编译、验证后才执行；不存在原型链、`this`、`eval` 或内嵌 JavaScript。

`#` 开始行注释。非嵌套块注释用 `### … ###` 包围，内容在布局与解析前被忽略；未闭合会报告词法错误。

标识符使用 Unicode XID 规则：首字符为 XID start 或 `_`，后续可为 XID continue 或 `_`，因此组合附标可出现在名称中；引擎不做 Unicode 规范化。

普通字符串支持常用控制字符转义以及 `\\xNN`、`\\uNNNN`、`\\u{...}` Unicode 转义；非法转义和非 Unicode 标量值会报告解析错误。

可使用 CoffeeScript 风格别名而不改变运行时类型：`yes`/`on` 等同 `true`，`no`/`off` 等同 `false`，`is`/`isnt` 等同严格的 `==`/`!=`。

相邻的严格或数值比较可写成链：`1 < middle() < 3` 只会计算一次 `middle()`；较早比较为假时，不会计算后续操作数。

## 起步

```sh
qcoffee -e "print(range(1, 4))"
qcoffee --fuel 10000 program.qc
qcoffee --check program.qc
qcoffee --dump-bytecode program.qc
```

`qcoffee -` 从标准输入读取源码，便于接入管道；`qcoffee --check FILE`（FILE 可为 `-`）只解析、编译和验证而不执行；`qcoffee --dump-bytecode -` 则反汇编标准输入而不执行。
`qcoffee --stats` 将指令数与剩余 fuel 写入标准错误，同时保持程序标准输出不变；不可与 `--check` 或 `--dump-bytecode` 合用。

`qcoffee --interactive`（或 `-i`）在输入行之间保持同一 Context；`:help` 列出命令，`:quit`/`:exit` 退出会话。管道输入不输出提示符。
交互模式加 `--stats` 时，仅实际执行或运行时失败的非空输入行把指令数与剩余 fuel 写入标准错误；解析、验证错误不生成新记录。
`for character, index in 'a☕中' then index` 得到 `[0, 1, 2]`；字符串按 Unicode 标量遍历，不接受 `by`。

`--` 之后的参数以普通字符串数组 `argv` 提供：`qcoffee program.qc -- first second` 中 `len(argv)` 为 `2`。引擎不会暴露宿主进程或环境对象。

`--fuel` 是每次执行的指令上限，耗尽会安全失败。标准库包括 `print`、`len`、`type`、`range`、`str`、`keys`、`values`、`join`、`split` 与 `assert`；`range(a, b)` 生成 `[a, b)`。

## 语法示例

```coffee
factor = 6
double = (x) -> x * 2
if double(factor) == 12 then print('ok') else print('bad')
```

函数捕获创建处的词法环境。普通名称形参可省略括号，如 `left, right -> left + right`；默认值、rest 和解构形参仍必须使用括号。尾部形参可设默认值，如 `(head, separator = '-') -> expression`；缺省或显式传入 `nil` 时，默认表达式在被调函数内计算，因此可引用较早形参与闭包变量。必选形参必须在默认形参之前。末位可变参数写作 `(head, tail...) -> expression`，其余实参会以数组绑定给 `tail`。映射键使用字符串索引：`{name: 'coffee'}['name']`。

`return expression` 只可用于函数体，立即结束当前函数；裸 `return` 返回 `nil`。它不会跨越嵌套函数。位于循环时会清理循环状态；穿过 `try` 或 `catch` 时会由内向外执行 `finally`。`finally` 中的 return 会覆盖先前的返回值。条件返回请写作 `if condition then return value`。

形参也可使用严格递归模式：`([left, right], {factor}) -> (left + right) * factor`。函数开始前，每个实参必须匹配对应模式；默认值仍只允许命名形参，rest 仍只能是末位名称。

映射字面量中的 `{name}` 是 `{name: name}` 的简写；字符串键仍必须显式给出值，如 `{'name': value}`。

赋值模式可嵌套数组与映射：`[first, {point: [x, y]}] = [1, {point: [20, 22]}]`。数组每层都要求长度精确相同，映射要求列出的标识符键存在。VM 会先验证整个模式再写任何绑定，故深层不匹配同样是原子的。

数组项或调用实参末尾的 `...` 会展开数组：`[1, values..., 4]` 拼入其元素，`fn(values...)` 将它们逐个传参。展开目标必须是数组，不会调用 JavaScript 风格的 `apply` 方法。

nil 安全后缀采用 CoffeeScript 风格写法：`record?.name`、`values?[index]` 与 `fn?(args)`。接收者为 `nil` 时结果为 `nil`，索引或实参也不会求值；接收者非 `nil` 时沿用普通访问的严格规则，因此映射缺键仍会报错。

数组循环写作 `for item in range(1, 4) then print(item)`；可在数组之后写 `by step`，如 `for item in [1..9] by 3 then print(item)`。第二个绑定可取得从零开始的实际下标，如 `for item, index in items then item + index`，步进时仍取数组位置。绑定位置可用严格递归模式，例如 `for [left, right] in pairs then left + right` 或 `for {point: {x, y}} in values then x + y`；每个项的全部绑定只会在模式完整匹配后写入。`for` 是收集表达式：每次循环体值组成新数组，`when` 拒绝的项不收集，`break` 返回已收集前缀。步长只求值一次，且必须是正的有限整数。`break` 和 `continue` 控制最内层循环；`while`/`until`/`loop` 的结果仍为 `nil`；映射循环不支持 `by`。

同一收集器也支持 CoffeeScript 风格后置推导：`value * 2 for value in items`，或写作 `[value * 2 for value in items]`。方括号只是推导界标，不产生额外嵌套数组；`by`、`when`、映射、模式、`break`、`continue` 仍遵循前置形式。

整数区间字面量由 VM 的专用字节码直接构造：`[1..3]` 包含上界，结果为 `[1, 2, 3]`；`[1...3]` 不包含上界，结果为 `[1, 2]`。边界必须是有限整数。

数组切片写作 `items[start..end]`（含末端）或 `items[start...end]`（不含末端），例如 `[0..4][1..3]` 为 `[1, 2, 3]`。端点从左到右各求值一次，必须是界内有限整数；负数从末尾计，`-1` 是最后一项。切片只用于数组且不隐式截断。nil 安全形式 `items?[start..end]` 在接收者为 `nil` 时不求端点而产生 `nil`。

`left ? right` 是仅针对 `nil` 的回退：只有左值为 `nil` 才会求值右侧。它不会把 `false`、`0`、空字符串或空容器视为空值。

后缀 `value?` 只检测值是否不是 `nil`：`nil?` 为 `false`，而 `false?`、`0?` 都为 `true`。它不隐藏未绑定名称错误，也不同于 `left ? right` 的回退。

`name ?= value` 仅在名称还未绑定或当前为 `nil` 时求值并写入 `value`；已有非 nil 值时右侧不会执行。名称还支持严格算术复合赋值，如 `total += amount`、`power **= 2`。这些形式只适用于名称，不能用于成员、索引或解构；普通未绑定名称读取仍是错误。

名称还支持严格数值前后置更新：`next = ++counter` 产生新值，`previous = counter--` 先产生旧值再减一。更新只接受名称，成员、索引和解构形式均拒绝。

CoffeeScript 算术还提供整除 `a // b` 与向下取模 `a %% b`；例如 `-7 // 5` 为 `-2`，`-7 %% 5` 为 `3`。普通 `%` 仍是随被除数取符号的余数。

位运算采用严格有符号 32 位数：`&`、`|`、`^`、`~`、`<<`、`>>`、`>>>`；移位计数限于 0 至 31，复合形式只接受名称。

物理行末的显式运算符可使表达式续至下一行；续行期间的缩进只作排版，不改变布局块。

普通引号字符串可以跨行；换行合为一个空格，行末反斜杠则去除换行。

诸如 `(1 + 2 * 3) == 7` 的纯字面量算术会在编译时折叠为经验证的常量。

`value in array` 按 QuickCoffee 相等性检查数组成员，`value not in array` 取其相反值。`key of map` 只检查映射自身的字符串键，`key not of map` 取其相反值；映射没有可查询的原型键。

`until condition then body` 是反向循环形式：它重复执行直到布尔条件为真，`break`、`continue`、缩进和 fuel 规则均与 `while` 相同。

语句位置还可写后置循环：`n = n + 1 while n < 3` 与前置 while 等价，重复整个赋值；`until` 同理。严格解构也可作为体。后置循环不能嵌入普通子表达式。

`loop body` 是无限的 `while true` 形式；使用 `break` 退出，仍受 fuel 限制。例如 `n = 0; loop then if n == 3 then break else n = n + 1`。

在 `for` 的可迭代对象与 `then` 之间放置 `when condition` 可过滤循环，不为被拒绝的绑定执行循环体：`for n in [1..5] when n > 2 then print(n)`。

无原型的数据工厂写作 `class Point(x, y = 0) -> {x: x, y: y}`，其默认参数规则与函数相同；调用后得到普通映射，可用 `Point(3).x` 读取成员。没有 `this`、`new` 或继承。

双引号可插入 QuickCoffee 表达式：`"答案 #{double(21)}"`。单引号没有插值；其中不会运行任何 JavaScript。

多分支表达式使用 `switch value` 与缩进的 `when pattern`；只会选择一个严格相等分支，且没有贯穿。

异常使用 `try`、`catch error`、可选 `finally` 与 `throw value`。catch 得到稳定的错误字符串，而非 JavaScript Error 对象；函数 return 也会经过适用的 finally。

## 嵌入 Rust

```rust
let mut cx = quickcoffee::Context::new().with_fuel(100_000);
cx.set_global(
    "host_values",
    quickcoffee::Value::array(vec![
        quickcoffee::Value::from(40_i64),
        quickcoffee::Value::from(2_i64),
    ]),
);
let value = cx.eval("host_values[0] + host_values[1]")?;
```

`Value::from`、`Value::string`、`Value::array` 与 `Value::map` 可以直接构造宿主值，无需接触 VM 的引用计数内部表示。宿主回调可返回 `Error::runtime("message")`，脚本可用 `catch` 捕获。

若需重复执行，使用 `Engine::compile_program` 编译并验证一次，再将共享 `Program` 传给 `run_program`；克隆该句柄不会复制字节码或重复验证。

`Context::last_execution()` 可读最近一次成功或运行时失败的 `ExecutionStats`，其中有执行指令数 `instructions` 与余下燃料 `fuel_remaining`；编译或验证错误不会改写上一条记录。

`cx.get_global("host_values")` 可在不执行脚本的情况下读取脚本或宿主设置的全局值；未知名称返回 `None`。它只返回公开 `Value` 的副本，不泄漏环境或调用帧。

嵌入错误具有结构：`error.kind()` 返回 `ErrorKind::Parse`、`ErrorKind::Verify` 或 `ErrorKind::Runtime`，`error.message()` 返回详情，`error.position()` 可返回从 1 开始的源码行号。宿主无需解析展示文本；`Display` 输出仍适合 CLI 与 QuickCoffee 的 `catch` 错误字符串。

## 文档与测试

`qdocco demo.qc -o demo.html` 生成并校验可执行文档；用 `qdocco --check demo.qc` 只校验。`qtest cases` 会递归运行目录中的 `.qc` 文件，要求每个脚本的最后值严格为 `true`。

`qtest --fuel N cases` 会为每个发现的测试文件分别设置指令预算，因此一个受限循环不会耗尽其他测试的预算。
`qtest --stats` 还会把每个文件的指令数与剩余 fuel 写入标准错误，不改变 `ok` 输出。

多行数组和映射可按行省略逗号；调用参数与普通括号内表达式仍须显式分隔。

单独赋值行（`record =`）后可缩进书写映射；嵌套的 `key: value` 条目会成为无原型映射，普通赋值续行不受影响。

同一逻辑行的调用可省略括号：`implicit_answer = implicit_add 20, 22`；比较或跨布局边界时仍可使用显式括号。
