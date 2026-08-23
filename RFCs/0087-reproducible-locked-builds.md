# RFC 0087：锁定依赖的可复现构建

- 状态：已采纳
- 依赖：RFC 0086

## 动机

QuickCoffee 的命令行工具、嵌入 API、文学编程手册和基准都由 Cargo 构建。仓库提交
`Cargo.lock` 后，若验收命令省略 `--locked`，Cargo 仍可能解析新的依赖版本并修改锁文件，
导致本地结果、CI 结果和发布包之间出现不可复现差异。

## 契约

除 `cargo fmt` 外，Makefile 中所有会解析或编译依赖的 Cargo 命令必须带 `--locked`：
测试、示例、元数据、Clippy、rustdoc、qdocco、qbench 与基准均包括在内。CI 只调用这些
入口，因此 pull request 和 push 验证使用与开发者相同的锁定依赖图。锁文件不应由验收命令
自动更新；依赖升级必须在独立变更中显式修改 `Cargo.lock`。

## 验收

`make check`、`make docs`、`make bench` 和 `make qbench` 均应在存在 `Cargo.lock` 时成功运行，
且 `git diff -- Cargo.lock` 为空。CI 继续检查生成的手册没有未提交差异。
