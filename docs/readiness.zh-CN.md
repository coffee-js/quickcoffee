# QuickCoffee 0.2 前业务适用性与性能基线

本文件是 2026-08-24 在 `main`（RFC 0000–0131）上的工程评估，不改变既有语言语义，也不是跨语言兼容性声明。动态优先级、验收状态和最新实测由 GitHub issues 维护。

后续拆分由 [#65：语言业务适用性](https://github.com/coffee-js/quickcoffee/issues/65) 与 [#66：QuickJS 量级性能收敛](https://github.com/coffee-js/quickcoffee/issues/66) 跟踪。

## 结论

QuickCoffee 已适合**单文件、确定性、由宿主明确注入能力**的规则、校验、配置变换、数据整形和受限插件任务。其核心表达式、严格集合、函数/闭包、模式解构、列表推导、异常控制流、Unicode 字符串和资源边界已经足以承载这类业务逻辑。

它尚不适合独立承担多文件服务、长期作业编排或需要丰富通用库的应用：嵌入 API 已有宿主控制的静态模块核心，但没有 CLI 路径策略、模块包、图指纹或循环初始化；异步/并发、正则、日期时间、JSON 编解码、文件/网络能力和宿主对象能力模型也尚未交付。这些不是遗漏的 JavaScript 兼容项；其中 I/O、时钟、随机和网络应保持为宿主显式注入的 capability，而非语言隐式全局。

性能方面，当前 VM 尚未达到与 QuickJS 相近的量级。同机 11 样本验证显示预编译标量与函数热循环约慢 **35–37 倍**；新增数组、不可变映射更新和 Unicode 负载已进入 `20×` 内，但固定键映射读取仍约慢 **28.9 倍**。启动已经相近，因此下一阶段仍应集中于 VM 查名、成员读取、调用与分配热路径。这些结果仍不足以外推到异常、较大数据集或真实宿主调用。

## 业务任务矩阵

| 任务类别 | 当前判断 | 依据与边界 |
|---|---|---|
| 规则计算、定价、资格校验 | 可用 | 严格数值/布尔、`if`/`switch`、函数、错误与 fuel 已具备；宿主负责输入输出。 |
| 配置归并、表单/事件整形 | 可用 | 不可变数组/Map、spread、严格解构、`for`/推导足够；大型输入仍缺容器长度与内存预算。 |
| 受限模板或策略插件 | 条件可用 | `Context` 可注入受控函数并有 fuel、深度与取消；尚缺 capability table、内存预算和稳定的上下文隔离模型。 |
| 多文件业务领域脚本 | 不可用 | 嵌入宿主已有静态模块图核心；CLI 加载、模块包、图指纹和初始化契约尚未定义。 |
| I/O、HTTP、任务调度、异步业务 | 不可用 | 不提供异步语法/事件循环；这些能力也不能作为隐式标准库加入。 |
| 文本处理/协议解析 | 条件可用 | 字符串与集合可处理基础场景；没有正则、JSON、字节序列或流式 API。 |
| 面向对象业务模型 | 条件可用 | `class` 是无原型工厂；可用 Map 与闭包表达数据和显式方法，但没有继承、`this` 或 `new`。 |

## 语言规划入口

- [#74](https://github.com/coffee-js/quickcoffee/issues/74)：CoffeeScript 2016 特性矩阵与源码范围诊断。
- [#75](https://github.com/coffee-js/quickcoffee/issues/75)：模块包、受限 CLI 加载与模块图指纹。
- [#76](https://github.com/coffee-js/quickcoffee/issues/76)：内存预算与运行时隔离。
- [#77](https://github.com/coffee-js/quickcoffee/issues/77)：嵌入 API 0.2 与显式宿主能力。
- [#78](https://github.com/coffee-js/quickcoffee/issues/78)：确定性的业务数据与文本基元。

明确非目标仍是：JavaScript 原型链、`this`、隐式转换、`eval`、反引号 JavaScript 和为了兼容而引入的隐式宿主能力。

## QuickJS 对照判断

RFC 0120–0124 已建立双方等价负载、语义护栏、11+ 样本中位数/MAD、VM 事件剖析以及启动/编译/预编译热执行阶段分离。测量协议与解释边界保存在 [PERFORMANCE.md](../PERFORMANCE.md)；最新机器、工具链、样本和完成状态保存在 [#66](https://github.com/coffee-js/quickcoffee/issues/66)。

2026-08-24 的验证在 Apple arm64 上使用官方 QuickJS 2026-06-04 与 release QuickCoffee：标量热执行约慢 `36.50×`，函数热执行约慢 `35.45×`；数组、不可变映射更新、Unicode 标量遍历/索引分别为 `9.41×`、`9.46×`、`3.92×` / `6.42×`，固定自身键映射读取为 `28.87×`。集合目标已由 [#79](https://github.com/coffee-js/quickcoffee/issues/79) 建立可重复测量，但尚未整体达成；完整阶段数据和交付状态仍只在 issue 中维护。局部槽位与符号 intern 由 [#80](https://github.com/coffee-js/quickcoffee/issues/80) 跟踪。

首个审阅目标保持为同机 scalar-loop 预编译热执行进入 QuickJS 的 `10×` 内、集合负载进入 `20×` 内。目标达成且噪声模型稳定前，不应宣称 QuickCoffee 已达到 QuickJS 量级。
