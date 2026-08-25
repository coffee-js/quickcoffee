# RFC 0118：嵌入执行资源治理

- 状态：已采纳
- 依赖：RFC 0002、RFC 0066、RFC 0084、RFC 0089、RFC 0114

## 动机

fuel 能限制无限循环，却不能明确区分资源耗尽与普通运行时错误，也不能约束递归函数。嵌入宿主还需要从另一线程或请求生命周期中取消尚未结束的脚本。因此执行上下文必须提供小而稳定的资源治理接口，同时不向脚本暴露 VM 帧、线程、对象地址或 JavaScript 运行时模型。

## 契约

1. `ErrorKind` 增加 `Resource`；`Error::resource_limit()` 返回稳定的 `ResourceLimit`，其他错误返回 `None`。首批执行类原因是 `Fuel`、`CallDepth`、`Cancellation`；RFC 0138 的首批数据大小原因是 `JsonInputBytes`、`JsonOutputBytes`、`JsonStringBytes`、`JsonContainerItems`、`JsonValueCount`、`JsonNestingDepth`。原生回调继续以 `Error::runtime` 创建 `Runtime` 错误。
2. fuel 耗尽是 `ResourceLimit::Fuel`，消息仍包含 `execution fuel exhausted`。资源错误不进入 QuickCoffee 的 `try`/`catch`，因此脚本不能吞掉取消、fuel 或深度限制后继续执行。
3. `Context` 默认允许最多 1,024 层嵌套 QuickCoffee 字节码函数调用；顶层程序不计入层数。`with_max_call_depth`、`set_max_call_depth` 与 `max_call_depth` 配置每次后续运行；零只允许顶层代码，拒绝任何字节码函数调用。原生 Rust 回调不新增 QuickCoffee 调用帧。
4. `CancellationToken` 是可克隆、一次性的宿主取消信号。`Context::with_cancellation_token` 或 `set_cancellation_token` 配置它，`clear_cancellation_token` 移除它。VM 在每条指令之前检查取消；已开始执行的同步原生回调不能被强行中断，宿主回调应自行遵守自己的取消策略。
5. `ExecutionStats` 增加 `call_depth_peak`，记录本次运行达到的最大嵌套 QuickCoffee 函数深度。编译/验证失败仍保留上一条统计；资源错误和普通运行时错误均写入本次统计。

## 2026-08-25：可配置数据大小策略

1. `ResourceLimits::default()` 保存确定性的 Context 数据大小策略；首个实现字段对应 RFC 0138 的 JSON 输入、输出、单字符串、单容器项数、单次值数及容器嵌套深度，默认值与 RFC 0138 原固定守卫一致。
2. `Context::with_resource_limits`、`set_resource_limits` 与 `resource_limits` 分别链式设置、替换和读取策略。替换只影响后续操作，不清空全局、共享 Program 或统计；已安装 builtin 与模块子 Context 观察同一轮运行所复制的当前策略。
3. JSON 语法、重复键、非法 Unicode、类型和不可编码值仍产生可捕获的 `json.parse` / `json.encode` Error；只有越过配置边界时产生不可捕获的 `Resource` error。消息包含操作与配置阈值，宿主分支必须使用 `resource_limit()` 而不是解析消息。
4. `parse_json` 在返回前只构造局部结果，`encode_json` 在返回前只构造局部 String；越界不写脚本状态且不返回部分值。输入长度在解析前检查，容器项与总值在新增前检查，输出在追加前检查；单个 UTF-8 scalar 的解码检查不允许越界结果逃逸。
5. 这只是 issue #76 的首个 data-size 切片。Integer/Decimal、一般字符串/容器、模块、源码/字节码、managed-object、retained-memory 和 lifetime/cycle 策略仍需独立扩展，不由 JSON 字段隐式代替。

## 验收

`tests/embedding_api.rs` 必须覆盖 fuel、递归深度、预取消 token、替换 token、JSON 策略替换、资源错误不可被 `catch` 吞掉、源码标签及深度峰值；模块测试覆盖策略继承，JSON 单元测试覆盖每个边界的等于/越过行为。CLI JSON 必须输出 `kind:"resource"` 的资源错误；嵌入示例、中英文语法索引与可执行手册说明该 API。完整 `make check` 必须通过。
