# RFC 0140：资源有界的稳定标量排序

- 状态：已采纳并实现
- 日期：2026-08-26
- 依赖：RFC 0001、RFC 0118、RFC 0123、RFC 0135、RFC 0137
- 跟踪：issue #65、issue #76、issue #124、issue #155

## API 与语义

QuickCoffee 增加普通纯函数 `sort(array)`。它不修改输入，而返回一个新 Array；空数组与单元素数组均合法。输入必须同质且属于以下一种类型：有限 Number、Integer、Decimal 或 String。混合数值类型、非有限 Number 与其他值均产生可捕获的 Runtime error。

排序为稳定升序。Number 使用 IEEE-754 数值顺序，但只接受有限值；相等值保留输入次序，因此 `-0` 与 `0` 的相对位置不变。Integer 与 Decimal 使用精确数值顺序。String 按 Unicode scalar value 序列作字典序比较，不读取 locale、不做 normalization 或 case folding，也不采用 JavaScript UTF-16 code-unit 顺序。QuickCoffee String 的规范 UTF-8 编码保持 scalar value 的字典序，因此实现可直接比较其 UTF-8 bytes。

## 资源与统计

`ResourceLimits` 增加 `max_collection_operation_items`，默认 100,000；getter 与 `with_max_collection_operation_items` builder 公开同一策略。`sort` 在复制输入或建立比较键之前检查长度，超限产生不可被脚本 `catch` 吞掉的 `ResourceLimit::CollectionOperationItems`。这个边界表示一次集合标准库操作处理的输入项数，后续无 callback 的集合 helper 可复用，但不替代 #76 的总容器大小、retained memory 或输出增长预算。

Decimal 排序沿用当前 Context 的 coefficient-bit/scale 策略。比较前按本次输入的最大 scale 建立精确对齐键，并在乘方或分配超限前拒绝；不会借由宿主 `Ord` 路径绕过数值资源策略。

成功返回记录一个 Array 后备存储和每个输出元素的托管值分配事件，即 `len + 1`；Rust 临时排序空间仍按 RFC 0123 不计入脚本可观察分配。失败不返回部分数组，也不修改输入。调用沿用一个 builtin call 的 fuel/call 统计，长度边界限制了单条调用内隐藏的 `O(n log n)` 工作规模。

## 延后工作

`sort_by` 以及 `find`、`any`、`all`、`fold` 需要可暂停/恢复脚本 callback 的 VM continuation 契约，继续由 #124 跟踪。`replace`、concat 与 immutable update 等输出增长型 helper 必须先接入 #76 的一般 String/container 预算。

核心与嵌入测试覆盖四种标量、Unicode、signed zero 稳定性、输入不变、严格错误、资源错误不可捕获和分配统计；`stdlib-stable-sort` 同时进入 qbench 与 cargo bench。
