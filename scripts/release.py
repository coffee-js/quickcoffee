#!/usr/bin/env python3
"""Build and verify deterministic QuickCoffee release archives."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tarfile
import zipfile


BINARIES = ("qcoffee", "qtest", "qdocco", "qbench")
DOCUMENTS = ("README.md", "CHANGELOG.md", "LICENSE-MIT", "LICENSE-APACHE")
VERSION_PATTERN = re.compile(
    r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?"
)
CHANGELOG_HEADING = re.compile(r"^## \[([^]]+)]\s*$")


class ReleaseError(Exception):
    """A deterministic release-contract violation."""


def package_version(repo: Path) -> str:
    """Read the package version without requiring a third-party TOML parser."""
    in_package = False
    for raw_line in (repo / "Cargo.toml").read_text(encoding="utf-8").splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if line.startswith("["):
            in_package = line == "[package]"
            continue
        if in_package:
            match = re.fullmatch(r'version\s*=\s*"([^"]+)"', line)
            if match:
                version = match.group(1)
                if not VERSION_PATTERN.fullmatch(version):
                    raise ReleaseError(f"unsupported Cargo package version: {version}")
                return version
    raise ReleaseError("Cargo.toml has no [package] version")


def changelog_entry(repo: Path, version: str) -> str:
    """Return one version body and reject absent or duplicate headings."""
    lines = (repo / "CHANGELOG.md").read_text(encoding="utf-8").splitlines()
    starts = [index for index, line in enumerate(lines) if line == f"## [{version}]"]
    if len(starts) != 1:
        raise ReleaseError(
            f"CHANGELOG.md must contain exactly one '## [{version}]' heading"
        )
    start = starts[0]
    end = len(lines)
    for index in range(start + 1, len(lines)):
        if CHANGELOG_HEADING.fullmatch(lines[index]):
            end = index
            break
    body = "\n".join(lines[start + 1 : end]).strip()
    if not body:
        raise ReleaseError(f"CHANGELOG.md entry {version} is empty")
    return body + "\n"


def validate_version(repo: Path, tag: str | None = None) -> str:
    """Validate Cargo/changelog/tag agreement and return the bare version."""
    version = package_version(repo)
    changelog_entry(repo, version)
    if tag is not None and tag != f"v{version}":
        raise ReleaseError(
            f"release tag {tag!r} does not match Cargo version v{version}"
        )
    return version


def archive_name(version: str, target: str) -> str:
    suffix = ".zip" if "windows" in target else ".tar.gz"
    return f"quickcoffee-{version}-{target}{suffix}"


def archive_entries(repo: Path, binary_dir: Path, target: str) -> list[tuple[str, Path, int]]:
    executable_suffix = ".exe" if "windows" in target else ""
    entries: list[tuple[str, Path, int]] = []
    for binary in BINARIES:
        path = binary_dir / f"{binary}{executable_suffix}"
        if not path.is_file():
            raise ReleaseError(f"missing release binary: {path}")
        if path.is_symlink():
            raise ReleaseError(f"release binary must not be a symlink: {path}")
        entries.append((path.name, path, 0o755))
    for document in DOCUMENTS:
        path = repo / document
        if not path.is_file():
            raise ReleaseError(f"missing release document: {path}")
        if path.is_symlink():
            raise ReleaseError(f"release document must not be a symlink: {path}")
        entries.append((document, path, 0o644))
    return sorted(entries)


def create_tar(path: Path, root: str, entries: list[tuple[str, Path, int]]) -> None:
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w", format=tarfile.PAX_FORMAT) as archive:
        for name, source, mode in entries:
            data = source.read_bytes()
            info = tarfile.TarInfo(f"{root}/{name}")
            info.size = len(data)
            info.mode = mode
            info.mtime = 0
            info.uid = 0
            info.gid = 0
            info.uname = ""
            info.gname = ""
            archive.addfile(info, io.BytesIO(data))
    with path.open("wb") as output:
        with gzip.GzipFile(filename="", mode="wb", fileobj=output, mtime=0) as compressed:
            compressed.write(buffer.getvalue())


def create_zip(path: Path, root: str, entries: list[tuple[str, Path, int]]) -> None:
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name, source, mode in entries:
            info = zipfile.ZipInfo(f"{root}/{name}", date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 3
            info.external_attr = mode << 16
            archive.writestr(info, source.read_bytes())


def create_archive(repo: Path, binary_dir: Path, output_dir: Path, target: str) -> Path:
    version = validate_version(repo)
    root = f"quickcoffee-{version}-{target}"
    output_dir.mkdir(parents=True, exist_ok=True)
    output = output_dir / archive_name(version, target)
    entries = archive_entries(repo, binary_dir, target)
    if output.suffix == ".zip":
        create_zip(output, root, entries)
    else:
        create_tar(output, root, entries)
    return output


def expected_members(version: str, target: str) -> set[str]:
    root = f"quickcoffee-{version}-{target}"
    executable_suffix = ".exe" if "windows" in target else ""
    names = [f"{binary}{executable_suffix}" for binary in BINARIES]
    names.extend(DOCUMENTS)
    return {f"{root}/{name}" for name in names}


def safe_member(name: str) -> bool:
    path = PurePosixPath(name)
    return not path.is_absolute() and ".." not in path.parts and len(path.parts) == 2


def verify_archive(path: Path, version: str, target: str) -> None:
    expected = expected_members(version, target)
    if path.name != archive_name(version, target):
        raise ReleaseError(f"unexpected archive name: {path.name}")
    if path.suffix == ".zip":
        with zipfile.ZipFile(path) as archive:
            infos = archive.infolist()
            members = [info.filename for info in infos]
            if any(info.is_dir() for info in infos):
                raise ReleaseError("release archive contains an unexpected directory member")
            for info in infos:
                if not archive.read(info):
                    raise ReleaseError(f"release archive contains an empty file: {info.filename}")
                basename = PurePosixPath(info.filename).name
                expected_mode = 0o755 if basename.removesuffix(".exe") in BINARIES else 0o644
                actual_mode = (info.external_attr >> 16) & 0o777
                if actual_mode != expected_mode:
                    raise ReleaseError(f"unexpected archive mode for {info.filename}")
    else:
        with tarfile.open(path, "r:gz") as archive:
            infos = archive.getmembers()
            if any(not info.isfile() for info in infos):
                raise ReleaseError("release archive contains a non-file member")
            members = [info.name for info in infos]
            for info in infos:
                source = archive.extractfile(info)
                if source is None or not source.read():
                    raise ReleaseError(f"release archive contains an empty file: {info.name}")
                basename = PurePosixPath(info.name).name
                expected_mode = 0o755 if basename in BINARIES else 0o644
                if info.mode & 0o777 != expected_mode:
                    raise ReleaseError(f"unexpected archive mode for {info.name}")
    if len(members) != len(set(members)):
        raise ReleaseError("release archive contains duplicate members")
    if any(not safe_member(member) for member in members):
        raise ReleaseError("release archive contains an unsafe or nested member path")
    actual = set(members)
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        raise ReleaseError(f"archive member mismatch; missing={missing}, extra={extra}")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def required_archives(dist: Path, version: str, targets: list[str]) -> list[Path]:
    if not targets or len(targets) != len(set(targets)):
        raise ReleaseError("required targets must be non-empty and unique")
    expected_names = {archive_name(version, target) for target in targets}
    actual_names = {
        path.name
        for path in dist.iterdir()
        if path.is_file() and (path.name.endswith(".tar.gz") or path.suffix == ".zip")
    }
    symlinks = sorted(
        path.name
        for path in dist.iterdir()
        if path.is_symlink()
        and (path.name.endswith(".tar.gz") or path.suffix == ".zip")
    )
    if symlinks:
        raise ReleaseError(f"release archives must not be symlinks: {symlinks}")
    if actual_names != expected_names:
        missing = sorted(expected_names - actual_names)
        extra = sorted(actual_names - expected_names)
        raise ReleaseError(f"release set mismatch; missing={missing}, extra={extra}")
    return [dist / name for name in sorted(expected_names)]


def write_checksums(dist: Path, version: str, targets: list[str]) -> Path:
    archives = required_archives(dist, version, targets)
    manifest = dist / "SHA256SUMS"
    manifest.write_text(
        "".join(f"{sha256(path)}  {path.name}\n" for path in archives),
        encoding="ascii",
        newline="\n",
    )
    return manifest


def verify_checksums(dist: Path, version: str, targets: list[str]) -> None:
    archives = required_archives(dist, version, targets)
    expected_names = [path.name for path in archives]
    manifest = dist / "SHA256SUMS"
    if not manifest.is_file():
        raise ReleaseError("missing SHA256SUMS")
    records: list[tuple[str, str]] = []
    for line in manifest.read_text(encoding="ascii").splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  ([^/\\]+)", line)
        if not match:
            raise ReleaseError(f"invalid SHA256SUMS line: {line!r}")
        records.append((match.group(1), match.group(2)))
    names = [name for _, name in records]
    if names != expected_names:
        raise ReleaseError("SHA256SUMS entries must exactly match sorted release archives")
    for digest, name in records:
        if sha256(dist / name) != digest:
            raise ReleaseError(f"checksum mismatch: {name}")


def smoke(binary_dir: Path, version: str, target: str) -> None:
    suffix = ".exe" if "windows" in target else ""
    for name in BINARIES:
        binary = binary_dir / f"{name}{suffix}"
        result = subprocess.run(
            [os.fspath(binary), "--version"],
            check=False,
            capture_output=True,
            text=True,
        )
        expected = f"{name} {version}\n"
        if result.returncode != 0 or result.stdout != expected or result.stderr:
            raise ReleaseError(
                f"{binary} --version failed: code={result.returncode}, "
                f"stdout={result.stdout!r}, stderr={result.stderr!r}"
            )


def parser() -> argparse.ArgumentParser:
    root = Path(__file__).resolve().parents[1]
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--repo", type=Path, default=root)
    commands = cli.add_subparsers(dest="command", required=True)

    version = commands.add_parser("version")
    version.add_argument("--tag")

    notes = commands.add_parser("notes")
    notes.add_argument("--version")
    notes.add_argument("--output", type=Path, required=True)

    package = commands.add_parser("package")
    package.add_argument("--binary-dir", type=Path, required=True)
    package.add_argument("--output-dir", type=Path, required=True)
    package.add_argument("--target", required=True)

    verify = commands.add_parser("verify-archive")
    verify.add_argument("--archive", type=Path, required=True)
    verify.add_argument("--version")
    verify.add_argument("--target", required=True)

    smoke_parser = commands.add_parser("smoke")
    smoke_parser.add_argument("--binary-dir", type=Path, required=True)
    smoke_parser.add_argument("--version")
    smoke_parser.add_argument("--target", required=True)

    for name in ("checksums", "verify-checksums"):
        checksums = commands.add_parser(name)
        checksums.add_argument("--dist", type=Path, required=True)
        checksums.add_argument("--version")
        checksums.add_argument("--target", action="append", required=True)
    return cli


def main() -> int:
    args = parser().parse_args()
    repo = args.repo.resolve()
    try:
        if args.command == "version":
            print(validate_version(repo, args.tag))
        elif args.command == "notes":
            version = args.version or validate_version(repo)
            if version != validate_version(repo):
                raise ReleaseError("requested notes version does not match Cargo version")
            args.output.write_text(changelog_entry(repo, version), encoding="utf-8")
        elif args.command == "package":
            print(create_archive(repo, args.binary_dir, args.output_dir, args.target))
        elif args.command == "verify-archive":
            version = args.version or validate_version(repo)
            verify_archive(args.archive, version, args.target)
        elif args.command == "smoke":
            version = args.version or validate_version(repo)
            smoke(args.binary_dir, version, args.target)
        elif args.command == "checksums":
            version = args.version or validate_version(repo)
            print(write_checksums(args.dist, version, args.target))
        elif args.command == "verify-checksums":
            version = args.version or validate_version(repo)
            verify_checksums(args.dist, version, args.target)
        else:
            raise AssertionError(args.command)
    except (OSError, ReleaseError, tarfile.TarError, zipfile.BadZipFile) as error:
        print(f"release error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
