# QuickCoffee 0.2 前业务适用性与性能基线

本文件是 2026-08-28 在 RFC 0000–0157 上的工程评估，不改变既有语言语义，也不是跨语言兼容性声明。动态优先级、验收状态和最新实测由 GitHub issues 维护。

后续拆分由 [#65：语言业务适用性](https://github.com/coffee-js/quickcoffee/issues/65) 与 [#66：QuickJS 量级性能收敛](https://github.com/coffee-js/quickcoffee/issues/66) 跟踪。

## 结论

QuickCoffee 已适合**单文件、确定性、由宿主明确注入能力**的规则、校验、配置变换、数据整形和受限插件任务。其核心表达式、严格集合、函数/闭包、模式解构、列表推导、异常控制流、Unicode 字符串和资源边界已经足以承载这类业务逻辑。

它尚不适合独立承担多文件服务、长期作业编排或需要丰富通用库的应用：嵌入 API 已有宿主控制的静态模块核心与内存内预编译 package，CLI 也只能通过显式 `--module-root ROOT ENTRY` 启用受限 `.coffee` / `.litcoffee` 文件图；持久化 manifest、异步/并发、正则、日期时间和内建网络实现仍缺失。脚本已有无隐式 I/O、精确 Integer/Decimal 映射和 Context 可配置资源守卫的 JSON 编解码，RFC 0139–0140 提供固定 Unicode 查询与稳定标量排序，RFC 0144–0150 再增加资源有界的不可变 String/Array 连接与字面量字符串替换。RFC 0154 提供 Context-owned typed capability allowlist，但 clock、random、logging、file、network 的具体实现仍完全由宿主提供并授权。一般 String/Array/Map 单值边界、确定性 retained census、高水位采样及 Context retained-state 提交限制已经交付；RFC 0155 又交付原始 source、递归 bytecode、静态模块数和累计模块 source 的独立边界，并在任何模块脚本运行前预检完整静态图；RFC 0156 交付单轮累计 transient managed-allocation 预算。RFC 0157 已交付默认关闭的 checkpointed logical live managed-memory 观测：嵌入方显式启用后可取得顶层、调用、迭代、异常处理和 contextual native 边界的最后快照与逐项高水位；单条 instruction/native callback 的宿主堆增长和跨 Context 生命周期预算仍由 #76 / #217 跟踪。该观测不是 RSS、GC 或任意 instruction 间 peak。这些不是遗漏的 JavaScript 兼容项；I/O、时钟、随机和网络保持为宿主显式注入的 capability，而非语言隐式全局。

性能方面，2026-08-28 对最新 `main` `4cc4474` 的 31 样本 Apple arm64 同机复测（Rust 1.94.0、release `qbench`、官方 QuickJS 2026-06-04、`--compare-iterations 1 --repeat 31`）显示：预编译热执行相对 QuickJS 的标量、函数、数组、Map 自身成员读取、不可变 Map 更新和两条 Unicode 负载分别约为 `12.84×`、`14.19×`、`9.09×`、`13.16×`、`6.46×`、`3.12×`、`4.37×`。此前唯一越过集合/文本 `20×` 审阅线的 Map 读取已在该同机指标内回到线下。#214 最终由 PR #216 收敛：Linux paired run `33140511336` 覆盖 51 个 workload，原始产物为 0 aggregate alerts，common execute 配对中位数为 `-4.016%`。issue #107/#116 建立的 Linux 同 runner 配对门禁仍是合并前的跨平台护栏，warning 也仍是非阻塞证据；这些结果不能外推到异常、较大数据集或真实宿主调用，更不表示 VM 整体达到 QuickJS 性能。

## 业务任务矩阵

| 任务类别 | 当前判断 | 依据与边界 |
|---|---|---|
| 规则计算、定价、资格校验 | 可用 | 严格数值/布尔、`if`/`switch`、函数、错误与 fuel 已具备；宿主负责输入输出。 |
| 配置归并、表单/事件整形 | 可用 | 不可变数组/Map、spread、严格解构、`for`/推导及资源有界 `concat` / `replace_all` 足够；source、bytecode、单值与 Context retained-state 已有上限，执行中 transient/live memory 仍未完整限制。 |
| 受限模板或策略插件 | 条件可用 | `Context` 可注入受控函数、typed capability allowlist，并有 fuel、深度、取消与 retained-state 提交限制；尚缺完整 live-memory 预算和跨线程隔离模型。 |
| 多文件业务领域脚本 | 条件可用 | 嵌入宿主与显式 `--module-root` CLI 已有模块数/累计 source 有界的静态图、执行前全图预检和非执行依赖图指纹；嵌入宿主还可复用内存预编译 package，持久化 manifest 与更完整初始化策略仍缺。 |
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

2026-08-28 的当前 `main` `4cc4474` 验证在 Apple arm64 上使用 Rust 1.94.0 release `qbench`、官方 QuickJS 2026-06-04 与 31 个样本；命令为 `qbench --compare-qjs PATH --compare-iterations 1 --repeat 31 --json`。七项预编译热执行比值依次为 `12.84×`、`14.19×`、`9.09×`、`13.16×`、`6.46×`、`3.12×`、`4.37×`。这轮刷新不再把已淘汰的 #215 候选值当作当前声明；#214 已由 PR #216 完成，其 Linux paired run `33140511336` 有 51 个共同 workload 和 0 aggregate alerts。完整命令、基线、采样与当前数据由 [#218](https://github.com/coffee-js/quickcoffee/issues/218) 和 [#66](https://github.com/coffee-js/quickcoffee/issues/66) 维护，测量协议由已完成的 [#79](https://github.com/coffee-js/quickcoffee/issues/79) 维护。

这使七条同机代表负载都进入各自的首轮审阅目标；#216 已用 Linux paired artifact 排除此前确认的非目标回归。跨平台证据与噪声模型稳定前，仍不应宣称 QuickCoffee 整体达到 QuickJS 量级。
