## QuickCoffee 用户手册
##
## QuickCoffee 先将源码解析并编译为经验证的字节码，随后由带 fuel 限制的 VM 执行。
## qcoffee - 可从标准输入读取 QuickCoffee 程序。
## qcoffee --quit 创建一个 Context 后静默退出，不能与源码或其他执行选项组合。
## qcoffee --stats 将指令数、剩余燃料及查名、调用、容器、迭代、异常、托管值分配与词法环境分配计数写入标准错误，同时保持程序标准输出不变；qcoffee 每次只接受一个源码输入，冲突执行模式会报用法错误。
## 嵌入模块可写 import { public as local } from 'name' 与 export；Engine::compile_module 和 Context::run_module 只经宿主 ModuleLoader 取源码，模块全局私有且整张图共享 fuel。
## 嵌入宿主可用 Context::with_fuel、with_max_call_depth、with_resource_limits 和 with_cancellation_token 分别限制指令、递归、JSON 数据大小与取消执行；资源错误不被脚本 catch 吞掉，JSON 语法错误仍可捕获。
## qcoffee --check FILE 只解析、编译并验证 FILE，不执行它。
## qcoffee --interactive（或 -i）逐行复用同一 Context；:help 显示命令，:quit 退出。
## qcoffee --interactive --stats 仅为实际执行或运行时失败的非空输入行输出指令/燃料统计；解析、验证错误不输出新记录。
## 'a☕中'[1] 为 '☕'，'a☕中'[1..2] 为 '☕中'；字符串索引按 Unicode 标量。
## for character, index in 'a☕中' then index 得 [0, 1, 2]；字符串按 Unicode 标量遍历，by 可用非零有符号整数，负步从末项起。
## do (name, other) -> ... 即刻调用，并按名转发外层值；do -> ... 仍为零参。
## [head, tail...] = [1, 2, 3] 将 tail 绑定为 [2, 3]；数组模式 rest 必须居末。
## qtest --fuel N 为每份可执行文档设置独立指令预算。
## qtest --stats 将每份文档的指令数与剩余燃料写入标准错误，不改变 ok 输出。
## qtest --json 为每份文档输出一行稳定 JSON，便于 CI；--stats 仍写标准错误。
## qtest --tap 输出 TAP 13 及确定编号的记录；--json 与 --tap 互斥。
## qtest --filter TEXT 按路径筛选；qtest --list 只列出筛选后的文件而不执行。

## qcoffee --json 单次执行输出一行 JSON 值或结构化错误，便于 CI 与宿主消费。
## Rust 嵌入错误有 ErrorKind::Parse、Verify、Runtime、Resource；error.resource_limit() 可分辨 fuel、调用深度、取消及 JSON 输入/输出/字符串/容器/值数/深度边界，宿主回调仍可返回 Error::runtime("message")，error.position() 可给出从 1 开始的源码行。
## Engine::compile_program 创建时验证一次；Context::run_program 重复执行时复用不可变的已验证字节码。
## Program::fingerprint 提供确定性的 u64 字节码缓存键，不改变执行语义。
## qcoffee --fingerprint FILE 以 16 位小写十六进制输出同一已验证字节码键，且不执行文件。
## qbench --json 为每个带语义护栏的负载输出一条计时记录；--iterations 设置样本次数。
## 每条 qbench 记录的 profile_* 字段来自一次不计时执行，给出热点与分配事件，不乘以 --iterations 或 --repeat。
## qbench --compare-qjs PATH 分别报告双方的启动、编译、预编译热执行与端到端 CLI 总耗时。正式报告宜用 --repeat 11；每个阶段都有中位数和 *_mad_ns。
## 指纹使用显式规范化字节码编码，不依赖 Rust 调试格式，故工具链显示变化不会改缓存键。
## qdocco --markdown 将说明、围栏 QuickCoffee 代码与最终值写成可审阅的 Markdown 产物。
## 嵌入方可在运行之间调用 Context::set_fuel、set_max_call_depth、set_resource_limits 与 set_cancellation_token；Context::fuel 和 resource_limits 返回当前策略，且不清除全局值；with_resource_limits、with_global 与 with_native 可链式配置宿主。
## cargo run --example embed 可编译最小 Rust 宿主：设置全局、注册原生回调并执行 QuickCoffee。
## 宿主可用 Value::kind() 分流类型，用 Value::is_nil() 判断 nil，无须检查内部容器。
## Cargo 包元数据指向仓库、docs.rs API、README 与许可证，便于嵌入方发现项目。
## Context::last_execution() 提供指令数、剩余燃料与调用深度峰值统计，不暴露 VM 帧。
## -- 后的参数以普通字符串数组 argv 暴露给程序。
## 它不是 JavaScript：没有公开原型链、全局/自由 this、eval 或内嵌 JavaScript；RFC 0134 已采纳仅限 class 内部的接收者、构造与继承，尚待实现。
## # 是行注释；### … ### 是不嵌套块注释，内容在布局和解析前忽略。
## 标识符遵循 Unicode XID：组合附标可作续字符，且引擎不做 Unicode 规范化。
## yes/on 与 no/off 是布尔别名；is/isnt 保持严格相等。
## ! 是严格 Bool 的 not 别名；!= 仍为严格不等。
## 严格或数值比较可成链，保留中间值且前段失败会短路。
## 标准库皆为普通函数：print、len、type、error、range、str、parse_json、encode_json、integer、number、decimal、decimal_div、round_decimal、abs、sum、min、max、keys、values、join、split 与 assert；error(code, message, data, cause) 构造密封的 RFC 0136 Error，catch 绑定 Error，资源错误仍不可捕获。
## RFC 0137 Decimal 字面量使用 m 后缀；精确除法拒绝循环小数，decimal_div 与 round_decimal 要求显式 scale 和舍入模式。
## 映射字面量可从左至右展开：{...defaults, theme: 'dark'}；后写键覆盖先写键。
## 映射解构末尾可用 ...metadata 捕获未列键，所得映射不可变。
## 数组与 Unicode 字符串支持负索引，-1 取末项。
##
## 函数捕获词法环境；末尾参数可写作 y = 2，缺省或 nil 时在函数内取默认值；末位 rest 参数写作 tail...。
## 普通名称参数可省略括号：left, right -> left + right；默认、rest 与解构参数仍须括号。
## return expression 只在函数内结束当前调用；裸 return 得 nil，并清理循环且执行沿途 finally。
## 形参可使用严格嵌套数组/映射模式，默认值和 rest 仍只用于名称。
## 整数区间 `[1..3]` 包含上界，`[1...3]` 不包含上界。
## 区间也可降序：`[3..1]` 得到 `[3, 2, 1]`，`[3...1]` 得到 `[3, 2]`。
## 三引号 heredoc 保留换行："""...""" 插值，'''...''' 为字面量。
## 数组切片 a[start..end] 包含末端，a[start...end] 不包含末端；端点须为界内有限整数，负数自末计，nil 安全切片在接收者 nil 时不求端点。
## 空值回退写作 left ? right，仅 nil 触发，false 与 0 保留。
## 后缀 value? 仅检验非 nil；nil? 为 false，false? 与 0? 为 true，且不隐藏未绑定名称错误。
## name ?= value 只在名称未绑定或为 nil 时求值并写入；已有非 nil 值时右侧短路，成员、索引与解构不适用。
## value in array 检查数组成员；key of map 只检查映射自身的字符串键。
## value not in array 与 key not of map 分别是否定的严格数组成员和映射自有键检查。
## 映射字面量中的 {name} 是 {name: name} 的简写。
## 赋值模式可嵌套数组和映射，VM 先完整验证，失败不留下部分绑定。
## 名称可作严格算术复合赋值：total += amount、power **= 2；成员与索引不可作之。
## 名称亦支持严格前后置更新：next = ++counter 得新值，previous = counter-- 先得旧值。
## 算术亦有整除 // 与向下取模 %%：-7 // 5 为 -2，-7 %% 5 为 3。
## 数组和调用中的 items... 会展开数组，不使用 JavaScript apply。
## nil 安全后缀 a?.name、a?[i]、f?(args) 只在接收者为 nil 时短路。
## until condition then body 重复执行，直至布尔条件为真。
## 语句位置可后置 while/until：重复整个赋值或严格解构，不能嵌入普通子表达式。
## loop body 是 while true 的无限形式，以 break 退出，仍受 fuel 限制。
## for 是收集表达式：每轮体值成新数组，when 与 continue 不收集，break 保留既得前缀。
## for 绑定可用严格模式：for [left, right] in pairs 会原子地绑定每个 pair。
## 数组循环可写 by step；步长只求一次，须为非零有限整数，负步从末项起，映射循环不用 by。
## 数组循环亦可绑定从零开始的下标：for value, index in items then value + index。
## 后置推导沿用严格收集：value * 2 for value in items，亦可写作 [value * 2 for value in items]。
base = 21
double = (x) -> x * 2
shorthand = 'yes'
[first, {point: [x, y]}] = [0, {point: [20, 22]}]
scale = ([left, right], {factor}) -> (left + right) * factor
double(base) == 42 and "答案 #{double(base)}" == '答案 42' and yes is on and no is off and 1 < 2 < 3 and x + y == 42 and scale([20, 1], {factor: 2}) == 42 and ((head, y = 2) -> head + y)(40) == 42 and ((head, tail...) -> head + len(tail))(40, 1, 2) == 42 and ((items) -> for n in items then if n == 42 then return n)([1, 42]) == 42 and ((-> try return 1 catch error then 2 finally 0)()) == 1 and len([1..3]) == 3 and len([1...3]) == 2 and (nil ? 42) == 42 and (false ? 42) == false and nil?.missing == nil and 2 in [1, 2] and 'name' of {name: 1} and {shorthand}.shorthand == 'yes' and len([1, [2, 3]..., 4]) == 4
步和 = 0
for n in [1..9] by 3 then 步和 = 步和 + n
步和 == 12
len(for [left, right] in [[20, 22], [1, 2]] then left + right) == 2
后置倍数 = value * 2 for value in [1..3]
后置倍数 == [2, 4, 6]
计数器 = 2
前置更新 = ++计数器
后置更新 = 计数器--
[前置更新, 后置更新, 计数器] == [3, 3, 3]
[-7 // 5, -7 %% 5] == [-2, 3]
[5 & 3, 5 | 2, 5 ^ 1, ~1, 1 << 3, -8 >> 2, -1 >>> 1] == [1, 7, 4, -2, 8, -2, 2147483647]
continued = 1 +
  2 * 3
continued == 7
message = "hello
  world"
message == 'hello world'
escaped = "A\\x42\\u{43}"
escaped == 'ABC'
folded = (1 + 2 * 3) == 7
folded
values = [
  1
  2
]
values == [1, 2]
record = {
  first: 20
  second: 22
}
record.first + record.second == 42
indented_record =
  first: 20
  nested:
    second: 22
indented_record.nested.second == 22
implicit_add = (left, right) -> left + right
implicit_answer = implicit_add 20, 22
implicit_answer == 42
3 not in [1, 2] and 'missing' not of {present: 1}
循环数 = 0
loop
  循环数 = 循环数 + 1
  break if 循环数 == 3
循环数 == 3
裸加 = left, right -> left + right
裸加(20, 22) == 42
后置数 = 0
后置数 = 后置数 + 1 while 后置数 < 3
后置数 == 3
切片数 = [0..4][1..3]
len(切片数) == 3 and 切片数[0] == 1 and [0..4][-3...-1][0] == 2
nil? == false and false? == true and 0? == true
默认数 ?= 42
默认数 == 42
多行 = """答案 #{double(base)}
次行"""
多行 == '答案 42\n次行'
### 这段含无效 ` 源文，却不会参与执行
###
0.1m + 0.2m == 0.3m and decimal_div(1m, 3m, 2, 'half_even') == 0.33m
json_payload = parse_json('{"money":12.30,"large":9007199254740993}')
encode_json(json_payload) == '{"large":9007199254740993,"money":12.3}'
42 == 42
