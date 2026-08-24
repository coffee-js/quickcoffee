# QuickCoffee 0.2 前业务适用性与性能基线

本文件是 2026-08-24 在 `main`（RFC 0000–0133）上的工程评估，不改变既有语言语义，也不是跨语言兼容性声明。动态优先级、验收状态和最新实测由 GitHub issues 维护。

后续拆分由 [#65：语言业务适用性](https://github.com/coffee-js/quickcoffee/issues/65) 与 [#66：QuickJS 量级性能收敛](https://github.com/coffee-js/quickcoffee/issues/66) 跟踪。

## 结论

QuickCoffee 已适合**单文件、确定性、由宿主明确注入能力**的规则、校验、配置变换、数据整形和受限插件任务。其核心表达式、严格集合、函数/闭包、模式解构、列表推导、异常控制流、Unicode 字符串和资源边界已经足以承载这类业务逻辑。

它尚不适合独立承担多文件服务、长期作业编排或需要丰富通用库的应用：嵌入 API 已有宿主控制的静态模块核心及显式根目录的受限 `.qc` 文件 loader，但没有 CLI 激活、模块包、图指纹或循环初始化；异步/并发、正则、日期时间、JSON 编解码、网络能力和宿主对象能力模型也尚未交付。这些不是遗漏的 JavaScript 兼容项；其中 I/O、时钟、随机和网络应保持为宿主显式注入的 capability，而非语言隐式全局。

性能方面，当前 VM 尚未达到与 QuickJS 相近的量级。issue #95 的稳定环境槽位、当前作用域名称提示与借用式调度曾把同机 11 样本的预编译标量热循环降至 QuickJS 的 **17.70×**，函数热循环降至 **24.89×**；五条集合/Unicode 目标负载全部进入 `20×` 内。issue #100 又为保守判定的隔离叶函数增加编译器解析的当前帧槽位：同机 `function-loop` 从 `247.598 ms` 降至 `173.761 ms`（改善 **29.8%**），为同轮 QuickJS 的 **16.74×**，而标量和集合负载没有显著变化。本轮标量为 `9.31×`，但这是基线跨轮变化而非该优化的收益；仍需继续处理通用 local/capture slot、调用与成员读取，并建立稳定的跨轮回归门禁，不能宣称已经达到 QuickJS 量级。这些结果也不足以外推到异常、较大数据集或真实宿主调用。

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

2026-08-24 的验证在 Apple arm64 上使用官方 QuickJS 2026-06-04 与 release QuickCoffee。issue [#95](https://github.com/coffee-js/quickcoffee/issues/95) 修改后的 11 样本中，标量与函数热执行分别为 `17.70×`、`24.89×`；数组、不可变映射更新、Unicode 标量遍历/索引分别为 `5.01×`、`5.35×`、`2.09×` / `3.58×`，固定自身键映射读取为 `12.92×`。集合 `20×` 目标现已达成，但首要 scalar `10×` 目标仍未达成；测量协议由已完成的 [#79](https://github.com/coffee-js/quickcoffee/issues/79) 维护，后续静态局部/捕获槽位、符号 intern 与调用优化继续由 [#80](https://github.com/coffee-js/quickcoffee/issues/80) 跟踪。

首个审阅目标保持为同机 scalar-loop 预编译热执行进入 QuickJS 的 `10×` 内、集合负载进入 `20×` 内。目标达成且噪声模型稳定前，不应宣称 QuickCoffee 已达到 QuickJS 量级。
