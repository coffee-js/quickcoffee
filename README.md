# QuickCoffee

QuickCoffee 是一台以 Rust 编写、受 CoffeeScript 启发的字节码脚本引擎。它保留紧凑、可读的表达式语法，却不兼容 JavaScript：没有原型链、`this`、`eval` 或嵌入 JavaScript。

当前实现遵循 [RFCs/0000-project-scope.md](RFCs/0000-project-scope.md) 至 [RFCs/0093-value-kind-inspection.md](RFCs/0093-value-kind-inspection.md)。

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
cargo run -- example.qc -- first second
cargo run -- --check example.qc
cargo run -- --dump-bytecode example.qc
cargo run -- --fingerprint example.qc
cargo run --release --bin qbench -- --json --iterations 100
cargo run --example embed
cargo run --bin qdocco -- example.qc -o example.html
cargo run --bin qdocco -- --markdown example.qc -o example.md
cargo run --bin qtest -- tests/scripts
cargo run --bin qtest -- --json tests/scripts
cargo run --bin qtest -- --tap tests/scripts
cargo run --bin qtest -- --version
cargo run --bin qdocco -- --version
cargo run --bin qbench -- --version
```

`qcoffee --interactive`（或 `-i`）提供持久上下文的交互会话；`:help` 显示命令，`:quit`/`:exit` 离开。管道输入时不会输出提示，适合脚本驱动；加 `--stats` 可为每个成功执行或运行时失败的非空输入行输出统计，解析错误不生成新记录。

## 验收

`make check` 运行格式检查、全部测试（含外部嵌入 API 集成测试和 1,024 条确定性编译压力语料）、零警告 Clippy 和五份可执行手册校验；`make docs` 从文学编程源重新生成 HTML；`make bench` 运行 release 基准。项目禁止 `unsafe`。

手册源在 `manuals/`，每份都是可执行的 Docco 输入。生成 HTML：

```sh
for source in manuals/*.qc; do
  locale="${source#manuals/manual.}"; locale="${locale%.qc}"
  cargo run --bin qdocco -- "$source" -o "docs/manual.$locale.html"
done
```

生成的手册：[中文](docs/manual.zh-CN.html)、[宋代官话古文](docs/manual.classical-zh.html)、[English](docs/manual.en.html)、[Latine](docs/manual.latin.html)、[天城文](docs/manual.devanagari-sa.html)。源文本见 [manuals](manuals)，语法范围见 [中文](docs/syntax.zh-CN.md) 与 [English](docs/syntax.en.md)。
