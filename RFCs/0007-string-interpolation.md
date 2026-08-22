# RFC 0007：受限表达式字符串插值

- 状态：已采纳
- 依赖：RFC 0001、RFC 0002

双引号字符串可包含 `#{expression}`。其中的内容由同一 QuickCoffee 表达式解析器编译；它不是 JavaScript，不能调用 `eval` 或访问宿主 JavaScript。单引号字符串不插值。

编译器把每段文字和表达式编译为 `Stringify`，再用 `Concat` 合成。所有值依其 QuickCoffee 显示形式转换为文字。花括号可在插值表达式中配对嵌套；未闭合插值或包含多个语句均为解析错误。
