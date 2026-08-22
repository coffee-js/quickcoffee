# RFC 0106：crate 发布包验收门禁

- 状态：已采纳
- 依赖：RFC 0086、RFC 0087

成熟发布验收除 `cargo metadata --locked` 外，还必须运行 `cargo package --locked`。Cargo 将按发布清单生成 crate，并在隔离的包目录中编译它；因此缺失源文件、示例、文档、RFC 或锁文件不一致会在 CI 中暴露，而不是等到发布时才发现。

`make check` 包含该门禁，CI 使用干净工作树运行同一命令。该步骤不发布 crate，也不修改版本或远程状态；生成物只位于 Cargo 的 `target/package` 临时目录。
