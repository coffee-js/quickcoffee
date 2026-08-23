# RFC 0106：crate 发布包验收门禁

- 状态：已采纳
- 依赖：RFC 0086、RFC 0087

成熟发布验收除 `cargo metadata --locked` 外，还必须运行 `cargo publish --dry-run --locked --allow-dirty`。Cargo 将按发布清单生成 crate、校验发布元数据，并在隔离的包目录中构建它；因此缺失源文件、示例、文档、RFC 或锁文件不一致会在 CI 中暴露，而不是等到真正发布时才发现。

`make check` 包含该门禁。`--dry-run` 会执行 registry 上传前的验证与构建，但不会上传 crate、修改版本或更改远程状态；`--allow-dirty` 允许此前 `make docs` 生成待检查的文档，CI 随后以 `git diff --exit-code -- docs` 拒绝未提交的文档变更。生成物只位于 Cargo 的临时目录。
