# RFC 0124：跨运行时基准阶段分离

- 状态：已采纳
- 依赖：RFC 0081、RFC 0103、RFC 0120、RFC 0121

`qbench --compare-qjs PATH` 在 RFC 0120 的端到端 CLI 读数之外，分别报告进程启动、编译与预编译热执行。启动阶段重复运行 `qcoffee --quit` 与 `qjs --quit`；`qcoffee --quit` 只创建一个 `Context` 后静默成功退出，且不能与源码或其他执行选项组合。启动值是每个样本内 `compare_iterations` 次子进程的墙钟总耗时。

QuickCoffee 编译阶段在 `qbench` 进程内重复调用 `Engine::compile`；热执行阶段先以 `Engine::compile_program` 创建并验证一个 `Program`，再在同一个 `Context` 中重复运行。QuickJS 阶段在一次 `qjs --std` 进程内用 `os.now()` 自计时：编译通过 `std.evalScript` 重复求值得到函数，热执行复用最后得到的函数。双方每次热执行都校验等价负载的最终值，计时区间不含标准输出。

`quickcoffee.qcompare.v1` 保留 `quickcoffee_cli_ns`、`quickjs_cli_ns` 及其 MAD 含义，并新增：

- `quickcoffee_startup_ns`、`quickjs_startup_ns`
- `quickcoffee_compile_ns`、`quickjs_compile_ns`
- `quickcoffee_hot_ns`、`quickjs_hot_ns`
- 上述每个字段对应的 `*_mad_ns`

所有 `*_ns` 都是一个样本内 `compare_iterations` 次操作的总耗时，再对 `repeat` 个样本取上侧中位数；每个 MAD 按 RFC 0121 计算。CLI 总耗时不减去启动中位数，阶段值之间也不相减。正式报告仍应使用至少 11 个样本，并只在同机、同构建、相同负载下比较；纳秒字段不表示计时器自身具有纳秒分辨率。

该协议不改变语言语义、fuel、`qbench.v1` 或既有 qcompare 字段。验收覆盖静默启动模式、冲突参数、QuickJS 阶段输出解析、语义护栏、文本/JSON 新字段和 `repeat=1` 时为零的 MAD。
