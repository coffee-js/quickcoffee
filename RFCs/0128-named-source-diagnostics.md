# RFC 0128：命名来源诊断传播

- 状态：已采纳
- 日期：2026-08-24
- 依赖：RFC 0117、RFC 0119、RFC 0126、RFC 0127

## 动机

RFC 0126 已把 `source_name` 定义为结构化 span 的可选不透明字段，RFC 0127 也能为来源未改写的 lexer/parser 错误生成精确范围；但单文件编译、`Context::eval`、命名模块和 CLI 文件入口都没有把宿主已知的来源名送入这些 label。结果是宿主能取得行列，却不能把错误稳定地关联回虚拟文档、模块或文件。

## 决策

1. 顶层 API 新增 `compile_named`、`compile_program_named` 和 `eval_named`；`Engine` 新增对应的 `compile_named`、`compile_program_named`，`Context` 新增 `eval_named`。参数顺序与 `compile_module` 一致，先传不透明来源名，再传源码。
2. 命名 API 只给错误中尚无来源名的现有 label 补上宿主字符串。引擎不规范化、不解析、不去除 `..`，也不访问该名称指向的文件或网络。没有可靠位置的错误仍保持无 label，不能为了附带名称而伪造行号。
3. 既有 `compile`、`compile_program`、`eval` 与 `Context::eval` 保持匿名；成功产物、字节码编码、指纹和 `Error::Display` 均不改变。
4. `Engine::compile_module` 使用调用者传入的模块规范名；经 `ModuleLoader` 返回的依赖使用 `ModuleSource::name()`。模块 parser/compiler 已产生的 label 因而携带对应规范名，而不是 import 字面量或引擎推测的路径。
5. `qcoffee` 对文件、`-`、`--check`、`--dump-bytecode` 与 `--fingerprint` 使用用户传入的原字符串作为来源名。普通人类错误文本保持兼容；`--json` 的错误对象在 label 有来源时增加可选字符串字段 `source`，原有 `line` 字段、匿名 `-e` 输出和退出码不变。
6. 本切片只传播编译期已有 label 的来源名。RFC 0129 已覆盖 AST lowering 的控制流与模块验证错误；一般成功表达式的 bytecode/verification 与 runtime label 保存仍由 #74 后续切片完成，`eval_named` 会沿用同一入口。

## 验收

测试必须覆盖顶层、`Engine`、`Context` 三类命名 API，确认匿名 API 仍返回 `source_name: None`；模块错误必须保留未经规范化的宿主模块名；CLI 文件 JSON 错误必须包含用户传入路径，而 `-e` 的既有 JSON 文本保持不变。`make check`、文档、指纹和 MSRV 门禁必须通过。
