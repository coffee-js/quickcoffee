# QuickCoffee 0.1 性能报告

## 口径

此报告测量的是当前 RFC 0001 核心实现，不代表 CoffeeScript、QuickJS 或 JavaScript 引擎之间的比较。所有测量都避开标准输出、文件 I/O 和调试构建；结果只能用来跟踪本仓库的回归。编译路径同时维护词法 token 的一基源码行，以支持 RFC 0047 诊断。

命令：

```sh
cargo bench --bench core
```

测量环境：Apple arm64（Darwin 25.5.0，T6000）、`rustc 1.94.0 (4a4ef493e 2026-03-02)`、`cargo bench --bench core`。基准以 release profile 运行；报告日期为 2026-08-22。

基准工作负载：

loop-core（20,000 次）：

```coffee
sum = 0
i = 0
while i < 100 then i = i + 1
sum + i
```

postfix-loops（20,000 次）：

```coffee
sum = 0
i = 0
i = i + 1 while i < 100
sum + i
```

array-slices（10,000 次）：

```coffee
items = [0...100]
sum = 0
i = 0
while i < 100
  slice = items[10...90]
  sum = sum + slice[0] + slice[79]
  i = i + 1
sum
```

existence-tests（10,000 次）：

```coffee
value = nil
sum = 0
i = 0
while i < 100
  sum = sum + (if value? then 0 else 1)
  i = i + 1
sum
```

existential-assignment（10,000 次）：

```coffee
value = 1
sum = 0
i = 0
while i < 100
  value ?= 2
  sum = sum + value
  i = i + 1
sum
```

name-updates（10,000 次）：

```coffee
i = 0
sum = 0
while i < 100
  sum += i
  i++
sum
```

floor-modulo（10,000 次）：

```coffee
sum = 0
i = -100
while i < 100
  sum += i // 3
  sum += i %% 7
  i += 1
sum
```

closures-and-ranges（10,000 次）：

```coffee
base = 1
add = (n) -> n + base
sum = 0
for n in [1...50] then sum = sum + add(n)
sum
```

bare-lambda（10,000 次）：

```coffee
base = 1
add = n -> n + base
sum = 0
for n in [1...50] then sum = sum + add(n)
sum
```

stepped-iteration（10,000 次）：

```coffee
sum = 0
for n in [1...100] by 3 then sum = sum + n
sum
```

for-collection（10,000 次）：

```coffee
values = for n in [1...100] when n % 3 == 0 then n * 2
len(values)
```

postfix-comprehension（10,000 次）：

```coffee
values = n * 2 for n in [1...100]
sum = 0
for n in values then sum = sum + n
sum
```

for-pattern-bindings（10,000 次）：

```coffee
pairs = for n in [1...100] then [n, n + 1]
sum = 0
for [left, right] in pairs then sum = sum + left + right
sum
```

maps-and-control（10,000 次）：

```coffee
record = {a: 1, b: 2, c: 3}
sum = 0
for own key, value of record when value > 1 then sum = sum + value
try sum ? 0 catch error then 0
```

soak-access（10,000 次）：

```coffee
record = {answer: 1}
none = nil
sum = 0
i = 0
while i < 100
  sum = sum + record?.answer + (none?[i] ? 0)
  i = i + 1
sum
```

nested-destructuring（10,000 次）：

```coffee
sum = 0
i = 0
while i < 100
  [first, {point: [x, y]}] = [1, {point: [2, 3]}]
  sum = sum + first + x + y
  i = i + 1
sum
```

chained-comparisons（10,000 次）：

```coffee
low = 0
middle = 1
high = 2
sum = 0
i = 0
while i < 100
  sum = sum + (if low < middle < high then 1 else 0)
  i = i + 1
sum
```

destructuring-parameters（10,000 次）：

```coffee
scale = ([left, right], {factor}) -> (left + right) * factor
sum = 0
i = 0
while i < 100
  sum = sum + scale([1, 2], {factor: 3})
  i = i + 1
sum
```

return-cleanup（10,000 次）：

```coffee
find = (items) ->
  try
    for n in items then if n == 73 then return n
    nil
  catch error
    0
  finally
    0
sum = 0
i = 0
while i < 100
  sum = sum + find([1...100])
  i = i + 1
sum
```

编译测量从 UTF-8 源码到已验证 `Chunk`；执行测量复用已编译的共享 `Program`，每轮新建带 100,000 fuel 的 `Context`，避免工作负载被默认安全预算截断，也避免重复深拷贝指令流（RFC 0046）。每个负载在计时前先以同一 program 执行一次，并将最终值与声明的期望值比较；该语义护栏不计入计时（RFC 0045）。计时样本以 `black_box` 消费编译结果和运行值，避免 release 优化消去被测路径。以下为同一 release 二进制连续三次运行的中位数。

## 2026-08-22 基线

| 工作负载 | 编译中位数 | 编译吞吐 | 执行中位数 | 执行吞吐 |
|---|---:|---:|---:|---:|
| loop-core | 68.799 ms | 290,702 programs/s | 524.605 ms | 38,124 programs/s |
| postfix-loops | 64.535 ms | 309,909 programs/s | 514.385 ms | 38,881 programs/s |
| array-slices | 61.956 ms | 161,405 programs/s | 1,017.359 ms | 9,829 programs/s |
| existence-tests | 51.557 ms | 193,960 programs/s | 472.186 ms | 21,178 programs/s |
| existential-assignment | 49.816 ms | 200,739 programs/s | 544.416 ms | 18,368 programs/s |
| name-updates | 35.403 ms | 282,462 programs/s | 492.203 ms | 20,317 programs/s |
| floor-modulo | 51.559 ms | 193,953 programs/s | 1,496.128 ms | 6,684 programs/s |
| closures-and-ranges | 50.954 ms | 196,255 programs/s | 368.589 ms | 27,130 programs/s |
| bare-lambda | 49.829 ms | 200,686 programs/s | 367.051 ms | 27,244 programs/s |
| stepped-iteration | 31.895 ms | 313,529 programs/s | 123.921 ms | 80,697 programs/s |
| for-collection | 36.532 ms | 273,733 programs/s | 310.488 ms | 32,207 programs/s |
| postfix-comprehension | 43.229 ms | 231,326 programs/s | 542.345 ms | 18,438 programs/s |
| for-pattern-bindings | 57.605 ms | 173,596 programs/s | 946.521 ms | 10,565 programs/s |
| maps-and-control | 60.804 ms | 164,463 programs/s | 38.901 ms | 257,063 programs/s |
| soak-access | 62.119 ms | 160,981 programs/s | 646.593 ms | 15,466 programs/s |
| nested-destructuring | 62.738 ms | 159,393 programs/s | 1,520.947 ms | 6,575 programs/s |
| chained-comparisons | 64.095 ms | 156,018 programs/s | 690.177 ms | 14,489 programs/s |
| destructuring-parameters | 74.410 ms | 134,391 programs/s | 1,370.906 ms | 7,294 programs/s |
| return-cleanup | 93.339 ms | 107,136 programs/s | 16,192.423 ms | 618 programs/s |

## RFC 0057 增量：严格位运算

本轮在同一 `cargo bench --bench core` 方法中加入 `bitwise` 负载；其语义护栏要求结果为 `-196`。三次 release 运行的中位数如下，原始样本为 `48.117 / 51.328 / 47.849 ms`（编译）与 `1,237.206 / 1,340.189 / 1,212.938 ms`（执行）：

| 工作负载 | 编译中位数 | 编译吞吐 | 执行中位数 | 执行吞吐 |
|---|---:|---:|---:|---:|
| bitwise | 48.117 ms | 207,827 programs/s | 1,237.206 ms | 8,083 programs/s |

负载源码为：

```coffee
sum = 0
i = -100
while i < 100
  sum += (i & 31) ^ (i << 1)
  i += 1
sum
```

## RFC 0059 增量：普通多行字符串

本轮加入 `multiline-strings` 负载，语义护栏要求多行字符串长度为 `10`。三轮 release 运行的编译中位数为 `18.551 ms`（原始 `18.146 / 18.551 / 19.194`），执行中位数为 `14.194 ms`（原始 `14.182 / 14.194 / 18.449`），吞吐分别为 `539,054` 与 `704,523 programs/s`。

```coffee
message = "alpha
  beta"
len(message)
```

## RFC 0060 增量：纯字面量常量折叠

本轮加入 `constant-folding` 负载，语义护栏要求结果为 `true`；编译器在严格类型边界内将纯字面量表达式折叠为常量字节码。三轮 release 运行的原始样本（编译 / 执行，单位为 ms）为 `35.883 / 37.228 / 36.067` 与 `26.207 / 27.020 / 26.546`，中位数及吞吐如下：

| 工作负载 | 编译中位数 | 编译吞吐 | 执行中位数 | 执行吞吐 |
|---|---:|---:|---:|---:|
| constant-folding (20,000 次) | 36.067 ms | 554,524 programs/s | 26.546 ms | 753,409 programs/s |

```coffee
value = (1 + 2 * 3) == 7
value
```

三轮完整基准均通过语义护栏；动态名称、非法严格位运算及其他运行时错误仍保留原有错误路径。

## RFC 0063 增量：多行集合省略逗号

本轮加入 `multiline-collections` 负载，语义护栏要求无逗号数组与映射的结果为 `6`。三轮 release 运行的原始样本（编译 / 执行，单位为 ms）为 `45.623 / 45.412 / 45.779` 与 `17.120 / 16.960 / 16.922`；中位数及吞吐如下：

| 工作负载 | 编译中位数 | 编译吞吐 | 执行中位数 | 执行吞吐 |
|---|---:|---:|---:|---:|
| multiline-collections (10,000 次) | 45.623 ms | 219,189 programs/s | 16.960 ms | 589,617 programs/s |

```coffee
values = [
  1
  2
  3
]
record = {
  first: 1
  second: 2
}
values[2] + record.first + record.second
```

三轮完整基准均通过语义护栏；调用参数仍须逗号，集合边界由分组栈严格验证。

## RFC 0064 增量：缩进式映射字面量

本轮加入 `indented-maps` 负载，语义护栏要求递归缩进映射求值为 `3`。三轮 release 运行的原始样本（编译 / 执行，单位为 ms）为 `41.764 / 41.977 / 42.544` 与 `14.427 / 14.697 / 14.844`；中位数及吞吐如下：

| 工作负载 | 编译中位数 | 编译吞吐 | 执行中位数 | 执行吞吐 |
|---|---:|---:|---:|---:|
| indented-maps (10,000 次) | 41.977 ms | 238,224 programs/s | 14.697 ms | 680,415 programs/s |

```coffee
record =
  first: 1
  nested:
    second: 2
record.nested.second + record.first
```

三轮完整基准均通过语义护栏；普通赋值续行与已有显式映射保持原有性能路径。

## RFC 0058 回归

行续行只改变词法布局，不增加 VM 指令。三轮完整 `cargo bench --bench core` 均通过全部负载的语义护栏；作为可比锚点，`bitwise` 的当前中位数为编译 `49.653 ms`（原始 `49.653 / 49.578 / 52.643`）和执行 `1,198.477 ms`（原始 `1,198.477 / 1,194.696 / 1,581.957`）。续行本身由核心解析、词法无 `Semi` 和手册执行测试覆盖。

本轮完整原始样本（顺序为第 1 / 2 / 3 次；单位为 ms）如下；吞吐差异来自 OS 调度、热状态及测量噪声，不能用单次结果作回归判断。

| 工作负载 | 编译三样本 | 执行三样本 |
|---|---|---|
| loop-core | 83.220 / 68.799 / 67.757 | 517.717 / 524.605 / 529.044 |
| postfix-loops | 63.128 / 65.375 / 64.535 | 507.602 / 514.385 / 528.926 |
| array-slices | 61.509 / 63.315 / 61.956 | 1,001.114 / 1,017.359 / 1,026.563 |
| existence-tests | 50.478 / 51.736 / 51.557 | 468.008 / 472.186 / 474.202 |
| existential-assignment | 49.261 / 51.375 / 49.816 | 536.903 / 544.416 / 551.613 |
| name-updates | 35.225 / 36.358 / 35.403 | 480.313 / 492.203 / 500.813 |
| floor-modulo | 50.084 / 51.559 / 52.047 | 1,492.277 / 1,496.128 / 1,515.833 |
| closures-and-ranges | 50.707 / 52.681 / 50.954 | 386.383 / 368.550 / 368.589 |
| bare-lambda | 49.267 / 51.139 / 49.829 | 367.051 / 366.815 / 367.863 |
| stepped-iteration | 31.774 / 32.983 / 31.895 | 123.424 / 123.921 / 125.822 |
| for-collection | 35.892 / 37.276 / 36.532 | 310.488 / 310.415 / 320.711 |
| postfix-comprehension | 42.830 / 45.106 / 43.229 | 542.345 / 541.648 / 555.071 |
| for-pattern-bindings | 57.142 / 59.312 / 57.605 | 943.416 / 946.521 / 965.377 |
| maps-and-control | 59.287 / 61.009 / 60.804 | 38.852 / 38.901 / 38.902 |
| soak-access | 61.240 / 63.440 / 62.119 | 650.676 / 646.593 / 645.671 |
| nested-destructuring | 62.132 / 62.987 / 62.738 | 1,517.981 / 1,520.947 / 1,524.559 |
| chained-comparisons | 63.340 / 65.384 / 64.095 | 690.177 / 690.420 / 689.192 |
| destructuring-parameters | 73.442 / 75.870 / 74.410 | 1,366.160 / 1,375.677 / 1,370.906 |
| return-cleanup | 94.363 / 93.339 / 91.560 | 16,192.423 / 16,053.721 / 17,772.984 |

下表保留上一轮格式化明细，供此前基线比较；当前中位数仅以上述完整三样本为准。

| 样本 | 工作负载 | 编译 ms | 编译 programs/s | 执行 ms | 执行 programs/s |
|---|---|---:|---:|---:|---:|
| 1 | loop-core | 58.901 | 339,554 | 523.863 | 38,178 |
| 1 | postfix-loops | 55.190 | 362,387 | 539.492 | 37,072 |
| 1 | array-slices | 53.911 | 185,490 | 1,019.960 | 9,804 |
| 1 | closures-and-ranges | 43.996 | 227,294 | 380.187 | 26,303 |
| 1 | bare-lambda | 42.609 | 234,692 | 378.161 | 26,444 |
| 1 | stepped-iteration | 28.035 | 356,691 | 130.219 | 76,794 |
| 1 | maps-and-control | 52.629 | 190,010 | 46.285 | 216,054 |
| 1 | soak-access | 55.042 | 181,680 | 678.452 | 14,739 |
| 1 | nested-destructuring | 54.455 | 183,636 | 1,536.912 | 6,507 |
| 1 | chained-comparisons | 56.077 | 178,327 | 713.518 | 14,015 |
| 1 | destructuring-parameters | 64.901 | 154,081 | 1,380.994 | 7,241 |
| 1 | return-cleanup | 80.313 | 124,513 | 16,176.585 | 618 |
| 2 | loop-core | 59.473 | 336,285 | 532.780 | 37,539 |
| 2 | postfix-loops | 55.338 | 361,418 | 545.378 | 36,672 |
| 2 | array-slices | 53.908 | 185,501 | 1,030.119 | 9,708 |
| 2 | closures-and-ranges | 44.428 | 225,081 | 377.575 | 26,485 |
| 2 | bare-lambda | 42.669 | 234,363 | 376.779 | 26,541 |
| 2 | stepped-iteration | 27.912 | 358,264 | 131.205 | 76,217 |
| 2 | maps-and-control | 52.791 | 189,425 | 46.435 | 215,356 |
| 2 | soak-access | 55.398 | 180,513 | 678.770 | 14,733 |
| 2 | nested-destructuring | 54.433 | 183,712 | 1,537.054 | 6,506 |
| 2 | chained-comparisons | 56.113 | 178,213 | 715.319 | 13,980 |
| 2 | destructuring-parameters | 65.639 | 152,348 | 1,388.537 | 7,202 |
| 2 | return-cleanup | 80.567 | 124,120 | 16,052.787 | 623 |
| 3 | loop-core | 60.364 | 331,323 | 523.358 | 38,215 |
| 3 | postfix-loops | 55.582 | 359,827 | 523.664 | 38,192 |
| 3 | array-slices | 54.292 | 184,191 | 1,032.454 | 9,686 |
| 3 | closures-and-ranges | 44.450 | 224,974 | 380.611 | 26,274 |
| 3 | bare-lambda | 43.215 | 231,399 | 380.036 | 26,313 |
| 3 | stepped-iteration | 28.394 | 352,188 | 134.315 | 74,452 |
| 3 | maps-and-control | 53.611 | 186,528 | 46.337 | 215,808 |
| 3 | soak-access | 55.828 | 179,121 | 677.933 | 14,751 |
| 3 | nested-destructuring | 55.239 | 181,033 | 1,536.267 | 6,509 |
| 3 | chained-comparisons | 56.485 | 177,039 | 713.971 | 14,006 |
| 3 | destructuring-parameters | 65.955 | 151,618 | 1,381.550 | 7,238 |
| 3 | return-cleanup | 80.997 | 123,461 | 16,328.641 | 612 |

环境为 Darwin arm64，`rustc 1.94.0 (LLVM 21.1.8)`；由 `cargo bench --bench core` 的 release/optimized profile 生成；依赖为 Rust 标准库及纯 Rust 的 `unicode-ident`，没有外部运行时。提交性能数据时，必须同时记录 `rustc -Vv`、机器 CPU/内存、操作系统、完整命令、样本数量和源程序；同一机器至少重复三次，报告中位数，而不能以此单次开发机读数作跨项目结论。

## 已知性能边界

解释器使用显式调用帧和 fuel 计数；名称以有序映射查找，数组与映射值以不可变 `Rc` 容器分享。切片为保持独立不可变值而复制所选元素，故其热路径吞吐低于纯索引。这换取了 API 简洁与可验证性，尚未进行 inline cache、寄存器分配、常量折叠、字符串 intern 或垃圾回收优化。后续优化必须保持 RFC 0002 的字节码验证与 fuel 语义，并更新本报告。
