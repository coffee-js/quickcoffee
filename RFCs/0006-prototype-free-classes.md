# RFC 0006：无原型工厂类（已取代）

- 状态：已由 RFC 0134 取代
- 依赖：RFC 0001、RFC 0002

## 历史决策

本 RFC 曾因为 QuickCoffee 不采用 JavaScript 原型链，而把 `class` 缩减为命名工厂函数：

```coffee
class Point(x, y) -> {x: x, y: y}
p = Point(3, 4)
```

这项历史语法已经退出；解析器会给出迁移到缩进 class 体与 `new` 的明确诊断，不再把它静默解释为普通工厂函数。

## 取代原因

该决策混淆了两个独立边界：QuickCoffee 不提供全局/自由 `this`，并不表示 class 内部也不应有接收者、构造和继承。RFC 0134 已恢复 CoffeeScript 风格的 class、class 内 `this`、`new`、`extends`、`super` 与受限 `=>` 接收者绑定，同时继续禁止这些能力泄漏到 class 外部。

## 迁移

RFC 0134 是 class 语义的规范性依据；实现进度由 issue #121 跟踪。构造、专用 class/instance 值和受限接收者阶段已由 issue #147 交付；继承/`super` 与接收者绑定 `=>` 仍是后续阶段。历史工厂形式的明确诊断是兼容契约的一部分。
