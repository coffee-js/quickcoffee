# QuickCoffee

QuickCoffee 是一台以 Rust 编写、受 CoffeeScript 启发的字节码脚本引擎。它保留紧凑、可读的表达式语法，却不兼容 JavaScript：没有原型链、`this`、`eval` 或嵌入 JavaScript。

当前实现遵循 [RFCs/0000-project-scope.md](RFCs/0000-project-scope.md) 至 [RFCs/0061-string-escapes.md](RFCs/0061-string-escapes.md)。

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
cargo run -- example.qc -- first second
cargo run -- --check example.qc
cargo run -- --dump-bytecode example.qc
cargo run --bin qdocco -- example.qc -o example.html
cargo run --bin qtest -- tests/scripts
```

## 验收

`make check` 运行格式检查、全部测试（含 1,024 条确定性编译压力语料）、零警告 Clippy 和五份可执行手册校验；`make docs` 从文学编程源重新生成 HTML；`make bench` 运行 release 基准。项目禁止 `unsafe`。

手册源在 `manuals/`，每份都是可执行的 Docco 输入。生成 HTML：

```sh
for source in manuals/*.qc; do
  locale="${source#manuals/manual.}"; locale="${locale%.qc}"
  cargo run --bin qdocco -- "$source" -o "docs/manual.$locale.html"
done
```

生成的手册：[中文](docs/manual.zh-CN.html)、[宋代官话古文](docs/manual.classical-zh.html)、[English](docs/manual.en.html)、[Latine](docs/manual.latin.html)、[天城文](docs/manual.devanagari-sa.html)。源文本见 [manuals](manuals)，语法范围见 [中文](docs/syntax.zh-CN.md) 与 [English](docs/syntax.en.md)。
