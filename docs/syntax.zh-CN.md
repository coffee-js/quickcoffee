# QuickCoffee 0.1 语法范围

嵌入宿主可用 `Program::fingerprint()` 作为确定性字节码缓存键；该指纹不改变验证与执行语义。

内建 `qtest --json` 每个文件输出一行稳定 JSON，供 CI 与宿主系统使用；`qtest --tap` 输出确定性的 TAP 13 记录；`qcoffee --fingerprint FILE` 在不执行脚本时输出已验证字节码的稳定 16 位十六进制键，指纹使用规范化编码而非 Rust 调试文本；`qbench --json` 输出带语义护栏的编译、验证、执行计时记录；`qdocco --markdown` 生成说明、围栏源码和最终值供审阅；嵌入方可用 `Context::set_fuel` 调整复用上下文的预算并用 `Context::fuel` 读取，`cargo run --example embed` 提供可编译宿主示例；`--stats` 的执行统计仍写入标准错误。

这是 RFC 0001 的中文索引；未列出的 CoffeeScript 2016 特性不是“隐式兼容”，而是明确不支持。

| 类别 | 支持 | 不支持（本版） |
|---|---|---|
| 字面量 | 十进制、十六进制 `0xff`、二进制 `0b1010`、八进制 `0o755` 与科学计数法数字、字符串、双引号 `#{expr}` 插值、保留换行的 `"""…"""` 插值 heredoc 与 `'''…'''` 字面 heredoc、`true`/`yes`/`on`、`false`/`no`/`off`、`nil`、数组与 `[head, items...]` 展开、整数区间 `[1..3]`（含上界）/`[1...3]`（不含上界）、映射、`{name}` 简写与映射展开 `{...base, key: value}` | 正则、JS 插值、`undefined` |
| 运算 | 算术、严格有符号 32 位位运算 `&`、`|`、`^`、`~`、`<<`、`>>`、`>>>` 及其名称复合赋值、名称复合赋值 `name += value`、`-=`, `*=`, `/=`, `%=`, `**=`、名称前后置更新 `++`/`--`、比较（`==`/`is`、`!=`/`isnt`，可短路成链 `a < b < c`）、`and`/`or`、`not`、仅对 `nil` 回退的 `left ? right`、后缀非 nil 测试 `value?`、仅名称的存在性赋值 `name ?= value`、数组成员 `value in array` / `value not in array`、映射自身键 `key of map` / `key not of map`、数组索引与严格切片 `a[start..end]` / `a[start...end]`、映射成员访问、nil 安全后缀 `a?.name`、`a?[i]`、`a?[start..end]`、`f?(args)` | 成员/索引/解构复合赋值、成员/索引/解构 `?=`、字符串/映射切片、隐式截断、未声明名称检查 |
| 控制 | `if`/`unless`、后置条件、`while … then …`、`until … then …`、语句后置 `body while/until condition`、前置或后置列表推导 `for value[, index] in xs [by step] [when condition] then …` / `value for value in xs`、`switch`/`when`、`try`/`catch`/`finally`、`throw`、函数内 `return`、`break`、`continue` | JS Error 对象、顶层 `return`、映射 `for` 的 `by` |
| 赋值/函数 | `a, b = array`、`[a, tail...] = array`（末项 rest 可为空）、`[a, {point: [b, c]}] = array`、`{key, from: to, ...metadata} = map` 的严格递归解构、`_` 忽略位；`x, y -> expression` 无括号名称闭包、`([a, b], {factor}) -> expression` 解构形参、`(x, y = 2, rest...) -> expression`（缺省或 `nil` 取默认）、`f(items...)` 展开调用、`=>` 同义箭头、`do` 立即调用 | 无括号默认/rest/解构参数、生成器 |
| OO/模块 | 无原型工厂类 `class Name(args) -> expression` | 继承、`this`、`new`、import/export |

字符串索引与严格切片按 Unicode 标量边界：`'a☕中'[1]` 为 `'☕'`，`'a☕中'[1..2]` 为 `'☕中'`；负索引从末项计数（`items[-1]`），越界仍报错；字符串 `for` 仍不支持 `by`。

映射字面量可从左至右展开：`{...defaults, theme: 'dark'}`；后写显式键或展开段覆盖先写键，展开值必须为映射。映射解构可用末尾尾部模式 `{id, ...metadata}` 捕获未列键，所得映射为新的不可变值。

普通单引号与双引号字符串支持 `\\0`、`\\b`、`\\f`、`\\n`、`\\r`、`\\t`、`\\v`、引号/反斜杠、两位 `\\xNN`、四位 `\\uNNNN` 及一至六位 `\\u{...}` Unicode 转义。字符串可以跨物理行；普通换行合并为一个空格，字符串内缩进被忽略。行末单个未转义反斜杠会同时去除反斜杠与换行。双引号多行字符串仍支持 `#{expression}` 插值，单引号仍是字面文本；三引号 heredoc 继续保留换行。非法转义或非 Unicode 标量转义是词法错误。

标识符采用 Unicode XID：首字符为 XID start 或 `_`，后续为 XID continue 或 `_`，不做规范化；组合附标可作为续字符。`#` 是行注释；`### … ###` 是不嵌套的块注释，块内容不影响布局。`for value in array` 遍历数组（包括 `range` 的结果）；可写 `by step`，步长只求值一次且必须为正有限整数。`for own key, value of map` 遍历映射且不支持 `by`。`for` 收集每轮体值为新数组；`when` 跳过的项与 `continue` 不收集，`break` 返回既得前缀；`while`、`until` 与 `loop` 的值仍为 `nil`。`return expression` 只可在函数体中使用，立即返回表达式值；裸 `return` 返回 `nil`，并在离开循环或 `try` 时完成清理与 finally。`if`、循环和函数可在换行后用空格缩进多个语句，且缩进必须一致；语句也可用 `;` 分隔。调用可写 `f(a, b)`，亦可在同一逻辑行写 `f a, b`。条件必须是布尔值，名称必须先赋值或由宿主注册。完整规范见 RFC。

隐式调用只消费同一逻辑行的普通表达式：`print value`、`add 20, 22`、`double add 20, 22`、`len [1, 2, 3]` 均可；跨布局边界请使用显式括号调用。

`for` 的绑定可用严格递归模式：`for [left, right] in pairs then left + right`、`for own _, value of record then value` 均可。模式不匹配是运行时错误，且本轮绑定绝不部分写入。

名称亦支持前置、后置数值更新：`next = ++counter` 产生更新后的值，`previous = counter--` 先产生旧值再减一。`++`、`--` 只接受名称并使用严格数值运算；成员、索引和解构形式在解析阶段拒绝。

算术还包括 CoffeeScript 风格整除 `a // b`（取 `floor(a / b)`）与向下取模 `a %% b`（`(a % b + b) % b`）。普通 `%` 仍是随被除数取符号的余数；名称可使用 `//=`、`%%=` 复合赋值。

位运算采用严格有符号 32 位数值模型：`&`、`|`、`^`、前缀 `~`、`<<`、算术右移 `>>` 与逻辑右移 `>>>`。操作数须为有限整数且在 32 位范围内，移位计数须为 `0..31`；`&=`、`|=`、`^=`、`<<=`、`>>=`、`>>>=` 只接受名称。逻辑右移结果仍以有符号 32 位数表示，例如 `-1 >>> 1` 为 `2147483647`；这不是 JavaScript 的隐式转换。

物理行末若留下显式运算符，下一行继续同一表达式，例如 `total = 1 +\n  2 * 3`。赋值、算术、比较、逻辑、成员、位运算、移位和幂运算均可续行；为保持后缀 `value?` 是完整语句，`?` 不作续行符，需要跨行回退时请使用括号。`->`、`=>` 仍开启原有的缩进函数体。续行期间的行首空格只作排版，不改变布局块。

纯字面量算术、比较、集合与插值在编译时折叠为经验证的常量；含动态名称或可能在运行时失败的严格表达式仍交给 VM 执行。多行数组和映射可按物理行省略逗号（`[\n  1\n  2\n]`、`{\n  first: 1\n  second: 2\n}`）；普通括号和调用参数仍须显式分隔。

映射亦可在单独赋值行后用缩进书写：`record =` 下一行的 `first: 1` 等条目会降低为无原型映射；嵌套映射递归处理。普通赋值续行（如 `value =` 下一行的 `1 + 2`）不会误判为映射。

数组遍历可另绑定从零开始的下标：`for value, index in items then value + index`；使用 `by step` 时下标仍是实际数组位置。

推导亦可用 CoffeeScript 风格后置形式：`value * 2 for value in items`，或以方括号包住写作 `[value * 2 for value in items]`。后置形式与前置形式共享 `by`、`when`、映射、模式、`break`、`continue` 语义；方括号只是推导界标，不再增加一层嵌套数组。
