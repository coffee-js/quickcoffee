# RFC 0074：映射字面量展开

状态：实现中

映射字面量支持 `...expression` 展开：表达式必须求值为映射，展开项按从左到右合并，后续显式键或展开项覆盖此前同名键。映射保持原型链无关的不可变值语义；非映射展开在运行时报告错误。

```coffee
defaults = {theme: 'light', size: 2}
config = {...defaults, theme: 'dark'}
```

编译器将每个显式项降低为单项映射，将展开项保留为映射段，再由 `MergeMaps` 字节码一次合并。验证器按段数检查栈，VM 不引入 JavaScript 对象或原型。
