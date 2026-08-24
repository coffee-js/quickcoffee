#!/usr/bin/env python3
"""Compare two quickcoffee.qbench.v1 JSONL runs without blocking on regressions."""

from __future__ import annotations

import argparse
import json
import os
import platform
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

QBENCH_SCHEMA = "quickcoffee.qbench.v1"
REPORT_SCHEMA = "quickcoffee.qbench-compare.v1"
METADATA_SCHEMA = "quickcoffee.qbench-run-metadata.v1"
PHASES = (
    ("compile", "compile_ns", "compile_mad_ns"),
    ("prepare", "prepare_ns", "prepare_mad_ns"),
    ("verify", "verify_ns", "verify_mad_ns"),
    ("execute", "execute_ns", "execute_mad_ns"),
)


class ComparisonError(ValueError):
    """Raised when benchmark inputs do not satisfy the comparison contract."""


@dataclass(frozen=True)
class Comparison:
    workload: str
    phase: str
    baseline_ns: int
    baseline_mad_ns: int
    candidate_ns: int
    candidate_mad_ns: int
    allowance_ns: float
    limit_ns: float
    alert: bool

    @property
    def delta_ratio(self) -> float | None:
        if self.baseline_ns == 0:
            return None
        return (self.candidate_ns - self.baseline_ns) / self.baseline_ns

    def as_json(self) -> dict[str, Any]:
        return {
            "workload": self.workload,
            "phase": self.phase,
            "baseline_ns": self.baseline_ns,
            "baseline_mad_ns": self.baseline_mad_ns,
            "candidate_ns": self.candidate_ns,
            "candidate_mad_ns": self.candidate_mad_ns,
            "delta_ratio": self.delta_ratio,
            "allowance_ns": self.allowance_ns,
            "limit_ns": self.limit_ns,
            "alert": self.alert,
        }


def _unsigned(record: dict[str, Any], field: str, source: Path, line: int) -> int:
    value = record.get(field)
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ComparisonError(
            f"{source}:{line}: field {field!r} must be an unsigned integer"
        )
    return value


def read_records(source: Path) -> dict[str, dict[str, Any]]:
    records: dict[str, dict[str, Any]] = {}
    try:
        lines = source.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise ComparisonError(f"cannot read {source}: {error}") from error
    for line_number, raw in enumerate(lines, 1):
        if not raw.strip():
            continue
        try:
            record = json.loads(raw)
        except json.JSONDecodeError as error:
            raise ComparisonError(f"{source}:{line_number}: invalid JSON: {error}") from error
        if not isinstance(record, dict):
            raise ComparisonError(f"{source}:{line_number}: record must be a JSON object")
        if record.get("schema") != QBENCH_SCHEMA:
            raise ComparisonError(
                f"{source}:{line_number}: expected schema {QBENCH_SCHEMA!r}"
            )
        name = record.get("name")
        if not isinstance(name, str) or not name:
            raise ComparisonError(
                f"{source}:{line_number}: field 'name' must be a non-empty string"
            )
        if name in records:
            raise ComparisonError(f"{source}:{line_number}: duplicate workload {name!r}")
        _unsigned(record, "iterations", source, line_number)
        _unsigned(record, "repeat", source, line_number)
        if not isinstance(record.get("expected"), str):
            raise ComparisonError(
                f"{source}:{line_number}: field 'expected' must be a string"
            )
        for _, median_field, mad_field in PHASES:
            _unsigned(record, median_field, source, line_number)
            _unsigned(record, mad_field, source, line_number)
        records[name] = record
    if not records:
        raise ComparisonError(f"{source}: no qbench records")
    return records


def compare_records(
    baseline: dict[str, dict[str, Any]],
    candidate: dict[str, dict[str, Any]],
    relative_floor: float,
    mad_multiplier: float,
    absolute_floor_ns: int,
) -> list[Comparison]:
    if relative_floor < 0 or mad_multiplier < 0 or absolute_floor_ns < 0:
        raise ComparisonError("comparison thresholds must be non-negative")
    baseline_names = set(baseline)
    candidate_names = set(candidate)
    if baseline_names != candidate_names:
        missing = sorted(baseline_names - candidate_names)
        added = sorted(candidate_names - baseline_names)
        raise ComparisonError(
            f"workload sets differ; missing={missing or 'none'}, added={added or 'none'}"
        )

    comparisons: list[Comparison] = []
    for name in baseline:
        before = baseline[name]
        after = candidate[name]
        for contract_field in ("iterations", "repeat", "expected"):
            if before[contract_field] != after[contract_field]:
                raise ComparisonError(
                    f"workload {name!r} changes {contract_field}: "
                    f"{before[contract_field]!r} -> {after[contract_field]!r}"
                )
        for phase, median_field, mad_field in PHASES:
            baseline_ns = before[median_field]
            baseline_mad_ns = before[mad_field]
            candidate_ns = after[median_field]
            candidate_mad_ns = after[mad_field]
            allowance_ns = max(
                baseline_ns * relative_floor,
                mad_multiplier * (baseline_mad_ns + candidate_mad_ns),
                absolute_floor_ns,
            )
            limit_ns = baseline_ns + allowance_ns
            comparisons.append(
                Comparison(
                    workload=name,
                    phase=phase,
                    baseline_ns=baseline_ns,
                    baseline_mad_ns=baseline_mad_ns,
                    candidate_ns=candidate_ns,
                    candidate_mad_ns=candidate_mad_ns,
                    allowance_ns=allowance_ns,
                    limit_ns=limit_ns,
                    alert=candidate_ns > limit_ns,
                )
            )
    return comparisons


def _milliseconds(value: float) -> str:
    return f"{value / 1_000_000:.3f}"


def _percentage(value: float | None) -> str:
    return "n/a" if value is None else f"{value * 100:+.2f}%"


def markdown_report(
    comparisons: Iterable[Comparison],
    baseline_ref: str,
    candidate_ref: str,
    relative_floor: float,
    mad_multiplier: float,
    absolute_floor_ns: int,
) -> str:
    comparisons = list(comparisons)
    alerts = [comparison for comparison in comparisons if comparison.alert]
    workload_count = len({comparison.workload for comparison in comparisons})
    lines = [
        "## Non-blocking qbench comparison",
        "",
        f"Base `{baseline_ref}` vs candidate `{candidate_ref}` on the same runner.",
        f"Policy: alert above `{relative_floor * 100:.1f}%`, "
        f"`{mad_multiplier:g} × (base MAD + candidate MAD)`, and "
        f"`{_milliseconds(absolute_floor_ns)} ms`; alerts do not fail the PR.",
        "",
        f"Compared {len(comparisons)} phase medians across {workload_count} workloads; "
        f"**{len(alerts)} alert(s)**.",
        "",
    ]
    if alerts:
        lines.extend(
            [
                "| Workload | Phase | Base ms (MAD) | Candidate ms (MAD) | Delta | Allowance ms |",
                "|---|---|---:|---:|---:|---:|",
            ]
        )
        for item in alerts:
            lines.append(
                f"| `{item.workload}` | {item.phase} | "
                f"{_milliseconds(item.baseline_ns)} ({_milliseconds(item.baseline_mad_ns)}) | "
                f"{_milliseconds(item.candidate_ns)} ({_milliseconds(item.candidate_mad_ns)}) | "
                f"**{_percentage(item.delta_ratio)}** | {_milliseconds(item.allowance_ns)} |"
            )
    else:
        lines.extend(["No phase exceeded the review-alert policy.", ""])

    lines.extend(
        [
            "",
            "<details>",
            "<summary>All phase comparisons</summary>",
            "",
            "| Workload | Phase | Base ms (MAD) | Candidate ms (MAD) | Delta | Status |",
            "|---|---|---:|---:|---:|---|",
        ]
    )
    for item in comparisons:
        status = "⚠️ alert" if item.alert else "ok"
        lines.append(
            f"| `{item.workload}` | {item.phase} | "
            f"{_milliseconds(item.baseline_ns)} ({_milliseconds(item.baseline_mad_ns)}) | "
            f"{_milliseconds(item.candidate_ns)} ({_milliseconds(item.candidate_mad_ns)}) | "
            f"{_percentage(item.delta_ratio)} | {status} |"
        )
    lines.extend(["", "</details>", ""])
    return "\n".join(lines)


def _rustc_version() -> str:
    try:
        result = subprocess.run(
            ["rustc", "-Vv"],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        return f"unavailable: {error}"
    return result.stdout.strip()


def build_metadata(args: argparse.Namespace) -> dict[str, Any]:
    environment_names = (
        "GITHUB_RUN_ID",
        "GITHUB_RUN_ATTEMPT",
        "GITHUB_WORKFLOW",
        "RUNNER_OS",
        "RUNNER_ARCH",
        "RUNNER_NAME",
    )
    return {
        "schema": METADATA_SCHEMA,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "baseline_ref": args.baseline_ref,
        "candidate_ref": args.candidate_ref,
        "benchmark_command": args.benchmark_command,
        "platform": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
            "python": platform.python_version(),
            "rustc": _rustc_version(),
        },
        "runner": {name: os.environ[name] for name in environment_names if name in os.environ},
    }


def _write(path: Path, contents: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(contents, encoding="utf-8")


def _annotation_escape(value: str) -> str:
    return value.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--summary", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--metadata", required=True, type=Path)
    parser.add_argument("--baseline-ref", default="unknown")
    parser.add_argument("--candidate-ref", default="unknown")
    parser.add_argument("--relative-floor", type=float, default=0.05)
    parser.add_argument("--mad-multiplier", type=float, default=3.0)
    parser.add_argument("--absolute-floor-ns", type=int, default=100_000)
    parser.add_argument("--benchmark-command", default="unspecified")
    parser.add_argument("--github-annotations", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        baseline = read_records(args.baseline)
        candidate = read_records(args.candidate)
        comparisons = compare_records(
            baseline,
            candidate,
            args.relative_floor,
            args.mad_multiplier,
            args.absolute_floor_ns,
        )
        metadata = build_metadata(args)
        summary = markdown_report(
            comparisons,
            args.baseline_ref,
            args.candidate_ref,
            args.relative_floor,
            args.mad_multiplier,
            args.absolute_floor_ns,
        )
        report = {
            "schema": REPORT_SCHEMA,
            "metadata": metadata,
            "policy": {
                "relative_floor": args.relative_floor,
                "mad_multiplier": args.mad_multiplier,
                "absolute_floor_ns": args.absolute_floor_ns,
                "blocking": False,
            },
            "alerts": sum(comparison.alert for comparison in comparisons),
            "comparisons": [comparison.as_json() for comparison in comparisons],
        }
        _write(args.summary, summary)
        _write(args.report, json.dumps(report, ensure_ascii=False, indent=2) + "\n")
        _write(args.metadata, json.dumps(metadata, ensure_ascii=False, indent=2) + "\n")
        if args.github_annotations:
            for comparison in comparisons:
                if comparison.alert:
                    message = (
                        f"{comparison.workload} {comparison.phase} regressed "
                        f"{_percentage(comparison.delta_ratio)}; "
                        f"candidate {_milliseconds(comparison.candidate_ns)} ms exceeds "
                        f"review limit {_milliseconds(comparison.limit_ns)} ms"
                    )
                    print(
                        f"::warning title=qbench performance alert::{_annotation_escape(message)}"
                    )
        return 0
    except ComparisonError as error:
        print(f"qbench comparison failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
