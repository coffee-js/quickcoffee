# QuickCoffee document

## Notes

QuickCoffee 用法

映射可展其项，后书之键胜前书：{...defaults, theme: 'dark'}。
映射解构末可用 ...metadata 收未列之键，所得映射不变。
数列与 Unicode 字符负索引，-一取其末。
此机先析其文，编为字节码，验而后行。非 JavaScript 也，故无原型之链、this、eval 与内嵌之文。
# 注一行；### … ### 注一段，弗相嵌，先于布局析法略之。
名从 Unicode XID 之法，合附之记可续名，而机不正其 Unicode。
yes、on 同 true；no、off 同 false；is、isnt 则严等也。
! 同 not，严反 Bool；!= 仍严不等。
严等与数较可相连，中项惟求一遍，前否则后不求。
qcoffee - 者，自标准输入读其文也。
qcoffee --stats 则书所试指令与余燃料于标准错误，程序之标准输出不改；每次惟一源码输入，执行模式相冲则报用法之误。
qcoffee --check FILE 者，析编验其文而不行也。
qcoffee --interactive（或 -i）者，逐行共用一 Context；:help 示命，:quit 出之。
qcoffee --interactive --stats 惟非空行之行而行或运行时有误者，书指令与余燃料一条；析验之误不更书。
'a☕中'[1] 即 '☕'，'a☕中'[1..2]' 得 '☕中'；字符串索引循 Unicode 标量。
for character, index in 'a☕中' then index，得 [0, 1, 2]；字符串循 Unicode 标量，by 可正可负，负者自末起。
do (name, other) -> ... 即调用之，转外层同名之值；do -> ... 仍零参。
[head, tail...] = [1, 2, 3]，tail 得 [2, 3]；数组之 rest 必居末。
qtest --fuel N 者，为各可行文别限其指令之数。
qtest --stats 更书各篇所试指令与余燃料于标准错误，而 ok 之出不改。
qtest --json 每篇出一行 JSON，便于 CI 取用；--stats 仍书于标准错误。
qtest --tap 出 TAP 13 及定次之记录；--json 与 --tap 不可并用。
qtest --filter TEXT 依路径择篇；qtest --list 但列所择之篇而不行其文。
qcoffee --json 一行以 JSON 载其值或错状，俾 CI 与宿主取用。
宿主之误，有 ErrorKind::Parse、Verify、Runtime 三类，且可别取其详；error.position() 或示从一始之源码行。
Engine::compile_program 创时验之；Context::run_program 屡行则复用不可变已验字节码。
Program::fingerprint 出确定 u64 码键，便宿主缓存，而不改执行。
qcoffee --fingerprint FILE 出十六位小写字节码键，先验之而不行其文。
qbench --json 每负载出一计时录，皆有语义护栏；--iterations 定其试数。
指纹以定式编码字节码，不取 Rust 调试辞，故工具链改其辞而缓存键不改。
qdocco --markdown 出说明、围栏 QuickCoffee 代码及终值为可阅 Markdown 文。
嵌者可于两行之间呼 Context::set_fuel；Context::fuel 示每行之限，而全局不失；with_global、with_native 可相次而呼以置宿主。
cargo run --example embed 可验最小 Rust 宿主，设全局、立原生回调而行 QuickCoffee。
宿主可用 Value::kind() 别其类，Value::is_nil() 验 nil，不窥其内容器。
Cargo 包志指仓、docs.rs API、README 与许可证，使嵌者易寻其用。
Context::last_execution() 示所试指令与余燃料，而不露 VM 之帧。
-- 后之参，以常字符串数组 argv 见于文中。
其内府皆常函，如 print、len、type、range、str、abs、sum、min、max、keys、values、join、split、assert；数聚函唯受有限数之列。
函式取词法之境；末常参可书 y = 2，参缺或传 nil 则于函中取其值；末有余参，则书 tail...。
常名之参可省其括：left, right -> left + right；常值、余参与解构仍须括之。
return expression 惟函中可用，反其值而终此函；徒 return 得 nil，且清环行 finally。
函参可层叠数组映射之式；常值与余参仍惟名可书。
整数之区间，`[1..3]` 及其终，`[1...3]` 则不及。
区间亦可逆行，`[3..1]` 得 `[3, 2, 1]`，`[3...1]` 得 `[3, 2]`。
数组之截，a[start..end] 及终，a[start...end] 不及终；端须界内有限整数，负自末计，受者 nil 则安截不求端。
空值回退，书 left ? right；惟 nil 发之，false 与零不易。
后缀 value? 惟验非 nil；nil? 为 false，false? 与 0? 皆 true，未名之误弗隐。
name ?= value，名未系或 nil 乃书之；既非 nil，右弗行，成员索引解构弗可。
名亦可前后置增减：next = ++counter 得新值，previous = counter-- 先得旧值；惟名可用。
算术亦有整除 // 与取模 %%：-7 // 5 得 -2，-7 %% 5 得 3。
value in array 验数组之有无；key of map 唯验映射己有之字符串键。
value not in array、key not of map，皆反其严验，不涉原型。
映射字面量之 {name}，即 {name: name} 之省文。
赋值之式，可层叠数组与映射；机先验其全式，后易诸名。
数组与调用之 items...，展其数组，非假 JavaScript apply。
安空值之缀，书 a?.name、a?[i]、f?(args)；惟受者 nil 则短之。
until condition then body 者，至其布尔条件真而止。
语句可后书 while/until，反复其全赋或解构，弗嵌寻常子式。
loop body 者，如 while true 恒行；以 break 出之，犹受 fuel 限。
for 为收集之式：轮体诸值成新数组，when 与 continue 弗收，break 存既得之首。
for 所系可为严式：for [left, right] in pairs，每项全合乃易名。
数组之环可书 by step；步惟求一遍，须非零有限整数，负者自末起，映射环弗用之。
数组之环亦可系从一始之下标：for value, index in items then value + index。
后置之推导亦循严收集：value * 2 for value in items，或括以 [value * 2 for value in items]。

## Code

````quickcoffee

甲 = 6
倍 = (x) -> x * 2
shorthand = 'yes'
[first, {point: [x, y]}] = [0, {point: [20, 22]}]
scale = ([left, right], {factor}) -> (left + right) * factor
倍(甲) == 12 and "其数为 #{倍(甲)}" == '其数为 12' and yes is on and no is off and 1 < 2 < 3 and x + y == 42 and scale([20, 1], {factor: 2}) == 42 and ((首, y = 2) -> 首 + y)(40) == 42 and ((首, 余...) -> 首 + len(余))(40, 1, 2) == 42 and ((items) -> for n in items then if n == 42 then return n)([1, 42]) == 42 and ((-> try return 1 catch error then 2 finally 0)()) == 1 and len([1..3]) == 3 and len([1...3]) == 2 and (nil ? 42) == 42 and (false ? 42) == false and nil?.missing == nil and 2 in [1, 2] and 'name' of {name: 1} and {shorthand}.shorthand == 'yes' and len([1, [2, 3]..., 4]) == 4
步和 = 0
for n in [1..9] by 3 then 步和 = 步和 + n
步和 == 12
len(for [left, right] in [[20, 22], [1, 2]] then left + right) == 2
后置之倍 = value * 2 for value in [1..3]
后置之倍 == [2, 4, 6]
计数 = 2
前增 = ++计数
后减 = 计数--
[前增, 后减, 计数] == [3, 3, 3]
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
3 not in [1, 2] and '缺' not of {在: 1}
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
### 此段有无效 ` 文，机不求之
###
42 == 42
````

## Final value

`true`
