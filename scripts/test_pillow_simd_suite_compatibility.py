"""Focused guards for raw Pillow-SIMD suite compatibility gating."""
from __future__ import annotations

import unittest

from run_pillow_simd_benchmark import gate_pillow_simd_suite_comparisons


class PillowSimdSuiteCompatibilityTests(unittest.TestCase):
    def test_nonpassing_timing_common_members_withhold_ratio(self) -> None:
        result = {
            "suites": [
                {
                    "members": [
                        {"workload_id": "compatible"},
                        {"workload_id": "incompatible"},
                        {"workload_id": "missing"},
                        {"workload_id": "already-excluded"},
                    ],
                    "comparisons": [
                        {
                            "baseline_subject": "pillow-simd",
                            "baseline_value": 10.0,
                            "subject_value": 5.0,
                            "ratio": 0.5,
                            "status": "comparable",
                            "excluded_members": [
                                {"workload_id": "already-excluded"}
                            ],
                        }
                    ],
                }
            ]
        }

        gate_pillow_simd_suite_comparisons(
            result,
            {
                "compatible": {"outcome": "pass"},
                "incompatible": {"outcome": "fail"},
            },
        )

        comparison = result["suites"][0]["comparisons"][0]
        self.assertIsNone(comparison["ratio"])
        self.assertEqual(comparison["status"], "not_comparable")
        self.assertNotIn("compatibility_excluded_members", comparison)
        self.assertEqual(comparison["baseline_value"], 10.0)
        self.assertEqual(comparison["subject_value"], 5.0)

    def test_all_passing_common_members_preserve_ratio_and_status(self) -> None:
        result = {
            "suites": [
                {
                    "members": [{"workload_id": "one"}, {"workload_id": "two"}],
                    "comparisons": [
                        {
                            "baseline_subject": "pillow-simd",
                            "baseline_value": 8.0,
                            "subject_value": 4.0,
                            "ratio": 0.5,
                            "status": "comparable",
                            "excluded_members": [],
                        },
                        {
                            "baseline_subject": "pillow",
                            "ratio": 0.75,
                            "status": "comparable",
                            "excluded_members": [],
                        },
                    ],
                }
            ]
        }

        gate_pillow_simd_suite_comparisons(
            result,
            {"one": {"outcome": "pass"}, "two": {"outcome": "pass"}},
        )

        simd_comparison, pillow_comparison = result["suites"][0]["comparisons"]
        self.assertEqual(simd_comparison["ratio"], 0.5)
        self.assertEqual(simd_comparison["status"], "comparable")
        self.assertNotIn("compatibility_excluded_members", simd_comparison)
        self.assertNotIn("compatibility_excluded_members", pillow_comparison)
        self.assertEqual(pillow_comparison["ratio"], 0.75)


if __name__ == "__main__":
    unittest.main()
