# RFC 0162：安全且确定性的 CSON 契约 / Safe deterministic CSON contract

- 状态：已采纳 / Status: Adopted
- 日期：2026-08-30 / Date: 2026-08-30
- 依赖：RFC 0135、RFC 0137、RFC 0138、RFC 0155–0157、RFC 0160 / Dependencies: RFC 0135, RFC 0137, RFC 0138, RFC 0155–0157, and RFC 0160
- 跟踪：#244、#258 / Tracking: #244 and #258

## 中文

### 动机与来源基线

CSON 适合人类维护带注释的 CoffeeScript 生态配置，但它没有一份独立于 CoffeeScript parser 的完整标准。QuickCoffee 必须先定义一个可验证的纯数据方言，才能避免把配置加载变成代码执行。

兼容研究固定在以下公开来源：

- [`bevry/cson` 8.4.0](https://github.com/bevry/cson/tree/954d892ce2444505346779cc7b03774f070e4c97)，master commit `954d892ce2444505346779cc7b03774f070e4c97`。README 展示顶层无括号对象、`#` 注释、缩进嵌套、无逗号数组和三单引号多行字符串；包本身把 CSON、CoffeeScript 和 JavaScript 作为不同解析模式。
- [`cson-parser` 4.0.9 npm tarball](https://registry.npmjs.org/cson-parser/-/cson-parser-4.0.9.tgz)，SHA-1 `eef0cf77edd057f97861ef800300c8239224eedb`。它依赖 CoffeeScript AST，接受正则和算术/位运算，并通过 JavaScript VM 解码字符串与正则。QuickCoffee 不继承这些可执行或宿主相关行为。
- GitHub Linguist commit `e0c78d62c42abae6122235d8e68a7aa43eef89da` 的 [`languages.yml`](https://github.com/github-linguist/linguist/blob/e0c78d62c42abae6122235d8e68a7aa43eef89da/lib/linguist/languages.yml) 把 `.cson` 分类为 data 并关联 CoffeeScript grammar。仓库 corpus 使用原生 `.cson`，不增加语言伪装。

因此“兼容”表示矩阵中被明确接受的常用数据写法具有相同数据意图；它不表示完整 CoffeeScript、上游所有边缘语法或字节级 serializer 等价。

### 安全边界

CSON v1 只构造 Nil、Bool、String、Integer、Decimal、Array 和 Map。parser 不调用 QuickCoffee compiler、`eval`、CoffeeScript/JavaScript runtime、正则引擎、module loader 或 host callback，也不执行文件、网络、时钟、随机数及其他 capability。

以下节点在词法或语法阶段拒绝，而不是先求值再检查结果：

- `#{...}` 插值；
- 函数、箭头、调用、成员访问与 `new`；
- 正则；
- 一元、算术和位运算表达式；
- 除 `true`、`false`、`null` 外的裸标识符值；
- CoffeeScript/JavaScript 解析模式、embedded JavaScript 与 import/require。

这比 `cson-parser` 4.0.9 更窄，是刻意的安全差异。

### 文本与行

输入是 UTF-8 `str`。UTF-8 BOM 被拒绝；CRLF 在 tokenization 前规范化为 LF，单独 CR 被拒绝。错误同时保留原输入的 UTF-8 byte span，以及从 1 开始的物理行和 Unicode scalar column；CRLF 计作一个换行。

字符串外的 `#` 到物理行尾是注释。空行和仅注释行不参与 indentation stack。

第一个缩进行确定本文件的单位：连续 N 个空格，或一个或多个 tab。之后每层必须恰好增加一个相同单位；同一前缀混合 space/tab、非整数层级或跳过父层均以 `E_CSON_INDENTATION` 拒绝。canonical serializer 始终输出两个空格，因此 parser 对 tab 的接受不会让输出依赖输入风格。

### 根值、Map 与 Array

文档必须包含恰好一个根 Value；空输入或只有注释的输入以 `E_CSON_SYNTAX` 拒绝。根可以是 Map、Array 或 scalar。非空根 Map 可以省略 `{}`；空 Map 写作 `{}`。

Map key 可以是 ASCII `[A-Za-z_$][A-Za-z0-9_$]*`，或不含换行的 quoted String。key 按字符串解码后比较，任意层级的重复 key 都以 `E_CSON_DUPLICATE_KEY` 拒绝，绝不 last-wins。缩进 Map 的 entry 由换行分隔；braced Map 的同一行 entry 需要逗号，跨行可用逗号或换行，末项允许一个尾逗号。

Array 必须使用 `[]`。同一物理行的相邻项需要逗号；不同逻辑行可省略逗号；允许一个尾逗号。Map/Array 可以为空。括号和缩进必须在 `CsonLimits` 深度内正确配对。

### 字符串

单引号与双引号字符串使用同一确定性 escape 集：`\\`、`\'`、`\"`、`\b`、`\f`、`\n`、`\r`、`\t` 和 `\uXXXX`。Unicode surrogate 必须是成对的有效 scalar；未知 escape、未配对 surrogate 和物理换行以 `E_CSON_SYNTAX` 拒绝。双引号内出现未 escape 的 `#{` 以 `E_CSON_INTERPOLATION` 拒绝。

v1 多行形式只有三单引号。opening delimiter 后和独立 closing delimiter 前的结构换行不进入值；所有非空 content line 必须至少具有 closing delimiter 的缩进，移除这一共同结构前缀后以 LF 连接。内容不处理 escape；包含 `'''` 的值由 serializer 回退为带 `\n` escape 的单行 quoted String。三双引号因为通常具有插值语义而 defer，不在 v1 被静默解释。

### 数值

普通 number token 使用 JSON 十进制词法：可选 `-`、无多余前导零的整数、可选 fraction、可选十进制 exponent。`+`、hex/binary/octal、数字分隔符、NaN、Infinity 和表达式不属于 v1。

- 不含 fraction/exponent 的 token 映射为 RFC 0135 Integer。
- 含 fraction 或 exponent 的 token 映射为 RFC 0137 Decimal。
- `-0` 规范化为 Integer `0`；负 Decimal 零保留 Decimal 类型但 canonical 文本去掉负号。
- coefficient bit 与 scale 超限或 exponent 无法在限额内展开时以 `E_CSON_NUMBER` 或对应 resource error 拒绝，绝不经过 f64。

QuickCoffee 源码的 `n` / `m` 后缀属于 defer 的版本化扩展。CSON v1 不需要它们表达精确数据，也不会把它们误当普通上游 CSON。parser 不产生 Number；serializer 对 Number 和所有非数据 Value 返回 `E_CSON_TYPE`，从而保证已支持值的 parse → serialize → parse 类型和值都等价。

### Canonical serializer

后续 `to_cson` 必须产生唯一字节序列：

- UTF-8、LF、两空格缩进、恰好一个末尾 LF；
- Map key 以 Unicode scalar lexicographic order 排序；符合 ASCII key 规则的 key 不加引号，其余用 quoted String；
- 非空根 Map 无 braces，嵌套非空 Map 使用缩进形式，空 Map 为 `{}`；
- Array 每项独占一行、无逗号；空 Array 为 `[]`；
- 单行 String 使用单引号和最小确定性 escape；包含 LF 且不含 `'''` 时使用三单引号，否则使用 `\n` escape；
- Integer 使用规范十进制，Decimal 使用 RFC 0138 的无指数文本并为 scale-zero Decimal 保留 `.0`；
- 不保留 comments、原 key 顺序、quote 选择、逗号或 indentation 风格。

连续 serialize 不改变字节；所有 serializer append 都在写入前检查输出上限，不返回部分文本。

### `CsonLimits` 与诊断

纯 Rust parser/serializer 使用显式 `CsonLimits`。默认值与既有通用数据边界同量级：1,000,000 input/output/string bytes、100,000 values、每容器 100,000 items、128 nesting、1,000,000 Integer/Decimal coefficient bits、100,000 Decimal scale、32 diagnostics 和 4,000,000 parser work units。每个字段可独立收紧至零。

script API 从 Context 的 `ResourceLimits` 导出对应上限，并取调用方显式 `CsonLimits` 与 Context 上限的较小值。token 消费、缩进变化、容器插入、字符串解码、数字扫描和输出 append 必须单调计入 work 或数据边界；检查发生在增长前。

稳定 code 至少包括：`E_CSON_SYNTAX`、`E_CSON_INDENTATION`、`E_CSON_DUPLICATE_KEY`、`E_CSON_INTERPOLATION`、`E_CSON_EXPRESSION`、`E_CSON_IDENTIFIER_VALUE`、`E_CSON_NUMBER`、`E_CSON_TYPE`，以及 input/string/depth/item/output/work limit codes。syntax 与 limit failure 不 panic、不递归爆栈、不返回部分 Value/String，并通过 RFC 0160 兼容的人类与 JSON labels 报告 primary span。

### 可执行兼容矩阵与交付顺序

`tests/cson/matrix.tsv` 是实现前可执行的决策清单；独立编写的 `.cson` fixtures 分为 accept/reject/defer，分别指向 canonical JSON、稳定 error code 或延期理由。测试验证 id/tag/path、文件完整性、JSON canonicality、CRLF/tab 样例与 GitHub 原生扩展，不需要 parser 已存在。

交付顺序为：本 RFC/corpus → 纯文本 core API → QuickCoffee builtin 与确定性 CSON↔JSON tooling → `.cson` 真实配置工作流/release smoke → fuzz/property 与 10/100/1000 条性能证据。静态 data import、source execution、comment-preserving formatter 和文件 I/O 不由本 RFC 授权。

---

## English

### Motivation and pinned sources

CSON is useful for human-maintained, commented configuration in the CoffeeScript ecosystem, but it has no complete specification independent of a CoffeeScript parser. QuickCoffee must define a verifiable data-only dialect before configuration loading can be added without becoming code execution.

Compatibility research is pinned to these public sources:

- [`bevry/cson` 8.4.0](https://github.com/bevry/cson/tree/954d892ce2444505346779cc7b03774f070e4c97) at master commit `954d892ce2444505346779cc7b03774f070e4c97`. Its README demonstrates unbraced root objects, `#` comments, indentation, comma-free arrays, and triple-single multiline strings. The package treats CSON, CoffeeScript, and JavaScript as separate parsing modes.
- The [`cson-parser` 4.0.9 npm tarball](https://registry.npmjs.org/cson-parser/-/cson-parser-4.0.9.tgz) with SHA-1 `eef0cf77edd057f97861ef800300c8239224eedb`. It depends on a CoffeeScript AST, accepts regex and arithmetic/bitwise operations, and decodes strings and regex through a JavaScript VM. QuickCoffee does not inherit those executable or host-dependent behaviors.
- GitHub Linguist [`languages.yml`](https://github.com/github-linguist/linguist/blob/e0c78d62c42abae6122235d8e68a7aa43eef89da/lib/linguist/languages.yml) at commit `e0c78d62c42abae6122235d8e68a7aa43eef89da` classifies `.cson` as data associated with a CoffeeScript grammar. The corpus uses the native extension without a language disguise.

Compatibility therefore means that explicitly accepted common data forms preserve their data intent. It does not mean complete CoffeeScript, every upstream edge form, or byte-identical serializer output.

### Security boundary

CSON v1 constructs only Nil, Bool, String, Integer, Decimal, Array, and Map. The parser calls no QuickCoffee compiler, `eval`, CoffeeScript/JavaScript runtime, regex engine, module loader, or host callback and performs no file, network, clock, random, or other capability operation.

The following are rejected lexically or syntactically instead of being evaluated and inspected afterward:

- `#{...}` interpolation;
- functions, arrows, calls, member access, and `new`;
- regular expressions;
- unary, arithmetic, and bitwise expressions;
- bare identifier values other than `true`, `false`, and `null`;
- CoffeeScript/JavaScript modes, embedded JavaScript, and import/require.

This is intentionally narrower than `cson-parser` 4.0.9.

### Text and lines

Input is a UTF-8 `str`. A UTF-8 BOM is rejected. CRLF is normalized to LF before tokenization, while a standalone CR is rejected. Errors retain UTF-8 byte spans over the original input plus one-based physical lines and Unicode-scalar columns; CRLF counts as one newline.

Outside strings, `#` starts a comment through the physical line ending. Blank and comment-only lines do not participate in the indentation stack.

The first indented line establishes either N consecutive spaces or one or more tabs as the file's indentation unit. Every later level adds exactly one identical unit. Mixed space/tab prefixes, fractional units, and skipped parent levels fail with `E_CSON_INDENTATION`. Canonical output always uses two spaces, so accepting tabs does not make output input-dependent.

### Root values, Maps, and Arrays

A document contains exactly one root Value. Empty and comment-only inputs fail with `E_CSON_SYNTAX`. The root may be a Map, Array, or scalar. A non-empty root Map may omit braces; an empty Map is `{}`.

A Map key is either ASCII `[A-Za-z_$][A-Za-z0-9_$]*` or a quoted String without a newline. Keys are compared after string decoding. A duplicate at any depth fails with `E_CSON_DUPLICATE_KEY`, never last-wins. Newlines separate entries in an indentation Map. Same-line entries in a braced Map require commas; cross-line entries may use commas or newlines, with one optional trailing comma.

Arrays always use `[]`. Adjacent items on one physical line require commas. Items on separate logical lines may omit them, and one trailing comma is allowed. Maps and Arrays may be empty. Delimiters and indentation must balance within the `CsonLimits` depth.

### Strings

Single- and double-quoted strings share one deterministic escape set: `\\`, `\'`, `\"`, `\b`, `\f`, `\n`, `\r`, `\t`, and `\uXXXX`. Unicode surrogates must form a valid scalar pair. Unknown escapes, unpaired surrogates, and physical newlines fail with `E_CSON_SYNTAX`. An unescaped `#{` in a double-quoted string fails with `E_CSON_INTERPOLATION`.

The only v1 multiline form is triple-single quoting. The structural newline after the opening delimiter and before a standalone closing delimiter is excluded. Every non-empty content line must have at least the closing delimiter's indentation; that common structural prefix is removed and remaining lines join with LF. Content is literal and processes no escapes. A value containing `'''` falls back to a quoted single-line representation with `\n` escapes. Triple-double quoting is deferred because it normally carries interpolation semantics and is never silently reinterpreted in v1.

### Numbers

Ordinary number tokens use JSON decimal lexical syntax: optional `-`, an integer without redundant leading zeros, optional fraction, and optional decimal exponent. Leading `+`, hex/binary/octal, digit separators, NaN, Infinity, and expressions are outside v1.

- A token without a fraction or exponent becomes an RFC 0135 Integer.
- A token with a fraction or exponent becomes an RFC 0137 Decimal.
- `-0` normalizes to Integer `0`; a negative Decimal zero retains the Decimal type but loses the sign in canonical text.
- Coefficient-bit or scale overflow and exponents that cannot expand within limits fail with `E_CSON_NUMBER` or the corresponding resource error, never through f64.

QuickCoffee source `n` / `m` suffixes are deferred as a versioned extension. CSON v1 does not need them for exact data and does not confuse them with common upstream CSON. The parser never produces Number. The serializer rejects Number and every non-data Value with `E_CSON_TYPE`, guaranteeing both type and value equality for supported parse → serialize → parse values.

### Canonical serializer

The later `to_cson` implementation emits one byte representation:

- UTF-8, LF, two-space indentation, and exactly one final LF;
- Map keys sorted by Unicode-scalar lexicographic order; ASCII-safe keys remain bare and all others are quoted;
- non-empty root Maps without braces, nested non-empty Maps in indentation form, and empty Maps as `{}`;
- one Array item per line without commas, and `[]` for an empty Array;
- single quotes plus minimal deterministic escaping for one-line Strings; triple-single form for LF-containing values without `'''`, otherwise quoted `\n` escapes;
- canonical decimal Integer text and RFC 0138 non-exponent Decimal text, retaining `.0` for scale-zero Decimal;
- no preservation of comments, original key order, quote choice, commas, or indentation.

Repeated serialization is byte-stable. Every output append checks its limit before growth and no partial String is returned.

### `CsonLimits` and diagnostics

Pure Rust parsing and serialization use explicit `CsonLimits`. Defaults match existing general data boundaries in scale: 1,000,000 input/output/string bytes, 100,000 values, 100,000 items per container, nesting 128, 1,000,000 Integer/Decimal coefficient bits, Decimal scale 100,000, 32 diagnostics, and 4,000,000 parser work units. Every field can independently be tightened to zero.

The script API derives matching bounds from Context `ResourceLimits` and takes the lower of caller-supplied `CsonLimits` and Context limits. Token consumption, indentation changes, container insertion, string decoding, number scanning, and output appends monotonically consume work or data budgets. Checks occur before growth.

Stable codes include at least `E_CSON_SYNTAX`, `E_CSON_INDENTATION`, `E_CSON_DUPLICATE_KEY`, `E_CSON_INTERPOLATION`, `E_CSON_EXPRESSION`, `E_CSON_IDENTIFIER_VALUE`, `E_CSON_NUMBER`, `E_CSON_TYPE`, and input/string/depth/item/output/work limit codes. Syntax and limit failures do not panic, overflow the Rust stack, or return a partial Value/String. They expose an RFC 0160-compatible primary span in human and JSON diagnostics.

### Executable matrix and delivery order

`tests/cson/matrix.tsv` is an executable pre-implementation decision list. Independently authored `.cson` fixtures are classified as accept/reject/defer and point to canonical JSON, a stable future error code, or a deferral reason. Tests validate ids, tags, paths, file completeness, JSON canonicality, the CRLF/tab case, and the native GitHub extension without requiring an implemented parser.

Delivery order is this RFC/corpus, pure-text core APIs, QuickCoffee builtins and deterministic CSON↔JSON tooling, a real `.cson` configuration workflow/release smoke, then fuzz/property and 10/100/1000-record performance evidence. Static data imports, source execution, comment-preserving formatting, and file I/O are not authorized by this RFC.
