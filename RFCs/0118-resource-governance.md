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
2. 同一 `Program`、裸字节码和宿主构造的 `Value` 不保存某个 Context 的策略。常量、名称读取、native 返回、成员读取与模块子 Context 在值进入脚本时按当前策略递归复核；不同 Context 可以确定性地以不同限制复用同一 Program 或宿主值。`nil`、Bool 和 Number 不携带该组可配置的大小状态，成员读取可跳过其空遍历；String、Array、Map、精确数值及错误值仍完整复核，特别是既有 class instance field 在 Context 限制收紧后首次读取时。
3. Array/Map 字面量、range、array spread/append、Map spread、string interpolation/concat/stringify、slice 与 class 实例/静态字段写入在结果可见或赋值提交前检查其边界。超限不执行随后 store，资源错误不能被 `try`/`catch` 吞掉。
4. 这仍不是总分配或 retained-memory 账本：一次失败路径可以有局部临时分配，且模块图、源码/bytecode、managed object 数、生命周期/cycle 与总 Context 预算继续由 issue #76 的后续切片定义。

## 2026-08-26：输出增长型连接

1. RFC 0144 的 `concat` 在分配新 buffer 前以 checked arithmetic 计算 String UTF-8 byte 数或 Array item 数，并复用 `StringBytes` / `ArrayItems` 通用输出边界；算术溢出也映射到对应稳定资源类别。
2. Array 连接还以左右输入项数总和复用 `CollectionOperationItems`，在复制任一元素之前拒绝超限工作。该操作边界与结果 `ArrayItems` 边界独立，宿主可分别收紧。
3. 失败不修改输入且不返回部分结果；资源错误仍不可被脚本捕获。String 连接不读取 locale 或宿主 Unicode 表，Array 连接只按不可变 Value 语义克隆现有项。

## 2026-08-27：字面量文本替换

1. RFC 0150 增加 `max_text_operation_bytes`，默认 1,000,000 UTF-8 bytes，并以 `TextOperationBytes` 标识扫描输入越界；它独立于 `StringBytes` 的单值/输出边界。
2. `replace_all` 在扫描 `text` 前检查操作输入，在分配前以 checked arithmetic 计算并检查最终 String bytes；空 needle、错误类型或参数数量仍是可捕获的 Runtime error。
3. 匹配从左到右、非重叠且不重新扫描插入文本；失败不修改输入、不返回部分结果，资源错误不可被脚本捕获。模块子 Context 继承同一策略。

## 2026-08-27：保留托管图快照

1. RFC 0146 的累计分配遥测不等于 retained-memory；RFC 0147 新增 `Context::retained_memory()`，以稳定逻辑对象/bytes 单位读取当前 Context 所有可达的托管值，不执行脚本且不改变执行统计。
2. 普查从 Context 自己的 global environment 开始，跳过进程共享 builtin parent；共享 Rc backing 按 identity 仅计一次，Environment/Function/Class/Instance cycle 必须终止。模块导出与宿主手中但未存回 Context 的 Value 不属于该 Context 根。
3. 快照不是 RSS、allocator/capacity、peak 或 hard limit。后续 retained-memory 限额必须在此基础上单独规定采样点、原子性、Context 生命周期与 host-visible failure state，不能从累计分配字段推导。

## 2026-08-27：显式保留图高水位采样

1. RFC 0148 的 `Context::sample_retained_memory()` 是宿主显式观测点：它读取 RFC 0147 的当前 Context 图快照，并更新 `retained_memory_high_water()` 中 objects 与 bytes 各自的 Context-lifetime 最大已采样值；两个最大值不必来自同一个时刻。
2. Context 创建时把空 writable global 作为第一个样本。设置 global、执行、模块运行、错误和 VM 指令都不会隐式普查；嵌入方应在业务边界（例如顶层执行返回后）自行采样，避免把 O(graph) 工作放入调度热路径。
3. 此记录仍不是 RSS、allocator/capacity、逐指令 live peak 或 hard limit。未来内存失败策略必须另行定义执行期间的检查、失败原子性和 host-visible state，不能把稀疏样本当作强制边界。

## 2026-08-27：事务性 retained-state 提交限制

1. RFC 0149 增加 `max_retained_managed_objects` 与 `max_retained_managed_bytes`；默认 `u64::MAX` 禁用，超限原因分别是 `RetainedManagedObjects` 与 `RetainedManagedBytes`。它们以 RFC 0147 Context-rooted logical 单位判断，不是 RSS 或 allocator 容量。
2. 启用后，执行前先检查既有 Context；执行后再检查将提交的 retained 图。后者超限会恢复可达 Environment、Class static fields 与 Instance fields，并返回不可捕获 Resource error；数组由事务快照持有 Rc 而使用既有 copy-on-write，Map 本身不可变。
3. 限额只约束顶层执行结束时 Context 将保留的状态，不限制同一轮中已丢弃的临时分配，也不承诺逐指令 live peak。模块子 Context 继承该策略；宿主 `set_global` 保持无失败 API，随后执行的 preflight 才会报告其预先存入的超限状态。

## 2026-08-28：单轮累计 transient managed allocation 限制

1. RFC 0156 增加 `max_transient_managed_objects` 与 `max_transient_managed_bytes`；默认 `u64::MAX` 禁用，超限原因分别是 `TransientManagedObjects` 与 `TransientManagedBytes`。它们复用 RFC 0146 logical allocation delta，每轮顶层执行重新计数，模块图共享聚合账本。
2. VM 与已建模 builtin 在既有分配记账点更新统计后立即检查。contextual native 继续通过无失败的协作 API 报告 delta，VM 在 callback 返回后强制检查，资源越界优先于 callback 同时返回的普通错误；opaque native 与未报告宿主堆不在边界内。
3. 启用策略时复用 RFC 0149 事务快照；本类别失败恢复脚本可变状态但保留失败点统计，宿主 side effect 不回滚。累计 transient budget 不是瞬时 live set、RSS、allocator capacity 或 retained graph；这些独立边界仍由 #76 跟踪。

## 验收

`tests/embedding_api.rs` 必须覆盖 fuel、递归深度、预取消 token、替换 token、JSON/数值/通用语言值策略替换、资源错误不可被 `catch` 吞掉、共享 Program、宿主全局/native 返回、失败后恢复、源码标签及深度峰值；模块测试覆盖策略继承，JSON 单元测试覆盖边界类别。CLI JSON 必须输出 `kind:"resource"` 的资源错误；嵌入示例、中英文语法索引与可执行手册说明该 API。完整 `make check` 必须通过。
