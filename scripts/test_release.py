#!/usr/bin/env python3
"""Unit tests for the repository-owned release packaging contract."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import unittest
import zipfile


SCRIPT = Path(__file__).with_name("release.py")
SPEC = importlib.util.spec_from_file_location("quickcoffee_release", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)


class ReleaseToolTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        (self.repo / "Cargo.toml").write_text(
            '[package]\nname = "quickcoffee"\nversion = "1.2.3"\n',
            encoding="utf-8",
        )
        (self.repo / "CHANGELOG.md").write_text(
            "# Changelog\n\n## [1.2.3]\n\nCurrent notes.\n\n"
            "## [1.2.2]\n\nOld notes.\n",
            encoding="utf-8",
        )
        for name in ("README.md", "LICENSE-MIT", "LICENSE-APACHE"):
            (self.repo / name).write_text(f"{name}\n", encoding="utf-8")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def binaries(self, target: str) -> Path:
        directory = self.root / target
        directory.mkdir(exist_ok=True)
        suffix = ".exe" if "windows" in target else ""
        for name in release.BINARIES:
            (directory / f"{name}{suffix}").write_bytes(f"binary:{name}\n".encode())
        return directory

    def test_version_requires_cargo_changelog_and_tag_agreement(self) -> None:
        self.assertEqual(release.validate_version(self.repo), "1.2.3")
        self.assertEqual(release.validate_version(self.repo, "v1.2.3"), "1.2.3")
        self.assertEqual(release.changelog_entry(self.repo, "1.2.3"), "Current notes.\n")
        with self.assertRaisesRegex(release.ReleaseError, "does not match"):
            release.validate_version(self.repo, "v1.2.4")
        (self.repo / "CHANGELOG.md").write_text(
            "# Changelog\n\n## [1.2.2]\n\nOld notes.\n", encoding="utf-8"
        )
        with self.assertRaisesRegex(release.ReleaseError, "exactly one"):
            release.validate_version(self.repo)

    def test_tar_and_zip_are_deterministic_and_verified(self) -> None:
        targets = ("x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc")
        for target in targets:
            first_dir = self.root / f"first-{target}"
            second_dir = self.root / f"second-{target}"
            first = release.create_archive(
                self.repo, self.binaries(target), first_dir, target
            )
            second = release.create_archive(
                self.repo, self.binaries(target), second_dir, target
            )
            self.assertEqual(first.read_bytes(), second.read_bytes())
            release.verify_archive(first, "1.2.3", target)

    def test_archive_verifier_rejects_unsafe_members(self) -> None:
        target = "x86_64-pc-windows-msvc"
        output = self.root / release.archive_name("1.2.3", target)
        with zipfile.ZipFile(output, "w") as archive:
            archive.writestr("../qcoffee.exe", b"bad")
        with self.assertRaises(release.ReleaseError):
            release.verify_archive(output, "1.2.3", target)

    def test_checksums_require_the_complete_sorted_release_set(self) -> None:
        targets = [
            "x86_64-unknown-linux-gnu",
            "x86_64-apple-darwin",
            "x86_64-pc-windows-msvc",
        ]
        dist = self.root / "dist"
        dist.mkdir()
        for target in reversed(targets):
            (dist / release.archive_name("1.2.3", target)).write_bytes(target.encode())
        manifest = release.write_checksums(dist, "1.2.3", targets)
        names = [line.split("  ", 1)[1] for line in manifest.read_text().splitlines()]
        self.assertEqual(names, sorted(names))
        release.verify_checksums(dist, "1.2.3", targets)

        first = dist / release.archive_name("1.2.3", targets[0])
        first.write_bytes(b"tampered")
        with self.assertRaisesRegex(release.ReleaseError, "checksum mismatch"):
            release.verify_checksums(dist, "1.2.3", targets)

        first.unlink()
        with self.assertRaisesRegex(release.ReleaseError, "missing"):
            release.write_checksums(dist, "1.2.3", targets)

    def test_repository_workflow_keeps_manual_runs_non_publishing(self) -> None:
        repository = SCRIPT.resolve().parents[1]
        workflow = (repository / ".github/workflows/release.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("workflow_dispatch:", workflow)
        self.assertIn("pull_request:", workflow)
        self.assertIn("tags:", workflow)
        self.assertIn("github.event_name == 'push'", workflow)
        self.assertIn("refs/tags/", workflow)
        self.assertIn("cargo test --locked --release --target", workflow)
        self.assertIn("cargo package --locked", workflow)
        self.assertIn("gh release create", workflow)
        workflows = "\n".join(
            path.read_text(encoding="utf-8")
            for path in sorted((repository / ".github/workflows").glob("*.yml"))
        )
        self.assertIn("actions/upload-artifact@v5", workflow)
        self.assertIn("actions/download-artifact@v5", workflow)
        self.assertNotIn("actions/upload-artifact@v4", workflows)
        self.assertNotIn("actions/download-artifact@v4", workflows)


if __name__ == "__main__":
    unittest.main()
