#!/usr/bin/env python3
"""Compare legacy or ordered paired qbench runs without blocking on regressions."""

from __future__ import annotations

import argparse
import json
import math
import os
import platform
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from statistics import median
from typing import Any, Iterable

QBENCH_SCHEMA = "quickcoffee.qbench.v1"
REPORT_SCHEMA = "quickcoffee.qbench-compare.v1"
ORDERED_SCHEMA = "quickcoffee.qbench-ordered.v1"
PAIRED_REPORT_SCHEMA = "quickcoffee.qbench-paired-compare.v1"
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


@dataclass(frozen=True)
class OrderedRun:
    pair_id: str
    sequence: int
    side: str
    record: dict[str, Any]


@dataclass(frozen=True)
class PairDelta:
    pair_id: str
    order: str
    baseline_ns: int
    baseline_mad_ns: int
    candidate_ns: int
    candidate_mad_ns: int
    allowance_ns: float

    @property
    def delta_ns(self) -> int:
        return self.candidate_ns - self.baseline_ns

    @property
    def delta_ratio(self) -> float | None:
        if self.baseline_ns == 0:
            return None
        return self.delta_ns / self.baseline_ns

    @property
    def alert(self) -> bool:
        return self.delta_ns > self.allowance_ns

    def as_json(self) -> dict[str, Any]:
        return {
            "pair_id": self.pair_id,
            "order": self.order,
            "baseline_ns": self.baseline_ns,
            "baseline_mad_ns": self.baseline_mad_ns,
            "candidate_ns": self.candidate_ns,
            "candidate_mad_ns": self.candidate_mad_ns,
            "delta_ns": self.delta_ns,
            "delta_ratio": self.delta_ratio,
            "allowance_ns": self.allowance_ns,
            "alert": self.alert,
        }


@dataclass(frozen=True)
class PairedComparison:
    workload: str
    phase: str
    baseline_ns: float
    baseline_mad_ns: float
    candidate_ns: float
    candidate_mad_ns: float
    paired_delta_ns: float
    paired_mad_ns: float
    allowance_ns: float
    alert: bool
    pairs: tuple[PairDelta, ...]

    @property
    def raw_delta_ratio(self) -> float | None:
        if self.baseline_ns == 0:
            return None
        return (self.candidate_ns - self.baseline_ns) / self.baseline_ns

    @property
    def delta_ratio(self) -> float | None:
        if self.baseline_ns == 0:
            return None
        return self.paired_delta_ns / self.baseline_ns

    @property
    def confirmed_pairs(self) -> int:
        return sum(pair.alert for pair in self.pairs)

    def as_json(self) -> dict[str, Any]:
        return {
            "workload": self.workload,
            "phase": self.phase,
            "baseline_ns": self.baseline_ns,
            "baseline_mad_ns": self.baseline_mad_ns,
            "candidate_ns": self.candidate_ns,
            "candidate_mad_ns": self.candidate_mad_ns,
            "raw_delta_ratio": self.raw_delta_ratio,
            "paired_delta_ns": self.paired_delta_ns,
            "paired_delta_ratio": self.delta_ratio,
            "paired_mad_ns": self.paired_mad_ns,
            "allowance_ns": self.allowance_ns,
            "alert": self.alert,
            "confirmed_pairs": self.confirmed_pairs,
            "pairs": [pair.as_json() for pair in self.pairs],
        }


def _unsigned(record: dict[str, Any], field: str, source: Path, line: int) -> int:
    value = record.get(field)
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ComparisonError(
            f"{source}:{line}: field {field!r} must be an unsigned integer"
        )
    return value


def _positive(record: dict[str, Any], field: str, source: Path, line: int) -> int:
    value = _unsigned(record, field, source, line)
    if value == 0:
        raise ComparisonError(
            f"{source}:{line}: field {field!r} must be a positive integer"
        )
    return value


def _read_json_lines(source: Path) -> list[tuple[int, Any]]:
    try:
        lines = source.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise ComparisonError(f"cannot read {source}: {error}") from error
    records = []
    for line_number, raw in enumerate(lines, 1):
        if not raw.strip():
            continue
        try:
            records.append((line_number, json.loads(raw)))
        except json.JSONDecodeError as error:
            raise ComparisonError(f"{source}:{line_number}: invalid JSON: {error}") from error
    return records


def _validate_qbench_record(
    record: Any, source: Path, line_number: int
) -> tuple[str, dict[str, Any]]:
    if not isinstance(record, dict):
        raise ComparisonError(f"{source}:{line_number}: record must be a JSON object")
    if record.get("schema") != QBENCH_SCHEMA:
        raise ComparisonError(f"{source}:{line_number}: expected schema {QBENCH_SCHEMA!r}")
    version = record.get("version")
    if not isinstance(version, str) or not version:
        raise ComparisonError(
            f"{source}:{line_number}: field 'version' must be a non-empty string"
        )
    name = record.get("name")
    if not isinstance(name, str) or not name:
        raise ComparisonError(
            f"{source}:{line_number}: field 'name' must be a non-empty string"
        )
    _positive(record, "iterations", source, line_number)
    _positive(record, "repeat", source, line_number)
    if not isinstance(record.get("expected"), str):
        raise ComparisonError(
            f"{source}:{line_number}: field 'expected' must be a string"
        )
    for _, median_field, mad_field in PHASES:
        _unsigned(record, median_field, source, line_number)
        _unsigned(record, mad_field, source, line_number)
    return name, record


def read_records(source: Path) -> dict[str, dict[str, Any]]:
    records: dict[str, dict[str, Any]] = {}
    for line_number, raw_record in _read_json_lines(source):
        name, record = _validate_qbench_record(raw_record, source, line_number)
        if name in records:
            raise ComparisonError(f"{source}:{line_number}: duplicate workload {name!r}")
        records[name] = record
    if not records:
        raise ComparisonError(f"{source}: no qbench records")
    return records


def read_ordered_records(source: Path) -> dict[str, list[OrderedRun]]:
    workloads: dict[str, list[OrderedRun]] = {}
    seen_sequences: set[int] = set()
    pair_workloads: dict[str, str] = {}
    for line_number, wrapper in _read_json_lines(source):
        if not isinstance(wrapper, dict):
            raise ComparisonError(f"{source}:{line_number}: record must be a JSON object")
        if wrapper.get("schema") != ORDERED_SCHEMA:
            raise ComparisonError(
                f"{source}:{line_number}: expected schema {ORDERED_SCHEMA!r}"
            )
        pair_id = wrapper.get("pair_id")
        if not isinstance(pair_id, str) or not pair_id:
            raise ComparisonError(
                f"{source}:{line_number}: field 'pair_id' must be a non-empty string"
            )
        side = wrapper.get("side")
        if side not in ("baseline", "candidate"):
            raise ComparisonError(
                f"{source}:{line_number}: field 'side' must be 'baseline' or 'candidate'"
            )
        sequence = _unsigned(wrapper, "sequence", source, line_number)
        if sequence in seen_sequences:
            raise ComparisonError(f"{source}:{line_number}: duplicate sequence {sequence}")
        seen_sequences.add(sequence)
        name, record = _validate_qbench_record(wrapper.get("record"), source, line_number)
        previous_workload = pair_workloads.setdefault(pair_id, name)
        if previous_workload != name:
            raise ComparisonError(
                f"{source}:{line_number}: pair {pair_id!r} spans multiple workloads"
            )
        workloads.setdefault(name, []).append(
            OrderedRun(pair_id=pair_id, sequence=sequence, side=side, record=record)
        )

    if not workloads:
        raise ComparisonError(f"{source}: no ordered qbench records")
    if seen_sequences != set(range(len(seen_sequences))):
        raise ComparisonError(f"{source}: sequence values must be contiguous from zero")

    for name, runs in workloads.items():
        pairs: dict[str, list[OrderedRun]] = {}
        first = runs[0].record
        for run in runs:
            for contract_field in ("iterations", "repeat", "expected"):
                if run.record[contract_field] != first[contract_field]:
                    raise ComparisonError(
                        f"workload {name!r} changes {contract_field} within ordered runs"
                    )
            pairs.setdefault(run.pair_id, []).append(run)
        if len(pairs) != 2:
            raise ComparisonError(
                f"workload {name!r} must contain exactly two AB/BA pairs"
            )
        directions = set()
        for pair_id, pair_runs in pairs.items():
            pair_runs.sort(key=lambda run: run.sequence)
            sides = [run.side for run in pair_runs]
            if len(pair_runs) != 2 or set(sides) != {"baseline", "candidate"}:
                raise ComparisonError(
                    f"pair {pair_id!r} must contain one baseline and one candidate run"
                )
            directions.add(tuple(sides))
        if directions != {
            ("baseline", "candidate"),
            ("candidate", "baseline"),
        }:
            raise ComparisonError(
                f"workload {name!r} must contain one AB and one BA pair"
            )
        runs.sort(key=lambda run: run.sequence)
    return workloads


def compare_records(
    baseline: dict[str, dict[str, Any]],
    candidate: dict[str, dict[str, Any]],
    relative_floor: float,
    mad_multiplier: float,
    absolute_floor_ns: int,
) -> list[Comparison]:
    if (
        not math.isfinite(relative_floor)
        or not math.isfinite(mad_multiplier)
        or relative_floor < 0
        or mad_multiplier < 0
        or absolute_floor_ns < 0
    ):
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


def compare_paired_records(
    workloads: dict[str, list[OrderedRun]],
    relative_floor: float,
    mad_multiplier: float,
    absolute_floor_ns: int,
) -> list[PairedComparison]:
    if (
        not math.isfinite(relative_floor)
        or not math.isfinite(mad_multiplier)
        or relative_floor < 0
        or mad_multiplier < 0
        or absolute_floor_ns < 0
    ):
        raise ComparisonError("comparison thresholds must be non-negative")

    comparisons = []
    for name, runs in workloads.items():
        grouped: dict[str, list[OrderedRun]] = {}
        for run in runs:
            grouped.setdefault(run.pair_id, []).append(run)
        for phase, median_field, mad_field in PHASES:
            baseline_values = [
                run.record[median_field] for run in runs if run.side == "baseline"
            ]
            candidate_values = [
                run.record[median_field] for run in runs if run.side == "candidate"
            ]
            baseline_mads = [
                run.record[mad_field] for run in runs if run.side == "baseline"
            ]
            candidate_mads = [
                run.record[mad_field] for run in runs if run.side == "candidate"
            ]
            pair_deltas = []
            for pair_id, pair_runs in grouped.items():
                pair_runs = sorted(pair_runs, key=lambda run: run.sequence)
                baseline = next(run for run in pair_runs if run.side == "baseline")
                candidate = next(run for run in pair_runs if run.side == "candidate")
                order = "".join(
                    "A" if run.side == "baseline" else "B" for run in pair_runs
                )
                pair_deltas.append(
                    PairDelta(
                        pair_id=pair_id,
                        order=order,
                        baseline_ns=baseline.record[median_field],
                        baseline_mad_ns=baseline.record[mad_field],
                        candidate_ns=candidate.record[median_field],
                        candidate_mad_ns=candidate.record[mad_field],
                        allowance_ns=max(
                            baseline.record[median_field] * relative_floor,
                            mad_multiplier
                            * (
                                baseline.record[mad_field]
                                + candidate.record[mad_field]
                            ),
                            absolute_floor_ns,
                        ),
                    )
                )

            baseline_ns = float(median(baseline_values))
            candidate_ns = float(median(candidate_values))
            baseline_mad_ns = float(median(baseline_mads))
            candidate_mad_ns = float(median(candidate_mads))
            paired_delta_ns = float(median(pair.delta_ns for pair in pair_deltas))
            paired_mad_ns = float(
                median(abs(pair.delta_ns - paired_delta_ns) for pair in pair_deltas)
            )
            allowance_ns = max(
                baseline_ns * relative_floor,
                mad_multiplier * (baseline_mad_ns + candidate_mad_ns),
                absolute_floor_ns,
            )
            comparisons.append(
                PairedComparison(
                    workload=name,
                    phase=phase,
                    baseline_ns=baseline_ns,
                    baseline_mad_ns=baseline_mad_ns,
                    candidate_ns=candidate_ns,
                    candidate_mad_ns=candidate_mad_ns,
                    paired_delta_ns=paired_delta_ns,
                    paired_mad_ns=paired_mad_ns,
                    allowance_ns=allowance_ns,
                    alert=paired_delta_ns > allowance_ns
                    and all(pair.alert for pair in pair_deltas),
                    pairs=tuple(pair_deltas),
                )
            )
    return comparisons


def common_mode_summaries(
    comparisons: Iterable[PairedComparison],
) -> dict[str, dict[str, Any]]:
    comparisons = list(comparisons)
    summaries = {}
    for phase in ("compile", "execute"):
        phase_items = [item for item in comparisons if item.phase == phase]
        paired_ratios = [
            item.delta_ratio for item in phase_items if item.delta_ratio is not None
        ]
        raw_ratios = [
            item.raw_delta_ratio for item in phase_items if item.raw_delta_ratio is not None
        ]
        order_ratios: dict[str, list[float]] = {"AB": [], "BA": []}
        for item in phase_items:
            for pair in item.pairs:
                if pair.delta_ratio is not None:
                    order_ratios[pair.order].append(pair.delta_ratio)
        def ratio_median(values: list[float]) -> float | None:
            return None if not values else float(median(values))

        summaries[phase] = {
            "workloads": len(phase_items),
            "raw_median_delta_ratio": ratio_median(raw_ratios),
            "paired_median_delta_ratio": ratio_median(paired_ratios),
            "ab_median_delta_ratio": ratio_median(order_ratios["AB"]),
            "ba_median_delta_ratio": ratio_median(order_ratios["BA"]),
            "paired_positive_workloads": sum(ratio > 0 for ratio in paired_ratios),
        }
    return summaries


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


def paired_markdown_report(
    comparisons: Iterable[PairedComparison],
    baseline_ref: str,
    candidate_ref: str,
    relative_floor: float,
    mad_multiplier: float,
    absolute_floor_ns: int,
) -> str:
    comparisons = list(comparisons)
    alerts = [comparison for comparison in comparisons if comparison.alert]
    workload_count = len({comparison.workload for comparison in comparisons})
    common_modes = common_mode_summaries(comparisons)
    lines = [
        "## Non-blocking paired qbench comparison",
        "",
        f"Base `{baseline_ref}` vs candidate `{candidate_ref}` on the same runner.",
        "Each workload uses one AB pair and one BA pair; warning decisions use the "
        "median paired effect without subtracting the run-wide common mode.",
        f"Policy: alert above `{relative_floor * 100:.1f}%`, "
        f"`{mad_multiplier:g} × (base MAD + candidate MAD)`, "
        f"and `{_milliseconds(absolute_floor_ns)} ms` in the aggregate and in both "
        "order directions; paired-delta MAD is reported diagnostically. Alerts do not fail the PR.",
        "",
        f"Compared {len(comparisons)} phase effects across {workload_count} workloads; "
        f"**{len(alerts)} alert(s)**.",
        "",
        "### Run-wide order/common-mode summary",
        "",
        "| Phase | AB median | BA median | Raw side median | Paired median | Paired positive |",
        "|---|---:|---:|---:|---:|---:|",
    ]
    for phase in ("compile", "execute"):
        summary = common_modes[phase]
        lines.append(
            f"| {phase} | {_percentage(summary['ab_median_delta_ratio'])} | "
            f"{_percentage(summary['ba_median_delta_ratio'])} | "
            f"{_percentage(summary['raw_median_delta_ratio'])} | "
            f"{_percentage(summary['paired_median_delta_ratio'])} | "
            f"{summary['paired_positive_workloads']}/{summary['workloads']} |"
        )
    lines.extend(["", "Common-mode values are diagnostic only; they are not subtracted.", ""])

    if alerts:
        lines.extend(
            [
                "| Workload | Phase | Base ms | Candidate ms | Raw delta | Paired delta (MAD) | Allowance ms |",
                "|---|---|---:|---:|---:|---:|---:|",
            ]
        )
        for item in alerts:
            lines.append(
                f"| `{item.workload}` | {item.phase} | "
                f"{_milliseconds(item.baseline_ns)} | "
                f"{_milliseconds(item.candidate_ns)} | "
                f"{_percentage(item.raw_delta_ratio)} | "
                f"**{_percentage(item.delta_ratio)}** "
                f"({_milliseconds(item.paired_mad_ns)}) | "
                f"{_milliseconds(item.allowance_ns)} |"
            )
    else:
        lines.extend(["No paired phase effect exceeded the review-alert policy.", ""])

    lines.extend(
        [
            "",
            "<details>",
            "<summary>All paired phase comparisons</summary>",
            "",
            "| Workload | Phase | Base ms | Candidate ms | Raw delta | Paired delta (MAD) | Status |",
            "|---|---|---:|---:|---:|---:|---|",
        ]
    )
    for item in comparisons:
        status = "⚠️ alert" if item.alert else "ok"
        lines.append(
            f"| `{item.workload}` | {item.phase} | "
            f"{_milliseconds(item.baseline_ns)} | "
            f"{_milliseconds(item.candidate_ns)} | "
            f"{_percentage(item.raw_delta_ratio)} | "
            f"{_percentage(item.delta_ratio)} ({_milliseconds(item.paired_mad_ns)}) | "
            f"{status} |"
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
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--ordered", type=Path)
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
        metadata = build_metadata(args)
        policy = {
            "relative_floor": args.relative_floor,
            "mad_multiplier": args.mad_multiplier,
            "absolute_floor_ns": args.absolute_floor_ns,
            "blocking": False,
        }
        if args.ordered is not None:
            if args.baseline is not None or args.candidate is not None:
                raise ComparisonError(
                    "--ordered cannot be combined with --baseline or --candidate"
                )
            ordered = read_ordered_records(args.ordered)
            paired_comparisons = compare_paired_records(
                ordered,
                args.relative_floor,
                args.mad_multiplier,
                args.absolute_floor_ns,
            )
            common_modes = common_mode_summaries(paired_comparisons)
            summary = paired_markdown_report(
                paired_comparisons,
                args.baseline_ref,
                args.candidate_ref,
                args.relative_floor,
                args.mad_multiplier,
                args.absolute_floor_ns,
            )
            policy.update(
                {
                    "paired": True,
                    "pairs_per_workload": 2,
                    "required_alerting_pairs": 2,
                    "paired_mad_diagnostic_only": True,
                    "common_mode_subtracted": False,
                }
            )
            report = {
                "schema": PAIRED_REPORT_SCHEMA,
                "metadata": metadata,
                "policy": policy,
                "common_mode": common_modes,
                "alerts": sum(item.alert for item in paired_comparisons),
                "comparisons": [item.as_json() for item in paired_comparisons],
            }
            annotations: Iterable[Comparison | PairedComparison] = paired_comparisons
        else:
            if args.baseline is None or args.candidate is None:
                raise ComparisonError(
                    "provide --ordered or both --baseline and --candidate"
                )
            baseline = read_records(args.baseline)
            candidate = read_records(args.candidate)
            comparisons = compare_records(
                baseline,
                candidate,
                args.relative_floor,
                args.mad_multiplier,
                args.absolute_floor_ns,
            )
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
                "policy": policy,
                "alerts": sum(comparison.alert for comparison in comparisons),
                "comparisons": [comparison.as_json() for comparison in comparisons],
            }
            annotations = comparisons
        _write(args.summary, summary)
        _write(
            args.report,
            json.dumps(report, ensure_ascii=False, indent=2, allow_nan=False) + "\n",
        )
        _write(args.metadata, json.dumps(metadata, ensure_ascii=False, indent=2) + "\n")
        if args.github_annotations:
            for comparison in annotations:
                if comparison.alert:
                    if isinstance(comparison, PairedComparison):
                        message = (
                            f"{comparison.workload} {comparison.phase} paired effect "
                            f"regressed {_percentage(comparison.delta_ratio)}; "
                            f"paired delta {_milliseconds(comparison.paired_delta_ns)} ms "
                            f"exceeds allowance {_milliseconds(comparison.allowance_ns)} ms"
                        )
                    else:
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
