# RFC 0034：编译器鲁棒性压力验收

- 状态：已采纳
- 依赖：RFC 0000、RFC 0002、RFC 0032、RFC 0033

## 目标

对不受信任源码，QuickCoffee 的编译入口只能成功产生已验证 chunk 或返回 `Error`；不得 panic。公开 Rust API 接收 `&str`，故 UTF-8 有效性由类型系统保证；本 RFC 所称 malformed source 是词法、布局或语法不完整/非法，而非无效 UTF-8 字节。

## 确定性压力语料

`tests/robustness.rs` 使用固定线性同余序列从精心挑选的片段生成 1,024 条输入。片段包括 Unicode XID 名称及组合附标、括号/数组/映射、运算符、缩进和 tab、未闭合字符串/插值/块注释、splat、箭头、禁用的反引号和 NUL。每个输入都在 `catch_unwind` 中执行 `compile`，并断言不会 panic。

该测试不执行任意生成源码，以避免无限循环或宿主资源差异污染确定性；运行时错误和 fuel 边界继续由独立测试覆盖。确定性种子使失败可复现、可最小化且不需要外部模糊测试运行器。

## 验收

`cargo test --test robustness` 必须通过三类测试：手工 malformed source 语料、1,024 条确定性编译压力语料、以及有界运行时错误的无 panic 语料。该集被 `make check` 强制执行。
