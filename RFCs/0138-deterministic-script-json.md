# RFC 0138：确定性脚本 JSON 与精确数值映射

- 状态：已采纳并实现核心
- 日期：2026-08-25
- 依赖：RFC 0056、RFC 0060、RFC 0118、RFC 0135、RFC 0136、RFC 0137
- 跟踪：issue #78、issue #125

## 边界与 API

QuickCoffee 增加普通纯函数 parse_json(string) 与 encode_json(value)。它们不读取文件、网络、环境变量或宿主对象，不借用 qcoffee --json 的 CLI 协议，也不引入 JavaScript 对象、原型、隐式类型转换或 ambient capability。

parse_json 只接受一个 UTF-8 String。encode_json 只接受一个值。语法、类型、重复键与固定实现上限失败分别产生密封的 json.parse / json.encode Error，操作不修改脚本状态。统一可配置资源错误由 issue #76 接管前，这些纯函数的固定上限错误保持可捕获且无部分结果。

## 精确数值

JSON 没有 QuickCoffee 类型标签。解析时，不含小数点或指数的 JSON number 映射为 RFC 0135 Integer；含小数点或指数的 JSON number 映射为 RFC 0137 Decimal。解析绝不经过 f64，因此超过 2^53 的标识符、金额与比率不丢精度。超过既有 Integer bit 或 Decimal coefficient/scale 上限的文本显式失败。

编码时 Integer 使用规范十进制整数；Decimal 使用无指数规范文本。scale 为 0 的 Decimal 追加 .0，使再次解析仍得到 Decimal。有限 Number 使用 Rust 定义的最短往返十进制文本；非有限 Number 被拒绝。由于 JSON 本身不能标记 IEEE-754 Number，parse_json(encode_json(number)) 可能得到 Integer 或 Decimal，而不会静默恢复为 Number；需要数值类型保持的业务数据应使用 Integer/Decimal。

## 数据、顺序与 Unicode

JSON null、Boolean、String、Array、object 分别映射 nil、Bool、String、Array、Map。Error 与 Function 不可编码；后续 Class、Instance、host handle、Bytes 与时间值默认同样拒绝，必须由各自 RFC 显式适配。

object key 必须为字符串，重复键直接报错，不采用 first/last-wins。Map 继续以 Unicode scalar 字典序存储，来源成员顺序不对脚本可见；encode_json 因此总是按同一键序输出紧凑 JSON，数组顺序保持不变。

解析器接受 JSON 空白和标准 escape，合并合法 UTF-16 surrogate pair，拒绝孤立 surrogate、非法 escape、未转义控制字符、前导零、不完整 fraction/exponent 与尾随数据。编码器保留合法非 ASCII Unicode scalar，对 quote、反斜线、常用控制符和其余 U+0000–U+001F 使用确定性 escape；不依赖 locale。

## 资源与输出

首个核心采用以下确定性固定守卫：

- 输入与输出各 1,000,000 UTF-8 bytes；
- 单 String 1,000,000 UTF-8 bytes；
- 单 Array/object 100,000 项；
- 一次解析或编码 100,000 个值；
- Array/object 最大嵌套 128 层。

每次容器加入、值创建、字符串增长与输出追加前后都检查对应边界。输入先按 byte 长度拒绝，深度在递归进入容器前拒绝，编码在超过输出边界前停止且不返回部分 String。解析得到的托管 String/Integer/Decimal/Array/Map 与 object key 纳入既有 ExecutionStats.value_allocations 事件估算。

issue #76 将这些固定守卫迁移为 Context/Runtime 可配置的统一 data-size、nesting、managed-object 与 retained-memory 预算，并把越界改为脚本不可捕获的 Resource error；迁移不得改变成功值、canonical JSON、重复键规则或精确数值映射。
