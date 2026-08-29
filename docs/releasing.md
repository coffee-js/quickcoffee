# 发布与平台归档 / Releases and platform archives

## 中文

正式版本使用 `vX.Y.Z` tag，并要求 `X.Y.Z` 同时匹配 `Cargo.toml` 的 package version 与 `CHANGELOG.md` 的版本标题。Release workflow 固定使用已提交的 `Cargo.lock` 和 Rust 1.85.0，在原生 runner 上生成以下制品：

- `x86_64-unknown-linux-gnu`：`.tar.gz`
- `x86_64-apple-darwin`：`.tar.gz`
- `aarch64-apple-darwin`：`.tar.gz`
- `x86_64-pc-windows-msvc`：`.zip`

macOS Intel 使用 `macos-15-intel`，Apple silicon 使用 `macos-15`；workflow 会在构建前校验 `uname -m`，避免 runner label 漂移后悄悄生成错误架构的制品。

每个归档包含 `qcoffee`、`qtest`、`qdocco`、`qbench`、README、更新日志和双许可证。`qbench` 用于维护者性能观测，不是运行用户脚本所需的组件。平台 job 会执行 release tests、crate package verification 和四个 CLI 的构建目录版本 smoke；归档完成后还会先验证成员、路径和权限，再解包到临时干净工作区，仅使用解包后的二进制运行：

- 四个 CLI 的版本检查；
- 一个独立 `.coffee` 脚本；
- 一个含 Markdown 行内代码与四空格可执行块的 GitHub-compatible `.litcoffee`；
- `qdocco --check`；
- 一个由 `.litcoffee` 规则和 `.coffee` 测试入口组成的 Decimal 定价 `qtest --module-root` 用例。

聚合 job 只接受完整且不重复的四平台制品，生成按文件名排序的 `SHA256SUMS`，再立即重新校验。

修改发布配置的 pull request 和在 GitHub Actions 手动运行该 workflow 都只会生成可下载的演练 artifacts，不会创建 GitHub Release。只有匹配的 tag push 且全部门禁通过时，聚合 job 才会发布 release。当前范围不包含代码签名、公证、包管理器配方、crates.io publish、Linux musl 或 Windows arm64。

下载后可在 Unix-like 系统校验：

```sh
shasum -a 256 -c SHA256SUMS
```

Windows PowerShell 可将 `Get-FileHash -Algorithm SHA256` 的结果与 `SHA256SUMS` 对照。归档是对提交内容和工具链的可验证打包，不构成代码签名或平台公证。

维护者可以在 tag 前本地运行发布工具测试和完整门禁：

```sh
python3 scripts/test_release.py
make check
```

## English

Formal releases use a `vX.Y.Z` tag whose `X.Y.Z` must match both the package version in `Cargo.toml` and a version heading in `CHANGELOG.md`. The release workflow uses the committed `Cargo.lock` and Rust 1.85.0 to build native `x86_64-unknown-linux-gnu`, `x86_64-apple-darwin`, and `aarch64-apple-darwin` `.tar.gz` archives plus an `x86_64-pc-windows-msvc` `.zip` archive.

macOS Intel uses `macos-15-intel`, while Apple silicon uses `macos-15`. The workflow asserts `uname -m` before building so runner-label drift cannot silently produce an artifact for the wrong architecture.

Every archive contains `qcoffee`, `qtest`, `qdocco`, `qbench`, the README, changelog, and dual licenses. `qbench` is maintainer-facing performance observability rather than a requirement for running user scripts. Each platform job runs release tests, crate package verification, and build-directory version smoke checks for all four CLIs. It then verifies archive members, paths, and modes before extracting into a temporary clean workspace and using only the extracted binaries to run:

- version checks for all four CLIs;
- one standalone `.coffee` script;
- one GitHub-compatible `.litcoffee` document containing Markdown inline code and a four-space executable block;
- `qdocco --check`; and
- a Decimal-pricing `qtest --module-root` case composed from a `.litcoffee` rule and `.coffee` test entry.

The aggregation job accepts exactly one artifact for every one of the four required platforms, writes a filename-sorted `SHA256SUMS`, and immediately verifies it again.

Pull requests that change release configuration and manual GitHub Actions dispatches produce downloadable rehearsal artifacts but never create a GitHub Release. Only a matching tag push publishes after every gate succeeds. Code signing, notarization, package-manager formulae, crates.io publishing, Linux musl, and Windows arm64 are outside this slice.

On Unix-like systems, verify downloads with:

```sh
shasum -a 256 -c SHA256SUMS
```

On Windows, compare `Get-FileHash -Algorithm SHA256` output with `SHA256SUMS`. These archives are verified packaging of a revision and toolchain, not code signatures or platform notarization.

Before tagging, maintainers can run:

```sh
python3 scripts/test_release.py
make check
```
