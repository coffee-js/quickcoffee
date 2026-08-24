#!/usr/bin/env python3

import json
import tempfile
import unittest
from argparse import Namespace
from pathlib import Path

from qbench_compare import (
    ComparisonError,
    build_metadata,
    compare_records,
    markdown_report,
    read_records,
)


def record(name: str, execute: int = 1_000, execute_mad: int = 10) -> dict:
    return {
        "schema": "quickcoffee.qbench.v1",
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

    def test_workload_and_run_contracts_must_match(self) -> None:
        baseline = {"one": record("one")}
        with self.assertRaisesRegex(ComparisonError, "workload sets differ"):
            compare_records(baseline, {"two": record("two")}, 0.05, 3.0)

        candidate = record("one")
        candidate["repeat"] = 3
        with self.assertRaisesRegex(ComparisonError, "changes repeat"):
            compare_records(baseline, {"one": candidate}, 0.05, 3.0)

    def test_relative_floor_alerts_and_improvements_do_not(self) -> None:
        baseline = {"hot": record("hot", execute=1_000, execute_mad=10)}
        regression = {"hot": record("hot", execute=1_060, execute_mad=5)}
        comparisons = compare_records(baseline, regression, 0.05, 3.0)
        execute = next(item for item in comparisons if item.phase == "execute")
        self.assertTrue(execute.alert)
        self.assertEqual(execute.allowance_ns, 50)

        improvement = {"hot": record("hot", execute=800, execute_mad=5)}
        comparisons = compare_records(baseline, improvement, 0.05, 3.0)
        self.assertFalse(any(item.alert for item in comparisons))

    def test_combined_mad_suppresses_noisy_relative_change(self) -> None:
        baseline = {"hot": record("hot", execute=1_000, execute_mad=20)}
        candidate = {"hot": record("hot", execute=1_100, execute_mad=20)}
        execute = next(
            item
            for item in compare_records(baseline, candidate, 0.05, 3.0)
            if item.phase == "execute"
        )
        self.assertEqual(execute.allowance_ns, 120)
        self.assertFalse(execute.alert)

    def test_markdown_contains_alert_and_complete_details(self) -> None:
        baseline = {"hot": record("hot", execute=1_000, execute_mad=0)}
        candidate = {"hot": record("hot", execute=1_100, execute_mad=0)}
        comparisons = compare_records(baseline, candidate, 0.05, 3.0)
        report = markdown_report(comparisons, "base", "head", 0.05, 3.0)
        self.assertIn("**1 alert(s)**", report)
        self.assertIn("`hot` | execute", report)
        self.assertIn("<summary>All phase comparisons</summary>", report)

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


if __name__ == "__main__":
    unittest.main()
