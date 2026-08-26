# RFC 0142：严格的嵌入 Value 转换

- 状态：已采纳
- 日期：2026-08-26
- 依赖：RFC 0089、RFC 0092、RFC 0135、RFC 0137

## 动机

嵌入宿主可读取 `Value` 的只读视图并手写递归匹配，但这种重复代码容易遗漏深层类型检查、产生不一致的错误，或重新引入 JavaScript 式隐式数值转换。常用的宿主数据模型需要一个小而稳定的、明确不执行脚本的转换层。

## 决策

1. 公开 `IntoValue` 与 `TryFromValue`。前者消费宿主值并构造不可变 `Value`；后者借用 `Value` 并返回拥有的宿主值或 `ErrorKind::Runtime`。它们不访问 Context、VM、文件、网络、时钟或其他 capability。
2. 首个稳定集合覆盖 `Value`、`() ↔ nil`、`bool`、`f64 ↔ Number`、`Integer`、`Decimal`、`String`、`Vec<T>`、`BTreeMap<String, T>` 与 `Option<T>`。容器递归使用同一契约；`Option<T>` 只有 `nil` 映射为 `None`。
3. Number、Integer 与 Decimal 保持互不转换；字符串、Bool、容器和嵌套子值同样不发生 coercion。非有限 Number 若由宿主提供则按原值保留，不被静默归一化。
4. 容器转换在第一个失败值返回确定性 runtime error，数组使用 `[index]`、Map 使用 `.key` 路径前缀；没有部分成功的宿主容器会被返回。原有 `From<Value>` 形式、`Value::array`、`Value::map` 和 `as_*` 访问器保持兼容。

## 验收

嵌入测试覆盖所有标量、精确数值、Option、嵌套数组/Map、路径错误和无 coercion 边界。公开 rustdoc、中英文语法索引、可执行手册和 README 必须说明所有权、严格性及不提供 capability 的边界；debug/release、Clippy、rustdoc、文档、发布 dry-run 和性能门禁必须通过。
