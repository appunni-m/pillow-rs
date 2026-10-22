"""Regression contracts for Rust size ratchets and baseline ownership."""

from __future__ import annotations

import unittest

from check_rust_file_sizes import (
    file_ratchet_errors,
    legacy_owner_errors,
    reconcile_legacy_owners,
)
from run_clippy_quality import function_ratchet_errors, stale_function_baseline_errors


def function_metric(path: str, lines: int) -> dict[str, object]:
    return {
        "path": path,
        "signature": "fn process() {}",
        "occurrence": 1,
        "lines": lines,
        "line_start": 10,
    }


class FileRatchetTests(unittest.TestCase):
    def test_new_over_limit_file_fails(self) -> None:
        errors = file_ratchet_errors({"src/new.rs": 1_001}, {})

        self.assertTrue(any("new over-limit file" in error for error in errors))

    def test_file_growth_beyond_baseline_fails(self) -> None:
        errors = file_ratchet_errors(
            {"src/legacy.rs": 1_002}, {"src/legacy.rs": 1_001}
        )

        self.assertTrue(any("file grew" in error for error in errors))

    def test_legacy_file_at_or_below_baseline_passes(self) -> None:
        errors = file_ratchet_errors(
            {"src/legacy.rs": 1_001}, {"src/legacy.rs": 1_002}
        )

        self.assertEqual(errors, [])

    def test_reduced_file_requires_baseline_retirement(self) -> None:
        errors = file_ratchet_errors(
            {"src/legacy.rs": 1_000}, {"src/legacy.rs": 1_002}
        )

        self.assertTrue(any("stale baseline entry" in error for error in errors))


class FunctionRatchetTests(unittest.TestCase):
    def test_new_over_limit_function_fails(self) -> None:
        errors, legacy_count = function_ratchet_errors(
            [function_metric("src/new.rs", 101)], {}
        )

        self.assertTrue(any("new over-limit function" in error for error in errors))
        self.assertEqual(legacy_count, 0)

    def test_function_growth_beyond_baseline_fails(self) -> None:
        metric = function_metric("src/legacy.rs", 102)
        baseline = {("src/legacy.rs", "fn process() {}", 1): 101}
        owners = {
            "src/legacy.rs": {
                "owner": "lazytrot",
                "reduction_task": "pillow-rs#abcd",
                "reason": "Tracked reduction",
            }
        }

        errors, legacy_count = function_ratchet_errors([metric], baseline, owners)

        self.assertTrue(any("function grew" in error for error in errors))
        self.assertTrue(any("limit 100" in error for error in errors))
        self.assertTrue(any("owner lazytrot" in error for error in errors))
        self.assertTrue(any("task pillow-rs#abcd" in error for error in errors))
        self.assertEqual(legacy_count, 0)

    def test_legacy_function_at_or_below_baseline_passes(self) -> None:
        metric = function_metric("src/legacy.rs", 101)
        baseline = {("src/legacy.rs", "fn process() {}", 1): 102}

        errors, legacy_count = function_ratchet_errors([metric], baseline)

        self.assertEqual(errors, [])
        self.assertEqual(legacy_count, 1)

    def test_reduced_function_requires_baseline_retirement(self) -> None:
        baseline = {("src/legacy.rs", "fn process() {}", 1): 101}

        errors = stale_function_baseline_errors([], baseline)

        self.assertTrue(any("stale function baseline entry" in error for error in errors))


class OwnershipMapTests(unittest.TestCase):
    def test_missing_owner_for_active_path_fails(self) -> None:
        errors = legacy_owner_errors({}, {"pillow-rs/src/lib.rs"})

        self.assertTrue(any("missing active paths" in error for error in errors))

    def test_retired_owner_path_fails_until_baseline_update(self) -> None:
        errors = legacy_owner_errors(
            {"pillow-rs/src/old.rs": {}}, {"pillow-rs/src/current.rs"}
        )

        self.assertTrue(any("contains retired paths" in error for error in errors))

    def test_owner_reason_and_qualified_task_are_required(self) -> None:
        errors = legacy_owner_errors(
            {
                "pillow-rs/src/lib.rs": {
                    "owner": "lazytrot",
                    "reduction_task": "wrgw",
                    "reason": "",
                }
            },
            {"pillow-rs/src/lib.rs"},
        )

        self.assertTrue(any("qualified Kata reduction_task" in error for error in errors))
        self.assertTrue(any("nonempty reason" in error for error in errors))

    def test_qualified_task_requires_repository_and_issue_id(self) -> None:
        for task in ("#abcd", "pillow-rs#", "pillow-rs#abcd#extra", "pillow-rs#abcd task"):
            with self.subTest(task=task):
                errors = legacy_owner_errors(
                    {
                        "pillow-rs/src/lib.rs": {
                            "owner": "lazytrot",
                            "reduction_task": task,
                            "reason": "Tracked reduction",
                        }
                    },
                    {"pillow-rs/src/lib.rs"},
                )

                self.assertTrue(
                    any("qualified Kata reduction_task" in error for error in errors)
                )

    def test_baseline_update_prunes_retired_owner_paths(self) -> None:
        baseline: dict[str, object] = {
            "legacy_owners": {
                "pillow-rs/src/current.rs": {
                    "owner": "lazytrot",
                    "reduction_task": "pillow-rs#abcd",
                    "reason": "Tracked reduction",
                },
                "pillow-rs/src/retired.rs": {
                    "owner": "lazytrot",
                    "reduction_task": "pillow-rs#efgh",
                    "reason": "Tracked reduction",
                },
            }
        }

        errors, retired_count = reconcile_legacy_owners(
            baseline, {"pillow-rs/src/current.rs"}
        )

        self.assertEqual(errors, [])
        self.assertEqual(retired_count, 1)
        self.assertEqual(set(baseline["legacy_owners"]), {"pillow-rs/src/current.rs"})


if __name__ == "__main__":
    unittest.main()
