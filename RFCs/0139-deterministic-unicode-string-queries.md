# RFC 0139：确定性 Unicode 字符串查询与裁剪

- 状态：已采纳并实现
- 日期：2026-08-26
- 依赖：RFC 0001、RFC 0061、RFC 0070、RFC 0072、RFC 0118、RFC 0123
- 跟踪：issue #78、issue #124、issue #153

## 边界与 API

QuickCoffee 增加四个普通纯函数：`trim(text)`、`contains(text, needle)`、`starts_with(text, prefix)` 与 `ends_with(text, suffix)`。它们只接受 String，不读取 locale、环境变量、宿主对象或外部 Unicode 数据，也不成为 String prototype 方法。

`contains`、`starts_with` 与 `ends_with` 是大小写敏感的精确子串查询。所有输入都是合法 UTF-8 QuickCoffee String，因此匹配起止只能落在 Unicode scalar 边界；多字节 scalar 不会被拆开。空 needle、prefix 与 suffix 按数学上的空串规则返回 true。函数不做 normalization、case folding、locale collation 或 JavaScript UTF-16 code-unit 适配。

## 固定空白集合

`trim` 从两端移除以下固定 Unicode White_Space scalar，并返回中间原顺序文本：U+0009–U+000D、U+0020、U+0085、U+00A0、U+1680、U+2000–U+200A、U+2028、U+2029、U+202F、U+205F、U+3000。

该表是 RFC 的规范内容，不委托给 Rust `char::is_whitespace`、操作系统、locale 或随工具链升级变化的 Unicode property table。U+200B ZERO WIDTH SPACE 等未列 scalar 不被裁剪。`trim` 只缩短或原样复制输入，不产生 normalization，也不修改原 String。

## 错误、资源与统计

四个函数严格检查 arity 与 String 类型，失败产生普通可捕获 Runtime error，不返回部分值。它们沿用普通 builtin 调用的 fuel/call 计数；内部没有脚本 callback，不能绕过 call depth、异常传播或 capability 边界。三个 predicate 返回 Bool，不产生托管值分配；`trim` 返回一个新托管 String，并记一个 `ExecutionStats.value_allocations` 事件。

本 slice 不增长输出：`trim` 的 UTF-8 byte 长度不超过输入，predicate 不返回容器或 String。因此它不先行定义 #76 的一般 String/container size policy。后续 `replace`、拼接和 immutable update 必须在输出追加前接入该策略。

## 延后工作

Locale-free 大小写转换需要固定 Unicode 版本和完整 mapping 表；不得直接继承宿主 locale 或工具链表。`find`、`any`、`all`、`fold` 与稳定 `sort_by` 需要能暂停/恢复脚本 callback 的 VM continuation 设计，并明确 short-circuit、fuel、call depth 与错误传播。输出增长型 `replace`、concat 和 update helpers 与 #76 通用预算共同交付。

核心测试覆盖空串、ASCII、多字节 Unicode、固定空白全集的代表项、U+200B 反例、严格类型/arity 与分配统计。五份可执行手册展示组合查询；`stdlib-string-queries` 同名 qbench/cargo-bench 负载保留最终值护栏和确定性执行画像。
