# QuickCoffee

QuickCoffee 是一台以 Rust 编写、受 CoffeeScript 启发的字节码脚本引擎。它保留紧凑、可读的表达式语法，却不兼容 JavaScript：没有公开原型链、全局/自由 `this`、`eval` 或嵌入 JavaScript；RFC 0134 另为 class 内部定义受限接收者、构造与继承。

当前实现遵循 [RFCs/0000-project-scope.md](RFCs/0000-project-scope.md) 至 [RFCs/0138-deterministic-script-json.md](RFCs/0138-deterministic-script-json.md)；RFC 0134 的 class 构造与受限接收者阶段已经实现，继承/`super` 与接收者绑定 `=>` 仍由 issue #121 分阶段推进，详见 [RFCs/0134-class-receivers-and-inheritance.md](RFCs/0134-class-receivers-and-inheritance.md)。
构建要求 Rust 1.85 或更新版本（Edition 2024）；CI 同时验证 MSRV 与 stable 工具链。
后续大需求、里程碑和拆分规则见 [ROADMAP.md](ROADMAP.md)。
当前业务适用范围、语言缺口与 QuickJS 性能对照见 [docs/readiness.zh-CN.md](docs/readiness.zh-CN.md)。
与 CoffeeScript 1.12.7 官方语言参考的逐项差异见 [特性矩阵](docs/coffeescript-2016-matrix.md)。

内部前端边界保持显式：`lexer` 生成带范围的 token，`parser` 只负责语法与恢复并产出 `ast`，`lowering` 把 AST 降为字节码及不参与编码的 source-map sidecar，`bytecode` 只负责指令表示、验证、指纹与反汇编，最后由 `vm` 执行。该拆分不改变公开 API、字节码编码/指纹、fuel 或诊断输出。

```coffee
square = (x) -> x * x
if square(7) == 49 then print('ok') else print('failed')
```

也可使用 CoffeeScript 风格的空格缩进块：

```coffee
double = (x) ->
  next = x + 1
  next * 2
```

```sh
cargo run -- -e "print(range(1, 4))"
cargo run -- - < program.qc
cargo run -- --interactive
cargo run -- --quit
cargo run -- example.qc -- first second
cargo run -- --check example.qc
cargo run -- --dump-bytecode example.qc
cargo run -- --fingerprint example.qc
cargo run -- --json -e "{answer: 42}"
cargo run --release --bin qbench -- --json --iterations 100
cargo run --release --bin qbench -- --json --iterations 100 --repeat 3
cargo run --release --bin qbench -- --list
cargo run --release --bin qbench -- --only map-spread --json --iterations 100
cargo build --release --bins && qbench --compare-qjs /path/to/qjs --compare-iterations 1 --repeat 11 --json
cargo run --locked --quiet --release --bin qbench -- --json --iterations 1 --repeat 3
cargo run --example embed
cargo run --example modules
cargo run --bin qdocco -- example.qc -o example.html
cargo run --bin qdocco -- --markdown example.qc -o example.md
cargo run --bin qtest -- tests/scripts
cargo run --bin qtest -- --json tests/scripts
cargo run --bin qtest -- --tap tests/scripts
cargo run --bin qtest -- --version
cargo run --bin qdocco -- --version
cargo run --bin qbench -- --version
```

`qbench` 把 `Engine::compile` 的普通编译记录为 `compile_*`，并用新增的 `prepare_*` 单独记录 `Engine::compile_program` 的端到端准备成本（含源码映射、验证与私有执行 sidecar）；二者都遵循 `--iterations` / `--repeat` 的中位数与 MAD 口径。每条记录还包含一次不计时执行的 `profile_*` 计数，用于比较指令热点、调用深度、托管值分配和词法环境分配；这些计数不乘以 `--iterations` 或 `--repeat`。

`qbench --compare-qjs` 的 `quickcoffee_startup_*` / `quickjs_startup_*`、`*_compile_*` 与 `*_hot_*` 分别报告启动、编译和预编译热执行；既有 `*_cli_*` 继续表示端到端子进程总耗时。各阶段均输出中位数与 MAD，正式报告宜使用 `--repeat 11`。

Pull request 的 `Performance report` workflow 会在同一 runner 上按 ABBA/BAAB 为每个共有负载运行两组 base/head 完整 11 样本 qbench，保存带顺序与 pair 标识的原始 JSONL 和机器/工具链元数据。只有两个运行方向都越过 5% 相对下限、3 倍组合 MAD 与 0.1 ms 绝对下限时才生成非阻塞 warning；全局 common-mode 只报告而不从单项效应中扣除。该报告是 review 信号，不是跨机器或发布阻塞阈值；比较策略及本地命令见 [PERFORMANCE.md](PERFORMANCE.md#issue-107-同-runner-非阻塞回归报告)。

`qcoffee --interactive`（或 `-i`）提供持久上下文的交互会话；`:help` 显示命令，`:quit`/`:exit` 离开。管道输入时不会输出提示，适合脚本驱动；加 `--stats` 可为每个成功执行或运行时失败的非空输入行输出统计，解析错误不生成新记录。

嵌入宿主可用 `compile_named`、`compile_program_named` 或 `Context::eval_named` 把虚拟文档名原样附到结构化错误 label；匿名 API 保持不变。`Engine::check_program*` 只作静态检查，并在安全的顶层边界收集多个 parser error；`qcoffee --check FILE` 将它们按源序写到标准错误而不执行。经 `Program` 编译的顶层、嵌套函数、默认参数和模块运行期/verification 错误保留 source map，运行期调用链追加有序 secondary label；宿主手工构造的裸 `Chunk` 不虚构来源。`qcoffee --json FILE` 的错误在位置已有来源时额外输出可选 `source` 字段。

模块文件权限保持显式：嵌入宿主可构造 `RestrictedFileModuleLoader`，把 `./` / `../` 导入限制在一个规范根目录及 `.qc` UTF-8 文件内，并拒绝词法越界与符号链接逃逸。普通编译、求值和 `qcoffee` 单文件模式不会自动读取依赖文件。

## 验收

`make check` 运行格式检查、debug 与 release 两套全部测试（含外部嵌入 API 集成测试和 1,024 条确定性编译压力语料）、零警告 Clippy 和五份可执行手册校验；`make docs` 从文学编程源重新生成 HTML；`make bench` 运行 release 基准。项目禁止 `unsafe`。

手册源在 `manuals/`，每份都是可执行的 Docco 输入。生成 HTML：

```sh
for source in manuals/*.qc; do
  locale="${source#manuals/manual.}"; locale="${locale%.qc}"
  cargo run --bin qdocco -- "$source" -o "docs/manual.$locale.html"
done
```

生成的手册：[中文](docs/manual.zh-CN.html)、[宋代官话古文](docs/manual.classical-zh.html)、[English](docs/manual.en.html)、[Latine](docs/manual.latin.html)、[天城文](docs/manual.devanagari-sa.html)。源文本见 [manuals](manuals)，语法范围见 [中文](docs/syntax.zh-CN.md) 与 [English](docs/syntax.en.md)。
