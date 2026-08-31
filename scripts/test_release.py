#!/usr/bin/env python3
"""Unit tests for the repository-owned release packaging contract."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock
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
        for source_name in release.EXAMPLE_SOURCES:
            source = self.repo / source_name
            source.parent.mkdir(parents=True, exist_ok=True)
            source.write_text(f"fixture:{source_name}\n", encoding="utf-8")

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
            "aarch64-apple-darwin",
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

    def test_clean_install_uses_extracted_binaries_and_packaged_examples(self) -> None:
        for target in ("aarch64-apple-darwin", "x86_64-pc-windows-msvc"):
            with self.subTest(target=target):
                original_binaries = self.binaries(target)
                archive = release.create_archive(
                    self.repo, original_binaries, self.root / "dist", target
                )
                calls: list[tuple[str, tuple[str, ...]]] = []

                def run(command, *, cwd, check, capture_output, text):
                    self.assertFalse(check)
                    self.assertTrue(capture_output)
                    self.assertTrue(text)
                    binary = Path(command[0])
                    self.assertTrue(binary.is_file())
                    self.assertNotEqual(binary.parent, original_binaries)
                    cwd = Path(cwd)
                    self.assertTrue(cwd.is_dir())
                    name = binary.name.removesuffix(".exe")
                    arguments = tuple(command[1:])
                    calls.append((name, arguments))
                    if arguments == ("--version",):
                        stdout = f"{name} 1.2.3\n"
                    elif name == "qcoffee" and arguments in {
                        ("plain.coffee",),
                        ("literate.litcoffee",),
                    }:
                        source = (cwd / arguments[0]).read_text(encoding="utf-8")
                        self.assertIn("answer + 2", source)
                        stdout = "42\n"
                    elif name == "qdocco" and arguments == (
                        "--check",
                        "document.litcoffee",
                    ):
                        source = (cwd / "document.litcoffee").read_text(
                            encoding="utf-8"
                        )
                        self.assertIn("Inline `qdocco`", source)
                        stdout = ""
                    elif name == "qcson" and arguments == (
                        "to-json",
                        "config.cson",
                    ):
                        self.assertEqual(
                            (cwd / "config.cson").read_text(encoding="utf-8"),
                            "enabled: true\namount: 12.30\n",
                        )
                        stdout = '{"amount":12.3,"enabled":true}\n'
                    elif name == "qcson" and arguments == (
                        "to-cson",
                        "config.json",
                    ):
                        self.assertEqual(
                            (cwd / "config.json").read_text(encoding="utf-8"),
                            '{"enabled":true,"amount":12.30}\n',
                        )
                        stdout = "amount: 12.3\nenabled: true\n"
                    elif (
                        name == "qcson"
                        and len(arguments) == 2
                        and arguments[0] == "to-json"
                        and Path(arguments[1]).name == "config.cson"
                    ):
                        pricing = Path(arguments[1]).parent
                        self.assertEqual(pricing.parent.parent, binary.parent)
                        stdout = (
                            '{"accepted":{"country":"CN",'
                            '"customer_tier":"member","item_count":3,'
                            '"subtotal":"120"},"rejected":{"country":"US",'
                            '"customer_tier":"standard","item_count":1,'
                            '"subtotal":"5"},"schema":"pricing-orders/v1"}\n'
                        )
                    elif (
                        name == "qcoffee"
                        and len(arguments) == 5
                        and arguments[0] == "--module-root"
                        and arguments[2:4] == ("configured", "--")
                    ):
                        pricing = Path(arguments[1])
                        self.assertEqual(pricing.parent.parent, binary.parent)
                        self.assertIn('"schema":"pricing-orders/v1"', arguments[4])
                        stdout = (
                            "{quote: {discount: 12m, net: 108m, subtotal: 120m, "
                            "tax: 14.04m, total: 122.04m}, "
                            "rejection: pricing.ineligible}\n"
                        )
                    elif (
                        name in {"qcoffee", "qtest"}
                        and len(arguments) == 3
                        and arguments[0] == "--module-root"
                    ):
                        pricing = Path(arguments[1])
                        self.assertEqual(pricing.parent.parent, binary.parent)
                        for source_name in release.EXAMPLE_SOURCES:
                            packaged = binary.parent / source_name
                            self.assertEqual(
                                packaged.read_text(encoding="utf-8"),
                                f"fixture:{source_name}\n",
                            )
                        if name == "qcoffee" and arguments[2] == "demo":
                            stdout = (
                                "{quote: {discount: 12m, net: 108m, subtotal: 120m, "
                                "tax: 14.04m, total: 122.04m}, "
                                "rejection: pricing.ineligible}\n"
                            )
                        elif name == "qtest" and arguments[2] == "test":
                            stdout = "ok test.coffee\n"
                        else:
                            self.fail(f"unexpected installed module command: {command!r}")
                    else:
                        self.fail(f"unexpected installed command: {command!r}")
                    return subprocess.CompletedProcess(command, 0, stdout, "")

                with mock.patch.object(release.subprocess, "run", side_effect=run):
                    release.verify_install(archive, "1.2.3", target)

                self.assertEqual(
                    [
                        name
                        for name, arguments in calls
                        if arguments == ("--version",)
                    ],
                    list(release.BINARIES),
                )
                self.assertEqual(len(calls), 14)

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
        self.assertIn(
            "os: macos-15-intel\n            target: x86_64-apple-darwin",
            workflow,
        )
        self.assertIn(
            "os: macos-15\n            target: aarch64-apple-darwin", workflow
        )
        self.assertIn("matrix.runner_arch", workflow)
        self.assertIn("scripts/release.py verify-install", workflow)
        self.assertNotIn("cargo publish --locked", workflow)
        distribution = workflow.split("  verify_distribution:\n", 1)[1].split(
            "\n  publish:\n", 1
        )[0]
        self.assertNotIn("actions/checkout", distribution)
        self.assertIn("dist/verify-release.py verify-checksums", distribution)
        self.assertIn("dist/verify-release.py verify-install", distribution)
        self.assertIn("needs: [validate, bundle, verify_distribution]", workflow)
        workflows = "\n".join(
            path.read_text(encoding="utf-8")
            for path in sorted((repository / ".github/workflows").glob("*.yml"))
        )
        self.assertIn("actions/upload-artifact@v7", workflow)
        self.assertIn("actions/download-artifact@v8", workflow)
        for deprecated in ("@v4", "@v5", "@v6"):
            self.assertNotIn(f"actions/upload-artifact{deprecated}", workflows)
        for deprecated in ("@v4", "@v5", "@v6", "@v7"):
            self.assertNotIn(f"actions/download-artifact{deprecated}", workflows)


if __name__ == "__main__":
    unittest.main()
