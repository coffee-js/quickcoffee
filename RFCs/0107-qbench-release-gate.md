# RFC 0107：release qbench 持续门禁

- 状态：已采纳
- 依赖：RFC 0045、RFC 0081、RFC 0096、RFC 0105

`make check` 除编译和测试外，必须以 release profile 执行 `qbench --json --iterations 1 --repeat 3`。该命令运行全部内建负载、每轮检查最终值语义护栏，并要求 qbench 进程成功退出；输出重定向到标准输出外的空设备，避免把耗时噪声混入 CI 日志。

该门禁不是跨机器性能比较，也不对纳秒阈值作断言；它确保优化构建、字节码验证缓存、VM 执行路径和可重复采样接口持续可运行。详细性能数据仍由 `cargo bench --bench core` 与 `PERFORMANCE.md` 记录。
