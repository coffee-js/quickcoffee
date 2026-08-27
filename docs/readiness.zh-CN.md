# QuickCoffee 0.2 前业务适用性与性能基线

本文件是 2026-08-28 在 RFC 0000–0155 上的工程评估，不改变既有语言语义，也不是跨语言兼容性声明。动态优先级、验收状态和最新实测由 GitHub issues 维护。

后续拆分由 [#65：语言业务适用性](https://github.com/coffee-js/quickcoffee/issues/65) 与 [#66：QuickJS 量级性能收敛](https://github.com/coffee-js/quickcoffee/issues/66) 跟踪。

## 结论

QuickCoffee 已适合**单文件、确定性、由宿主明确注入能力**的规则、校验、配置变换、数据整形和受限插件任务。其核心表达式、严格集合、函数/闭包、模式解构、列表推导、异常控制流、Unicode 字符串和资源边界已经足以承载这类业务逻辑。

它尚不适合独立承担多文件服务、长期作业编排或需要丰富通用库的应用：嵌入 API 已有宿主控制的静态模块核心，CLI 也只能通过显式 `--module-root ROOT ENTRY` 启用受限 `.coffee` / `.litcoffee` 文件图，模块图指纹已经交付，但模块包与预编译 manifest 尚未交付；异步/并发、正则、日期时间和内建网络实现仍缺失。脚本已有无隐式 I/O、精确 Integer/Decimal 映射和 Context 可配置资源守卫的 JSON 编解码，RFC 0139–0140 提供固定 Unicode 查询与稳定标量排序，RFC 0144–0150 再增加资源有界的不可变 String/Array 连接与字面量字符串替换。RFC 0154 提供 Context-owned typed capability allowlist，但 clock、random、logging、file、network 的具体实现仍完全由宿主提供并授权。一般 String/Array/Map 单值边界、确定性 retained census、高水位采样及 Context retained-state 提交限制已经交付；RFC 0155 又交付原始 source、递归 bytecode、静态模块数和累计模块 source 的独立边界，并在任何模块脚本运行前预检完整静态图。执行中 transient/live managed memory、单条 instruction/native callback 的宿主堆增长和跨 Context 生命周期预算仍由 #76 跟踪。这些不是遗漏的 JavaScript 兼容项；I/O、时钟、随机和网络保持为宿主显式注入的 capability，而非语言隐式全局。

性能方面，当前 VM 尚未达到与 QuickJS 相近的量级。issue #95 的稳定环境槽位、当前作用域名称提示与借用式调度曾把同机 11 样本的预编译标量热循环降至 QuickJS 的 **17.70×**，函数热循环降至 **24.89×**；五条集合/Unicode 目标负载全部进入 `20×` 内。issue #100 又为保守判定的隔离叶函数增加编译器解析的当前帧槽位，把当轮 `function-loop` 改善 **29.8%** 至 **16.74×**。issue #104 在入口槽位全部初始化时省去物理空环境，使后续同机函数循环再改善 **10.9%** 至 **14.90×**，而标量为 **9.24×**、集合/Unicode 负载未回退。issue #107 建立同一 CI runner 的原始 artifact 与 MAD 感知非阻塞 warning；issue #116 再以 ABBA/BAAB 配对测量消除固定 base/head 方向偏差，并明确 common-mode 只作诊断、不从真实广泛回退中扣除。该门禁仍需 A/A 与已知优化持续校准后才能升级为稳定阻塞阈值。也仍需继续处理符号 intern 与成员读取，不能宣称已经达到 QuickJS 量级。这些结果不足以外推到异常、较大数据集或真实宿主调用。

## 业务任务矩阵

| 任务类别 | 当前判断 | 依据与边界 |
|---|---|---|
| 规则计算、定价、资格校验 | 可用 | 严格数值/布尔、`if`/`switch`、函数、错误与 fuel 已具备；宿主负责输入输出。 |
| 配置归并、表单/事件整形 | 可用 | 不可变数组/Map、spread、严格解构、`for`/推导及资源有界 `concat` / `replace_all` 足够；source、bytecode、单值与 Context retained-state 已有上限，执行中 transient/live memory 仍未完整限制。 |
| 受限模板或策略插件 | 条件可用 | `Context` 可注入受控函数、typed capability allowlist，并有 fuel、深度、取消与 retained-state 提交限制；尚缺完整 live-memory 预算和跨线程隔离模型。 |
| 多文件业务领域脚本 | 条件可用 | 嵌入宿主与显式 `--module-root` CLI 已有模块数/累计 source 有界的静态图、执行前全图预检和非执行依赖图指纹；模块包、预编译 manifest 与更完整初始化策略仍缺。 |
| I/O、HTTP、任务调度、异步业务 | 不可用 | 不提供异步语法/事件循环；这些能力也不能作为隐式标准库加入。 |
| 文本处理/协议解析 | 条件可用 | RFC 0138 已提供确定性精确 JSON，RFC 0139 提供 locale-free trim 与严格子串查询，RFC 0144/0150 提供资源有界 String/Array 连接与字面量替换。仍没有受限模式匹配、字节序列、大小写映射或流式 API；完整 live-memory 预算继续由 #76 统一。 |
| 面向对象业务模型 | 条件可用 | RFC 0134 已完整交付缩进 class、构造器、实例/静态方法、class 内 `this`/`@`、`new`、私有继承链、静态解析 `super`、默认派生构造转发、receiver-bound `=>`、专用 Class/Instance 值与受限字段写入；绑定闭包可安全逸出，模块可显式传递或扩展 class。全局/自由 `this`、任意函数构造和公开原型能力仍明确禁止。 |

## 语言规划入口

- [#74](https://github.com/coffee-js/quickcoffee/issues/74)：CoffeeScript 2016 特性矩阵与源码范围诊断。
- [#75](https://github.com/coffee-js/quickcoffee/issues/75)：模块包、预编译 manifest 与后续模块生命周期。
- [#76](https://github.com/coffee-js/quickcoffee/issues/76)：内存预算与运行时隔离。
- [#77](https://github.com/coffee-js/quickcoffee/issues/77)：嵌入 API 0.2 与显式宿主能力。
- [#78](https://github.com/coffee-js/quickcoffee/issues/78)：确定性的业务数据与文本基元。
- [#125](https://github.com/coffee-js/quickcoffee/issues/125)：确定性、精确数值、资源有界的脚本 JSON。
- [#121](https://github.com/coffee-js/quickcoffee/issues/121)：CoffeeScript 风格 class 总体跟踪；#147、#149 与 #150 已完成构造、继承/`super` 及 receiver-bound `=>`。

明确非目标仍是：JavaScript 公开原型链、全局/自由 `this`、任意函数构造、隐式转换、`eval`、反引号 JavaScript 和为了兼容而引入的隐式宿主能力。RFC 0134 的 class 内接收者与私有继承链不属于这些全局能力。

## QuickJS 对照判断

RFC 0120–0124 已建立双方等价负载、语义护栏、11+ 样本中位数/MAD、VM 事件剖析以及启动/编译/预编译热执行阶段分离。测量协议与解释边界保存在 [PERFORMANCE.md](../PERFORMANCE.md)；最新机器、工具链、样本和完成状态保存在 [#66](https://github.com/coffee-js/quickcoffee/issues/66)。

2026-08-24 的历史验证在 Apple arm64 上使用官方 QuickJS 2026-06-04 与 release QuickCoffee。#95 时标量与函数热执行分别为 `17.70×`、`24.89×`；后续 slot、call、pattern 与 frame 优化在 #158 后的同机新鲜测量约为 scalar `6.9×`、function-loop `9.5×`，进入最初数量级目标。该结果不是最新 `main` 的跨平台 QuickJS 结论；测量协议由已完成的 [#79](https://github.com/coffee-js/quickcoffee/issues/79) 维护，是否开启新优化子项继续由 [#66](https://github.com/coffee-js/quickcoffee/issues/66) 在最新 profile 后决定。

首个同机审阅目标已经在一台 Apple arm64 环境上达到，但仍需对最新 `main` 重跑官方 QuickJS 对照，并继续用 Linux AB/BA 报告排除回归。跨平台证据与噪声模型稳定前，不应宣称 QuickCoffee 整体达到 QuickJS 量级。
