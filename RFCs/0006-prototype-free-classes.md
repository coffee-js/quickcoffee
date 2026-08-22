# RFC 0006：无原型工厂类

- 状态：已采纳
- 依赖：RFC 0001、RFC 0002

## 动机

CoffeeScript 的 `class` 通常编译到 JavaScript 原型链；QuickCoffee 故意没有该运行时模型。为保留声明数据对象的可读形式而不泄漏原型，类被定义为命名工厂函数。

## 语法

`class Name(parameter, ...) -> expression` 等价于把表达式函数绑定到 `Name`。类形参与普通函数共享默认参数规则，故可写 `class Point(x, y = 0) -> ...`；缺省或 `nil` 的参数在工厂函数内取默认值。类也可使用严格解构形参，如 `class Point([x, y]) -> ...`（RFC 0026）。典型构造体：

```coffee
class Point(x, y) -> {x: x, y: y}
p = Point(3, 4)
p.x
```

类调用返回普通值，通常是映射；成员访问仅查询该映射。不存在继承、`super`、`this`、`new`、隐式构造器、原型或隐藏对象槽。方法可由工厂返回的闭包实现，但接收者传递保持显式。

## 验收

至少验证带参数与默认参数类、映射成员读取、函数元数错误，以及 `class` 仅产生普通可调用值而不引入特殊 Value 类型。
