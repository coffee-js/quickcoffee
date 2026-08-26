# RFC 0000：工程范围及验收基线

- 状态：已采纳
- 日期：2026-08-22

QuickCoffee 是一个 Rust 实现的、受 CoffeeScript 2016 启发的脚本引擎。它不是 JavaScript 解释器，亦不追求与浏览器或 Node.js 互操作。

## 目标

1. 源码经词法、语法分析、编译为紧凑字节码，再由无递归栈 VM 执行。
2. 提供 QuickJS 风格的单文件 CLI（求值、执行、反汇编），以及简单的 Rust 嵌入 API。
3. 避免 JavaScript 全局与内部模型：没有全局/自由 `this`、任意函数构造、公开可变原型链、`undefined`、隐式类型转换、`eval` 或嵌入 JavaScript。RFC 0134 的 class 内接收者、`new` 与私有继承链是受限语言能力，不构成 JavaScript 全局对象或通用原型能力。
4. 以可执行测试作为 RFC 的规范性验收；以可重复 benchmark 记录性能。

## 非目标（首个实现阶段）

模块系统、生成器、异步、正则式、源映射和垃圾回收器不属于 0.1 核心。嵌套解构与严格范围切片已由 RFC 0023、RFC 0037 纳入当前核心；class 构造与继承由 RFC 0134 纳入后续语言契约。其余非目标可在不破坏字节码验证规则的条件下由后续 RFC 扩展。

## 版本化承诺

本仓库中的测试即 0.1 的语义基线。对语法或运行时的新增特性必须先以 RFC 补充定义，并至少添加：成功测试、错误测试及字节码验证测试。

当前后续语义与工具 RFC 延伸至 RFC 0140；RFC 0134 已完整实现 class 内受限接收者、构造、私有继承链与 receiver-bound `=>`，RFC 0135 与 0137 已实现与 Number 严格分离的精确 Integer/Decimal，RFC 0136 以密封 Error 值取代字符串 catch 并保持资源错误不可捕获，RFC 0138 增加无隐式 I/O、重复键拒绝、精确数值映射及 Context 可配置资源守卫的脚本 JSON，RFC 0139 以固定 White_Space 表增加确定性 Unicode trim 与子串查询，RFC 0140 增加资源有界、不可变且 locale-free 的稳定标量排序。RFC 0077–0118 维持既有 CLI JSON、测试、指纹、文档、资源、性能与发布契约，RFC 0119 定义宿主控制的静态模块核心，RFC 0120–0124 建立跨运行时性能与剖析契约，RFC 0125 建立 CoffeeScript 2016 特性矩阵，RFC 0126–0132 建立结构化源码范围、运行期归因与 parser recovery，RFC 0133 提供显式根目录的受限文件模块 loader。RFC 0134、0139 与 0140 均不引入全局对象、公开 JavaScript 原型或 ambient locale 能力。
