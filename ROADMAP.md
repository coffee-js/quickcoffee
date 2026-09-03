# QuickCoffee 路线图索引

本文件只保存长期方向、治理规则与 GitHub issue 入口。会频繁变化的优先级、负责人、验收状态和性能数据在 issues 中维护，避免仓库快照与实际进度分叉。

## 信息归属

- `RFCs/`：已采纳或待采纳的语言语义、字节码、公开 API 与工具契约。
- `docs/syntax.*.md`：当前版本已经实现的语言与 CLI 范围。
- `PERFORMANCE.md`：可复现的测量协议、历史基线和解释边界。
- GitHub issues：阶段计划、依赖、清单、实测结果与交付状态。

## 当前规划入口

### 0.2 产品收口

总入口：[#65](https://github.com/coffee-js/quickcoffee/issues/65)

| 状态 | Issue |
|---|---|
| 活跃：只使用现有 API 的生产部署 cookbook | [#77](https://github.com/coffee-js/quickcoffee/issues/77) |
| 活跃：一次统一业务性能基线，完成即关闭 | [#66](https://github.com/coffee-js/quickcoffee/issues/66) |
| 冻结：只由真实外部任务触发的能力候选 | [#78](https://github.com/coffee-js/quickcoffee/issues/78) |

没有可复现用户阻塞时，不启动新的语言、标准库、运行时架构或性能优化工作。

## 不变方向

- QuickCoffee 是严格、无原型、可嵌入的字节码语言，不以 JavaScript 兼容为目标。
- 不引入公开原型链、全局/自由 `this`、任意函数构造、隐式类型转换、`eval`、反引号 JavaScript 或隐式文件/网络权限；RFC 0134 的 class 内接收者、`new` 与私有继承链保持为受限语言能力。
- 模块、时钟、随机、日志、文件和网络等能力必须由宿主明确提供并受资源策略约束。
- 优化必须保持值、错误种类、源码位置、验证器和 fuel 规则；有意改变字节码格式时必须版本化。

## 拆分与治理规则

- 先用聚焦 issue 验证用户问题；只有确实跨越多个独立交付时才建立 tracking issue。
- 每个 PR 只完成一个可验证切片，处理完 review comments 并通过本地门禁与 Actions 后合并。
- 只有公开语义或兼容性变化才新增 RFC；新语言特性还必须更新中英文语法索引、正反例测试和适用的可执行手册。
- 新字节码同时更新验证器、反汇编、指纹/版本规则、恶意输入测试和性能报告。
- 新公开 API 同时更新 Rust 示例、外部集成测试、API 文档和迁移说明。
- 优先级与完成状态只在对应 issue 更新；本文件不复制动态清单。
