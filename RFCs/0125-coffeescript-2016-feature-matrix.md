# RFC 0125：CoffeeScript 2016 特性矩阵

- 状态：已采纳
- 日期：2026-08-23

## 动机

QuickCoffee 长期以“受 CoffeeScript 2016 启发但不兼容 JavaScript”描述自身，语法索引与 125 份 RFC 已能定义实际行为，却缺少按 CoffeeScript 1.x 官方语言参考逐项核对的稳定视图。单纯列出“未提及即不支持”不足以区分完整实现、保留表面语法但重写语义、以及明确拒绝三种决策，也容易让 `=>`、`class`、切片和模块被误认为 JavaScript 兼容实现。

## 决策

新增 `docs/coffeescript-2016-matrix.md`，以 CoffeeScript 1.12.7 官方 Language Reference 的全部 21 个语言章节为行，并使用且仅使用以下状态：

1. `Implement`：可识别的表面语法已在 QuickCoffee 严格值模型下实现；
2. `Adapt`：保留部分可识别语法，但 JavaScript 依赖语义或同章节的部分能力被明确改写；
3. `Reject`：该能力不属于语言契约，且不得静默回退到 JavaScript 行为。

每行同时说明 QuickCoffee 边界并链接规范性 RFC。矩阵是 review 与覆盖索引，不能替代 RFC 或可执行测试；语言行为仍以 RFC 和测试为准。

矩阵必须明确以下兼容性陷阱：

- `=>` 已按 RFC 0134 仅在 class 接收者上下文中绑定 `this`；`->` 保持无接收者，其他位置的 `=>` 拒绝编译；
- 当前实现仍是无原型命名工厂；RFC 0134 已采纳 CoffeeScript 风格 class、class 内 `this`、`new`、私有继承与 `super`，并禁止向 class 外泄漏这些能力；
- 切片是严格不可变操作，不提供省略边界、隐式截断或 splice 赋值；
- 存在性只围绕 `nil`，未绑定名称仍报错；
- 模块只由嵌入宿主的 `ModuleLoader` 解析，不隐式访问文件、包或网络；
- 嵌入 JavaScript、生成器、正则与 tagged templates 保持拒绝。

## 维护契约

影响矩阵章节的语言改动必须在同一变更中更新对应行、RFC 证据和可执行验收测试。新增 CoffeeScript 衍生语法须先由 RFC 分类。矩阵之外的 CoffeeScript 特性保持不支持，直至后续 RFC 明确纳入。

`tests/rfc_index.rs` 锁定官方 21 个章节各出现一次、合法状态和值得追踪的证据链接，从而防止文档重排或新增 RFC 时意外丢失覆盖。

## 验收

1. README 与中英文语法索引链接矩阵；
2. 矩阵覆盖 CoffeeScript 1.x 官方 Language Reference 全部语言章节；
3. 每行仅使用 `Implement`、`Adapt` 或 `Reject`，并至少链接一份仓库内 RFC；
4. `cargo test --locked --test rfc_index` 与 `make doc-check` 通过。
