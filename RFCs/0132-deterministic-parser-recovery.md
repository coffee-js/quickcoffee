# RFC 0132：确定性的 check-mode parser recovery

- 状态：已采纳
- 日期：2026-08-24
- 依赖：RFC 0027、RFC 0126、RFC 0127、RFC 0128、RFC 0131

## 动机

此前 `qcoffee --check FILE` 在 lexer、parser、lowering 或 verifier 的第一个错误处停止。对于编辑器、CI 和批量脚本检查，这迫使调用方每修复一个互不相关的语法错误就重新运行一次。RFC 0127 已保证单个 parser 错误的精确 span，因而可以在不改变已有单错误编译契约的前提下，让 check-only 路径收集可安全恢复的多个错误。

## 决策

1. `compile`、`compile_named`、`Engine::compile_program*` 与模块编译继续在第一个错误处返回一个 `Error`，保持既有 embedding 行为和类型签名。
2. `Engine::check_program*` 是独立的静态检查 API：它解析、lower、编译并验证，但不创建执行上下文；返回 `Result<(), Vec<Error>>`。成功时不会分配公开 `Program`，失败时按源序返回诊断。
3. parser recovery 只在顶层语句边界同步：`Semi`、`Dedent` 或 `Eof`。失败语句的剩余 token 以及紧随的 dedent 会被消费；因此不会停滞，也不会把同一失败块的 dedent 报成第二个合成错误。lexer 完成后一次性建立 token-boundary layout-depth side table，每次恢复以 O(1) 取得当前位置深度，全部恢复只线性遍历待丢弃 token。它不是任意嵌套的或增量 IDE parser recovery。
4. lexer、lowering 和 bytecode verifier 没有可靠的后续输入边界，仍各返回一个错误。每个收集到的 parser `Error` 继续使用 RFC 0127 的精确/降级 span；命名检查将不透明 caller source name 附到每一个现有 label。
5. `qcoffee --check FILE` 改用该 API：所有可恢复的 parser error 以源序写到 stderr，退出码为 `1`，标准输出为空，也绝不运行字节码。单错误和非 parser 错误的显示文本、`--json` 互斥规则及普通执行模式均不变。

## 验收

测试必须覆盖两个独立顶层错误的稳定源序、跨 failed indentation block 的 dedent 同步、每个命名 label 的 source name、既有 `compile_program_named` 仍只返回第一个错误，以及 check 模式的空 stdout 与不执行语义。`make check`、debug/release、Clippy、手册、MSRV 与 crate package 门禁必须通过。
