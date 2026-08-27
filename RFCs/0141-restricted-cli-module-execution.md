# RFC 0141：显式根目录的 CLI 模块执行

- 状态：已采纳
- 日期：2026-08-26
- 依赖：RFC 0117、RFC 0119、RFC 0133

## 动机

RFC 0133 已把受限文件模块加载器作为显式宿主能力交付，但 `qcoffee` 仍只能执行单个文件、标准输入或 `-e` 源码。多文件脚本需要一个可直接使用的 CLI 入口，同时不能让通常的单文件、REPL 或嵌入执行悄然获得当前目录、搜索路径或网络的权限。

## 决策

1. 新增执行模式：`qcoffee --module-root ROOT ENTRY [--fuel N] [--stats] [--json] [-- ARG...]`。`ROOT` 必须由调用者显式给出，并原样传给 `RestrictedFileModuleLoader::new`；CLI 不会从当前目录、entry 路径、环境变量或搜索路径推导根目录。
2. `ENTRY` 是相对于该根目录的模块入口名，而不是普通文件输入。`.coffee` 默认推断、显式 `.coffee` / `.litcoffee`、规范名称、UTF-8、显式 `./` / `../` 导入、词法越界和符号链接逃逸均完全复用 RFC 0133 的 loader 契约。
3. 入口以规范模块名编译，并以 `Context::run_module` 执行完整静态图；图共享 fuel、取消和当前 Context 的资源政策。成功时 CLI 输出按名称排序的不可变导出 Map；`--json` 输出唯一记录 `{"ok":true,"exports":VALUE}`。没有部分初始化或部分导出会在失败时输出。
4. `--module-root ROOT ENTRY` 与 `-e`、普通文件/标准输入、`--interactive`、`--check`、`--dump-bytecode` 和 `--fingerprint` 互斥。`--fuel`、`--stats`、`--json` 和 `--` 后的字符串 `argv` 保持可用。普通 CLI 模式与嵌入 API 不会因此取得文件加载能力。
5. loader、编译、导入、运行和资源错误沿用既有结构化错误类别；已命名模块的 parse/verify/runtime JSON 诊断携带其规范模块来源名。`--stats` 继续写入标准错误。

## 验收

黑盒 CLI 测试必须覆盖嵌套导入、扩展名推断、导出结果、argv、JSON、统计、fuel、缺失 root/entry、依赖 parse 错误来源、循环、词法越界、支持平台上的符号链接逃逸以及所有执行模式冲突。中英文语法索引、可执行手册和 README 必须说明只有该显式开关授予此能力；debug/release、Clippy、rustdoc、crate package、文档和性能门禁必须通过。

## 2026-08-27：非执行图检查

RFC 0151 修订第 4 条：`--fingerprint` 可与 `--module-root ROOT ENTRY` 明确组合，仅加载并验证图后输出模块图指纹，不执行模块。它仍与 `--json`、`--stats`、普通源码、check、反汇编和交互模式互斥；没有 `--module-root` 的普通 fingerprint 绝不取得文件图权限。
