# RFC 0001：CoffeeScript 风格核心语言

- 状态：已采纳
- 依赖：RFC 0000

## 源码形式

UTF-8 文本，以换行或分号分隔语句。缩进可形成 `if`、`unless`、`while`、`for`、函数和工厂类的多语句体；缩进只能使用空格，且同一块必须对齐。圆括号、方括号及花括号内的换行不参与块布局。标识符首字符是 Unicode XID start 或 `_`，其余字符为 XID continue 或 `_`，不做 Unicode 规范化（RFC 0033）。`#` 开始行注释；`### … ###` 是非嵌套块注释（RFC 0032）。禁止反引号 JavaScript 插值形式及所有内嵌 JavaScript。

## 值

`Number`（IEEE-754 `f64`）、`Bool`、`String`、`Array`、`Map`、函数、`nil`。`true`/`false` 可分别写作 CoffeeScript 风格的 `yes`/`on` 与 `no`/`off`。`nil` 是唯一空值；名称由词法环境或宿主全局在执行时解析，找不到的名称是运行时错误（RFC 0051）。`==`（亦可写 `is`）采用同型值相等；不同型不等，绝不强制转换。

## 表达式

支持括号、数组 `[a, b]` 与数组项展开 `[a, values...]`、整数区间 `[start..end]` / `[start...end]`、对象 `{name: value}` 及标识符简写 `{name}`、索引 `a[i]`、映射成员访问 `a.name`、调用 `f(a, b)` 与实参展开 `f(values...)`、nil 安全的 `a?.name`、`a?[i]`、`f?(args)`、一元 `-`/`not`，二元 `? + - * / % ** < <= > >= == is != isnt in of not in not of and or`。相邻严格相等或数值比较可成链，如 `a < b < c`；中间值仅求值一次且失败短路（RFC 0025）。成员访问仅适用于映射，绝不经由原型链。安全后缀只在接收者为 `nil` 时短路；非 nil 时仍采用普通访问或调用的严格错误语义（RFC 0022）。`and`、`or` 短路并返回其操作数；`left ? right` 仅在左值为 `nil` 时求值右值；`value in array` 与 `key of map` 分别检查数组成员与映射自身键，`not in` / `not of` 是其 Bool 取反（RFC 0030）。条件仅接受 Bool；其他值进入条件位置为运行时错误。为保持无歧义的单遍解析，0.1 不接受 CoffeeScript 的省略括号调用。

字符串可用单引号或双引号，支持 `\\n`、`\\r`、`\\t`、`\\\\`、对应引号的转义。双引号支持 `#{expression}` 插值；其中只能求值 QuickCoffee 表达式，不能嵌入 JavaScript。

## 语句与函数

名称更新的 `++`/`--` 见 RFC 0055；整除与向下取模及其复合赋值见 RFC 0056。

严格有符号 32 位位运算及其复合赋值见 RFC 0057。

行尾显式运算符续行见 RFC 0058；续行不改变字节码指令集合。

普通单引号/双引号多行字符串见 RFC 0059；三引号 heredoc 仍按 RFC 0040 保留换行。

纯字面量常量折叠见 RFC 0060；折叠不改变严格运行时错误边界。多行数组与映射可按 RFC 0063 省略逗号，但调用参数仍须显式分隔。

`name = expression` 绑定或更新当前词法环境；顶层执行时该环境即全局环境。名称也支持严格数值前置/后置 `++`、`--`（RFC 0055）。函数调用建立子环境，因此函数内新赋值不泄漏到全局。数组与映射支持严格解构赋值；`_` 是显式忽略位。`if condition then expression else expression` 是表达式，`else` 可省略并产生 `nil`；`unless` 是条件取反的同义结构。后置 `expression if condition` 与 `expression unless condition` 在条件不满足时产生 `nil`。后缀 `value?` 仅检查是否非 nil，保持 Bool 值且不检查未绑定名称（RFC 0038）。`while condition then expression` 重复求值，`until condition then expression` 则重复至条件为真，语句位置的 `expression while condition` / `until condition` 是其后置形式（RFC 0036），`loop body` 则无限重复，三者结果均为 `nil`。数组可用严格切片 `items[start..end]`（含末端）或 `items[start...end]`（不含末端），负端点自末尾计，且端点必须为界内整数（RFC 0037）。数组可用 `for pattern in items [by step] [when condition] then expression` 遍历，映射可用 `for own key_pattern, value_pattern of map [when condition] then expression` 遍历；表达式也可写成后置推导 `expression for pattern in items` 或 `expression for own key_pattern, value_pattern of map`（RFC 0054）。绑定模式严格递归且每轮原子写入（RFC 0044），`by` 只用于数组且步长为一次求值的正整数（RFC 0029），`for` 收集每次体值为新数组，`when` 拒绝的项不收集，`break` 产生已收集前缀，`continue` 不收集当前项（RFC 0042）。`switch`/`when` 选择单一分支；`try`/`catch`/`finally` 与 `throw` 处理 QuickCoffee 运行时错误。`return expression` 仅在函数内结束当前调用并返回其值，裸 return 返回 `nil`；它清理循环并执行沿途 finally（RFC 0028）。

函数为 `(a, b) -> expression` 或无括号普通名称形式 `a, b -> expression`，以创建时的词法环境捕获自由变量；无括号形式不接受默认、rest 或解构参数（RFC 0035）。形参可写为 `name = expression` 取默认值，且默认形参必须在必选形参之后。缺省或传入 `nil` 时，默认式在被调函数内求值；最后一个形参可写成 `rest...` 以接收剩余实参数组。`=>` 作为无 `this` 运行时中的可读同义箭头；两者均不提供 JavaScript 的绑定接收者语义。`do function` 以零实参立即调用函数。没有 rest 时实参不得多于全部形参，也不得少于必选形参。递归函数通过函数自身绑定可用。默认参数细则见 RFC 0021；`for` 见 RFC 0004；无原型工厂类见 RFC 0006；缩进块见 RFC 0009。

## 标准库

预置函数是普通名称，不是对象原型方法：`print(value...)`、`len(value)`、`type(value)`、`range(start, end)`、`str(value)`、`keys(map)`、`values(map)`、`join(array, separator)`、`split(string, separator)`、`assert(bool, message?)`。它们的行为由 RFC 0003 的宿主接口定义。不存在 `console`、`Object.prototype`、`Array.prototype`。
