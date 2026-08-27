# RFC 0001：CoffeeScript 风格核心语言

- 状态：已采纳
- 依赖：RFC 0000

## 源码形式

UTF-8 文本，以换行或分号分隔语句。缩进可形成 `if`、`unless`、`while`、`for`、函数、当前工厂 class 以及 RFC 0134 的 class 成员体；缩进只能使用空格，且同一块必须对齐。圆括号、方括号及花括号内的换行不参与块布局。标识符首字符是 Unicode XID start 或 `_`，其余字符为 XID continue 或 `_`，不做 Unicode 规范化（RFC 0033）。`#` 开始行注释；`### … ###` 是非嵌套块注释（RFC 0032）。禁止反引号 JavaScript 插值形式及所有内嵌 JavaScript。

## 值

`Number`（IEEE-754 `f64`）、`Bool`、`String`、`Array`、`Map`、函数、`nil`。`true`/`false` 可分别写作 CoffeeScript 风格的 `yes`/`on` 与 `no`/`off`。`nil` 是唯一空值；名称由词法环境或宿主全局在执行时解析，找不到的名称是运行时错误（RFC 0051）。`==`（亦可写 `is`）采用同型值相等；不同型不等，绝不强制转换。

## 表达式

映射字面量支持从左至右的 `...base` 展开（RFC 0074）；数组与字符串索引支持从末项计数的有限负索引（RFC 0076）。映射解构模式可用末尾 `...metadata` 捕获未列键（RFC 0075）。

支持括号、数组 `[a, b]` 与数组项展开 `[a, values...]`、整数区间 `[start..end]` / `[start...end]`、对象 `{name: value}` 及标识符简写 `{name}`、数组/字符串索引与严格切片 `a[i]`、`a[start..end]`（RFC 0037、RFC 0072）、映射成员访问 `a.name`、调用 `f(a, b)` 与实参展开 `f(values...)`、nil 安全的 `a?.name`、`a?[i]`、`f?(args)`、一元 `-`/`not`，二元 `? + - * / % ** < <= > >= == is != isnt in of not in not of and or`。相邻严格相等或数值比较可成链，如 `a < b < c`；中间值仅求值一次且失败短路（RFC 0025）。成员访问仅适用于映射，绝不经由原型链。安全后缀只在接收者为 `nil` 时短路；非 nil 时仍采用普通访问或调用的严格错误语义（RFC 0022）。`and`、`or` 短路并返回其操作数；`left ? right` 仅在左值为 `nil` 时求值右值；`value in array` 与 `key of map` 分别检查数组成员与映射自身键，`not in` / `not of` 是其 Bool 取反（RFC 0030）。条件仅接受 Bool；其他值进入条件位置为运行时错误。调用可按 RFC 0065 在同一逻辑行省略括号，跨布局边界仍须显式括号。

字符串可用单引号或双引号，支持 `\\n`、`\\r`、`\\t`、`\\\\`、对应引号的转义。双引号支持 `#{expression}` 插值；其中只能求值 QuickCoffee 表达式，不能嵌入 JavaScript。

## 语句与函数

名称更新的 `++`/`--` 见 RFC 0055；整除与向下取模及其复合赋值见 RFC 0056。

严格有符号 32 位位运算及其复合赋值见 RFC 0057。

行尾显式运算符续行见 RFC 0058；续行不改变字节码指令集合。

普通单引号/双引号多行字符串见 RFC 0059；三引号 heredoc 仍按 RFC 0040 保留换行。

纯字面量常量折叠见 RFC 0060；折叠不改变严格运行时错误边界。多行数组与映射可按 RFC 0063 省略逗号，但调用参数仍须显式分隔；RFC 0064 另定义缩进式映射字面量；RFC 0065 增加受约束的同一逻辑行隐式调用。

`name = expression` 绑定或更新当前词法环境；顶层执行时该环境即全局环境。名称也支持严格数值前置/后置 `++`、`--`（RFC 0055）。函数调用建立子环境，因此函数内新赋值不泄漏到全局。数组与映射支持严格解构赋值；`_` 是显式忽略位。`if condition then expression else expression` 是表达式，`else` 可省略并产生 `nil`；`unless` 是条件取反的同义结构。后置 `expression if condition` 与 `expression unless condition` 在条件不满足时产生 `nil`。后缀 `value?` 仅检查是否非 nil，保持 Bool 值且不检查未绑定名称（RFC 0038）。`while condition then expression` 重复求值，`until condition then expression` 则重复至条件为真，语句位置的 `expression while condition` / `until condition` 是其后置形式（RFC 0036），`loop body` 则无限重复，三者结果均为 `nil`。数组可用严格切片 `items[start..end]`（含末端）或 `items[start...end]`（不含末端），负端点自末尾计，且端点必须为界内整数（RFC 0037）。数组或字符串可用 `for pattern in iterable [by step] [when condition] then expression` 遍历；字符串按 Unicode 标量产生单字符字符串，第二绑定为标量下标（RFC 0070）。映射可用 `for own key_pattern, value_pattern of map [when condition] then expression` 遍历；表达式也可写成后置推导 `expression for pattern in iterable` 或 `expression for own key_pattern, value_pattern of map`（RFC 0054）。绑定模式严格递归且每轮原子写入（RFC 0044），`by` 只用于数组或字符串，步长一次求值且必须为非零有限整数（RFC 0029、0100），负步长从末项开始；`for` 收集每次体值为新数组，`when` 拒绝的项不收集，`break` 产生已收集前缀，`continue` 不收集当前项（RFC 0042）。`switch`/`when` 选择单一分支；`try`/`catch`/`finally` 与 `throw` 处理 QuickCoffee 运行时错误。`return expression` 仅在函数内结束当前调用并返回其值，裸 return 返回 `nil`；它清理循环并执行沿途 finally（RFC 0028）。

函数为 `(a, b) -> expression` 或无括号普通名称形式 `a, b -> expression`，以创建时的词法环境捕获自由变量；无括号形式不接受默认、rest 或解构参数（RFC 0035）。形参可写为 `name = expression` 取默认值，且默认形参必须在必选形参之后。缺省或传入 `nil` 时，默认式在被调函数内求值；最后一个形参可写成 `rest...` 以接收剩余实参数组。`->` 始终无接收者；RFC 0134 的 `=>` 只在合法 class 接收者上下文中词法捕获当前 instance/class，顶层、默认参数和普通函数中的 `=>` 为编译错误。`do function` 以零实参立即调用函数。没有 rest 时实参不得多于全部形参，也不得少于必选形参。递归函数通过函数自身绑定可用。默认参数细则见 RFC 0021；`for` 见 RFC 0004；历史工厂类 RFC 0006 已由 class 接收者与继承 RFC 0134 取代；缩进块见 RFC 0009。

## 标准库

RFC 0095 扩展字符串 `for` 也可使用 `by` 步长；RFC 0100 规定其支持非零有限有符号整数并以负步长倒序；RFC 0097 规定 `do (name) -> ...` 从同名外层变量转发立即调用实参。

预置函数是普通名称，不是对象原型方法：`print(value...)`、`len(value)`、`type(value)`、`range(start, end)`、`str(value)`、`trim(text)`、`contains(text, needle)`、`starts_with(text, prefix)`、`ends_with(text, suffix)`、`replace_all(text, needle, replacement)`、`sort(array)`、`concat(left, right)`、`abs(number)`、`sum(array)`、`min(array)`、`max(array)`、`keys(map)`、`values(map)`、`join(array, separator)`、`split(string, separator)`、`assert(bool, message?)`。数值聚合只接受有限数；空数组的 `sum` 为 `0`，`min`/`max` 要求非空。RFC 0139 的字符串查询严格、大小写敏感且不读取 locale；`trim` 使用 RFC 固定的 Unicode White_Space 表。RFC 0140 的 `sort` 返回新数组，只接受同质的有限 Number、Integer、Decimal 或 String，并使用稳定的数值或 Unicode scalar 字典序。RFC 0144 的 `concat` 只连接两个同为 String 或同为 Array 的值；RFC 0150 的 `replace_all` 执行非重叠、不重扫插入文本的字面量替换；两者都在分配前检查资源边界。不存在 `console`、`Object.prototype`、`Array.prototype` 或 String prototype。

### 后续契约修订

RFC 0095 将字符串 Unicode 标量迭代纳入 `by` 步进语义，覆盖本节早期仅举数组的简写；RFC 0097 进一步规定 `do (name) -> ...` 从同名外层变量转发立即调用实参。
