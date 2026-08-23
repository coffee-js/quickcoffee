# QuickCoffee 路线图索引

本文件只保存长期方向、治理规则与 GitHub issue 入口。会频繁变化的优先级、负责人、验收状态和性能数据在 issues 中维护，避免仓库快照与实际进度分叉。

## 信息归属

- `RFCs/`：已采纳或待采纳的语言语义、字节码、公开 API 与工具契约。
- `docs/syntax.*.md`：当前版本已经实现的语言与 CLI 范围。
- `PERFORMANCE.md`：可复现的测量协议、历史基线和解释边界。
- GitHub issues：阶段计划、依赖、清单、实测结果与交付状态。

## 当前规划入口

### 0.2 语言与业务就绪

总入口：[#65](https://github.com/coffee-js/quickcoffee/issues/65)

| 工作流 | Issue |
|---|---|
| CoffeeScript 2016 特性矩阵与源码范围诊断 | [#74](https://github.com/coffee-js/quickcoffee/issues/74) |
| 模块包、受限 CLI 加载与模块图指纹 | [#75](https://github.com/coffee-js/quickcoffee/issues/75) |
| 内存预算与运行时隔离 | [#76](https://github.com/coffee-js/quickcoffee/issues/76) |
| 嵌入 API 0.2 与显式宿主能力 | [#77](https://github.com/coffee-js/quickcoffee/issues/77) |
| 确定性的业务数据与文本基元 | [#78](https://github.com/coffee-js/quickcoffee/issues/78) |

### VM 性能收敛

总入口：[#66](https://github.com/coffee-js/quickcoffee/issues/66)

| 工作流 | Issue |
|---|---|
| 数组、映射与字符串跨运行时负载 | [#79](https://github.com/coffee-js/quickcoffee/issues/79) |
| 局部槽位、符号 intern 与差分执行 | [#80](https://github.com/coffee-js/quickcoffee/issues/80) |

### 持续工程

CLI 契约、fuzz、发布制品和性能回归门禁由 [#81](https://github.com/coffee-js/quickcoffee/issues/81) 跟踪。它是所有版本的横向工作流，不集中到版本末尾补做。

## 不变方向

- QuickCoffee 是严格、无原型、可嵌入的字节码语言，不以 JavaScript 兼容为目标。
- 不引入原型链、`this`、隐式类型转换、`eval`、反引号 JavaScript 或隐式文件/网络权限。
- 模块、时钟、随机、日志、文件和网络等能力必须由宿主明确提供并受资源策略约束。
- 优化必须保持值、错误种类、源码位置、验证器和 fuel 规则；有意改变字节码格式时必须版本化。

## 拆分与治理规则

- 大需求先建立 tracking issue；正文维护依赖、ordered work、验收状态和相关 PR。
- 每个 PR 只完成一个可验证切片，处理完 review comments 并通过本地门禁与 Actions 后合并。
- 新语言特性同时更新 RFC、中文/英文语法索引、正反例测试和适用的可执行手册。
- 新字节码同时更新验证器、反汇编、指纹/版本规则、恶意输入测试和性能报告。
- 新公开 API 同时更新 Rust 示例、外部集成测试、API 文档和迁移说明。
- 优先级与完成状态只在对应 issue 更新；本文件不复制动态清单。
