# 小型 JSON 文件试用 / Small JSON file trial

## 中文

此 Unix/macOS 流程使用发行归档里的 `qcoffee` 和 `examples/getting-started/`，再用 Python 3 明确读写文件。先在解包目录运行；从源码试用时先执行 `cargo build --locked --bin qcoffee`，再执行 `export QCOFFEE=./target/debug/qcoffee` 后运行下方命令。脚本本身不获得隐式文件权限。

准备一个小于 4 KiB 的 UTF-8 JSON 文件。例如将以下内容保存为 `我的任务 1.json`，末尾换行也可以：

```json
{"name":"  修复登录  ","tags":[" bug ","紧急"]}
```

把以下命令中的两个文件名替换为自己的路径，在解包目录执行。Python 3 是此段宿主命令的依赖；QuickCoffee 发行归档不包含 Python。

```sh
python3 - '我的任务 1.json' '处理结果 1.json' <<'PY'
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

source = Path(sys.argv[1])
destination = Path(sys.argv[2])
with source.open("rb") as file:
    data = file.read(4097)
if len(data) > 4096:
    raise SystemExit("input exceeds this small-file example's 4096-byte limit")
try:
    input_json = data.decode("utf-8")
except UnicodeDecodeError:
    raise SystemExit("input must be UTF-8 JSON") from None
result = subprocess.run(
    [os.environ.get("QCOFFEE", "./qcoffee"), "--json", "--module-root", "examples/getting-started",
     "demo", "--", input_json],
    capture_output=True, text=True,
)
if result.returncode != 0:
    sys.stderr.write(result.stdout or result.stderr)
    raise SystemExit(result.returncode)
envelope = json.loads(result.stdout)
if envelope.get("ok") is not True:
    raise SystemExit("qcoffee returned an unexpected result")
payload = json.dumps(envelope["exports"]["result"], ensure_ascii=False,
                     separators=(",", ":"), allow_nan=False) + "\n"
temporary = None
try:
    with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8",
            dir=destination.parent, prefix=".quickcoffee-", delete=False) as file:
        temporary = file.name
        file.write(payload)
    os.replace(temporary, destination)
finally:
    if temporary and os.path.exists(temporary):
        os.unlink(temporary)
PY
```

成功后，`处理结果 1.json` 是业务结果：`{"name":"修复登录","tags":["bug","紧急"]}`。`qcoffee --json` 自身输出的是含 `ok`、`exports` 的 **CLI 信封**；这里明确提取 `exports.result`，不把整个信封写入结果文件。输入 JSON 不合法或字段不合要求时，`qcoffee` 的错误会显示在终端且命令以非零状态退出；原结果文件不会被覆盖。脚本仍按原有规则排序，输入中的换行和 Unicode 会保留到解析阶段。

这个示例只处理小文件：CLI 通过 `argv` 接收整个 JSON，操作系统还会限制单个参数和总命令行长度，不能靠增大此处的 4096 字节上限来处理大数据。较大文件请让宿主直接读取并调用嵌入 API；仓库的 `examples/normalization.rs` 展示了由 Rust 宿主按路径读取 JSON 的做法。若希望测量本机一次端到端耗时，可在命令前加 `/usr/bin/time -p`；这包括 Python 和 `qcoffee` 的启动及文件读写，不是纯脚本执行耗时。

本地试运行记录：2026-09-23，Darwin arm64、Python 3.13.3、Rust 1.94.0 的 debug 源码构建，输入 64 UTF-8 bytes；`/usr/bin/time -p` 首次运行约 1.10 秒，随后一次为 0.04 秒。这里只记录一次冷/热观察，桌面环境与文件缓存未控制，不能据此推断其他机器或发行构建的速度。

## English

Run the Unix/macOS code block above from an extracted release directory with Python 3. Save the example JSON as a UTF-8 file, then replace the two quoted paths with your input and output paths. Spaces, Unicode, and a trailing newline are supported. For a source checkout, run `cargo build --locked --bin qcoffee` and then `export QCOFFEE=./target/debug/qcoffee` before the block. Python performs the file I/O; the QuickCoffee script receives only the explicit argument.

The example handles up to 4096 input bytes. On success, the output file contains only the business result, such as `{"name":"修复登录","tags":["bug","紧急"]}`. `qcoffee --json` returns a CLI envelope with `ok` and `exports`; the host extracts `exports.result`. Invalid JSON or invalid fields report an error and leave any existing output untouched. The temporary file is created beside the destination and replaced only after successful execution.

Larger inputs need a host that reads the file directly, as in `examples/normalization.rs`. OS limits on individual arguments and total command-line size vary, so raising the example's byte limit is not a large-file solution. Prefix the block with `/usr/bin/time -p` to observe one local end-to-end run, including process startup and file I/O; it is not a script-only performance figure. On 2026-09-23, a 64-byte input on Darwin arm64 with Python 3.13.3 and a Rust 1.94.0 debug source build took 1.10 seconds on the first invocation and 0.04 seconds on a subsequent one. These are uncalibrated local observations.
