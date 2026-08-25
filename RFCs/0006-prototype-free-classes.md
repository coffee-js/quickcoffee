# RFC 0006：无原型工厂类（已取代）

- 状态：已由 RFC 0134 取代
- 依赖：RFC 0001、RFC 0002

## 历史决策

本 RFC 曾因为 QuickCoffee 不采用 JavaScript 原型链，而把 `class` 缩减为命名工厂函数：

```coffee
class Point(x, y) -> {x: x, y: y}
p = Point(3, 4)
```

当前实现仍只接受这项历史语法。它把 class 调用当成普通函数调用并返回普通值，不提供构造器、实例方法、`this`、`new`、`extends` 或 `super`。

## 取代原因

该决策混淆了两个独立边界：QuickCoffee 不提供全局/自由 `this`，并不表示 class 内部也不应有接收者、构造和继承。RFC 0134 已恢复 CoffeeScript 风格的 class、class 内 `this`、`new`、`extends`、`super` 与受限 `=>` 接收者绑定，同时继续禁止这些能力泄漏到 class 外部。

## 迁移

RFC 0134 是 class 语义的规范性依据；实现进度由 issue #121 跟踪。在该 issue 完成前，语法矩阵与用户文档必须把工厂类标为“当前实现”，把完整 class 标为“已采纳、待实现”。历史工厂形式退出时必须提供明确诊断，不能静默解释成 CoffeeScript 构造器。
