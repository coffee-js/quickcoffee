# QuickCoffee 0.1 性能报告

## 口径

此报告测量的是当前 RFC 0001 核心实现，不代表 CoffeeScript、QuickJS 或 JavaScript 引擎之间的比较。所有测量都避开标准输出、文件 I/O 和调试构建；结果只能用来跟踪本仓库的回归。基准分别记录普通编译、含源码映射与私有执行 sidecar 的 `Program` 准备、字节码验证和执行；编译路径同时维护词法 token 的一基源码行，以支持 RFC 0047 诊断。

命令：

```sh
cargo bench --bench core
```

机器可读的快速护栏可运行：

```sh
make qbench
# 或 cargo run --release --bin qbench -- --json --iterations 100
```

要自动取得稳健统计，正式报告应取至少 11 次样本；输出中的 `repeat` 记录样本数，`compile_ns`、`prepare_ns`、`verify_ns` 与 `execute_ns` 都是每轮迭代总耗时的中位数，配套的 `*_mad_ns` 是相对该中位数的 median absolute deviation（MAD）。`compile_*` 测 `Engine::compile`；`prepare_*` 测 `Engine::compile_program` 的端到端准备，不用两者相减来声称精确隔离 sidecar。MAD 为零只表示该组样本没有可观测离散度，不代表跨机器可比：

```sh
cargo run --locked --release --bin qbench -- --json --iterations 100 --repeat 11
```

与官方 QuickJS 的同机 CLI 对比使用独立的 `qcompare.v1` schema，不和上述进程内 `qbench.v1` 字段混用。先构建全部 CLI，再由调用者显式提供 QuickJS 路径：

```sh
cargo build --locked --release --bins
target/release/qbench --compare-qjs /path/to/qjs --compare-iterations 1 --repeat 11 --json
```

当前比较集合包含双方可等价表达并校验结果的标量循环、函数调用、数组构造/索引/遍历、固定自身键映射读取、不可变映射更新、Unicode 标量遍历与标量索引。既有 `quickcoffee_cli_ns` 与 `quickjs_cli_ns` 继续表示包含启动、解析、编译和执行的端到端子进程总耗时，不由其他阶段相减得出。

RFC 0124 在同一 `qcompare.v1` 记录中追加独立阶段：`quickcoffee_startup_ns` / `quickjs_startup_ns` 通过各自 `--quit` 测进程启动；`*_compile_ns` 在进程内重复编译函数或程序；`*_hot_ns` 复用已编译函数或已验证 `Program` 与运行上下文，并在每次执行后校验最终值。每个阶段都是一轮 `--compare-iterations` 的总耗时，并配有对 `--repeat` 样本计算的 `*_mad_ns`。QuickJS 阶段由 `qjs --std` 内的 `os.now()` 自计时，因此字段使用纳秒单位不代表底层时钟具有纳秒分辨率。它用于量级判断和回归观察，不是 JavaScript 兼容性声明。

### RFC 0120 / issue #79 集合与 Unicode 对照扩展

新增五条记录保持稳定名称和期望值：

| 负载 | 主要操作 | 期望值 | 等价边界 |
|---|---|---:|---|
| `array-build-index-iterate` | 单数组构造和下标循环 | 1498500 | 两端各构造 `0..999` 一份数组，并以相同的显式下标循环读取 |
| `map-own-lookup` | 固定自身键重复读取 | 100000 | 只读字面量自身键，不依赖原型 |
| `map-functional-update` | spread 后覆盖一个键 | 507502 | 两端每轮都产生新映射/对象，不使用原地更新 |
| `unicode-scalar-iterate` | Unicode 标量遍历与下标绑定 | 15000 | JavaScript 使用字符串 `for...of`，两端都按标量枚举 |
| `unicode-scalar-index` | 重复 Unicode 标量索引 | 22000 | QuickJS 在计时函数内先用 `Array.from` 解码标量；这是显式语义适配，不代表底层索引同构 |

2026-08-24 在同一 Apple arm64（T6000、Darwin 25.5.0）、`rustc 1.94.0` release、官方 QuickJS 2026-06-04 上运行 `--compare-iterations 1 --repeat 11 --json`。下表为预编译热执行中位数与 MAD；完整启动、编译、热执行和 CLI 数据保存在 #79，避免把持续变化的阶段数据复制成多个规划源：

| 负载 | QuickCoffee hot ms（MAD） | QuickJS hot ms（MAD） | 比例 | 审阅目标 |
|---|---:|---:|---:|---|
| `scalar-loop` | 514.995（1.629） | 14.108（0.087） | 36.50× | 未达 10× |
| `function-loop` | 203.200（0.970） | 5.732（0.021） | 35.45× | 观测项 |
| `array-build-index-iterate` | 0.649（0.006） | 0.069（0.002） | 9.41× | 达到 20× |
| `map-own-lookup` | 9.269（0.114） | 0.321（0.005） | 28.87× | 未达 20× |
| `map-functional-update` | 1.617（0.030） | 0.171（0.002） | 9.46× | 达到 20× |
| `unicode-scalar-iterate` | 3.770（0.033） | 0.962（0.011） | 3.92× | 达到 20× |
| `unicode-scalar-index` | 10.529（0.049） | 1.639（0.022） | 6.42× | 达到 20×（含上述适配） |

因此集合目标已经可以测量，但尚未整体达成；固定键映射读取是新增负载中最明确的热点。该结论只适用于同机同源码的当前实验，不把启动占优或 Unicode 适配解释为语言兼容或引擎总体性能。

### issue #95 当前环境槽位与借用式调度

2026-08-24 在同一机器、工具链与 QuickJS 上，以 `--compare-iterations 1 --repeat 11 --json` 对 issue #95 修改前后的 release 构建重新采样。修改包括稳定环境槽位、带完整名称守卫的当前环境指令提示、闭包/default chunk 执行计划传播，以及避免逐指令深拷贝的借用式调度。表中“相对改善”只比较 QuickCoffee 修改前后中位数；最后一列使用修改后同轮 QuickJS 中位数作为分母：

| 负载 | 修改前 QC hot ms | 修改后 QC hot ms（MAD） | 相对改善 | 修改后 QC / QJS |
|---|---:|---:|---:|---:|
| `scalar-loop` | 529.854 | 253.724（1.259） | 52.1% | 17.70× |
| `function-loop` | 207.821 | 143.947（0.078） | 30.7% | 24.89× |
| `array-build-index-iterate` | 0.676 | 0.316（0.001） | 53.3% | 5.01× |
| `map-own-lookup` | 9.388 | 4.185（0.004） | 55.4% | 12.92× |
| `map-functional-update` | 1.662 | 0.921（0.004） | 44.6% | 5.35× |
| `unicode-scalar-iterate` | 3.908 | 2.051（0.004） | 47.5% | 2.09× |
| `unicode-scalar-index` | 11.026 | 5.834（0.018） | 47.1% | 3.58× |

五条集合/Unicode 目标负载现已全部进入 `20×` 内，固定自身键映射读取不再越过集合门槛；但首要的标量循环仍未进入 `10×`，函数循环仍为 `24.89×`，所以 #80 不能据此关闭。`qbench` 的同机修改前后 11 样本还显示 `loop-core` 执行中位数从 `32.376 ms` 降至 `14.962 ms`（改善 53.8%），`closures-and-ranges` 从 `43.963 ms` 降至 `27.183 ms`（改善 38.2%）。内建环境模板按线程复用后，短程序反复创建 Context 的 `constant-folding`（200,000 次）为 `120.666 ms`，低于未修改 `main` 的 `349.676 ms`（改善 65.5%），避免用长循环收益掩盖启动型回退。

该优化仍保留父环境动态查找，尚未实现经 verifier 证明的静态 local/capture slot、Context relocation 或成员名 intern。`Program` 私有执行计划的构建发生在 `compile_program`，不包含在 `compile_ns`（其测量 `Engine::compile`）中；issue #102 后续增加的 `prepare_ns` 才覆盖完整 `compile_program` 准备路径。因此本节只对当时已列出的热执行与 Context 构造数据作结论。

### issue #100 编译器解析的隔离叶函数槽位

2026-08-24 在同一开发机上，以修改前 `main` 与 issue #100 的 release 二进制顺序执行 11 样本。第一阶段只把必填简单参数、直接局部赋值且不含调用、闭包创建、解构、迭代绑定或异常处理的隔离叶函数放入私有当前帧槽位；顶层、可逃逸函数、capture/global 名称和裸 `Chunk` 仍走既有路径。

进程内 `qbench --iterations 2000 --repeat 11` 的 `closures-and-ranges` 执行中位数从 `111.830 ms` 降至 `71.304 ms`（改善 **36.2%**，修改后 MAD `1.758 ms`）；每次执行的 656 条指令、246 次名称读取、52 次名称写入、49 次调用和 49 次环境分配计数保持一致。无函数调用的 `execution-stats` 从 `70.232 ms` 到 `68.471 ms`，2.5% 差异在本轮噪声范围内，没有显示新的标量热路径回退。

同轮 `qcompare --compare-iterations 1 --repeat 11` 中，`function-loop` 的 QuickCoffee hot 中位数从 `247.598 ms` 降至 `173.761 ms`（改善 **29.8%**），修改后为 QuickJS `10.381 ms` 的 **16.74×**。`scalar-loop` 的 QuickCoffee 中位数从 `325.427 ms` 到 `326.988 ms`（回退 0.48%），修改后同轮为 QuickJS 的 `9.31×`；五条集合/Unicode 负载的 QuickCoffee 中位数变化均不超过 2%。本轮标量比值进入 `10×`，但该变化不是 issue #100 带来的收益，不能用来替代后续跨轮稳定性验证，也不表示 #80 的 capture slot、成员名 intern 或 sidecar 成本工作已经完成。

### issue #102 prepared Program 成本门禁

`quickcoffee.qbench.v1` 追加可选的 `prepare_ns` / `prepare_mad_ns`，测量 `Engine::compile_program` 的端到端准备；既有 `compile_*` 继续只测 `Engine::compile`。准备阶段包含重复解析、mapped lowering、验证、源码映射与私有执行 sidecar，所以下表差值只表示嵌入方选择完整 `Program` 路径的总增量，不能全归因于 sidecar。

2026-08-24 在 Apple arm64、macOS 14.8.7、`rustc 1.97.1` 的 release 构建上，以 `--iterations 1000 --repeat 11` 得到：

| 负载 | compile ms（MAD） | prepare ms（MAD） | prepare / compile | 每个 Program 总增量 |
|---|---:|---:|---:|---:|
| `constant-folding` | 4.381（0.158） | 7.236（0.182） | 1.65× | 2.855 µs |
| `execution-stats` | 8.856（0.213） | 14.086（0.363） | 1.59× | 5.230 µs |
| `closures-and-ranges` | 12.506（1.267） | 23.263（2.692） | 1.86× | 10.757 µs |
| `for-pattern-bindings` | 13.092（0.348） | 20.477（0.582） | 1.56× | 7.385 µs |

这组数据建立报告门禁而不设置阻塞阈值；未来扩大 local/capture slot sidecar 时，必须同时检查 `prepare_*` 与 `execute_*`，性能告警的机器归一化与阈值仍由 #81 处理。

### issue #104 隔离叶函数空环境消除

issue #100 的快帧已经把当前局部值放入私有槽位，但仍为每次调用分配一个空的 `Rc<RefCell<Environment>>`。issue #104 只在所有静态槽位都于调用入口由必填参数初始化时，直接复用捕获环境作为非局部查找起点；带条件局部变量的快帧继续分配子环境，以保持 unset fallback。`ExecutionStats` 仍把每个函数调用计为一次逻辑 `environment_allocations`，所以既有 profile 可逐字段比较。

2026-08-24 在同一 Apple arm64 开发机上，以修改前 `main` 与最终 release 二进制执行 11 样本：

| 负载 | 修改前 QC hot/execute ms（MAD） | 修改后 ms（MAD） | 相对变化 | 修改后 QC / QJS |
|---|---:|---:|---:|---:|
| qbench `closures-and-ranges`（2,000 次） | 68.402（0.175） | 60.651（1.036） | **改善 11.3%** | — |
| qbench `execution-stats`（2,000 次） | 66.933（0.475） | 67.319（0.168） | 回退 0.58% | — |
| qcompare `function-loop` | 171.964（1.461） | 153.268（0.494） | **改善 10.9%** | **14.90×** |
| qcompare `scalar-loop` | 322.505（0.298） | 321.597（0.357） | 改善 0.28% | 9.24× |

同轮五条集合/Unicode QuickCoffee hot 中位数均未回退；额外使用相同 Rust 驱动分别链接修改前后库的 31 样本数组负载，也从约 `376.5 µs` 降至 `372.4 µs`。`execution-stats` 的 0.58% 差异低于修改前 MAD，四个代表负载的 `prepare_*` 没有超出离散度的变化。实现过程中曾发现未赋值快槽位回退的代码形状会触发 ThinLTO 集合路径回退，因此最终只消除入口槽位全部初始化的环境，并保持通用名称读取路径不变。

调试单一回归时，先用 `qbench --list` 查看确定性的内建负载名，再用 `qbench --only NAME` 只运行该负载；不指定 `--only` 始终运行完整集合，持续门禁口径不变。

`qbench` 为每个内建负载输出一行 JSON，分别记录普通编译、prepared Program、验证和执行的纳秒总耗时，并在计时循环中校验预期最终值。默认 `--repeat 1` 适合快速 CI 回归；需要正式三次 release 中位数时使用 `--repeat 3`，其结果可直接作为下文报告数据。

### issue #107 同 runner 非阻塞回归报告

`.github/workflows/performance.yml` 在 pull request 与手工触发时，把 base 与 candidate 分别检出并在同一个 Ubuntu runner、同一个 stable Rust 工具链上顺序构建。candidate 不得删除 base 已有的 qbench 负载；新增负载会单独验证并保存，但不伪造 base 对照。

报告控制脚本单独从触发 workflow 的 revision 检出，不从被测 candidate revision 读取；因此手工指定尚未包含新版比较器的历史 base/candidate commit 时，仍使用当前配对 schema 和策略完成校准，而两个被测 qbench 二进制保持来自显式 refs。

issue #116 把每个共有负载的采集从单一方向升级为 ABBA 或 BAAB：同一对 release 二进制各运行两次 `--only NAME --json --iterations 2000 --repeat 11`，形成一组 base→candidate 与一组 candidate→base。`quickcoffee.qbench-ordered.v1` JSONL wrapper 为每次原始 qbench 记录保留全局 `sequence`、`pair_id` 与 `side`；分 side 的原始 runs、candidate-only runs、负载列表和 metadata 也一并保存。

issue #119 在改动公开 `Value::Map` 或内部成员键表示之前加入 `member-lookup-loop`：四键自身映射执行 100 轮固定 `.alpha` / `.beta` / `.gamma` / `.delta` 读取，结果必须为 `1000`，单次 profile 必须报告 400 次 `Member` container 操作；映射字面量会常量折叠，不在执行 profile 中产生 `MakeMap`。它同时进入 qbench 与 `cargo bench --bench core`，为后续 symbol/member intern 切片提供独立 same-runner 护栏。

issue #147 为 RFC 0134 的首个 class 阶段加入 `class-construction-dispatch`：100 轮分别构造带接收者字段的 `Counter` 并调用一次实例方法，结果必须为 `5050`。单次 profile 锁定 200 次调用（构造器与方法各 100）、301 次 class/instance 容器操作、303 次托管值分配事件和 200 个调用环境事件；负载同时进入 qbench 与 `cargo bench --bench core`，后续 class dispatch、字段槽位和继承查找优化必须保留语义结果并解释这些事件变化。

`scripts/qbench_compare.py` 要求每个负载恰好包含一组 AB 与一组 BA、序号连续、pair 完整且迭代数/样本数/期望值一致。每个 phase 先分别计算两组 candidate-base 效应，再以配对中位数作为审阅效应。只有聚合效应以及 AB、BA 两个方向各自都超过 `max(5% × baseline, 3 × (base MAD + candidate MAD), 0.1 ms)` 才产生 warning；配对差值 MAD、AB/BA 全局中位数、未配对 side 中位数及正向负载数用于诊断，不从单项结果中扣除，因此不会隐藏真实的全局回退。

绝对下限避免把亚毫秒 verify 等阶段的几十微秒 runner 抖动放大成告警；它不替代相对与 MAD 门槛。warning **不阻塞** PR；workflow job 本身也设置 `continue-on-error`，因为共享 runner 数据仍用于建立噪声模型，不能当成稳定发布阈值。解析错误、删除既有负载或配对契约失配仍让该实验 job 显式报错，避免把缺失数据当成“无回归”。版本化 ordered JSONL、两边原始 runs、candidate-only 记录、paired comparison JSON，以及包含 base/head revision、UTC 时间、runner、平台、Python、`rustc -Vv` 和实际命令的 metadata JSON 保留 30 天；Markdown step summary 展示告警、全部 phase 明细和 compile/execute 顺序偏差摘要。跨 runner、跨平台或历史 artifact 不直接套用这个阈值；在 A/A 校准不再显示持久方向偏差前，报告继续保持非阻断。

比较器只依赖 Python 标准库；本地复核可运行：

```sh
PYTHONDONTWRITEBYTECODE=1 python3 scripts/test_qbench_compare.py
python3 scripts/qbench_compare.py \
  --baseline /path/to/baseline.jsonl \
  --candidate /path/to/candidate.jsonl \
  --summary /tmp/qbench-summary.md \
  --report /tmp/qbench-comparison.json \
  --metadata /tmp/qbench-metadata.json

python3 scripts/qbench_compare.py \
  --ordered /path/to/ordered.jsonl \
  --summary /tmp/qbench-paired-summary.md \
  --report /tmp/qbench-paired-comparison.json \
  --metadata /tmp/qbench-paired-metadata.json
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

stepped-string-iteration（20,000 次）：

```coffee
sum = 0
for character, index in 'a☕中x' by 2 then sum += index
sum
```

signed-by-iteration（20,000 次）：

```coffee
sum = 0
for n, index in [1...100] by -3 then sum += n + index
sum
```

该负载验证 RFC 0100 的反向数组步进与实际下标绑定；期望值为 `3333`。它与正向 `stepped-iteration` 负载并列，避免性能报告只覆盖顺序遍历。

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
| stepped-string-iteration | 69.630 ms | 287,232 programs/s | 49.288 ms | 405,780 programs/s |
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

## RFC 0061 增量：普通字符串转义

本轮加入 `string-escapes` 负载，语义护栏要求转义后的 `ABC` 长度与比较结果合计为 `4`。在集成验证缓存后的三轮 release 运行中，原始样本（单位为 ms）为编译 `69.111 / 69.670 / 69.473`、验证 `3.561 / 3.539 / 3.478`、执行 `27.486 / 27.628 / 27.460`；中位数及吞吐如下：

| 工作负载 | 编译中位数 | 编译吞吐 | 验证中位数 | 验证吞吐 | 执行中位数 | 执行吞吐 |
|---|---:|---:|---:|---:|---:|---:|
| string-escapes (20,000 次) | 69.473 ms | 287,880 programs/s | 3.539 ms | 5,651,181 programs/s | 27.486 ms | 727,652 programs/s |

```coffee
message = "A\x42\u{43}"
len(message) + (if message == 'ABC' then 1 else 0)
```

三轮完整基准均通过语义护栏；非法转义、代理项和超出 Unicode 标量范围的输入均在词法阶段拒绝。

## RFC 0070 增量：字符串 Unicode 标量迭代

本轮加入 `string-iteration` 负载，语义护栏要求对 `'a☕中'` 的标量下标求和为 `3`。三轮 release 运行的原始样本（单位为 ms）为编译 `75.840 / 65.714 / 66.620`、验证 `3.549 / 3.344 / 3.589`、执行 `62.512 / 55.202 / 56.156`；中位数及吞吐如下：

| 工作负载 | 编译中位数 | 编译吞吐 | 验证中位数 | 验证吞吐 | 执行中位数 | 执行吞吐 |
|---|---:|---:|---:|---:|---:|---:|
| string-iteration (20,000 次) | 66.620 ms | 300,209 programs/s | 3.549 ms | 5,635,390 programs/s | 56.156 ms | 356,153 programs/s |

```coffee
sum = 0
for character, index in 'a☕中' then sum += index
sum
```

字符串工作负载按 Unicode 标量构造单字符值，不拆分 UTF-8 字节；`by` 仍只适用于数组。三轮完整基准均通过语义护栏。

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

## RFC 0065 增量：受约束的隐式调用

本轮加入 `implicit-calls` 负载，语义护栏要求 `add 20, 22` 的结果为 `42`。修正边界后的三轮 release 运行原始样本（编译 / 执行，单位为 ms）为 `72.198 / 70.738 / 71.563` 与 `42.193 / 40.584 / 40.796`；中位数及吞吐如下：

| 工作负载 | 编译中位数 | 编译吞吐 | 执行中位数 | 执行吞吐 |
|---|---:|---:|---:|---:|
| implicit-calls (20,000 次) | 71.563 ms | 279,473 programs/s | 40.796 ms | 490,246 programs/s |

```coffee
add = (left, right) -> left + right
answer = add 20, 22
answer
```

三轮完整基准均通过语义护栏；显式括号调用与隐式调用共享同一字节码调用路径。

## RFC 0066 增量：嵌入执行统计

本轮加入 `execution-stats` 负载，并在语义护栏中读取 `Context::last_execution()`，确认公开统计不会改变字节码执行结果。三轮 release 运行的原始样本（编译 / 执行，单位为 ms）为 `41.927 / 41.812 / 42.088` 与 `490.755 / 491.043 / 489.478`；中位数及吞吐如下：

| 工作负载 | 编译中位数 | 编译吞吐 | 执行中位数 | 执行吞吐 |
|---|---:|---:|---:|---:|
| execution-stats (10,000 次) | 41.927 ms | 238,511 programs/s | 490.755 ms | 20,377 programs/s |

```coffee
sum = 0
i = 0
while i < 100
  sum += i
  i++
sum
```

三轮完整基准均通过语义护栏；统计仅记录已尝试的指令数与停止时余下 fuel，编译或验证错误不会伪造执行记录。

## RFC 0069 增量：共享 Program 验证缓存

本轮将 `Program` 的递归验证状态与不可变字节码一同缓存：`compile_program` 创建时验证一次，重复 `run_program` 不再重扫验证图；公开 `Program::from(Chunk)` 仍在首次运行时验证。基准新增独立 `verify` 阶段，且执行阶段使用缓存路径。三轮 release 运行的原始样本（单位为 ms）如下：

| 工作负载 | 验证三样本 | 验证中位数 | 执行三样本 | 执行中位数 |
|---|---|---:|---|---:|
| implicit-calls (20,000 次) | 3.441 / 3.325 / 3.830 | 3.441 | 36.890 / 37.437 / 37.510 | 37.437 |
| execution-stats (10,000 次) | 2.194 / 2.245 / 2.384 | 2.245 | 488.079 / 487.269 / 485.315 | 487.269 |

`implicit-calls` 的缓存执行中位数为 `37.437 ms`（534,225 programs/s），相较 RFC0065 基线 `40.796 ms`（490,246 programs/s）改善约 8.2%；所有完整基准均通过语义护栏。验证缓存不复制指令流，也不共享 VM fuel、调用帧或脚本全局状态。

## RFC 0071 增量：数组模式 rest

本轮加入 `destructuring-rest` 负载，语义护栏要求每轮 `[head, tail...] = [1, 2, 3, 4]` 的 `head + len(tail)` 累计为 `400`。三轮 release 运行的原始样本（单位为 ms）为编译 `64.163 / 64.219 / 63.945`、验证 `2.352 / 2.477 / 2.376`、执行 `979.340 / 968.432 / 961.712`；中位数及吞吐如下：

| 工作负载 | 编译中位数 | 编译吞吐 | 验证中位数 | 验证吞吐 | 执行中位数 | 执行吞吐 |
|---|---:|---:|---:|---:|---:|---:|
| destructuring-rest (10,000 次) | 64.163 ms | 155,853 programs/s | 2.376 ms | 4,208,384 programs/s | 968.432 ms | 10,326 programs/s |

```coffee
sum = 0
i = 0
while i < 100
  [head, tail...] = [1, 2, 3, 4]
  sum += head + len(tail)
  i += 1
sum
```

rest 绑定会复制剩余元素到新的不可变数组，以保持宿主存储不泄漏；模式验证失败时不会写入部分名称。三轮完整基准均通过语义护栏。

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
| stepped-string-iteration | 69.630 / 68.868 / 72.102 | 49.175 / 50.030 / 49.288 |
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

## RFC 0113/0116 数值标准库

标准库数值路径由四个负载覆盖：`stdlib-abs` 测量单值绝对值，`stdlib-sum` 测量小数组聚合，`stdlib-min-max` 测量严格最小/最大值，`stdlib-range-sum` 测量 `range` 与 `sum` 的组合。四者同时存在于 `qbench --json` 与 `cargo bench --bench core`，并检查最终值 `42`、`10`、`4`、`4950`。

复现机器可读记录：

```sh
cargo run --locked --release --bin qbench -- --json --only stdlib-sum --iterations 100 --repeat 3
```

复现完整 release 负载与持续门禁：

```sh
make qbench-check
make bench
```

这些负载只用于同一实现、同一环境的回归跟踪；报告不设跨机器硬时间阈值，比较时应记录 `rustc -Vv`、机器、操作系统、迭代次数和重复次数。

本轮 Apple arm64 release 基准样本（`cargo bench --locked --bench core`，单位 ms；仅作仓库内回归锚点）：

| 工作负载 | 编译 | 验证 | 执行 |
|---|---:|---:|---:|
| stdlib-abs（20,000 次） | 23.700 | 1.428 | 32.712 |
| stdlib-sum（20,000 次） | 34.830 | 1.457 | 33.823 |
| stdlib-min-max（20,000 次） | 56.280 | 1.849 | 37.775 |
| stdlib-range-sum（10,000 次） | 17.852 | 0.896 | 30.155 |

## RFC 0076 负索引

负索引在数组上做一次长度归一化，在字符串上按 Unicode 标量计数后定位；两者均保持越界错误，不复制序列。`negative-indexing` workload（20,000 次）单次样本为：编译 75.609 ms，验证 2.750 ms，执行 33.463 ms。

## RFC 0100 有符号步长

`qbench --json` 现在包含 `signed-by-iteration`，并在每轮编译、验证和执行后检查 `3333`。该负载不把数组复制到宿主侧，反向位置由 VM 的有符号步长直接推进；运行基准时应与 `stepped-iteration` 一起比较编译、验证和执行三个阶段。

本次可复现实测（Apple arm64、Darwin 25.5.0、`rustc 1.94.0`，release，命令 `cargo run --locked --release --bin qbench -- --json --iterations 100`）得到 `signed-by-iteration` 一条记录：编译总计 `618750 ns`，验证总计 `28917 ns`，执行总计 `2363209 ns`，期望值为 `3333`。这是单次开发机样本，只用于确认工作负载已纳入语义护栏与性能采集；跨版本比较仍须按本报告口径重复至少三次并取中位数。

标准 `cargo bench --locked --bench core`（10,000 次）同样已纳入该负载；本次样本为编译 `38.544 ms`、验证 `2.032 ms`、执行 `157.776 ms`，期望值 `3333`。它与正向 `stepped-iteration` 的执行样本（`124.616 ms`）同场输出，便于观察有符号步进的额外边界检查成本。

## RFC 0074 映射展开

在同一 Darwin arm64 开发机上，`cargo bench --bench core` 的 `map-spread` workload（20,000 次）单次样本为：编译 91.995 ms，验证 2.767 ms，执行 48.784 ms。映射展开为每个显式项生成单项映射，再由 `MergeMaps` 合并；后续键覆盖前值。

解释器使用显式调用帧和 fuel 计数；环境以稳定插入槽位保存值并以有序映射维护名称索引，编译 `Program` 对当前环境的名称读写使用带完整名称守卫的私有提示，并对一组经保守判定的隔离叶函数使用当前帧局部槽位。入口槽位全部初始化的快帧还会省去物理子环境，父环境仍动态查找。数组与映射值以不可变 `Rc` 容器分享。切片为保持独立不可变值而复制所选元素，故其热路径吞吐低于纯索引。这换取了 API 简洁与可验证性，尚未进行静态 capture slot、通用可逃逸 local slot、成员名 intern、寄存器分配或垃圾回收优化。后续优化必须保持 RFC 0002 的字节码验证与 fuel 语义，并更新本报告。

## RFC 0123 托管分配剖析

`qbench` 的每条 `quickcoffee.qbench.v1` 记录现在追加一次不计时执行的 `profile_*` 字段。热点计数来自 RFC 0122；`profile_value_allocations` 记录新建的引用计数字符串、数组、映射与字节码函数后备存储，`profile_environment_allocations` 记录 QuickCoffee 函数调用环境。它们是确定性事件数，不是字节数、存活对象数或系统分配器调用次数，也不包含编译期常量和嵌入宿主回调内部的分配。

可用一个迭代快速复现计时和单次执行剖析；调整 `--iterations` 或 `--repeat` 只改变计时样本，不会放大 `profile_*`：

```sh
cargo run --locked --release --bin qbench -- --only closures-and-ranges --json --iterations 1
```

在本 RFC 的语义门禁运行中，`loop-core` 的单次执行为 `1117` 条指令、`203` 次查名、`102` 次存名且无托管值/环境分配；`closures-and-ranges` 为 `656` 条指令、`49` 次调用、`2` 次托管值分配和 `49` 次环境分配；`stepped-string-iteration` 为 `25` 条指令和 `5` 次托管值分配。这些确定性计数说明纯标量循环首先受名称访问影响，闭包负载同时受调用环境分配影响，字符串迭代会为标量快照创建后备存储，可作为后续局部槽位、intern 与迭代器优化的对照。
