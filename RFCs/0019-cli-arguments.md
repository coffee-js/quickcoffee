# RFC 0019：CLI 脚本参数

- 状态：已采纳
- 依赖：RFC 0002

`qcoffee` 将 `--` 后的全部命令行片段作为脚本参数。例如 `qcoffee program.qc -- first second` 在执行前绑定全局 `argv` 为字符串数组 `['first', 'second']`；`-e` 和标准输入模式同样适用。没有 `--` 时 `argv` 是空数组。

`argv` 是 QuickCoffee 的显式宿主初值，不模拟 Node.js `process.argv` 或 QuickJS 的 JavaScript 全局对象。脚本可以像其他全局名称一样读取或重新赋值它；引擎不提供路径、环境变量或宿主对象访问。

## 验收

CLI 集成测试须验证 `qcoffee -e "len(argv)" -- one two` 输出 `2`，且原有 `-e`、标准输入与反汇编模式不变。
交互式会话见 RFC 0062；`qcoffee --interactive` 在同一 Context 中逐行执行，并沿用 argv 与 fuel 规则。
