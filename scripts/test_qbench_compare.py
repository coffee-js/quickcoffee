#!/usr/bin/env python3

import json
import math
import tempfile
import unittest
from argparse import Namespace
from pathlib import Path

from qbench_compare import (
    ComparisonError,
    build_metadata,
    common_mode_summaries,
    compare_paired_records,
    compare_records,
    main,
    markdown_report,
    paired_markdown_report,
    read_ordered_records,
    read_records,
)


def record(name: str, execute: int = 1_000, execute_mad: int = 10) -> dict:
    return {
        "schema": "quickcoffee.qbench.v1",
        "version": "0.1.0",
        "name": name,
        "iterations": 100,
        "repeat": 11,
        "expected": "42",
        "compile_ns": 100,
        "compile_mad_ns": 1,
        "prepare_ns": 200,
        "prepare_mad_ns": 2,
        "verify_ns": 50,
        "verify_mad_ns": 1,
        "execute_ns": execute,
        "execute_mad_ns": execute_mad,
    }


def ordered(
    pair_id: str, sequence: int, side: str, item: dict
) -> dict:
    return {
        "schema": "quickcoffee.qbench-ordered.v1",
        "pair_id": pair_id,
        "sequence": sequence,
        "side": side,
        "record": item,
    }


class QbenchCompareTests(unittest.TestCase):
    def write_records(self, directory: Path, name: str, records: list[dict]) -> Path:
        path = directory / name
        path.write_text("".join(json.dumps(item) + "\n" for item in records), encoding="utf-8")
        return path

    def test_reader_rejects_duplicates_and_invalid_unsigned_fields(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            duplicate = self.write_records(
                directory, "duplicate.jsonl", [record("same"), record("same")]
            )
            with self.assertRaisesRegex(ComparisonError, "duplicate workload"):
                read_records(duplicate)

            invalid = record("invalid")
            invalid["execute_ns"] = -1
            invalid_path = self.write_records(directory, "invalid.jsonl", [invalid])
            with self.assertRaisesRegex(ComparisonError, "unsigned integer"):
                read_records(invalid_path)

    def test_reader_requires_version_and_positive_run_counts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            missing_version = record("missing-version")
            del missing_version["version"]
            missing_path = self.write_records(
                directory, "missing-version.jsonl", [missing_version]
            )
            with self.assertRaisesRegex(ComparisonError, "field 'version'"):
                read_records(missing_path)

            zero_repeat = record("zero-repeat")
            zero_repeat["repeat"] = 0
            zero_path = self.write_records(directory, "zero.jsonl", [zero_repeat])
            with self.assertRaisesRegex(ComparisonError, "positive integer"):
                read_records(zero_path)

    def test_workload_and_run_contracts_must_match(self) -> None:
        baseline = {"one": record("one")}
        with self.assertRaisesRegex(ComparisonError, "workload sets differ"):
            compare_records(baseline, {"two": record("two")}, 0.05, 3.0, 0)

        candidate = record("one")
        candidate["repeat"] = 3
        with self.assertRaisesRegex(ComparisonError, "changes repeat"):
            compare_records(baseline, {"one": candidate}, 0.05, 3.0, 0)

    def test_relative_floor_alerts_and_improvements_do_not(self) -> None:
        baseline = {"hot": record("hot", execute=1_000, execute_mad=10)}
        regression = {"hot": record("hot", execute=1_060, execute_mad=5)}
        comparisons = compare_records(baseline, regression, 0.05, 3.0, 0)
        execute = next(item for item in comparisons if item.phase == "execute")
        self.assertTrue(execute.alert)
        self.assertEqual(execute.allowance_ns, 50)

        improvement = {"hot": record("hot", execute=800, execute_mad=5)}
        comparisons = compare_records(baseline, improvement, 0.05, 3.0, 0)
        self.assertFalse(any(item.alert for item in comparisons))

    def test_combined_mad_suppresses_noisy_relative_change(self) -> None:
        baseline = {"hot": record("hot", execute=1_000, execute_mad=20)}
        candidate = {"hot": record("hot", execute=1_100, execute_mad=20)}
        execute = next(
            item
            for item in compare_records(baseline, candidate, 0.05, 3.0, 0)
            if item.phase == "execute"
        )
        self.assertEqual(execute.allowance_ns, 120)
        self.assertFalse(execute.alert)

    def test_markdown_contains_alert_and_complete_details(self) -> None:
        baseline = {"hot": record("hot", execute=1_000, execute_mad=0)}
        candidate = {"hot": record("hot", execute=1_100, execute_mad=0)}
        comparisons = compare_records(baseline, candidate, 0.05, 3.0, 0)
        report = markdown_report(comparisons, "base", "head", 0.05, 3.0, 0)
        self.assertIn("**1 alert(s)**", report)
        self.assertIn("`hot` | execute", report)
        self.assertIn("<summary>All phase comparisons</summary>", report)

    def test_absolute_floor_suppresses_tiny_phase_drift(self) -> None:
        baseline = {"tiny": record("tiny", execute=200_000, execute_mad=500)}
        candidate = {"tiny": record("tiny", execute=280_000, execute_mad=500)}
        execute = next(
            item
            for item in compare_records(baseline, candidate, 0.05, 3.0, 100_000)
            if item.phase == "execute"
        )
        self.assertEqual(execute.allowance_ns, 100_000)
        self.assertFalse(execute.alert)

    def test_non_finite_thresholds_are_rejected(self) -> None:
        baseline = {"hot": record("hot")}
        for relative_floor, mad_multiplier in (
            (math.nan, 3.0),
            (math.inf, 3.0),
            (0.05, math.nan),
            (0.05, math.inf),
        ):
            with self.subTest(
                relative_floor=relative_floor, mad_multiplier=mad_multiplier
            ):
                with self.assertRaisesRegex(ComparisonError, "non-negative"):
                    compare_records(
                        baseline,
                        baseline,
                        relative_floor,
                        mad_multiplier,
                        100_000,
                    )

    def test_metadata_is_versioned_and_records_refs(self) -> None:
        metadata = build_metadata(
            Namespace(
                baseline_ref="base-sha",
                candidate_ref="head-sha",
                benchmark_command="qbench --json",
            )
        )
        self.assertEqual(metadata["schema"], "quickcoffee.qbench-run-metadata.v1")
        self.assertEqual(metadata["baseline_ref"], "base-sha")
        self.assertIn("rustc", metadata["platform"])

    def test_ordered_reader_requires_complete_ab_and_ba_pairs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            valid = self.write_records(
                directory,
                "ordered.jsonl",
                [
                    ordered("hot-0", 0, "baseline", record("hot")),
                    ordered("hot-0", 1, "candidate", record("hot")),
                    ordered("hot-1", 2, "candidate", record("hot")),
                    ordered("hot-1", 3, "baseline", record("hot")),
                ],
            )
            runs = read_ordered_records(valid)
            self.assertEqual([run.sequence for run in runs["hot"]], [0, 1, 2, 3])

            invalid = self.write_records(
                directory,
                "invalid-ordered.jsonl",
                [
                    ordered("hot-0", 0, "baseline", record("hot")),
                    ordered("hot-0", 1, "candidate", record("hot")),
                    ordered("hot-1", 2, "baseline", record("hot")),
                    ordered("hot-1", 3, "candidate", record("hot")),
                ],
            )
            with self.assertRaisesRegex(ComparisonError, "one AB and one BA"):
                read_ordered_records(invalid)

    def test_paired_effect_cancels_order_bias_and_reports_common_mode(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            source = self.write_records(
                Path(temporary),
                "ordered.jsonl",
                [
                    ordered("hot-0", 0, "baseline", record("hot", execute=1_000)),
                    ordered("hot-0", 1, "candidate", record("hot", execute=1_100)),
                    ordered("hot-1", 2, "candidate", record("hot", execute=900)),
                    ordered("hot-1", 3, "baseline", record("hot", execute=1_000)),
                ],
            )
            comparisons = compare_paired_records(
                read_ordered_records(source), 0.05, 3.0, 0
            )
            execute = next(item for item in comparisons if item.phase == "execute")
            self.assertEqual(execute.paired_delta_ns, 0)
            self.assertEqual(execute.paired_mad_ns, 100)
            self.assertFalse(execute.alert)
            common = common_mode_summaries(comparisons)["execute"]
            self.assertAlmostEqual(common["ab_median_delta_ratio"], 0.1)
            self.assertAlmostEqual(common["ba_median_delta_ratio"], -0.1)
            self.assertAlmostEqual(common["paired_median_delta_ratio"], 0)

    def test_swapping_aa_labels_preserves_paired_decision(self) -> None:
        def compare(swapped: bool):
            def side(value: str) -> str:
                if not swapped:
                    return value
                return "candidate" if value == "baseline" else "baseline"

            with tempfile.TemporaryDirectory() as temporary:
                source = self.write_records(
                    Path(temporary),
                    "ordered.jsonl",
                    [
                        ordered(
                            "hot-0", 0, side("baseline"), record("hot", execute=1_000)
                        ),
                        ordered(
                            "hot-0", 1, side("candidate"), record("hot", execute=1_100)
                        ),
                        ordered(
                            "hot-1", 2, side("candidate"), record("hot", execute=900)
                        ),
                        ordered(
                            "hot-1", 3, side("baseline"), record("hot", execute=1_000)
                        ),
                    ],
                )
                comparisons = compare_paired_records(
                    read_ordered_records(source), 0.05, 3.0, 0
                )
                execute = next(item for item in comparisons if item.phase == "execute")
                return execute, common_mode_summaries(comparisons)["execute"]

        original, original_common = compare(False)
        swapped, swapped_common = compare(True)
        self.assertFalse(original.alert)
        self.assertFalse(swapped.alert)
        self.assertEqual(original.paired_delta_ns, 0)
        self.assertEqual(swapped.paired_delta_ns, 0)
        self.assertAlmostEqual(original_common["paired_median_delta_ratio"], 0)
        self.assertAlmostEqual(swapped_common["paired_median_delta_ratio"], 0)

    def test_paired_alert_requires_both_order_directions(self) -> None:
        def compare(second_candidate: int):
            with tempfile.TemporaryDirectory() as temporary:
                source = self.write_records(
                    Path(temporary),
                    "ordered.jsonl",
                    [
                        ordered("hot-0", 0, "baseline", record("hot", execute=1_000)),
                        ordered("hot-0", 1, "candidate", record("hot", execute=1_100)),
                        ordered(
                            "hot-1",
                            2,
                            "candidate",
                            record("hot", execute=second_candidate),
                        ),
                        ordered("hot-1", 3, "baseline", record("hot", execute=1_000)),
                    ],
                )
                return next(
                    item
                    for item in compare_paired_records(
                        read_ordered_records(source), 0.05, 3.0, 0
                    )
                    if item.phase == "execute"
                )

        confirmed = compare(1_100)
        self.assertTrue(confirmed.alert)
        self.assertEqual(confirmed.confirmed_pairs, 2)

        directional = compare(1_040)
        self.assertGreater(directional.delta_ratio, 0.05)
        self.assertEqual(directional.confirmed_pairs, 1)
        self.assertFalse(directional.alert)

    def test_paired_markdown_explains_non_subtracted_common_mode(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            source = self.write_records(
                Path(temporary),
                "ordered.jsonl",
                [
                    ordered("hot-0", 0, "baseline", record("hot", execute=1_000)),
                    ordered("hot-0", 1, "candidate", record("hot", execute=1_100)),
                    ordered("hot-1", 2, "candidate", record("hot", execute=1_100)),
                    ordered("hot-1", 3, "baseline", record("hot", execute=1_000)),
                ],
            )
            comparisons = compare_paired_records(
                read_ordered_records(source), 0.05, 3.0, 0
            )
            report = paired_markdown_report(
                comparisons, "base", "head", 0.05, 3.0, 0
            )
            self.assertIn("Non-blocking paired qbench comparison", report)
            self.assertIn("Common-mode values are diagnostic only", report)
            self.assertIn("**1 alert(s)**", report)

    def test_paired_cli_writes_versioned_report_and_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            source = self.write_records(
                directory,
                "ordered.jsonl",
                [
                    ordered("hot-0", 0, "baseline", record("hot", execute=1_000)),
                    ordered("hot-0", 1, "candidate", record("hot", execute=1_100)),
                    ordered("hot-1", 2, "candidate", record("hot", execute=1_100)),
                    ordered("hot-1", 3, "baseline", record("hot", execute=1_000)),
                ],
            )
            summary = directory / "summary.md"
            report = directory / "report.json"
            metadata = directory / "metadata.json"
            self.assertEqual(
                main(
                    [
                        "--ordered",
                        str(source),
                        "--summary",
                        str(summary),
                        "--report",
                        str(report),
                        "--metadata",
                        str(metadata),
                        "--absolute-floor-ns",
                        "0",
                    ]
                ),
                0,
            )
            payload = json.loads(report.read_text(encoding="utf-8"))
            self.assertEqual(payload["schema"], "quickcoffee.qbench-paired-compare.v1")
            self.assertEqual(payload["policy"]["required_alerting_pairs"], 2)
            self.assertFalse(payload["policy"]["common_mode_subtracted"])
            self.assertEqual(payload["alerts"], 1)
            self.assertIn("execute", payload["common_mode"])


if __name__ == "__main__":
    unittest.main()
