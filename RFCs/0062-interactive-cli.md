# RFC 0062：交互式命令行会话

- 状态：Accepted
- 依赖：RFC 0019、RFC 0027、RFC 0046

## 摘要

`qcoffee --interactive`（简写 `-i`）启动持久的 QuickCoffee 会话。每个非空输入行独立编译并在同一个 `Context` 中执行，因此前一行的名称绑定、宿主 `argv` 和标准库状态可被后续输入使用。

```text
qcoffee> answer = 40
40
qcoffee> answer + 2
42
qcoffee> :quit
```

管道或重定向输入时不输出提示和欢迎语，便于脚本驱动；终端输入显示 `qcoffee> ` 提示。`:help` 打印命令，`:quit` 与 `:exit` 结束会话。空行忽略，EOF 等价于退出。

## 错误与资源边界

单行 Parse、Verify 或 Runtime 错误打印到 stderr，当前会话继续运行，已有全局绑定保持不变。`--fuel N` 为每一行执行建立同一上限，防止单行无限循环阻塞后续输入。交互模式不能与源文件、`-e`、`--check` 或 `--dump-bytecode` 合用。

## 非目标

本 RFC 不加入隐式调用、JavaScript REPL 对象、原型链或跨行自动恢复；需要多行程序时使用文件、标准输入或 qdocco。

## 验收

CLI 集成测试覆盖持久绑定、错误恢复、无提示管道输出、`:quit`、argv 继承和模式冲突；用户手册与 README 记录交互入口。
