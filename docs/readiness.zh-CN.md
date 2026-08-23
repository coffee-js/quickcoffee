# QuickCoffee 0.2 前业务适用性与性能基线

本文件是 2026-08-23 在 `main`（RFC 0000–0118）上的工程评估，不改变既有语言语义，也不是跨语言兼容性声明。它把下一阶段的工作拆成可审阅、可测量的目标。

后续拆分由 [#65：语言业务适用性](https://github.com/coffee-js/quickcoffee/issues/65) 与 [#66：QuickJS 量级性能收敛](https://github.com/coffee-js/quickcoffee/issues/66) 跟踪。

## 结论

QuickCoffee 已适合**单文件、确定性、由宿主明确注入能力**的规则、校验、配置变换、数据整形和受限插件任务。其核心表达式、严格集合、函数/闭包、模式解构、列表推导、异常控制流、Unicode 字符串和资源边界已经足以承载这类业务逻辑。

它尚不适合独立承担多文件服务、长期作业编排或需要丰富通用库的应用：没有模块系统、包解析、异步/并发、正则、日期时间、JSON 编解码、文件/网络能力或宿主对象能力模型。这些不是遗漏的 JavaScript 兼容项；其中 I/O、时钟、随机和网络应保持为宿主显式注入的 capability，而非语言隐式全局。

性能方面，当前 VM 尚未达到与 QuickJS 相近的量级。相同机器上的纯数值热循环显示 QuickCoffee 约慢 **42 倍**；这足以把字节码热路径优化列为 0.2 的最高优先级，但不足以外推到字符串、集合、异常或真实宿主调用。

## 业务任务矩阵

| 任务类别 | 当前判断 | 依据与边界 |
|---|---|---|
| 规则计算、定价、资格校验 | 可用 | 严格数值/布尔、`if`/`switch`、函数、错误与 fuel 已具备；宿主负责输入输出。 |
| 配置归并、表单/事件整形 | 可用 | 不可变数组/Map、spread、严格解构、`for`/推导足够；大型输入仍缺容器长度与内存预算。 |
| 受限模板或策略插件 | 条件可用 | `Context` 可注入受控函数并有 fuel、深度与取消；尚缺 capability table、内存预算和稳定的上下文隔离模型。 |
| 多文件业务领域脚本 | 不可用 | `import`/`export`、模块图、加载器、循环依赖与缓存契约尚未定义。 |
| I/O、HTTP、任务调度、异步业务 | 不可用 | 不提供异步语法/事件循环；这些能力也不能作为隐式标准库加入。 |
| 文本处理/协议解析 | 条件可用 | 字符串与集合可处理基础场景；没有正则、JSON、字节序列或流式 API。 |
| 面向对象业务模型 | 条件可用 | `class` 是无原型工厂；可用 Map 与闭包表达数据和显式方法，但没有继承、`this` 或 `new`。 |

## 语言工作优先级

1. **模块与加载器（路线图 B）**：先写 RFC，定义静态 `import`/`export`、模块初始化、循环依赖、`ModuleLoader` 和 CLI 路径安全。它解除单文件限制，但不授予文件或网络权限。
2. **能力化标准库（路线图 E）**：先定 `RuntimeBuilder`/`ContextBuilder`、`TryFrom<Value>`/`IntoValue` 和 capability table；JSON、时间、随机、日志等应先以宿主能力形式进入，而非复制 JavaScript 全局。
3. **面向业务的数据文本基元**：在前两项稳定后，分别 RFC 化 JSON 兼容数据模型、正则或受限模式匹配、日期时间策略。每项必须说明确定性、资源上限和跨平台语义。
4. **前端诊断与特性矩阵（路线图 A）**：在增加语法前先给 CoffeeScript 2016 的每项特性标注“实现、改写或拒绝”，并提供文件名、范围与多错误诊断。

明确非目标仍是：JavaScript 原型链、`this`、隐式转换、`eval`、反引号 JavaScript 和为了兼容而引入的隐式宿主能力。

## QuickJS 对照基线

### 环境与命令

- Apple Silicon arm64，Darwin 25.5.0（T6000）
- QuickCoffee `0.1.0`，`rustc 1.94.0`，release 二进制
- 官方 QuickJS `2026-06-04`，Apple clang `21.0.0`，`-O2` 构建
- 两者均只执行无 I/O 的数值循环，并验证最终和为 `49,999,995,000,000`

QuickCoffee：

```sh
qcoffee --fuel 1000000000 -e 'sum = 0
i = 0
while i < 10000000
  sum += i
  i++
sum'
```

QuickJS：

```sh
qjs -e 'let sum = 0; for (let i = 0; i < 10000000; i++) sum += i; if (sum !== 49999995000000) throw Error()'
```

`/usr/bin/time -p` 的单次端到端读数分别为 **5.48 s** 与 **0.13 s**，即约 **42×**。QuickCoffee 的燃料必须显式提高；默认 1,000,000 fuel 会在该测试中按设计中止。该比值包含各自 CLI 启动、解析、编译与执行，且 QuickJS 的短读数受计时分辨率影响，故它是方向性基线，不是发布门槛。

同一构建下 `qbench --json --iterations 100 --repeat 5` 的 QuickCoffee 执行中位数还显示热路径集中在循环、值分派、查名和分配：`loop-core` 3.52 ms、`array-slices` 11.18 ms、`nested-destructuring` 23.46 ms、`return-cleanup` 184.30 ms（均为 100 次程序执行总计）。现有 `qbench` 对 QuickCoffee 自身回归有效，但不能直接与 QuickJS 比较，因为负载含有本语言特有语义和每次新建 `Context`。

官方 QuickJS 将自身定位为小型可嵌入的 ES2025 引擎，使用引用计数和 cycle removal；其功能范围远大于 QuickCoffee，因此只把它作为性能参照，不作为语义或 API 模板。[QuickJS 官方说明](https://bellard.org/quickjs/)

## 性能收敛计划

1. 新建独立的跨运行时基准 harness：固定 CPU/优先级、预热、至少 11 次样本与中位数，并区分 CLI 启动、解析编译和预编译程序热执行；只纳入双方可等价表达的标量循环、调用、数组和字符串负载。
2. 先剖析 QuickCoffee：记录指令数、每类指令、分配和查名；不得在没有火焰图/采样证据时选择优化。
3. 按路线图 C 先实现局部槽位和符号 intern，再评估紧凑编码与短指令。每一项保留未优化路径差分执行、字节码验证和 fuel 等价测试。
4. 首个可审阅目标不是承诺一次赶平，而是在同机 scalar-loop 热执行上降至 QuickJS 的 **10× 内**，集合负载降至 **20× 内**；达成后再以完整噪声模型决定是否把阈值作为 CI 告警。

在该 harness 和第一轮剖析完成前，不应宣称 QuickCoffee 已达到 QuickJS 量级。
