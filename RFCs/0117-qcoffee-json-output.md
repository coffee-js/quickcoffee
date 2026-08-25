# RFC 0117：`qcoffee` 单结果 JSON 输出

- 状态：已采纳
- 依赖：RFC 0002、RFC 0047、RFC 0062、RFC 0077

## 动机

`qtest` 与 `qbench` 已有稳定的机器输出，而 `qcoffee` 执行模式仍把值和错误写成面向人的文本。CI、编辑器和嵌入宿主若直接消费 CLI，必须自行解析展示文本，既不能可靠区分 `nil` 与字符串，也无法稳定取得错误类别和源码行。

## 契约

1. `qcoffee --json -e SOURCE`、`qcoffee --json FILE` 和 `qcoffee --json -` 各输出恰好一行 JSON；成功时形如 `{"ok":true,"value":VALUE}`，`nil` 映射为 JSON `null`。
2. QuickCoffee 的 Bool、有限 Number、String、Array、Map 递归映射为对应 JSON 值；函数、Class 与 Instance 分别是 `{"$quickcoffee":"function"}`、`{"$quickcoffee":"class"}` 与 `{"$quickcoffee":"instance"}`，以保留其不可序列化宿主值的类型信息。Map 键按确定性字典序输出，字符串和控制字符采用 JSON 转义。
3. 编译或执行失败时退出码仍为 `1`，标准输出至少包含 `{"ok":false,"kind":KIND,"message":TEXT,"line":N}` 这些字段；`line` 无位置时为 `null`。RFC 0128 起，命名输入的诊断 label 有来源时增加可选字符串字段 `source`。读取文件失败使用 `stage:"read"` 与 `kind:"io"`。错误模式不向标准错误重复输出详情。
4. `--json` 只适用于单次执行，不得与 `--interactive`、`--check`、`--dump-bytecode` 或 `--fingerprint` 合用；`--stats` 仍可使用且只写标准错误。
5. JSON 协议不改变普通输出、退出码、fuel 或 QuickCoffee 值模型；它不引入 JavaScript `undefined`、原型或隐式转换。

## 验收

`tests/cli_tools.rs` 必须覆盖复合值、`nil`、解析错误、fuel 资源耗尽错误和 JSON/普通模式隔离；五份可执行手册与中英文语法索引说明该选项。`make check` 必须继续通过。

RFC 0135、0136、0137 后续分别为 Integer、Error、Decimal 增加 `$quickcoffee` 标签表示；普通 JSON 值的既有映射不变。
