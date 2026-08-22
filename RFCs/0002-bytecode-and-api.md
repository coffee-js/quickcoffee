# RFC 0002：字节码、VM、CLI 与嵌入 API

- 状态：已采纳
- 依赖：RFC 0000、RFC 0001

## 编译与验证

编译器生成 `Chunk { constants, code }`。常量池去重不构成语义承诺。跳转以相对指令索引表示；装载前验证所有跳转目标在边界内、常量索引和种类有效、嵌套函数递归有效、可达控制流的值栈/迭代器栈深度一致，且字节码以 `Return` 结束。验证失败绝不执行；`Context::run` 对嵌入方提供的 `Chunk` 也重复这一验证，不能绕过它。

核心指令包括：常量/局部载入保存与严格解构、算术比较、`Jump`、`JumpIfFalse`、`JumpIfNil`、异常处理、数组/映射/区间构造、字符串拼接、索引、调用、闭包创建、数组/映射迭代及 `Return`。VM 使用显式值栈、调用帧、帧私有迭代器栈与处理器栈；执行预算（fuel）每条指令递减，耗尽即报错，防止不受控循环。

## 宿主边界

`Engine` 为可复用编译器；`Context` 持有每次执行的全局环境和可注入的原生函数。`Context::eval` 返回 `Result<Value, Error>`；原生函数仅接收 `&[Value]`，不暴露 VM 内部或原型对象。`Engine::compile_program` 与 `Context::run_program` 提供可重复执行的共享 `Program` 句柄（RFC 0046），创建时验证并缓存验证状态（RFC 0069），不要求宿主管理 `Rc` 或每轮深拷贝 `Chunk`。`Value` 提供安全的类型访问器，以及 `string`、`array`、`map` 宿主构造器和基础 `From` 转换，调用方无需了解内部 `Rc` 存储。`Error::kind()` 返回稳定的 `ErrorKind::{Parse, Verify, Runtime}`，`message()` 返回不含展示前缀的详情；`position()` 在可确定时返回源码行（RFC 0043、RFC 0047），使嵌入方无需解析错误文本。

## CLI

`qcoffee FILE` 执行文件，`qcoffee -e SOURCE` 求值，`qcoffee -` 从标准输入执行，`qcoffee --check FILE`（其中 FILE 可为 `-`）只验证编译产物而不执行，`qcoffee --dump-bytecode FILE`（其中 FILE 可为 `-`）输出稳定的反汇编，`--fuel N` 设置执行预算，`--` 后的参数以字符串数组全局 `argv` 注入，`--version` 显示版本。非零退出码表示读取、编译或执行失败。

## 测量

以 `cargo bench`（或同等可重复 release 计时）分别测量编译和执行。基准至少覆盖核心循环、闭包/调用/区间，以及映射/过滤循环/异常控制流；报告必须记录硬件、Rust 版本、命令行、样本数、输入程序与结果，并为每个负载校验期望最终值（RFC 0045）；不得把调试构建或 I/O 计入纯 VM 吞吐数据。
嵌入者可用 `Context::last_execution()` 读取最近一次执行的指令数与剩余 fuel；详见 RFC 0066。该统计接口不暴露 VM 帧或环境。
