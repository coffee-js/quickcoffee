# RFC 0080：`qcoffee` 字节码指纹命令

- 状态：已采纳
- 依赖：RFC 0002、RFC 0078

## 动机

嵌入 API 已提供 `Program::fingerprint()`，但构建系统和缓存工具通常只调用命令行。CLI 应在不执行脚本的情况下输出同一稳定指纹，并使用与 API 相同的字节码内容算法。

## CLI 契约

`qcoffee --fingerprint FILE` 读取、编译并验证文件（`FILE` 可为 `-`），然后在标准输出输出一个不带前缀的 16 位小写十六进制 `u64` 和换行。它不执行脚本，不创建脚本全局状态，也不输出 fuel 统计。

`--fingerprint` 与 `--check`、`--dump-bytecode`、`--interactive`、`--stats` 互斥，且只能指定一个源码输入；冲突或缺少文件参数为用法错误，退出码为 2。读取、解析或验证错误沿用普通 CLI 的非零退出与结构化错误文本。相同源码必须产生相同指纹，不同有效字节码内容应产生不同指纹（碰撞概率遵循 64 位指纹的通常边界）。

## 验收

CLI 集成测试必须验证输出格式、同一文件的重复稳定性、不同源码的指纹差异、`--fingerprint` 不执行副作用，以及与其他执行模式的互斥规则。

## 2026-08-27：模块图扩展

RFC 0151 复用该 inspection flag：`qcoffee --fingerprint --module-root ROOT ENTRY` 通过显式受限 loader 加载并验证静态模块图，输出独立 v1 canonical graph fingerprint 且不执行模块。普通 `--fingerprint FILE` 的 bytecode 值和权限边界不变；两类模式都输出 16 位小写十六进制 `u64`，但属于不同编码域。
