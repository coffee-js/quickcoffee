# QuickCoffee 0.1 语法范围

与 CoffeeScript 1.12.7 官方语言参考的逐章节对照，以及明确的“实现 / 改写 / 拒绝”决策，见 [CoffeeScript 2016 特性矩阵](coffeescript-2016-matrix.md)。

嵌入诊断通过 `Error::labels()` 暴露有序的 primary/secondary 标签。`SourceSpan` 包含可选的不透明来源名、起点与可选的不含终点；列从 1 开始按 Unicode 标量计数。物理行未被预处理改写时，lexer/parser 错误携带精确范围；合成或已改写输入保持行级位置，不虚构列。`compile_named`、`compile_program_named` 与 `Context::eval_named` 会把宿主给出的名称原样保存在已有 label，命名模块和 CLI 文件输入亦同；匿名调用仍无名称。`Error::position()` 继续作为 primary 起点的兼容访问器。

`break`、`continue`、`return` 的 lowering 上下文错误、单文件误用模块指令及重复模块导出会保留触发关键字 span。这些验证 span 不改变公开字节码；预处理改写输入继续降级为可靠行号，而不猜测列。

编译后的 `Program` 以私有 source-map sidecar 保存顶层表达式、嵌套/默认参数 chunk、assignment/destructuring statement、模块、verifier、运行期错误、宿主调用错误与资源停止位置。sidecar 不进入字节码指纹和反汇编，只在编译/VM 错误慢路径查询。运行期错误先保留 primary 失败位置，再按由近及远顺序追加 `called from here` 的 QuickCoffee 调用点 secondary label；宿主构造的裸 `Chunk` 刻意保持无来源。

`Engine::check_program*` 只进行静态编译而不执行，可返回多个彼此独立、可恢复的 parser error。恢复刻意限定在确定性的顶层语句边界；普通 `compile*` API 仍只返回首个错误。`qcoffee --check FILE` 按源序把这些 parser error 写入标准错误，标准输出为空且绝不执行字节码。

嵌入宿主可用 `Program::fingerprint()` 作为确定性字节码缓存键；该指纹不改变验证与执行语义。

内建 `qtest --json` 每个文件输出一行稳定 JSON，供 CI 与宿主系统使用；`qtest --tap` 输出确定性的 TAP 13 记录；`qtest --filter TEXT` 按路径筛选，`qtest --list` 只枚举最终文件而不执行；`qcoffee --json` 单次执行输出一行稳定 JSON 值或结构化错误（资源耗尽的 kind 为 `resource`），`qcoffee --fingerprint FILE` 在不执行脚本时输出已验证字节码的稳定 16 位十六进制键，指纹使用规范化编码而非 Rust 调试文本；`qbench --json` 输出带语义护栏的编译、验证、执行计时记录，`qbench --list` 枚举负载而 `qbench --only NAME` 可只运行一个负载；`qdocco --markdown` 生成说明、围栏源码和最终值供审阅；嵌入方可用 `Context::set_fuel`、`set_max_call_depth` 与 `CancellationToken` 管理复用上下文的燃料、嵌套调用和取消，资源错误不能由脚本 `catch` 吞掉，也可链式调用 `Context::with_global` 与 `Context::with_native`，`cargo run --example embed` 提供可编译宿主示例；`--stats` 的执行统计仍写入标准错误。

`qbench --compare-qjs PATH --compare-iterations 1 --repeat 11 --json` 输出独立的 `quickcoffee.qcompare.v1` 同机对比；PATH 完全由调用者提供，记录覆盖标量循环、函数调用、数组构造/索引/遍历、映射读取/不可变更新和 Unicode 标量遍历/索引，并分别包含启动、编译、预编译热执行与端到端 CLI 总耗时，不能与进程内 `qbench.v1` 混比。JavaScript 的字符串下标是 UTF-16 code unit；Unicode 索引负载以 `Array.from` 预解码标量来匹配 QuickCoffee 的结果语义，此适配不表示底层操作同构。

这是 RFC 0001 的中文索引；未列出的 CoffeeScript 2016 特性不是“隐式兼容”，而是明确不支持。Cargo 包元数据提供仓库、README、许可证和 docs.rs API 链接。

标准库是普通函数：`print`、`len`、`type`、`range`、`str`、`integer`、`number`、`abs`、`sum`、`min`、`max`、`keys`、`values`、`join`、`split` 与 `assert`。聚合只收同质的有限 Number 或 Integer 数组；`sum([])` 为 Number `0`，`min([])` 与 `max([])` 报错。

`123n`、`0xffn`、`0b101n`、`0o755n` 是任意精度 Integer；它与 IEEE-754 Number 严格分型，不做混合算术或排序。Integer 支持精确算术、有符号位运算（不含 `>>>`）、range、索引/切片和 `by`；`integer(value)` 与 `number(value)` 是唯一数值类型转换，其中 Integer 转 Number 只接受安全整数范围。完整契约见 RFC 0135。

| 类别 | 支持 | 不支持（本版） |
|---|---|---|
| 字面量 | 十进制、十六进制 `0xff`、二进制 `0b1010`、八进制 `0o755` 与科学计数法数字、字符串、双引号 `#{expr}` 插值、保留换行的 `"""…"""` 插值 heredoc 与 `'''…'''` 字面 heredoc、`true`/`yes`/`on`、`false`/`no`/`off`、`nil`、数组与 `[head, items...]` 展开、整数区间 `[1..3]`（含上界）/`[1...3]`（不含上界）、映射、`{name}` 简写与映射展开 `{...base, key: value}` | 正则、JS 插值、`undefined` |
| 运算 | 算术、严格有符号 32 位位运算 `&`、`|`、`^`、`~`、`<<`、`>>`、`>>>` 及其名称复合赋值、名称复合赋值 `name += value`、`-=`, `*=`, `/=`, `%=`, `**=`、名称前后置更新 `++`/`--`、比较（`==`/`is`、`!=`/`isnt`，可短路成链 `a < b < c`）、`and`/`or`、`not`、仅对 `nil` 回退的 `left ? right`、后缀非 nil 测试 `value?`、仅名称的存在性赋值 `name ?= value`、数组成员 `value in array` / `value not in array`、映射自身键 `key of map` / `key not of map`、数组索引与严格切片 `a[start..end]` / `a[start...end]`、映射成员访问、nil 安全后缀 `a?.name`、`a?[i]`、`a?[start..end]`、`f?(args)` | 成员/索引/解构复合赋值、成员/索引/解构 `?=`、字符串/映射切片、隐式截断、未声明名称检查 |
| 控制 | `if`/`unless`、后置条件、`while … then …`、`until … then …`、语句后置 `body while/until condition`、前置或后置列表推导 `for value[, index] in xs [by step] [when condition] then …` / `value for value in xs`、`switch`/`when`、`try`/`catch`/`finally`、`throw`、函数内 `return`、`break`、`continue` | JS Error 对象、顶层 `return`、映射 `for` 的 `by` |
| 赋值/函数 | `a, b = array`、`[a, tail...] = array`（末项 rest 可为空）、`[a, {point: [b, c]}] = array`、`{key, "first-name": first, ...metadata} = map`（标识符或字面字符串键）的严格递归解构、`_` 忽略位；`x, y -> expression` 无括号名称闭包、`([a, b], {factor}) -> expression` 解构形参、`(x, y = 2, rest...) -> expression`（缺省或 `nil` 取默认）、`f(items...)` 展开调用、当前实现中的 `=>` 同义箭头、`do` 立即调用；RFC 0134 将 `=>` 修订为仅在 class 接收者上下文绑定 `this` | 动态 computed 映射键、无括号默认/rest/解构参数、class 外接收者绑定、生成器 |
| OO/模块 | 当前实现为工厂类 `class Name(args) -> expression`。RFC 0134 已采纳 CoffeeScript 风格 class、class 内 `this`、`new`、`extends`、`super` 与受限 `=>` 接收者绑定，issue #121 跟踪实现；仅嵌入 `Engine::compile_module` 的命名 import/export；宿主可显式构造根目录受限的 `RestrictedFileModuleLoader` | 全局/自由 `this`、任意函数构造、公开原型能力、class 外 `super`/接收者绑定、CLI 文件模块、隐式文件/网络加载 |

整数区间支持升序与降序：`[2..4]` 为 `[2, 3, 4]`，`[4..2]` 为 `[4, 3, 2]`；排除上界形式相应省略终点（`[4...2]` 为 `[4, 3]`）。边界必须是有限整数，过长区间仍报错。

字符串索引与严格切片按 Unicode 标量边界：`'a☕中'[1]` 为 `'☕'`，`'a☕中'[1..2]` 为 `'☕中'`；负索引从末项计数（`items[-1]`），越界仍报错；字符串 `for` 支持按 Unicode 标量下标以非零有限有符号整数 `by` 步进，例如 `by 2` 得 `[0, 2]`，`by -2` 从末项开始。

映射字面量可从左至右展开：`{...defaults, theme: 'dark'}`；后写显式键或展开段覆盖先写键，展开值必须为映射。映射解构可用末尾尾部模式 `{id, ...metadata}` 捕获未列键，所得映射为新的不可变值。

普通单引号与双引号字符串支持 `\\0`、`\\b`、`\\f`、`\\n`、`\\r`、`\\t`、`\\v`、引号/反斜杠、两位 `\\xNN`、四位 `\\uNNNN` 及一至六位 `\\u{...}` Unicode 转义。字符串可以跨物理行；普通换行合并为一个空格，字符串内缩进被忽略。行末单个未转义反斜杠会同时去除反斜杠与换行。双引号多行字符串仍支持 `#{expression}` 插值，单引号仍是字面文本；三引号 heredoc 继续保留换行。非法转义或非 Unicode 标量转义是词法错误。

标识符采用 Unicode XID：首字符为 XID start 或 `_`，后续为 XID continue 或 `_`，不做规范化；组合附标可作为续字符。`#` 是行注释；`### … ###` 是不嵌套的块注释，块内容不影响布局。`for value in array` 遍历数组（包括 `range` 的结果）；可写 `by step`，步长只求值一次且必须为非零有限整数，负步长从末项开始。`for own key, value of map` 遍历映射且不支持 `by`。`for` 收集每轮体值为新数组；`when` 跳过的项与 `continue` 不收集，`break` 返回既得前缀；`while`、`until` 与 `loop` 的值仍为 `nil`。`return expression` 只可在函数体中使用，立即返回表达式值；裸 `return` 返回 `nil`，并在离开循环或 `try` 时完成清理与 finally。`if`、循环和函数可在换行后用空格缩进多个语句，且缩进必须一致；语句也可用 `;` 分隔。调用可写 `f(a, b)`，亦可在同一逻辑行写 `f a, b`。条件必须是布尔值，名称必须先赋值或由宿主注册。完整规范见 RFC。

`do` 可立即调用函数；`do (name, other) -> ...` 将同名外层值按序转发，默认、rest 与解构形参在 `do` 中拒绝，`do -> ...` 仍是零参 IIFE。

`!` 是严格 Bool 的 `not` 别名；`!=` 仍是不等比较，`!1` 等非布尔操作数产生运行时错误。

隐式调用只消费同一逻辑行的普通表达式：`print value`、`add 20, 22`、`double add 20, 22`、`len [1, 2, 3]` 均可；跨布局边界请使用显式括号调用。

`for` 的绑定可用严格递归模式：`for [left, right] in pairs then left + right`、`for own _, value of record then value` 均可。模式不匹配是运行时错误，且本轮绑定绝不部分写入。

模块由嵌入宿主显式加载：`ModuleLoader` 只返回规范名称和源码，`Context::run_module` 在私有顶层环境执行并只返回声明的 `ModuleExports`；同名依赖在一次运行中复用，循环依赖明确报错，且整张图共享 fuel 与取消边界。宿主可显式构造 `RestrictedFileModuleLoader`，只读取一个规范根目录内的 UTF-8 `.qc` 文件；依赖必须使用 `./` 或 `../`，省略扩展名时补 `.qc`，绝对/bare 名、平台专用分隔符、词法越界及符号链接逃逸均拒绝。`qcoffee` CLI 暂不解析模块路径。

名称亦支持前置、后置数值更新：`next = ++counter` 产生更新后的值，`previous = counter--` 先产生旧值再减一。`++`、`--` 只接受名称并使用严格数值运算；成员、索引和解构形式在解析阶段拒绝。

算术还包括 CoffeeScript 风格整除 `a // b`（取 `floor(a / b)`）与向下取模 `a %% b`（`(a % b + b) % b`）。普通 `%` 仍是随被除数取符号的余数；名称可使用 `//=`、`%%=` 复合赋值。

位运算采用严格有符号 32 位数值模型：`&`、`|`、`^`、前缀 `~`、`<<`、算术右移 `>>` 与逻辑右移 `>>>`。操作数须为有限整数且在 32 位范围内，移位计数须为 `0..31`；`&=`、`|=`、`^=`、`<<=`、`>>=`、`>>>=` 只接受名称。逻辑右移结果仍以有符号 32 位数表示，例如 `-1 >>> 1` 为 `2147483647`；这不是 JavaScript 的隐式转换。

物理行末若留下显式运算符，下一行继续同一表达式，例如 `total = 1 +\n  2 * 3`。赋值、算术、比较、逻辑、成员、位运算、移位和幂运算均可续行；为保持后缀 `value?` 是完整语句，`?` 不作续行符，需要跨行回退时请使用括号。`->`、`=>` 仍开启原有的缩进函数体。续行期间的行首空格只作排版，不改变布局块。

纯字面量算术、比较、集合与插值在编译时折叠为经验证的常量；含动态名称或可能在运行时失败的严格表达式仍交给 VM 执行。多行数组和映射可按物理行省略逗号（`[\n  1\n  2\n]`、`{\n  first: 1\n  second: 2\n}`）；普通括号和调用参数仍须显式分隔。

映射亦可在单独赋值行后用缩进书写：`record =` 下一行的 `first: 1` 等条目会降低为无原型映射；嵌套映射递归处理。普通赋值续行（如 `value =` 下一行的 `1 + 2`）不会误判为映射。

数组遍历可另绑定从零开始的下标：`for value, index in items then value + index`；使用 `by step` 时下标仍是实际数组位置。

推导亦可用 CoffeeScript 风格后置形式：`value * 2 for value in items`，或以方括号包住写作 `[value * 2 for value in items]`。后置形式与前置形式共享 `by`、`when`、映射、模式、`break`、`continue` 语义；方括号只是推导界标，不再增加一层嵌套数组。
## 基准统计

`qbench --json --repeat 11` 对编译、验证与执行输出上侧中位数及对应 `*_mad_ns`（median absolute deviation）离散度。`qbench --compare-qjs PATH` 对两端的启动、编译、预编译热执行及 CLI 总耗时分别输出中位数与 MAD。
