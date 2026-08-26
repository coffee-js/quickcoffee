# RFC 0145：规范源码扩展名与文学源码

- 状态：已采纳
- 依赖：RFC 0003、RFC 0005、RFC 0126、RFC 0133

## 文件契约

普通 QuickCoffee 源码使用 `.coffee`，以便 GitHub Linguist 和编辑器按 CoffeeScript 高亮；`.litcoffee` 是文学源码。旧的 `.qc` 不再是源码扩展名，工具不得发现、推断或加载它。GitHub Linguist 原生将 `.litcoffee` 识别为独立的 `Literate CoffeeScript`（归入 CoffeeScript group，使用 `source.litcoffee` grammar）；仓库不得把它强制覆盖为普通 `CoffeeScript`。

命名编译、执行和检查 API 只在不透明源码名以 `.litcoffee` 结尾时启用文学预处理；匿名源码、stdin 与 `-e` 保持普通源码语义。`qcoffee` 文件模式继承命名 API 的行为。`qtest` 递归发现 `.coffee` 与 `.litcoffee`；`qdocco` 只接受 `.litcoffee`。受限文件模块 loader 省略扩展名时仍推断 `.coffee`，并接受显式 `.coffee` 或 `.litcoffee`。

## 文学语义

`.litcoffee` 使用 GitHub 与 CoffeeScript 工具链支持的 Markdown 文学形式：未缩进文本是正文；在文件开头或与正文至少相隔一个空行、以四个空格或一个 tab 开始的连续行是代码。预处理只移除一层文学缩进，同一文件的代码块必须统一使用四空格或 tab。正文被替换为空行，因此源码行号保持不变。

预处理改变了横向位置，文学源码的结构化诊断保留源码名与一基行号，但列号为未知；普通 `.coffee` 的精确列号不受影响。

## 验收

测试必须覆盖文学源码的命名编译、执行、检查、运行时诊断、`qtest`、`qdocco` 和模块加载，覆盖缩进风格不一致以及旧扩展名拒绝。所有项目源码、示例、测试与权威手册分别迁移到 `.coffee` 或 `.litcoffee`，生成文档中的代码围栏使用 `coffee`。
