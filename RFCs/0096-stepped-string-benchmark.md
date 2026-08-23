# RFC 0096：字符串步进迭代性能基准覆盖

- 状态：已采纳
- 依赖：RFC 0045、RFC 0081、RFC 0095、RFC 0100

## 动机

RFC 0095 扩展了字符串 `for ... by` 的执行路径，但若只保留功能测试，字符串 Unicode 标量步进可能在 VM 优化或回归中变慢而不被发现。项目既要求可执行语义护栏，也要求性能报告能按工作负载追踪编译、验证和执行成本。

## 契约

`cargo bench --bench core` 必须包含 `stepped-string-iteration` 工作负载；`qbench --json` 也必须输出同名记录。RFC 0100 另要求 `signed-by-iteration` 覆盖反向数组步进。字符串负载使用 ASCII 与多字节 Unicode、实际标量下标和 `by 2`，最终值严格为 `2`：

```coffee
sum = 0
for character, index in 'a☕中x' by 2 then sum += index
sum # => 2
```

```coffee
sum = 0
for n, index in [1...100] by -3 then sum += n + index
sum # => 3333
```

两条基准路径都必须先编译/验证并执行语义检查，再计时；结果不得包含标准输出、文件 I/O 或调试构建。性能报告记录机器、工具链、命令、迭代数和至少三次 release 样本的中位数，不把单次读数当成跨机器比较。

## 验收

`cargo bench --locked --bench core` 的两个步进工作负载通过最终值护栏；`cargo run --locked --release --bin qbench -- --json --iterations 1` 输出 `stepped-string-iteration`（`expected` 为 `2`）和 `signed-by-iteration`（`expected` 为 `3333`）记录；`make check` 与性能报告中的基线数据保持一致。
