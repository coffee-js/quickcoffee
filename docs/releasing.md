# 发布与平台归档 / Releases and platform archives

## 中文

正式版本使用 `vX.Y.Z` tag，并要求 `X.Y.Z` 同时匹配 `Cargo.toml` 的 package version 与 `CHANGELOG.md` 的版本标题。Release workflow 固定使用已提交的 `Cargo.lock` 和 Rust 1.85.0，在原生 runner 上生成以下制品：

- `x86_64-unknown-linux-gnu`：`.tar.gz`
- `x86_64-apple-darwin`：`.tar.gz`
- `x86_64-pc-windows-msvc`：`.zip`

每个归档包含 `qcoffee`、`qtest`、`qdocco`、`qbench`、README、更新日志和双许可证。平台 job 会执行 release tests、crate package verification、四个 CLI 的版本 smoke check，并重新读取归档验证成员与路径。聚合 job 只接受完整且不重复的三平台制品，生成按文件名排序的 `SHA256SUMS`，再立即重新校验。

修改发布配置的 pull request 和在 GitHub Actions 手动运行该 workflow 都只会生成可下载的演练 artifacts，不会创建 GitHub Release。只有匹配的 tag push 且全部门禁通过时，聚合 job 才会发布 release。当前范围不包含代码签名、公证、包管理器配方、crates.io publish、macOS arm64、Linux musl 或 Windows arm64。

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

Formal releases use a `vX.Y.Z` tag whose `X.Y.Z` must match both the package version in `Cargo.toml` and a version heading in `CHANGELOG.md`. The release workflow uses the committed `Cargo.lock` and Rust 1.85.0 to build native `x86_64-unknown-linux-gnu` and `x86_64-apple-darwin` `.tar.gz` archives plus an `x86_64-pc-windows-msvc` `.zip` archive.

Every archive contains `qcoffee`, `qtest`, `qdocco`, `qbench`, the README, changelog, and dual licenses. Each platform job runs release tests, crate package verification, version smoke checks for all four CLIs, and archive member/path verification. The aggregation job accepts exactly one artifact for every required platform, writes a filename-sorted `SHA256SUMS`, and immediately verifies it again.

Pull requests that change release configuration and manual GitHub Actions dispatches produce downloadable rehearsal artifacts but never create a GitHub Release. Only a matching tag push publishes after every gate succeeds. Code signing, notarization, package-manager formulae, crates.io publishing, macOS arm64, Linux musl, and Windows arm64 are outside this slice.

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
