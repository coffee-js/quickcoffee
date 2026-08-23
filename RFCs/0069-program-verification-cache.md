# RFC 0069：共享 `Program` 验证缓存

- 状态：已接受
- 依赖：RFC 0002、RFC 0046、RFC 0048

## 摘要

`Program` 是不可变的共享字节码句柄。`Engine::compile_program` 创建它时完成递归验证；验证成功后，克隆句柄并用 `Context::run_program` 重复执行不再重复遍历相同字节码。由公开 `From<Chunk>` 构造的句柄初始为未验证，第一次运行仍必须通过完整验证。

## 安全边界

验证状态与私有不可变字节码存放在同一共享内部对象中，宿主不能通过 `Program` 修改 `Chunk`。`Program::verify()` 成功后记录验证完成；失败不缓存结果。未验证句柄的错误执行不会进入 VM，也不会更新执行统计。

每次真正执行仍创建新的 VM、fuel 预算、调用帧和迭代器状态；缓存只跳过重复验证，不共享脚本全局环境，也不削弱 RFC 0048 的恶意字节码检查。

## 性能契约

基准必须分别记录编译、验证（如单独测量）和执行吞吐；共享 `Program` 的执行路径不得因验证缓存复制指令流。语义护栏仍须在执行前后检查期望值与公开执行统计。

## 验收

- `compile_program` 返回的程序可重复运行且不复制字节码。
- `Program::from(Chunk)` 构造的非法字节码在首次 `run_program` 时被拒绝。
- 验证失败不会更新 `Context::last_execution()`。
- `make check` 与三轮 `cargo bench --bench core` 通过，并在性能报告中记录结果。
