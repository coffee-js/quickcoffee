# 发布与平台归档 / Releases and platform archives

## 中文

正式版本使用 `vX.Y.Z` tag，并要求 `X.Y.Z` 同时匹配 `Cargo.toml` 的 package version 与 `CHANGELOG.md` 的版本标题。Release workflow 固定使用已提交的 `Cargo.lock` 和 Rust 1.85.0，在原生 runner 上生成以下制品：

- `x86_64-unknown-linux-gnu`：`.tar.gz`
- `x86_64-apple-darwin`：`.tar.gz`
- `aarch64-apple-darwin`：`.tar.gz`
- `x86_64-pc-windows-msvc`：`.zip`

macOS Intel 使用 `macos-15-intel`，Apple silicon 使用 `macos-15`；workflow 会在构建前校验 `uname -m`，避免 runner label 漂移后悄悄生成错误架构的制品。

每个归档包含 `qcoffee`、`qtest`、`qdocco`、`qbench`、`qcson`、README、更新日志、双许可证，`examples/getting-started/` 下可修改和测试的 JSON 清洗任务，以及 `examples/pricing/` 下的规范 Decimal `.litcoffee` 规则、人工维护的 `.cson` 配置和薄 `.coffee` demo/qtest/configured 入口。`qbench` 用于维护者性能观测，不是运行用户脚本所需的组件。平台 job 会执行 release tests、crate package verification 和五个 CLI 的构建目录版本 smoke；归档完成后还会先验证成员、路径和权限，再解包到临时干净工作区，仅使用解包后的二进制和归档内源码运行：

- 五个 CLI 的版本检查；
- `qcson` 的 CSON → JSON 与 JSON → CSON canonical 转换；
- README 入门任务的 JSON 输入、稳定输出与隔离 `qtest`；
- 归档内 `.cson` 经 `qcson` 转换后通过显式 `argv` 驱动 Decimal 定价，结果与 Rust embedding 基线一致；
- 一个独立 `.coffee` 脚本；
- 一个含 Markdown 行内代码与四空格可执行块的 GitHub-compatible `.litcoffee`；
- `qdocco --check`；
- 归档自带的 Decimal 定价 `qcoffee --module-root` demo 与 `qtest --module-root` 用例。

聚合 job 只接受完整且不重复的四平台制品，生成按文件名排序的 `SHA256SUMS`，再立即重新校验。聚合 artifact 上传完成后，另一个没有仓库 checkout、没有 Rust 工具链的 Ubuntu job 会重新下载最终 bundle，校验全部四个 checksum，并从下载的 Linux 归档完成同一 clean-install 工作流。只有这个 artifact round-trip 通过后，tag workflow 才能发布 GitHub Release。

修改发布配置的 pull request 和在 GitHub Actions 手动运行该 workflow 都只会生成可下载的演练 artifacts，不会创建 GitHub Release。只有匹配的 tag push 且全部门禁通过时，聚合 job 才会发布 release。当前范围不包含代码签名、公证、包管理器配方、crates.io publish、Linux musl 或 Windows arm64。

下载后可在 Unix-like 系统校验：

```sh
shasum -a 256 -c SHA256SUMS
```

Windows PowerShell 可将 `Get-FileHash -Algorithm SHA256` 的结果与 `SHA256SUMS` 对照。归档是对提交内容和工具链的可验证打包，不构成代码签名或平台公证。

### 无仓库 checkout 的快速验收

正式 `v0.1.0` 发布后，Linux 或 macOS 用户可以只下载平台归档和 checksum：

```sh
VERSION=0.1.0
TARGET=aarch64-apple-darwin
ARCHIVE="quickcoffee-${VERSION}-${TARGET}.tar.gz"
BASE="https://github.com/coffee-js/quickcoffee/releases/download/v${VERSION}"
curl -fLO "${BASE}/${ARCHIVE}"
curl -fLO "${BASE}/SHA256SUMS"
if command -v sha256sum >/dev/null; then
  grep "  ${ARCHIVE}$" SHA256SUMS | sha256sum -c -
else
  grep "  ${ARCHIVE}$" SHA256SUMS | shasum -a 256 -c -
fi
tar -xzf "${ARCHIVE}"
cd "quickcoffee-${VERSION}-${TARGET}"
./qcoffee --version
./qcoffee --json --module-root examples/getting-started demo -- '{"name":"  Fix login  ","tags":[" bug ","urgent"]}'
./qtest --module-root examples/getting-started test
./qcoffee --module-root examples/pricing demo
./qtest --module-root examples/pricing test
CONFIG_JSON="$(./qcson to-json examples/pricing/config.cson)"
./qcoffee --module-root examples/pricing configured -- "$CONFIG_JSON"
```

Intel macOS 将 `TARGET` 改为 `x86_64-apple-darwin`，Linux 改为 `x86_64-unknown-linux-gnu`。成功时入门任务会输出清洗后的 JSON，入门 qtest 输出 `ok test/normalize_task.coffee`；定价 demo 会输出精确 Decimal 报价与 `pricing.ineligible` 业务拒绝，定价 qtest 输出 `ok test.coffee`。

Windows PowerShell 使用同一 release 中的 zip：

```powershell
$Version = "0.1.0"
$Target = "x86_64-pc-windows-msvc"
$Archive = "quickcoffee-$Version-$Target.zip"
$Base = "https://github.com/coffee-js/quickcoffee/releases/download/v$Version"
Invoke-WebRequest "$Base/$Archive" -OutFile $Archive
Invoke-WebRequest "$Base/SHA256SUMS" -OutFile SHA256SUMS
$Expected = ((Get-Content SHA256SUMS | Where-Object { $_ -match "  $([regex]::Escape($Archive))$" }) -split "  ")[0]
$Actual = (Get-FileHash -Algorithm SHA256 $Archive).Hash.ToLowerInvariant()
if ($Actual -ne $Expected) { throw "checksum mismatch: $Archive" }
Expand-Archive $Archive -DestinationPath .
Set-Location "quickcoffee-$Version-$Target"
.\qcoffee.exe --version
.\qcoffee.exe --json --module-root examples\getting-started demo -- '{"name":"  Fix login  ","tags":[" bug ","urgent"]}'
.\qtest.exe --module-root examples\getting-started test
.\qcoffee.exe --module-root examples\pricing demo
.\qtest.exe --module-root examples\pricing test
$ConfigJson = (.\qcson.exe to-json examples\pricing\config.cson | Out-String).TrimEnd()
.\qcoffee.exe --module-root examples\pricing configured -- $ConfigJson
```

维护者可以在 tag 前本地运行发布工具测试和完整门禁：

```sh
python3 scripts/test_release.py
make check
```

### Rust crate 分发决策

0.1 暂缓 crates.io 发布。CLI 用户使用上述正式 Release；Rust embedding 用户固定经过验证的 `v0.1.0` 完整 commit，不使用浮动分支：

```toml
[dependencies]
quickcoffee = { git = "https://github.com/coffee-js/quickcoffee.git", rev = "b3d27d24d15d76786baa21614b9cc2a97b28579e" }
```

只有真实分发需求出现时才建立独立 crates.io 发布 issue。完整宿主选择见[生产嵌入指南](deployment.md)。

## English

Formal releases use a `vX.Y.Z` tag whose `X.Y.Z` must match both the package version in `Cargo.toml` and a version heading in `CHANGELOG.md`. The release workflow uses the committed `Cargo.lock` and Rust 1.85.0 to build native `x86_64-unknown-linux-gnu`, `x86_64-apple-darwin`, and `aarch64-apple-darwin` `.tar.gz` archives plus an `x86_64-pc-windows-msvc` `.zip` archive.

macOS Intel uses `macos-15-intel`, while Apple silicon uses `macos-15`. The workflow asserts `uname -m` before building so runner-label drift cannot silently produce an artifact for the wrong architecture.

Every archive contains `qcoffee`, `qtest`, `qdocco`, `qbench`, `qcson`, the README, changelog, dual licenses, a modifiable and testable JSON-cleanup task under `examples/getting-started/`, and the canonical Decimal `.litcoffee` rule, human-maintained `.cson` configuration, and thin `.coffee` demo/qtest/configured entries under `examples/pricing/`. `qbench` is maintainer-facing performance observability rather than a requirement for running user scripts. Each platform job runs release tests, crate package verification, and build-directory version smoke checks for all five CLIs. It then verifies archive members, paths, and modes before extracting into a temporary clean workspace and using only the extracted binaries and packaged sources to run:

- version checks for all five CLIs;
- canonical CSON → JSON and JSON → CSON conversion through `qcson`;
- the README starter task with JSON input, stable output, and isolated `qtest`;
- the packaged `.cson` configuration through `qcson` and explicit `argv` into Decimal pricing, matching the Rust embedding baseline;
- one standalone `.coffee` script;
- one GitHub-compatible `.litcoffee` document containing Markdown inline code and a four-space executable block;
- `qdocco --check`; and
- the packaged Decimal-pricing `qcoffee --module-root` demo and `qtest --module-root` case.

The aggregation job accepts exactly one artifact for every one of the four required platforms, writes a filename-sorted `SHA256SUMS`, and immediately verifies it again. After uploading the aggregate artifact, a separate Ubuntu job with no repository checkout and no Rust toolchain downloads the final bundle again, verifies all four checksums, and exercises the same clean-install workflow from the downloaded Linux archive. The tag workflow cannot publish a GitHub Release until this artifact round-trip passes.

Pull requests that change release configuration and manual GitHub Actions dispatches produce downloadable rehearsal artifacts but never create a GitHub Release. Only a matching tag push publishes after every gate succeeds. Code signing, notarization, package-manager formulae, crates.io publishing, Linux musl, and Windows arm64 are outside this slice.

On Unix-like systems, verify downloads with:

```sh
shasum -a 256 -c SHA256SUMS
```

On Windows, compare `Get-FileHash -Algorithm SHA256` output with `SHA256SUMS`. These archives are verified packaging of a revision and toolchain, not code signatures or platform notarization.

### Quick acceptance without a repository checkout

After the formal `v0.1.0` release, a Linux or macOS user needs only the platform archive and checksum manifest:

```sh
VERSION=0.1.0
TARGET=aarch64-apple-darwin
ARCHIVE="quickcoffee-${VERSION}-${TARGET}.tar.gz"
BASE="https://github.com/coffee-js/quickcoffee/releases/download/v${VERSION}"
curl -fLO "${BASE}/${ARCHIVE}"
curl -fLO "${BASE}/SHA256SUMS"
if command -v sha256sum >/dev/null; then
  grep "  ${ARCHIVE}$" SHA256SUMS | sha256sum -c -
else
  grep "  ${ARCHIVE}$" SHA256SUMS | shasum -a 256 -c -
fi
tar -xzf "${ARCHIVE}"
cd "quickcoffee-${VERSION}-${TARGET}"
./qcoffee --version
./qcoffee --json --module-root examples/getting-started demo -- '{"name":"  Fix login  ","tags":[" bug ","urgent"]}'
./qtest --module-root examples/getting-started test
./qcoffee --module-root examples/pricing demo
./qtest --module-root examples/pricing test
CONFIG_JSON="$(./qcson to-json examples/pricing/config.cson)"
./qcoffee --module-root examples/pricing configured -- "$CONFIG_JSON"
```

Use `x86_64-apple-darwin` for Intel macOS or `x86_64-unknown-linux-gnu` for Linux. The getting-started task prints normalized JSON and its qtest prints `ok test/normalize_task.coffee`; the pricing demo prints an exact Decimal quote and a `pricing.ineligible` business rejection, and its qtest prints `ok test.coffee`.

On Windows PowerShell, use the zip from the same release:

```powershell
$Version = "0.1.0"
$Target = "x86_64-pc-windows-msvc"
$Archive = "quickcoffee-$Version-$Target.zip"
$Base = "https://github.com/coffee-js/quickcoffee/releases/download/v$Version"
Invoke-WebRequest "$Base/$Archive" -OutFile $Archive
Invoke-WebRequest "$Base/SHA256SUMS" -OutFile SHA256SUMS
$Expected = ((Get-Content SHA256SUMS | Where-Object { $_ -match "  $([regex]::Escape($Archive))$" }) -split "  ")[0]
$Actual = (Get-FileHash -Algorithm SHA256 $Archive).Hash.ToLowerInvariant()
if ($Actual -ne $Expected) { throw "checksum mismatch: $Archive" }
Expand-Archive $Archive -DestinationPath .
Set-Location "quickcoffee-$Version-$Target"
.\qcoffee.exe --version
.\qcoffee.exe --json --module-root examples\getting-started demo -- '{"name":"  Fix login  ","tags":[" bug ","urgent"]}'
.\qtest.exe --module-root examples\getting-started test
.\qcoffee.exe --module-root examples\pricing demo
.\qtest.exe --module-root examples\pricing test
$ConfigJson = (.\qcson.exe to-json examples\pricing\config.cson | Out-String).TrimEnd()
.\qcoffee.exe --module-root examples\pricing configured -- $ConfigJson
```

Before tagging, maintainers can run:

```sh
python3 scripts/test_release.py
make check
```

### Rust crate distribution decision

crates.io publication is deferred for 0.1. CLI users use the formal Release above; Rust embedders pin the complete verified `v0.1.0` commit rather than a moving branch:

```toml
[dependencies]
quickcoffee = { git = "https://github.com/coffee-js/quickcoffee.git", rev = "b3d27d24d15d76786baa21614b9cc2a97b28579e" }
```

Open a focused crates.io publication issue only when real distribution demand appears. See the [production embedding cookbook](deployment.md) for the complete host choices.
