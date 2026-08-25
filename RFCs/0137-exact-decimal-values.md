# RFC 0137：精确 Decimal 值与显式舍入

- 状态：已采纳并实现
- 日期：2026-08-25
- 依赖：RFC 0050、RFC 0056、RFC 0060、RFC 0077、RFC 0113、RFC 0117、RFC 0118、RFC 0135
- 跟踪：issue #123、issue #135

## 动机与表示

金额、税率与百分比不能经过 IEEE-754 `Number` 的二进制近似。QuickCoffee 因此加入独立的精确 `Decimal`，内部规范化为任意精度有符号 coefficient 乘以 `10^-scale`。非零值移除 coefficient 末尾的十进制零，零的 scale 恒为 0；相等、排序、显示和指纹都基于这一规范值，不保留输入格式中的无意义尾零。

十进制 Number 字面量添加 `m` 后缀产生 Decimal，例如 `12m`、`0.1m`、`1.2300m`、`1e2m`。基数字面量不得使用 `m`。普通显示保留类型后缀并采用无指数的规范文本，例如 `1.2300m` 显示为 `1.23m`；`str` 与字符串插值省略后缀。`type(1m)` 为 `"decimal"`。

Decimal、Integer 与 Number 是三个严格类型。跨类型相等为 false；混合算术、排序和聚合报错，不作隐式提升。Decimal 不可作为 range、索引、切片或 `for ... by` 步长。

## 运算与舍入

同类型 Decimal 支持一元 `-`、更新、`+`、`-`、`*`、有序比较、`//`、`%`、`%%` 与非负整值 Decimal 指数 `**`。`//` 向负无穷取整并返回 scale 0 Decimal；`%` 随被除数取符号，`%%` 随除数取符号。bitwise 运算不接受 Decimal。

普通 `/` 只返回能够用有限十进制精确表示的结果；除零或重复小数报错，不选择隐式精度。`decimal_div(dividend, divisor, scale, mode)` 在明确的非负 scale 上执行除法，`round_decimal(value, scale, mode)` 在同一规则下舍入已有值。scale 参数可为有界非负整数 Number 或 Integer，仅是控制参数，不参与混合数值运算。

mode 必须是以下字符串之一：`down` 向零、`up` 离零、`floor` 向负无穷、`ceiling` 向正无穷、`half_up` 五入、`half_even` 银行家舍入。舍入完成后结果再次规范化，因此目标 scale 不承担格式补零职责。

## 显式转换与宿主

`decimal(value)` 接受 Decimal、Integer 或不含空白的十进制语法 String；拒绝 Number，避免把二进制近似误认为精确十进制。`integer(decimal)` 只接受规范 scale 为 0 的 Decimal。`number(decimal)` 只接受约分后分母为 2 的幂且有效分子不超过 53 bit、结果有限且不下溢为零的值；其他转换显式失败。`abs`、`sum`、`min`、`max` 接受同质 Decimal，空 `sum([])` 继续按既有契约返回 Number `0`。

Rust API 暴露不透明 `Decimal`，提供字符串解析、coefficient/scale 构造与读取、规范文本以及 `Value` 转换，不把任意精度 crate 放入公开签名。`qcoffee --json` 使用 `{"$quickcoffee":"decimal","value":"1.23"}` 的无损标签；普通脚本 JSON 的数字映射仍由 issue #125 定义。结构化 Error 的 data 可包含 Decimal。

## 资源与确定性

单个 Decimal coefficient 暂设 1,000,000 bit 上限，scale 暂设 100,000 上限；解析、对齐、乘法、幂、精确除法和显式舍入越界均报错，绝不回退成 Number。issue #76 将这些固定守卫纳入统一可配置资源预算。字节码指纹给 Decimal 独立类型标签，并编码规范 coefficient 与 scale；同值不同输入格式产生相同指纹。

RFC 0050、0056、0060、0113、0117 与 0135 中只列 Number/Integer 的数值范围由本 RFC 扩展；既有 Number 与 Integer 行为保持不变。
