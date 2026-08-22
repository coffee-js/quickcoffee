# RFC 0027：CLI 编译检查模式

## 决议

`qcoffee --check FILE`（FILE 可为 `-`）读取源码，执行词法、解析、编译与 `Chunk::verify`，但绝不创建执行 Context 或运行字节码。成功时无标准输出并以零退出；读取、语法、编译或验证错误以非零退出。它不能与 `--dump-bytecode` 组合。

该模式面向 CI、编辑器与文学文档流程：例如无限循环是有效程序，`qcoffee --fuel 1 --check program.qc` 必须成功，因为 fuel 仅限制执行。

## 验证

CLI 集成测试覆盖正常检查的静默成功、无限循环在极低 fuel 下仍不执行、非法源码失败，以及既有运行与反汇编模式不变。
