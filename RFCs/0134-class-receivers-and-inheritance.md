# RFC 0134：受限接收者的 class、构造与继承

- 状态：已采纳（部分实现）
- 日期：2026-08-25
- 依赖：RFC 0000、RFC 0001、RFC 0002
- 取代：RFC 0006
- 跟踪：issue #121

## 动机

QuickCoffee 不提供 JavaScript 全局对象，也不允许顶层或普通函数凭空取得 `this`。这项安全边界不应被扩大成删除 class 自身语义。class 需要封装实例状态、构造对象、复用行为与调用父实现；若只保留 `class Name(args) -> expression` 工厂形式，CoffeeScript 的 class、`new`、`extends`、`super` 和绑定方法模型就被实质移除。

本 RFC 恢复 CoffeeScript 1.x 可识别的 class 设计，同时把接收者、构造器与父类能力限制在 class 产生的值和编译器验证的成员上下文内。QuickCoffee 仍不采用 JavaScript 全局 `this`、任意函数构造、公开可变原型或嵌入 JavaScript。

## 表面语法

class 使用 CoffeeScript 风格的缩进成员体：

```coffee
class Point
  constructor: (@x, @y = 0) ->
  lengthSquared: -> @x * @x + @y * @y
  @origin: -> new Point(0, 0)

class NamedPoint extends Point
  constructor: (name, x, y) ->
    super(x, y)
    @name = name
  label: -> "#{@name}: #{@x}, #{@y}"

point = new NamedPoint('origin', 0, 0)
point.label()
```

支持构造器、实例方法、CoffeeScript 的 `@name`（`this.name`）简写、静态成员形式、`new Class(args...)`、`class Child extends Parent` 与 `super`/`super(args...)`。class 声明产生专用 class 值；`new` 产生专用 instance 值。历史 `class Name(args) -> expression` 工厂形式不再是目标 class 语法，实现迁移时必须给出明确兼容诊断，不能静默改变其调用结果。

## 接收者与箭头

`this` 是上下文关键字，不是可从词法环境或宿主全局读取的普通名称。

1. `this` 只在 class 成员上下文内有效：实例构造器/方法中指向当前 instance，静态方法中指向当前 class；两种接收者不可混用。
2. 在有效接收者上下文中创建的 `=>` 词法捕获当前接收者；该闭包可作为显式绑定方法返回或传递，但调用者只得到封装后的可调用值，不获得通用 `this` 名称或接收者修改能力。
3. `->` 永不隐式捕获接收者。实例方法内嵌套的普通 `->` 形成无接收者函数，其中使用 `this` 或 `@name` 是编译错误。
4. 顶层、普通函数、模块顶层、默认参数以及不处于合法 class 接收者范围的 `=>` 都不能取得 `this`；需要接收者绑定的 `=>` 在这些位置是编译错误，而普通无接收者闭包继续使用 `->`。
5. `object.method(args...)` 的成员调用向方法提供 `object` 接收者。取出未绑定方法后直接调用不得回退到全局对象；它以稳定的“缺少接收者”错误失败。class 中显式声明或创建的绑定方法可携带接收者。

静态方法以 class 为接收者，但不能把 class 接收者伪装成实例。静态与实例成员的合法 `this`/`@` 细节在实现测试中分别锁定；二者都不建立全局接收者。

实例字段可由合法实例构造器/方法通过 `this.name` 或 `@name` 写入，供外部读取和成员调用；普通顶层/函数代码的成员赋值仍保持拒绝，调用者不能借 instance 获得通用对象修改能力。静态字段同理只由合法 class 成员上下文管理。该受限可变状态不改变 Array/Map 的不可变语义。

## 构造、继承与 `super`

`new` 只接受 QuickCoffee class 值。普通函数、原生函数、Map、模块容器和宿主不透明值不参与通用构造协议；class 值经显式模块导入导出后仍保持 class 身份并可构造。构造器初始化新 instance；没有显式构造器时使用确定性的默认构造器，派生 class 的默认构造器向父构造器转发实参。

`extends` 只接受 QuickCoffee class 值，并建立私有、不可由脚本修改的父类链接。继承环是 class 定义错误。成员查找先查当前 class，再沿私有父链查找；instance 自身字段不把 Map 原型语义引入语言。

`super` 是静态解析的上下文形式，不是可存储、返回或传递的一等值：

- 派生构造器中的 `super(args...)` 调用直接父类构造器；
- 覆盖方法中的 `super(args...)` 调用同名父实现，并保留当前实例接收者；
- 非派生 class、非覆盖成员、普通/嵌套无接收者函数或 class 外部使用 `super` 均为编译错误；
- 派生实例在父构造完成前不得读取或写入 `this`/`@`，重复调用父构造器亦报错。

## 无泄漏运行时边界

实现可以使用专用 class/instance Value、私有方法表和私有父链，但不得暴露 JavaScript 风格的 `prototype` 对象、`__proto__`、构造器属性或全局对象。`of` 仍只检查 Map 自身键；class 成员查找、instance 字段与继承查找必须走彼此可审计的指令/运行时路径。

宿主只有通过明确新增并文档化的嵌入 API 才能创建或检查 class/instance；现有全局、原生函数和模块能力不会自动获得构造、继承或接收者权限。序列化、相等、`type`、格式化、fuel、调用深度、source span 与资源统计必须在实现 RFC/测试中确定，不能借用 JavaScript 的隐式行为。

## 实现与验收

issue #121 按 parser/AST、运行时值、接收者调用、继承/`super`、绑定箭头、文档与性能护栏分阶段实现。issue #147 已交付首个可用阶段：缩进 class 体、构造器、实例/静态方法、`this`/`@`、`new`、专用 Class/Instance 值、受限字段写入、接收者成员调用、模块传递、宿主不透明边界、源码诊断、资源统计和基准护栏。issue #149 又交付私有 `extends` 链、继承查找、默认派生构造转发、父构造顺序约束与静态解析的 `super`。历史工厂语法现在给出明确迁移诊断。

尚未实现的规范部分只剩 issue #150 跟踪的、在 class 接收者上下文捕获接收者的 `=>`。普通 `=>` 暂时继续保持 `->` 的历史同义行为。完整交付前，文档与 issue 必须持续区分已实现阶段和剩余契约。最终至少验证：

1. 构造器、默认参数、实例/静态方法、`@`、`new`、继承、覆盖和 `super` 的成功路径；
2. 顶层与普通函数中的 `this`，无 class 接收者的 `=>`，越界 `super`，非 class 的 `new`/`extends`，继承环、父构造前访问和分离未绑定方法的失败路径；
3. 绑定 `=>` 可安全逸出但不向调用者泄漏接收者语法或全局能力；
4. verifier 拒绝伪造的接收者/父调用字节码，错误保留准确 source span；
5. fuel、取消、调用深度、模块隔离、宿主 capability 和无嵌入 JavaScript 保证保持不变；
6. class 构造、字段读取、成员调用和继承查找具有带语义护栏的 benchmark，避免在实现新值模型时无证据地扩大热路径成本。
