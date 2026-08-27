# RFC 0150：资源有界的字面量字符串替换 / Resource-bounded literal string replacement

- 状态：已采纳 / Status: Adopted
- 日期：2026-08-27 / Date: 2026-08-27
- 依赖：RFC 0118、RFC 0139、RFC 0144 / Dependencies: RFC 0118, RFC 0139, RFC 0144

## 中文

### 契约

标准库增加普通纯函数 `replace_all(text, needle, replacement)`。三个参数必须是 String，`needle` 不得为空；参数数量、类型或空 needle 错误是可捕获的 Runtime error。

匹配采用原 UTF-8 字符串上的精确、大小写敏感、无 normalization 字面量语义。所有 QuickCoffee String 都是合法 UTF-8，因此匹配边界不会拆分 Unicode scalar。匹配从左到右且不重叠，新插入的 `replacement` 不会被再次扫描；例如 `replace_all('aaaa', 'aa', 'b')` 为 `'bb'`，`replace_all('a', 'a', 'aa')` 为 `'aa'`。

`ResourceLimits` 增加 `max_text_operation_bytes`，默认 1,000,000；getter 与 builder 公开同一策略。`replace_all` 在扫描前按 `text` 的 UTF-8 byte 长度检查该边界，越界产生不可捕获的 `ResourceLimit::TextOperationBytes`。`needle` 与 `replacement` 仍分别服从通用 `StringBytes` 单值边界。

实现先在不分配输出的遍历中，用 checked arithmetic 计算所有非重叠匹配后的最终 UTF-8 byte 长度，再检查 `StringBytes`。只有全部检查通过后才分配一次输出并进行第二次确定性遍历；失败不会返回部分结果或改变输入。无匹配仍返回一个新的 String 值。

一次调用沿用普通 builtin 的指令/fuel/call 记账；native 扫描工作由 `TextOperationBytes` 限制。成功结果记录一个托管 String 对象及最终 payload bytes，不执行脚本 callback。模块子 Context 继承相同资源策略。

### 非目标

本 RFC 不增加 regex、case folding、locale collation、Unicode normalization、callback replacement、捕获组或 prototype 方法。受限文本匹配语言继续由 #78 独立设计。

### 验收

测试覆盖 ASCII、多字节 Unicode、非重叠与不重扫语义、无匹配、空 needle、严格参数、输入扫描限制、输出增长限制、不可捕获资源错误、源码标签、模块继承和确定性分配 profile。五语可执行手册、中英文语法索引、readiness、qbench 与 cargo bench 同步更新。

---

## English

### Contract

The standard library adds the ordinary pure function `replace_all(text, needle, replacement)`. All three arguments must be Strings and `needle` must be non-empty. Wrong arity, wrong types, and an empty needle are catchable Runtime errors.

Matching is exact, case-sensitive, normalization-free literal matching over the original UTF-8 string. Every QuickCoffee String is valid UTF-8, so match boundaries never split a Unicode scalar. Matches are non-overlapping and processed left to right; inserted replacement text is never rescanned. Therefore `replace_all('aaaa', 'aa', 'b')` is `'bb'`, while `replace_all('a', 'a', 'aa')` is `'aa'`.

`ResourceLimits` adds `max_text_operation_bytes`, defaulting to 1,000,000, with a matching getter and builder. Before scanning, `replace_all` checks the UTF-8 byte length of `text`; exceeding it produces an uncatchable `ResourceLimit::TextOperationBytes`. `needle` and `replacement` remain independently governed by the general `StringBytes` per-value boundary.

The implementation first computes the final UTF-8 byte length for all non-overlapping matches with checked arithmetic and without allocating output, then checks `StringBytes`. Only after every check succeeds does it allocate one output buffer and perform the deterministic second traversal. Failure returns no partial output and cannot mutate an input. A no-match result is still a fresh String value.

One invocation uses ordinary builtin instruction/fuel/call accounting; `TextOperationBytes` bounds native scanning work. A successful result records one managed String object and its final payload bytes, with no script callback. Module child Contexts inherit the same policy.

### Non-goals

This RFC adds no regex, case folding, locale collation, Unicode normalization, callback replacement, capture groups, or prototype methods. Restricted text matching remains a separate #78 design.

### Acceptance

Tests cover ASCII, multibyte Unicode, non-overlap and non-rescanning semantics, no-match behavior, empty needles, strict arguments, scan-input limits, output-growth limits, uncatchable resource errors, source labels, module inheritance, and deterministic allocation profiles. Five executable manuals, Chinese/English syntax indexes, readiness, qbench, and cargo bench are updated together.
