# RFC 0046：共享编译程序嵌入 API

- 状态：已采纳
- 依赖：RFC 0002、RFC 0043、RFC 0045

`Program` 是字节码的公开句柄，内部持有共享存储但不向宿主暴露 `Rc`；由编译器产生的句柄已验证，由 `Chunk` 转换的句柄在首次使用时验证。`Engine::compile_program` 与顶层 `compile_program` 产生 `Program`；宿主可克隆该句柄，`Context::run_program(&program)` 可重复执行它。`Program::verify()` 与 `disassemble()` 提供与 `Chunk` 对应的验证/反汇操作。

旧的 `Context::run(Chunk)` 保留兼容性并继续验证传入字节码；`Context::eval` 使用共享程序路径。共享程序首次验证成功后缓存验证状态，后续运行仍新建 VM fuel 预算和调用帧，不能共享脚本全局状态或绕过验证。该 API 使长期缓存的编译结果不必为每次执行深拷贝指令流或重复验证（RFC 0069）。

验收覆盖程序克隆、重复运行、反汇编/验证以及旧 `Chunk` 入口；性能基准使用 `Program` 执行路径并保留 RFC 0045 的语义护栏。
