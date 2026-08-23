# RFC 0119：嵌入式静态模块核心

- 状态：已采纳
- 依赖：RFC 0001、RFC 0002、RFC 0118

## 范围

模块语法仅由 `Engine::compile_module(name, source)` 接受，单文件 `compile`/`qcoffee` 仍拒绝模块指令。本切片支持顶层 `import { public as local } from 'specifier'`、`export name = expression` 与 `export { local as public }`。`specifier` 必须是无插值字符串；导入和导出均为命名绑定。

`Context::run_module` 只通过宿主 `ModuleLoader` 取得源码。引擎不读取文件、目录、环境变量或网络；`ModuleSource::name` 必须由宿主规范化。`MemoryModuleLoader` 只按精确名称用于测试和小型嵌入。CLI 相对路径、搜索根、模块包和序列化另立 RFC。

每个模块在私有顶层环境执行，父环境仅包含宿主显式注入的全局和内建函数；执行结束后只有 `ModuleExports` 中声明的值离开模块。已求值的同名依赖在一次 `run_module` 内复用；循环依赖返回确定性运行时错误，不执行部分初始化。

依赖按导入顺序先执行。整张模块图共享发起 Context 的 fuel、取消令牌和深度配置：已耗 fuel 不会在依赖边界重置；统计汇总指令数并保留最大调用深度。依赖编译、缺失模块或缺失导出不伪造执行统计。

## 验收

集成测试覆盖命名别名、私有顶层、重复依赖、缺失模块/导出、循环依赖、跨依赖 fuel 以及单文件模式拒绝模块语法。五种手册与中英文语法索引必须说明嵌入边界。`make check` 必须通过。
