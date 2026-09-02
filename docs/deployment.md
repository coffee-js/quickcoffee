# 生产嵌入指南 / Production embedding cookbook

## 中文

### 固定依赖

QuickCoffee 0.1 暂未发布到 crates.io。Rust 宿主应固定经过验证的 `v0.1.0` 完整 commit，而不是跟随浮动分支：

```toml
[dependencies]
quickcoffee = { git = "https://github.com/coffee-js/quickcoffee.git", rev = "b3d27d24d15d76786baa21614b9cc2a97b28579e" }
```

升级时只把 `rev` 替换为已经审阅和测试的新完整 commit。

### 默认：每个请求一个 Context

对普通规则、校验和数据整形，采用以下结构：

1. 每个 OS worker 创建一个 `Runtime`，并使用 `ExecutionPolicy::isolated_request()`。
2. 在 worker 启动时编译 `Program` 或准备 `ModulePackage`，随后复用已验证产物。
3. 每个请求创建新的 `Context`，显式注入输入、host state、capability 和 cancellation。
4. 执行后在 worker 内把 `Value` 或 `Error` 转成普通、owned Rust 数据，再跨线程返回。
5. 丢弃 `Context`；不要把一个请求的 globals、host state 或 capability 带入下一个请求。

现有多文件策略示例实现了这条路径：

```sh
cargo run --example policy_package
cargo test --test policy_workflow
```

### 生命周期选择

| 模式 | 何时使用 | 约束 |
|---|---|---|
| 短请求 Context | 默认选择；独立规则请求 | 每次新建 Context，复用同 worker 的 Runtime 和已验证产物。 |
| 有限批次 Context | 同一可信任务需要显式共享少量状态 | 设置明确的批次大小，批次结束即丢弃 Context。 |
| 长生命周期 Context | REPL 或业务明确要求持久脚本状态 | 宿主接受 retained state、cycle 和 callback 生命周期，并自行观测与回收 worker。 |

不要为了减少 Context 创建成本直接选择长生命周期模式；现有基准已单独测量创建成本，只有真实宿主预算证明必要时才改变默认方式。

### Worker 与取消

`Runtime`、`Program`、`ModulePackage`、`Context`、`Value` 和 `Error` 都留在创建它们的 worker。每个 worker 独立准备运行时和模块包。只有普通 owned Rust 输入输出以及 `CancellationToken` 跨线程。

```sh
cargo run --example per_worker
cargo test --test thread_ownership
```

`CancellationToken` 是协作式停止信号。QuickCoffee 执行会检查它；同步且不协作的 native callback 不会被强制中断，宿主 panic 也不会被 VM 捕获。

### 不可信脚本

进程内 `Context` 提供确定性执行控制和资源纵深防御，不是完整的敌对代码沙箱。运行真正不可信的脚本时，宿主还必须：

- 使用独立进程或容器，并限制 CPU、内存、进程、文件描述符和可见文件系统。
- 设置外部 wall-clock deadline；超时后终止并回收 worker 进程。
- 限制进入子进程的源码、模块、请求和输出大小。
- 默认不提供 capability；只向受信任 adapter 显式传入必要权限。
- 把子进程结果解析成普通 Rust 数据，不跨边界保留 VM 对象。

QuickCoffee 不内建进程管理器或系统 capability adapter；这些边界属于部署宿主。

## English

### Pin the dependency

QuickCoffee 0.1 is not published on crates.io. Rust hosts should pin the complete verified `v0.1.0` commit rather than follow a moving branch:

```toml
[dependencies]
quickcoffee = { git = "https://github.com/coffee-js/quickcoffee.git", rev = "b3d27d24d15d76786baa21614b9cc2a97b28579e" }
```

To upgrade, replace `rev` only with another complete commit that you have reviewed and tested.

### Default: one Context per request

Use this structure for ordinary rules, validation, and data shaping:

1. Create one `Runtime` per OS worker with `ExecutionPolicy::isolated_request()`.
2. Compile a `Program` or prepare a `ModulePackage` when the worker starts, then reuse the verified artifact.
3. Create a fresh `Context` per request and explicitly inject input, host state, capabilities, and cancellation.
4. Convert `Value` or `Error` into ordinary owned Rust data inside the worker before returning it across a thread boundary.
5. Drop the `Context`; do not carry globals, host state, or capabilities from one request into the next.

The existing multi-file policy example implements this path:

```sh
cargo run --example policy_package
cargo test --test policy_workflow
```

### Choose a lifecycle

| Mode | When to use it | Constraint |
|---|---|---|
| Short-request Context | Default for independent rule requests | Create a fresh Context and reuse the worker-owned Runtime and verified artifacts. |
| Bounded-batch Context | One trusted job explicitly shares limited state | Set a fixed batch size and drop the Context when the batch ends. |
| Long-lived Context | A REPL or business case explicitly requires persistent script state | The host accepts retained state, cycles, and callback lifetimes, and monitors and recycles the worker. |

Do not select a long-lived Context merely to avoid Context creation. Existing benchmarks measure that cost separately; change the default only when a real host budget proves it necessary.

### Workers and cancellation

Keep `Runtime`, `Program`, `ModulePackage`, `Context`, `Value`, and `Error` inside the worker that created them. Each worker prepares its own runtime and package. Only ordinary owned Rust inputs and outputs plus `CancellationToken` cross threads.

```sh
cargo run --example per_worker
cargo test --test thread_ownership
```

`CancellationToken` is cooperative. QuickCoffee execution checks it, but a synchronous uncooperative native callback cannot be forcibly interrupted, and the VM does not catch host panics.

### Untrusted scripts

An in-process `Context` provides deterministic execution controls and defense in depth, not a complete hostile-code sandbox. For truly untrusted scripts, the host must also:

- Use a separate process or container with CPU, memory, process, file-descriptor, and visible-filesystem limits.
- Enforce an external wall-clock deadline, then terminate and recycle the worker process on timeout.
- Bound source, modules, requests, and output entering or leaving the child process.
- Grant no capabilities by default and pass only required authority to trusted adapters.
- Parse child-process results into ordinary Rust data without retaining VM objects across the boundary.

QuickCoffee deliberately provides neither a process manager nor system capability adapters; those boundaries belong to the deployment host.
