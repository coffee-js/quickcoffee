# RFC 0108：qtest 可执行示例语料

- 状态：已采纳
- 依赖：RFC 0003、RFC 0045

## 动机

`qtest` 的内建脚本既是测试工具，也是 QuickCoffee 语义的最小可执行示例。仅有算术和闭包样例不足以让新用户验证语言的核心边界，也不足以在后续优化中保护这些语义。

## 约定

`tests/scripts/` 中的每个 `.qc` 文件都必须：

1. 不依赖宿主文件、网络或时间；
2. 以严格的 `true` 作为最终值；
3. 使用已经采纳的 QuickCoffee 语法，覆盖至少一个可观察的核心语义；
4. 可由 `qtest tests/scripts` 递归发现并以默认 fuel 完成。

示例语料至少覆盖数值与闭包、map spread/解构、Unicode 字符串索引、筛选 comprehension、循环控制，以及无原型标准库的组合调用。标准库样例必须覆盖 `range`、`len`、`type`、`str`、`keys`、`values`、`join`、`split` 与成功的 `assert`。语料只验证语义结果，不比较实现细节或运行时间；性能门禁仍由 RFC 0045、RFC 0081、RFC 0096 与 RFC 0107 负责。

## 验收

CI 的 `cargo test --locked` 必须执行目录级 `qtest tests/scripts` 检查；每个样例也应保持可单独传给 `qtest`。新增样例若最终值不是 `true`，或需要放宽 qtest 的严格结果规则，则必须先修改本 RFC。
