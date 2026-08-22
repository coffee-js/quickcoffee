# QuickCoffee 用法（宋代官话古文体）

QuickCoffee 者，Rust 所为字节码机也，非 JavaScript 之运行时。其文先析，次编，复验，而后行之。故无原型之链，无 `this`，无 `eval`，亦禁嵌 JavaScript。

三引号之 heredoc，保其换行：`"""…"""` 可插 `#{expression}`，`'''…'''` 则字面也；不削其缩进，不闭则为词误。

`#` 注一行；`### … ###` 注一段，弗相嵌。其文先于布局与析法而略之；不闭则为词误。

名从 Unicode XID 之法：首用 XID start 或 `_`，续用 XID continue 或 `_`；合附之记可续名，而机不正其 Unicode。

又从 CoffeeScript 之便称：`yes`、`on` 同 `true`；`no`、`off` 同 `false`；`is`、`isnt` 同严等之 `==`、`!=`，不易其类。

严等与数之比较，相连可书 `1 < middle() < 3`；中项惟求一遍，前较既否，后项不求。

欲试之，曰：`qcoffee -e "print(range(1, 4))"`。`--check FILE` 者，析、编、验其文而不行；`--fuel N` 者，限所行指令之数也；数尽则止而报误。内府有 `print`、`len`、`type`、`range`、`str`、`keys`、`values`、`join`、`split`、`assert`；`range(a, b)` 取自 a 至 b 前。

`qcoffee -` 自标准输入读其文，便于管道；`qcoffee --dump-bytecode -` 则析其指令而不行之。

`--` 后之参，以常字符串数组 `argv` 见：`qcoffee program.qc -- first second`，则 `len(argv)` 为 `2`。不暴宿主之进程与环境对象。

函式书作 `(x) -> expression`，或省其括而曰 `left, right -> left + right`，取其所生之词法境；常值、余参与解构之参仍须括之。末之参可定其常值，如 `(head, sep = '-') -> expression`。参缺，或明传 `nil`，则于被调函中求其常值，故可引先参与所取之境；必参当先于定值之参。若末参为余参，则书 `(head, tail...) -> expression`，余实参合为数组。欲为可行之文，曰：`qdocco FILE -o FILE.html`；欲试诸例，曰：`qtest FILE...`，各篇终值皆须 `true`。

函中可曰 `return expression`，即反其值而终此函；徒曰 `return`，则反 `nil`。不越内函。其在环中，则清环之机；其经 `try`、`catch`，则由内而外行 `finally`。若 `finally` 中复曰 return，则夺前所欲反之值。欲因条件而反，当书 `if condition then return value`。

函参亦可层叠解构：`([left, right], {factor}) -> (left + right) * factor`。调用之参，必先合其式；常值惟名参可设，余参惟末一名。

映射字面量中，`{name}` 即 `{name: name}` 之省文；若键为字符串，犹须明书其值。

赋值之式，可层叠数组与映射：`[first, {point: [x, y]}] = [1, {point: [20, 22]}]`。数组各层必等其数，映射必有其所列键；机先验其全式而后易名，故深处有误亦不半易。

数组项与调用实参末之 `...` 展其数组：`[1, values..., 4]` 拼其元素，`fn(values...)` 逐个传参。所展必为数组。

欲安于空值，则书 `record?.name`、`values?[index]`、`fn?(args)`：受者为 `nil`，即得 `nil`，索引与参亦不求；非空则仍循常法，映射缺键犹报误。

`qtest --fuel N FILE...` 则各篇别限其指令，故一篇受限之环，不耗他篇之数。

遍数组，则曰 `for item in items then expression`；其所系可为严式，如 `for [left, right] in pairs then left + right`，一项之诸名必待全合而后易。若欲间取，则置 `by step`，如 `for item in [1..9] by 3 then expression`。其体诸值聚为新数组；`when` 所拒者不聚，`break` 则反已聚之先段。其步惟求一遍，且须正有限整数；映射之遍不得用之。`break` 止其内环，`continue` 逾其一轮；while、until、loop 之值恒为 `nil`。

同一收集之法，亦可后置作 `value * 2 for value in items`，或括之为 `[value * 2 for value in items]`。方括惟为推导之界，不更生一层数组；`by`、`when`、映射、诸式、`break`、`continue` 皆循前式。

整数之区间，`[1..3]` 及其终，得 `[1, 2, 3]`；`[1...3]` 则不及，得 `[1, 2]`。其界必为有限整数。

后缀 `value?`，惟验其非 nil：`nil?` 为 false，而 `false?` 与 `0?` 皆 true；未名之误弗隐，亦非 `left ? right` 之回退。

`name ?= value` 者，名未系或值为 nil，乃求 value 而书之；既非 nil，则右不行。惟名可用，成员、索引、解构皆弗许；寻常读未名，犹为误。

名亦可前后置增减：`next = ++counter` 得新值，`previous = counter--` 先得旧值再减一；惟普通名称可用。

算术亦有整除 `a // b` 与取模 `a %% b`；如 `-7 // 5` 得 `-2`，`-7 %% 5` 得 `3`，寻常 `%` 仍随被除数取符号。

位运算皆守有符号三十二位之数：`&`、`|`、`^`、`~`、`<<`、`>>`、`>>>`；移位之数限于零至三十一，复合式惟名可用。

行末若有显式运算符，次行可承其表达式；承行之缩进惟饰文，不改布局之块。

常引号之文亦可跨行；换行合为一空格，行末反斜杠则去之。

如 `(1 + 2 * 3) == 7` 之纯字面算术，编时折为所验常量。

数组之截，书 `items[start..end]` 以包括终，`items[start...end]` 以不及终；二端左至右各求一遍，皆须界内有限整数。负数自末计，`-1` 末项也；惟数组可截，弗暗截短。受者 nil，则 `items?[start..end]` 得 nil，端不求焉。

`left ? right` 者，空值之回退也：左为 `nil`，乃求右；`false`、零、空串与空器皆不以为空。

`value in array` 验数组之成员，`value not in array` 反其验；`key of map` 唯验映射己有之字符串键，`key not of map` 反其验。映射无原型键可寻。

`until condition then body` 者，反环也：反复至布尔条件真而止；`break`、`continue`、缩进与 fuel 之法，皆同 `while`。

语句之位，亦可后书 `n = n + 1 while n < 3`，等前书之环，而每轮尽行其赋；`until` 同此。解构赋亦可为体，然不可嵌为寻常子式。

`loop body` 者，恒行如 `while true`；以 `break` 出之，犹受 fuel 之限。

`for` 之可迭者与 `then` 间置 `when condition`，可筛其环；不合者不行其体：`for n in [1..5] when n > 2 then print(n)`。

嵌入方遇误，可由 `error.kind()` 别 `ErrorKind::Parse`、`Verify`、`Runtime`；`error.message()` 得其详，`error.position()` 或得从一始之源码行，不必析展示之文。

欲屡行已编之文，可用 `Engine::compile_program` 得共享 `Program`，以 `Context::run_program` 行之；复制其柄，不复制字节码。

数组映射跨行时，逗号可省；调用之参与寻常括中之式，仍须明分其隔。
