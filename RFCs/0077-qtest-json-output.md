# RFC 0077：`qtest` JSON 输出

状态：实现中

`qtest --json FILE_OR_DIRECTORY...` 为每个测试文件输出一行 JSON：成功记录含 `ok: true` 与 `file`，失败记录含 `ok: false`、`file` 与经过 JSON 转义的 `error`。退出码仍以所有测试是否通过为准；`--stats` 继续把执行统计写到标准错误，不污染机器可读标准输出。

该格式不依赖第三方 JSON 库，字段稳定，适合 CI、编辑器和宿主系统消费。
