# RFC 0135：精确 Integer 值

- 状态：已采纳并实现
- 日期：2026-08-25
- 依赖：RFC 0000、RFC 0001、RFC 0056、RFC 0057、RFC 0077
- 跟踪：issue #123、issue #127

## 动机

QuickCoffee 的 `Number` 是 IEEE-754 `f64`，适合普通计算，却不能精确保存超过 `2^53 - 1` 的订单号、账务整数与计数。语言需要一个不靠隐式转换、不会把任意精度依赖泄露进嵌入 API 的精确整数类型。Decimal 定点数仍由 issue #123 的后续阶段处理，本 RFC 不用 Integer 冒充金额小数。

## 字面量与类型

十进制或基数字面量添加 `n` 后缀产生 `Integer`：`123n`、`0xffn`、`0b101n`、`0o755n`。小数或指数形式不得带 `n`。`Integer` 与 `Number` 是不同类型；`type(1n)` 为 `"integer"`，严格混合相等为 false，混合算术、排序、range 和聚合报错，不作隐式提升。

## 运算

同类型 Integer 支持一元 `-`/`~`，`+`、`-`、`*`、`/`、`//`、`%`、`%%`、`**`，有序比较，以及 `&`、`|`、`^`、`<<`、算术 `>>`。`/` 向零截断，`//` 向负无穷取整，`%` 随被除数取符号，`%%` 随除数取符号。除零、负数或超范围幂指数、超范围移位均报错；无符号 `>>>` 只属于现有 32 位 Number 模型，不接受 Integer。

Integer 可作为 range 边界、Array/String 索引与切片边界以及 `for ... by` 步长；range 的元素保持 Integer。`abs`、`sum`、`min`、`max` 接受同质的有限 Number 或 Integer，不接受混合数组。

## 显式转换与外部表示

`integer(value)` 接受 Integer 或有限整数 Number；`number(value)` 接受 Number，或安全整数范围 `[-(2^53-1), 2^53-1]` 内的 Integer。`str(1n)` 和插值产生 `"1"`，普通值显示为 `1n` 以保留类型。`qcoffee --json` 使用 `{"$quickcoffee":"integer","value":"1"}`，避免 JSON consumer 把精度再次丢失。

公开 Rust API 只暴露不透明 `Integer`，提供基数解析、`i64` 检查和十进制文本；底层任意精度 crate 不进入公开签名。`Value::from(i64)` 精确地产生 Integer，`Value::from(f64)` 继续产生 Number。字节码指纹给 Integer 独立标签和规范有符号字节编码，既有 Number 指纹保持不变。

## 资源边界

本阶段对单个 Integer 使用 1,000,000 bit 的硬上限，并在幂和左移前做增长检查；range 继续受既有最大元素数约束。issue #76 将把固定上限纳入可配置、可统计的统一内存预算。任何失败均是可审计错误，不回退成 Number。
