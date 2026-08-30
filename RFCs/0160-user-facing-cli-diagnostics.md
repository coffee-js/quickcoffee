# RFC 0160：用户可用的 CLI 诊断 / User-facing CLI diagnostics

- 状态：已采纳 / Status: Adopted
- 日期：2026-08-30 / Date: 2026-08-30
- 依赖：RFC 0117、RFC 0126–0132、RFC 0141、RFC 0145 / Dependencies: RFC 0117, RFC 0126–0132, RFC 0141, RFC 0145

## 中文

`Error::labels()` 已保存 primary range、按最近调用点到最外层排列的 secondary call-site ranges、来源名与局部 message，但 `qcoffee` 的普通输出此前只显示 legacy `Error::Display`，`--json` 也只公开 primary 的 `source` / `line`。本 RFC 只完成展示层，不改变 parser、bytecode、错误分类、脚本可观察 Error、fuel、指纹或执行语义。

### 人类输出

- `qcoffee` 与 `qtest` 的脚本错误先保留原有 `Error::Display` 首行，再按 `Error::labels()` 的稳定顺序输出全部标签。
- 每个标签显示不透明来源名（匿名输入使用 `<input>`）、start 与可用的 exclusive end。未知 column/end 保持省略，不推测第 1 列或范围。
- 若调用者已经持有源码，则显示错误物理行和可用范围标记：primary 使用 `^`，secondary 使用 `-`，局部 message 单独显示。单行 excerpt 最多 120 个 Unicode scalar，并围绕 start 截取；control characters 被替换为可见占位，tab 按一个 scalar 展示。
- `.litcoffee` excerpt 来自原始文学源码，因此 Markdown 正文、四空格代码边距与物理行号保持一致。预处理后无法可靠恢复的 column 继续省略。
- REPL 明确保留逐物理行求值，不尝试猜测未完成的多行块。每个非命令输入获得稳定的 `<repl:N>` 来源名，并在会话内保留对应文本，因此旧函数在后续输入失败时，primary 定义行与 secondary 调用行不会被混为当前输入。多行程序应使用 `.coffee` / `.litcoffee` 文件。
- renderer 不把 `source_name` 当作文件路径。单文件 CLI 只提供已经读入的 source；模块 CLI/qtest 只记录调用者已显式创建的 `RestrictedFileModuleLoader` 在本次预检中实际返回的规范模块源码，不扩大 root authority，也不为展示执行第二次文件读取。
- 对严格数值混用、缺失 Map key 与函数参数数量/严格类型错误给出展示层 `help:`。hint 不改变错误 kind/message/code/data、catch 行为或机器语义。

### JSON 输出

所有 `qcoffee --json` 错误保留既有 `ok`、`kind`、`message`、可选领域字段、可选 `source` 与 `line`。记录末尾新增：

```json
"diagnostic": {
  "version": 1,
  "labels": [
    {
      "kind": "primary",
      "source": "rules.coffee",
      "start": {"line": 2, "column": 3},
      "end": {"line": 2, "column": 4},
      "message": null
    }
  ]
}
```

`labels` 与公开 API 顺序一致；`kind` 为 `primary` 或 `secondary`，未知 source/column/end/message 使用 JSON `null`。read/I/O 错误没有引擎 label，仍包含 version 1 与空数组。成功记录完全不变。消费者可继续读取 legacy 字段，也可按 `diagnostic.version` 消费完整 ranges；未来不兼容的诊断结构必须提升该 version。

`qtest` 的 plain、TAP、JUnit 与既有 JSON `error` 字符串共享同一确定性人类详情；本 RFC 不更改 qtest record schema，也不加入颜色、终端探测依赖、LSP、formatter 或自动 coercion。

### 验收

CLI integration tests 覆盖命名 `.coffee` 的 primary/secondary 顺序、源码 excerpt、三类 hint、version 1 JSON 全 ranges、无 label read error、跨输入 REPL 来源，以及 GitHub-compatible `.litcoffee` 的 Markdown inline code、四空格代码与物理行归因。qtest 的 plain/JSON/TAP/JUnit escaping 和模块 loader 权限保持确定。

---

## English

`Error::labels()` already preserves a primary range, secondary call-site ranges ordered from the nearest caller outward, opaque source names, and local messages. Ordinary `qcoffee` output previously stopped at legacy `Error::Display`, while `--json` exposed only the primary `source` and `line`. This RFC completes the presentation layer without changing parsing, bytecode, error classification, script-visible Error values, fuel, fingerprints, or execution semantics.

### Human output

- Script failures in `qcoffee` and `qtest` retain the legacy `Error::Display` first line, followed by every label in stable `Error::labels()` order.
- Each label renders its opaque source name (`<input>` for anonymous input), start, and exclusive end when available. Unknown columns or ends remain absent rather than inventing column 1 or a range.
- When the caller already holds the source, diagnostics show the physical source line and available range marker: `^` for primary and `-` for secondary, with the local label message separately. A one-line excerpt is capped at 120 Unicode scalars around the start; control characters become visible replacements and a tab occupies one scalar.
- `.litcoffee` excerpts come from the original literate source, preserving Markdown prose, the four-space code margin, and physical line numbers. Columns that cannot be recovered reliably after preprocessing remain absent.
- The REPL remains explicitly one physical line per evaluation and does not guess whether a multiline block is incomplete. Every non-command entry receives a stable `<repl:N>` source name whose text is retained for the session, so a previously defined function can report its primary definition separately from a later secondary call site. Multiline programs use `.coffee` / `.litcoffee` files.
- The renderer never interprets `source_name` as a filesystem path. Single-file CLI paths use only the source already read. Module CLI/qtest paths record exactly the canonical sources returned during this preflight by the caller's explicit `RestrictedFileModuleLoader`, without widening root authority or performing a second display-only file read.
- Presentation-only `help:` text covers strict numeric mixing, absent Map keys, and function argument-count/strict-type errors. A hint changes no kind/message/code/data, catch behavior, or machine semantics.

### JSON output

Every `qcoffee --json` error retains the existing `ok`, `kind`, `message`, optional domain fields, optional `source`, and `line`. It appends a `diagnostic` object with `version: 1` and ordered labels. Each label contains `kind`, `source`, structured `start`, optional `end`, and optional `message`; unknown values are JSON `null`. Read/I/O errors carry version 1 with an empty label array. Successful records remain byte-for-byte unchanged.

Consumers may keep reading legacy fields or select complete ranges through `diagnostic.version`. Any incompatible future diagnostic structure must increment that version.

Plain, TAP, JUnit, and the existing qtest JSON `error` string share the same deterministic human detail. This RFC does not change the qtest record schema and adds no color, terminal dependency, LSP, formatter, or automatic coercion.

### Acceptance

CLI integration tests cover named `.coffee` primary/secondary ordering, source excerpts, all three hint categories, complete version 1 JSON ranges, read errors without labels, cross-entry REPL sources, and physical-line attribution for GitHub-compatible `.litcoffee` prose, inline code, and four-space executable code. qtest escaping and restricted-module authority remain deterministic.
