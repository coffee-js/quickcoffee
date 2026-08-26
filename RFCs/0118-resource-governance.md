# RFC 0118：嵌入执行资源治理

- 状态：已采纳
- 依赖：RFC 0002、RFC 0066、RFC 0084、RFC 0089、RFC 0114

## 动机

fuel 能限制无限循环，却不能明确区分资源耗尽与普通运行时错误，也不能约束递归函数。嵌入宿主还需要从另一线程或请求生命周期中取消尚未结束的脚本。因此执行上下文必须提供小而稳定的资源治理接口，同时不向脚本暴露 VM 帧、线程、对象地址或 JavaScript 运行时模型。

## 契约

1. `ErrorKind` 增加 `Resource`；`Error::resource_limit()` 返回稳定的 `ResourceLimit`，其他错误返回 `None`。执行类原因是 `Fuel`、`CallDepth`、`Cancellation`；数据大小原因包括 RFC 0138 的 JSON 六类边界、`IntegerBits`、`DecimalCoefficientBits`、`DecimalScale` 及 RFC 0140 的 `CollectionOperationItems`。原生回调主动失败时继续以 `Error::runtime` 创建 `Runtime` 错误；其成功返回值仍须通过 Context 数值策略。
2. fuel 耗尽是 `ResourceLimit::Fuel`，消息仍包含 `execution fuel exhausted`。资源错误不进入 QuickCoffee 的 `try`/`catch`，因此脚本不能吞掉取消、fuel 或深度限制后继续执行。
3. `Context` 默认允许最多 1,024 层嵌套 QuickCoffee 字节码函数调用；顶层程序不计入层数。`with_max_call_depth`、`set_max_call_depth` 与 `max_call_depth` 配置每次后续运行；零只允许顶层代码，拒绝任何字节码函数调用。原生 Rust 回调不新增 QuickCoffee 调用帧。
4. `CancellationToken` 是可克隆、一次性的宿主取消信号。`Context::with_cancellation_token` 或 `set_cancellation_token` 配置它，`clear_cancellation_token` 移除它。VM 在每条指令之前检查取消；已开始执行的同步原生回调不能被强行中断，宿主回调应自行遵守自己的取消策略。
5. `ExecutionStats` 增加 `call_depth_peak`，记录本次运行达到的最大嵌套 QuickCoffee 函数深度。编译/验证失败仍保留上一条统计；资源错误和普通运行时错误均写入本次统计。

## 2026-08-25：可配置数据大小策略

1. `ResourceLimits::default()` 保存确定性的 Context 数据大小策略；首个实现字段对应 RFC 0138 的 JSON 输入、输出、单字符串、单容器项数、单次值数及容器嵌套深度，默认值与 RFC 0138 原固定守卫一致。
2. `Context::with_resource_limits`、`set_resource_limits` 与 `resource_limits` 分别链式设置、替换和读取策略。替换只影响后续操作，不清空全局、共享 Program 或统计；已安装 builtin 与模块子 Context 观察同一轮运行所复制的当前策略。
3. JSON 语法、重复键、非法 Unicode、类型和不可编码值仍产生可捕获的 `json.parse` / `json.encode` Error；只有越过配置边界时产生不可捕获的 `Resource` error。消息包含操作与配置阈值，宿主分支必须使用 `resource_limit()` 而不是解析消息。
4. `parse_json` 在返回前只构造局部结果，`encode_json` 在返回前只构造局部 String；越界不写脚本状态且不返回部分值。输入长度在解析前检查，容器项与总值在新增前检查，输出在追加前检查；单个 UTF-8 scalar 的解码检查不允许越界结果逃逸。
5. 这只是 issue #76 的首个 data-size 切片。一般字符串/容器、模块、源码/字节码、managed-object、retained-memory 和 lifetime/cycle 策略仍需独立扩展，不由 JSON 字段隐式代替。

## 2026-08-25：精确数值策略

1. `ResourceLimits` 增加单个 Integer magnitude bit、规范 Decimal coefficient magnitude bit 与规范 Decimal scale 三个字段，默认分别为 1,000,000、1,000,000、100,000，与 RFC 0135/0137 的原固定守卫兼容。配置值不能抬高编译器和公开宿主构造 API 仍保留的同值绝对实现天花板。
2. `Engine` 在 Context 之前按绝对实现天花板解析和 lowering 字面量；共享 `Program`、字节码与指纹不写入某个 Context 的策略。每次运行在常量、全局值、模块/default chunk 和 native 返回值进入脚本前按当前 Context 复核，因此同一 Program 可在不同策略下确定性执行。
3. `Context::set_global` 保持兼容的无失败宿主 API，`Integer::parse_radix`、`Decimal::parse` 与 `Decimal::from_parts` 继续报告宿主构造失败；低于绝对天花板的 Context 策略在脚本实际读取该值时产生带源码标签、不可捕获的 Resource error。宿主仍可用 `get_global` 取回未被脚本接受的值。
4. Integer 乘法、幂、移位和聚合，以及 Decimal 对齐、比较、聚合、乘法、幂、除法、舍入和转换，在可证明超限时先拒绝；所有最终数值在入栈、存储或返回前再次检查。失败指令不会执行其后的 assignment store。
5. `parse_json` 的合法数字若越过同一数值策略，使用上述三种 ResourceLimit；JSON 数字语法错误仍为可捕获的 `json.parse`。JSON 容器计数与数值大小策略彼此独立。

## 2026-08-26：集合操作项数策略

1. RFC 0140 增加 `max_collection_operation_items`，默认 100,000，并以 `CollectionOperationItems` 标识越界；它限制一次集合标准库操作读取的输入项数，首个使用者是 `sort`。
2. 操作在复制输入或分配比较键之前检查边界。该策略限制单次 builtin 内隐藏的工作规模，但不替代一般容器长度、输出增长、总内存或 retained-memory 策略。
3. Decimal 集合操作继续同时遵守 coefficient-bit 与 scale 策略；集合项数允许并不授权比较过程构造超限的精确数值中间量。

## 2026-08-26：通用语言值大小策略

1. `ResourceLimits` 增加 `max_string_bytes`、`max_array_items` 与 `max_map_entries`，默认分别为 1,000,000 UTF-8 bytes、100,000 items 与 100,000 entries；`StringBytes`、`ArrayItems` 与 `MapEntries` 是稳定、不可捕获的 `ResourceLimit` 原因。这些字段约束普通 QuickCoffee 值，不复用 JSON 专用字段。
2. 同一 `Program`、裸字节码和宿主构造的 `Value` 不保存某个 Context 的策略。常量、名称读取、native 返回、成员读取与模块子 Context 在值进入脚本时按当前策略递归复核；不同 Context 可以确定性地以不同限制复用同一 Program 或宿主值。
3. Array/Map 字面量、range、array spread/append、Map spread、string interpolation/concat/stringify、slice 与 class 实例/静态字段写入在结果可见或赋值提交前检查其边界。超限不执行随后 store，资源错误不能被 `try`/`catch` 吞掉。
4. 这仍不是总分配或 retained-memory 账本：一次失败路径可以有局部临时分配，且模块图、源码/bytecode、managed object 数、生命周期/cycle 与总 Context 预算继续由 issue #76 的后续切片定义。

## 验收

`tests/embedding_api.rs` 必须覆盖 fuel、递归深度、预取消 token、替换 token、JSON/数值/通用语言值策略替换、资源错误不可被 `catch` 吞掉、共享 Program、宿主全局/native 返回、失败后恢复、源码标签及深度峰值；模块测试覆盖策略继承，JSON 单元测试覆盖边界类别。CLI JSON 必须输出 `kind:"resource"` 的资源错误；嵌入示例、中英文语法索引与可执行手册说明该 API。完整 `make check` 必须通过。
