# QuickCoffee

> 实验性 0.1 · Rust 1.85+ · MIT OR Apache-2.0

QuickCoffee 是一台以 Rust 编写、受 CoffeeScript 启发的紧凑字节码脚本引擎。它适合把**确定性的业务规则、校验、数据整形和受限插件逻辑**嵌入应用；也提供一个单文件 CLI，便于直接运行和检查脚本。

它不是 JavaScript 运行时，也不试图兼容浏览器、Node.js 或 CoffeeScript 的全部历史行为。没有公开原型链、全局或自由 `this`、`eval`、反引号 JavaScript，以及隐式文件、网络或时钟权限。class 本身是完整的受限语言能力：支持 `new`、`extends`、`super`、实例/静态方法及 class 内 `this`/`@`，但这些接收者能力不会泄露到 class 外。

## 快速开始

从源码试用需要 Rust 1.85 或更新版本：

```sh
git clone https://github.com/coffee-js/quickcoffee.git
cd quickcoffee
cargo run -- -e "print(range(1, 4))"
```

要把 CLI 安装到本机：

```sh
cargo install --path .
qcoffee --version
```

正式版本也会提供不要求本地 Rust 工具链的 Linux x86_64、macOS x86_64 和 Windows x86_64 归档。每个归档包含四个 CLI、README、更新日志与双许可证，并与按文件名稳定排序的 `SHA256SUMS` 一起发布；下载、校验和维护者发布流程见[发布与平台归档](docs/releasing.md)。Release archives and checksum verification are documented bilingually in the same guide.

创建 `invoice.coffee`：

```coffee
discount = (amount) ->
  if amount >= 100 then amount * 0.9 else amount

print discount(120)
```

运行它：

```sh
qcoffee invoice.coffee
# 108
```

class 使用 CoffeeScript 风格的缩进成员体，同时保持接收者边界：

```coffee
class Counter
  constructor: (@value = 0) ->
  increment: ->
    @value = @value + 1
    @value

counter = new Counter()
print counter.increment()
```

## 你可以用它做什么

| 场景 | 当前状态 | 说明 |
|---|---|---|
| 规则计算、定价、资格校验 | 适合 | 严格数值、函数、异常、`switch` 和 fuel 都已具备。 |
| 配置归并、表单/事件数据整形 | 适合 | 有不可变数组/Map、spread、解构、推导、Unicode 字符串和精确 JSON。 |
| class 形式的业务模型 | 适合 | 支持构造、继承、覆盖、`super` 和安全逸出的 receiver-bound `=>`。 |
| 受控嵌入式策略/插件 | 条件适合 | 宿主可注入全局值/原生函数，并设置 fuel、调用深度、数据资源限制和取消。 |
| 多文件 CLI 应用 | 受限适合 | 使用显式 `--module-root ROOT ENTRY` 运行静态 `.coffee` / `.litcoffee` 模块图，并可非执行地生成依赖敏感图指纹；嵌入宿主还可显式复用内存模块包。 |
| HTTP、文件 I/O、异步任务、定时调度 | 尚不适合 | 语言没有隐式环境能力、事件循环或异步语法；这些应由宿主以明确 capability 提供。 |
| 直接替换 JavaScript/CoffeeScript 项目 | 不适合 | 语义刻意不同，且缺少正则、日期时间、字节/流、生成器等能力。 |

完整的业务适用性、边界和规划请看[业务就绪度评估](docs/readiness.zh-CN.md)。CoffeeScript 1.12.7 的逐项“实现 / 改写 / 拒绝”对照在[特性矩阵](docs/coffeescript-2016-matrix.md)。

## 当前语言与运行时

QuickCoffee 已提供：

- 严格的 Bool 条件与数值运算；`Number`、任意精度 `Integer` 与精确 `Decimal` 彼此分型，转换必须显式。
- 数组、无原型 Map、spread、严格递归解构、范围、切片、列表推导，以及 Unicode 标量级字符串索引和遍历。
- 函数、默认参数、rest 参数、闭包、`try` / `catch` / `finally`、`throw`、`return`、循环与 `switch`。
- JSON 编解码、稳定标量排序、不可变 `map_set` / `map_delete`、`trim` / `contains` / `starts_with` / `replace_all` 等确定性标准库函数；JSON 保留 Integer/Decimal 精度。
- 受限 class：`constructor`、实例/静态方法、`new`、私有继承链、静态解析的 `super`，以及只在合法 class 成员内可用的 `this`、`@` 和 `=>`。
- 编译检查、结构化诊断、字节码反汇编/指纹，以及带隔离 Context 和有界共享编译缓存的 `Runtime` 嵌入 API。

请把这些差异当作语言设计，而不是待补的 JavaScript 兼容性：没有隐式类型转换、`undefined`、公开 `prototype` / `__proto__`、任意函数构造、自由 `this` 或 `eval`。class 外使用 `this`、`super` 或 receiver-bound `=>` 是编译错误。

完整语法和标准库边界见[中文语法索引](docs/syntax.zh-CN.md)与[English syntax index](docs/syntax.en.md)。

## CLI 速查

| 目的 | 命令 |
|---|---|
| 执行表达式 | `qcoffee -e "print(1 + 2)"` |
| 执行文件并传参 | `qcoffee script.coffee -- first second`（脚本中读取 `argv`） |
| 从标准输入执行 | `qcoffee - < script.coffee` |
| 运行受限模块图 | `qcoffee --module-root modules app/main -- first` |
| 检查模块图指纹 | `qcoffee --fingerprint --module-root modules app/main` |
| 运行隔离模块测试 | `qtest --module-root examples/pricing test` |
| 规范化 JSON 文件 | `cargo run --example normalization -- examples/normalization/input.v1.json` |
| 持久交互会话 | `qcoffee --interactive`（`:help`、`:quit`） |
| 只检查、不执行 | `qcoffee --check script.coffee` |
| 稳定 JSON 输出 | `qcoffee --json script.coffee` |
| 限制本次执行 fuel | `qcoffee --fuel 100000 script.coffee` |
| 限制源码与字节码 | `qcoffee --max-source-bytes 1000000 --max-bytecode-instructions 1000000 script.coffee` |
| 限制模块图 | `qcoffee --max-module-graph-modules 1024 --max-module-graph-source-bytes 16000000 --module-root modules app/main` |
| 检查编译结果 | `qcoffee --dump-bytecode script.coffee` 或 `qcoffee --fingerprint script.coffee` |

`.coffee` 是普通 QuickCoffee 源码的规范扩展名；`.litcoffee` 使用 GitHub 原生支持的 Literate CoffeeScript 形式：Markdown 正文保持未缩进，技术标识使用反引号行内代码，可执行代码块统一缩进四个空格并与正文留出空行。`` ```coffee `` 围栏只适用于生成后的普通 Markdown，不会成为 `.litcoffee` 的可执行代码。命名编译、执行、检查、模块加载和 `qtest` 都会自动识别 `.litcoffee`；`qdocco` 只接受 `.litcoffee` 并生成带版本标记的文档，`--incremental` 会在最终产物字节不变时保留已有文件。`qbench` 用于可重复的基准和 QuickJS 同机对照。这些都是项目工具，不是部署时的必需组件。

`qtest --module-root ROOT ENTRY...` 显式授予一个受限模块根，把每个规范化入口预检为内存 `ModulePackage` 后在隔离 Context 中运行；入口必须 `export test = true`。该模式复用 fuel、每 case timeout、filter/list、stats、JSON、TAP 与 JUnit 契约，普通文件模式仍不获得模块权限。

## 嵌入 Rust 应用

最小嵌入可以直接创建一个 `Context`；需要让多个隔离 Context 复用编译产物时，由一个 `Runtime` 统一创建：

```rust
use quickcoffee::{Error, Runtime};

fn evaluate_rule() -> Result<(), Error> {
    let runtime = Runtime::new();
    let value = runtime
        .context_builder()
        .fuel(100_000)
        .build()
        .eval_named("rules/discount.coffee", "amount = 120; amount * 0.9")?;
    println!("{value}");
    Ok(())
}
```

生产嵌入应显式设置合适的 `CompileLimits`、fuel、调用深度和 `ResourceLimits`，并按需要通过 `ContextBuilder` 提供 `CancellationToken`、global 与 native。`CompileLimits` 在预处理与 cache-key 复制前限制原始 source，在验证和执行前限制递归 bytecode，并让模块执行与图指纹共享唯一模块数及累计 source bytes 预算；模块执行会在任何脚本运行前预检完整静态图。contextual native 可通过 `NativeCallContext` 协作检查取消、扣减 fuel、记录分配遥测并访问类型化、脚本不可见的 Context-owned `HostState`。`HostCapabilities` 与 typed `CapabilityKey<T>` 进一步把 clock、random、logging、file、network 权限放进显式 allowlist；QuickCoffee 不提供这些系统能力的实现，callback 必须继续显式检查和记账。同步 callback 在调用线程内执行，panic 不会被 VM 捕获或保证回滚；Runtime/Context 因 `Rc` 保持 non-Send/non-Sync，只有 `CancellationToken` 可跨线程发出停止信号。`RuntimeBuilder` 分别限制共享 Program/Module 编译缓存条目；缓存只保存已验证编译产物，不共享 globals、模块 exports、host state、capabilities 或任何执行账本，且可用 `cache_stats()` 审计、用 `clear_compile_caches()` 清空。`IntoValue` / `TryFromValue` 可在不执行脚本且不做 Number/Integer/Decimal coercion 的前提下递归转换常用 Rust 标量、`Vec`、`BTreeMap` 与 `Option`。`Engine::fingerprint_module_graph` 只通过宿主 loader 加载并验证静态依赖图，不执行模块，可作为依赖敏感的缓存失效键；`MODULE_GRAPH_FINGERPRINT_VERSION` 标识其 canonical encoding 版本。`Context::retained_memory()` 可读取当前 Context global 可达托管图的确定性 logical object/byte 快照；它去重共享值和循环，但不是 RSS、峰值或硬内存限制。宿主若要保留可重复的观测高水位，应在业务边界显式调用 `sample_retained_memory()`，再读取 `retained_memory_high_water()`；该记录不扫描 VM 指令，且只代表已采样的逐项最大值。`ResourceLimits` 可分别设置 retained object/byte 的执行提交上限，以及默认关闭的每轮累计 transient managed object/byte 上限；后者也覆盖协作记账的 contextual native 和整张模块图，并在越界时保留失败统计、回滚脚本状态。累计分配预算仍不是 RSS 或逐时刻 live-memory 峰值。当前资源限制覆盖多项计算与数据边界，但**尚不是完整的总内存预算或隔离沙箱**；不可信代码仍需要由宿主承担进程隔离。可运行的完整示例见[嵌入示例](examples/embed.rs)；显式模块加载与图指纹见[模块示例](examples/modules.rs)。

完整的业务验收路径见[可执行定价规则](examples/pricing/rule.litcoffee)、[CLI 模块](examples/pricing/demo.coffee)、[`qtest` 模块用例](examples/pricing/test.coffee)与[复用模块包的 Rust 宿主](examples/pricing.rs)，四者共享同一份规则源码；运行 `qtest --module-root examples/pricing test` 即可执行隔离验收。

[JSON 规范化规则](examples/normalization/rule.litcoffee)同样由[固定 corpus](examples/normalization/input.v1.json)、[CLI 模块](examples/normalization/demo.coffee)、[`qtest` 用例](examples/normalization/test.coffee)和[显式文件 I/O 的 Rust 宿主](examples/normalization.rs)共享。它验证精确 Integer/Decimal、固定 Unicode trim、scalar sort、不可变 Map 更新、规范 JSON、结构化业务错误及 JSON/资源失败；运行 `qtest --module-root examples/normalization test` 可执行隔离验收。

`Context::with_live_memory_observation(LiveMemoryObservation::Checkpointed)` 允许宿主额外记录顶层、调用、迭代、异常处理与 contextual native 边界的 logical live managed-memory；报告给出最后快照、逐项高水位、相应检查点、样本数与成功/错误/资源/取消结果。它默认关闭，不扫描 VM roots，不改变资源限制或脚本语义，也不是 RSS、GC 或任意 instruction 间的峰值。

## 当前状态与已知缺口

QuickCoffee 的核心语言、class、精确数值、确定性 JSON、Unicode 基元、CLI 诊断和基础嵌入 API 已实现，并由 RFC 与测试锁定。项目持续针对 VM 分配、调用和局部变量路径做性能优化；最近的 class 调用优化显著减少了临时绑定方法对象。

但它仍处于实验性 0.1：

- CLI 已支持显式根目录的受限模块加载和非执行模块图指纹；嵌入宿主可显式构建内存模块包，但没有持久化 manifest。普通文件、stdin、`-e` 和 REPL 不会隐式获得模块/文件权限。
- 原始 source、递归 bytecode、静态模块数、累计模块 source 与每轮累计 transient managed allocation 已有独立上限，但尚无逐时刻 live managed-memory、cycle 回收或跨 Context 生命周期隔离；剩余工作由 [#76](https://github.com/coffee-js/quickcoffee/issues/76) 跟踪。capability allowlist 已可显式配置，但具体系统能力仍必须由宿主实现并授权。
- 没有异步/并发、正则、日期时间、字节与流 API、网络或文件标准库；I/O 类能力保持为宿主显式责任。
- 性能已建立可重复的本地与 Linux 配对报告，但尚不能宣称达到 QuickJS 的整体量级；结果会随负载、平台和宿主交互而变化。
- 语言和嵌入 API 会继续通过 RFC 演进；需要长期稳定接口的项目应先锁定版本并运行自己的语义与资源测试。

动态优先级与完成状态在 [路线图](ROADMAP.md)和 GitHub tracking issues 中维护：[#65](https://github.com/coffee-js/quickcoffee/issues/65)（业务语言就绪）、[#66](https://github.com/coffee-js/quickcoffee/issues/66)（VM 性能）、[#81](https://github.com/coffee-js/quickcoffee/issues/81)（CLI、发布和性能门禁）。

## 性能与质量

`qbench` 分别报告普通编译、`Program` 准备、验证与执行，并可记录指令、调用、容器操作和托管值分配。`qbench --compare-qjs /path/to/qjs --compare-iterations 1 --repeat 11 --json` 可在同一机器上将启动、编译、预编译热执行和 CLI 总时长分开比较。

这些数字用于本仓库的回归判断，不是跨机器或跨语言的通用排名。测量协议、历史证据和解释边界见[性能报告](PERFORMANCE.md)，最新优化进度见 [#66](https://github.com/coffee-js/quickcoffee/issues/66)。

前端、verifier 与 VM 执行另有独立的 cargo-fuzz 基线：`make fuzz-smoke` 使用固定 nightly、确定 seed、受审阅 seed corpus 与输入/资源边界运行三个 target；scheduled/manual workflow 还用同一 nightly 执行隔离的 Miri library smoke。`make dependency-audit` 通过 RustSec 审计根与 fuzz 两个 lockfile。nightly 不进入发布 crate 或每个 PR 的稳定工具链门禁；发现的 crash 必须最小化并转为普通回归测试。详见 [fuzz README](fuzz/README.md)。

项目禁止 `unsafe`。修改源码后可运行：

```sh
make check
```

该命令覆盖格式、debug/release 测试、示例、Clippy、公开 API 文档、crate 打包检查、确定性 qbench 护栏和全部可执行手册检查。

## 文档与规范

| 你想了解什么 | 入口 |
|---|---|
| 当前语法、标准库和 CLI 边界 | [中文语法索引](docs/syntax.zh-CN.md) · [English syntax index](docs/syntax.en.md) |
| 平台归档、校验和与发布门禁 | [发布与平台归档 / Releases](docs/releasing.md) |
| 业务适用范围、性能判断和未完成能力 | [业务就绪度评估](docs/readiness.zh-CN.md) |
| CoffeeScript 兼容性差异 | [特性矩阵](docs/coffeescript-2016-matrix.md) |
| class / `this` / `new` / `extends` / `super` 的安全边界 | [RFC 0134](RFCs/0134-class-receivers-and-inheritance.md) |
| 项目范围与不变设计原则 | [RFC 0000](RFCs/0000-project-scope.md) |
| 性能测量与历史基线 | [PERFORMANCE.md](PERFORMANCE.md) |
| 长期方向与 issue 入口 | [ROADMAP.md](ROADMAP.md) |
| 可执行语言手册 | [中文](docs/manual.zh-CN.md) · [English](docs/manual.en.md) |

[RFC 0000](RFCs/0000-project-scope.md) 至 [RFC 0159](RFCs/0159-resource-bounded-immutable-map-updates.md) 是当前已采纳的语义、字节码、嵌入 API 和工具契约；测试是这些契约的可执行验收。
