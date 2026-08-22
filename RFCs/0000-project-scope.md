# RFC 0000：工程范围及验收基线

- 状态：已采纳
- 日期：2026-08-22

QuickCoffee 是一个 Rust 实现的、受 CoffeeScript 2016 启发的脚本引擎。它不是 JavaScript 解释器，亦不追求与浏览器或 Node.js 互操作。

## 目标

1. 源码经词法、语法分析、编译为紧凑字节码，再由无递归栈 VM 执行。
2. 提供 QuickJS 风格的单文件 CLI（求值、执行、反汇编），以及简单的 Rust 嵌入 API。
3. 避免 JavaScript 内部模型：没有原型链、`this`、`new`、`undefined`、隐式类型转换、`eval` 或嵌入 JavaScript。
4. 以可执行测试作为 RFC 的规范性验收；以可重复 benchmark 记录性能。

## 非目标（首个实现阶段）

模块系统、继承、生成器、异步、正则式、源映射和垃圾回收器不属于 0.1 核心。嵌套解构与严格范围切片已由 RFC 0023、RFC 0037 纳入当前核心；其余非目标可在不破坏字节码验证规则的条件下由后续 RFC 扩展。

## 版本化承诺

本仓库中的测试即 0.1 的语义基线。对语法或运行时的新增特性必须先以 RFC 补充定义，并至少添加：成功测试、错误测试及字节码验证测试。

当前已实现的后续语义与工具 RFC 延伸至 RFC 0111；其中 RFC 0077 定义 JSON 输出、RFC 0079 定义 TAP 输出、RFC 0080 定义 CLI 字节码指纹、RFC 0081 定义可机器读取的基准输出、RFC 0082 规范化指纹编码、RFC 0083 定义 Markdown 文学编程产物、RFC 0084 定义嵌入上下文 fuel 控制、RFC 0085 定义可执行 Rust 嵌入示例、RFC 0086 定义 crate 发布元数据、RFC 0094 定义 qdocco 最终值门禁、RFC 0095 定义字符串步进迭代、RFC 0096 定义其性能基准、RFC 0097 定义 `do` 参数转发、RFC 0098 定义 RFC 索引门禁、RFC 0099 定义 `!` 否定别名、RFC 0100 定义有符号 `by` 步长、RFC 0101 定义 qdocco 原子输出、RFC 0102 定义 qtest 规范文件去重、RFC 0103 定义 qbench schema 版本、RFC 0104 定义 qdocco 块注释代码保留、RFC 0105 定义 qbench 重复采样中位数、RFC 0106 定义 crate 发布包验收门禁、RFC 0107 定义 release qbench 持续门禁、RFC 0108 定义 qtest 可执行示例语料、RFC 0109 定义 qbench 核心负载全套护栏、RFC 0110 定义 Rust MSRV 契约、RFC 0111 定义 release profile 完整测试门禁，均不改变脚本语言值模型的原型无关约束。
