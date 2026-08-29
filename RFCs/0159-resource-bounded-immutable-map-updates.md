# RFC 0159：资源有界的不可变 Map 更新 / Resource-bounded immutable Map updates

- 状态：已采纳 / Status: Adopted
- 日期：2026-08-29 / Date: 2026-08-29
- 依赖：RFC 0001、RFC 0118、RFC 0146、RFC 0156 / Dependencies: RFC 0001, RFC 0118, RFC 0146, RFC 0156

## 中文

`map_set(map, key, value)` 与 `map_delete(map, key)` 是普通、无 callback 的标准库函数。key 必须为 String；前者替换或加入键，后者删除键，缺失键不报错。两者均返回按既有 `BTreeMap` 字典序组织的新 Map，输入与嵌套值保持不可变且只作浅克隆。

`map_set` 在复制前以输出条目数检查 `max_collection_operation_items`，因此新增键计入本次操作，而替换键不额外增长；`map_delete` 则以必须复制的输入条目数检查。`map_set` 另以输出条目数检查 `max_map_entries`；算术溢出归为 `MapEntries`。参数和值仍经过 Context 的通用值资源检查。失败不可捕获、无部分结果，并保留现有 transient allocation 遥测。

本 RFC 不增加可变容器、prototype、任意键、coercion、callback transform 或 copy-on-write 承诺。测试覆盖新增/替换/删除/缺失、输入不变、字典序、严格类型与两类资源边界。

---

## English

`map_set(map, key, value)` and `map_delete(map, key)` are ordinary callback-free standard-library functions. Keys must be Strings; the former replaces or inserts a key, while the latter removes one and accepts an absent key. Both return a new Map in the existing `BTreeMap` lexical order, leave inputs and nested values immutable, and clone shallowly.

Before copying, `map_set` checks its output entry count against `max_collection_operation_items`, so a new key counts as work while replacement adds no growth; `map_delete` checks the input entries that it must copy. `map_set` also checks the output count against `max_map_entries`; arithmetic overflow is `MapEntries`. Arguments and values retain the Context's general value-resource checks. Failure is uncatchable and atomic, and existing transient-allocation telemetry is preserved.

This RFC adds no mutable container, prototype, arbitrary key, coercion, callback transform, or copy-on-write promise. Tests cover insertion/replacement/deletion/absence, input immutability, lexical order, strict types, and both resource boundaries.
