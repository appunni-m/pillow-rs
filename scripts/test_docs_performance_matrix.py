"""Keep the public performance matrix on the generated contract denominator."""
from __future__ import annotations

import unittest

from build_performance_optimization_matrix import PROFILES, operation_matrix_rows


class OperationMatrixTests(unittest.TestCase):
    def test_rows_cover_manifest_profiles_without_expanding_to_unselected_api_paths(self) -> None:
        selected = "PIL.Image.Image.resize"
        operations = {
            selected: {
                "api_path": selected,
                "kind": "method",
                "classification": "endpoint",
                "workload_ids": [],
                "manifest_profiles": {"python-cpu"},
                "workload_profiles": set(),
                "declared_profiles": ["python-cpu"],
            }
        }
        api_inventory = [
            {
                "manifest_operation": selected,
                "manifest_status": "selected_manifest",
                "public_path": selected,
            },
            {
                "manifest_operation": "",
                "manifest_status": "outside_selected_manifest",
                "public_path": "PIL.Image.Image.private_helper",
            },
        ]

        rows = operation_matrix_rows(operations, [], api_inventory)

        self.assertEqual(len(rows), len(PROFILES))
        self.assertEqual({row["profile"] for row in rows}, set(PROFILES))
        self.assertEqual({row["api_path"] for row in rows}, {selected})
        by_profile = {row["profile"]: row for row in rows}
        self.assertEqual(
            by_profile["python-cpu"]["profile_status"], "no_benchmark_mapping"
        )
        for profile in set(PROFILES) - {"python-cpu"}:
            self.assertEqual(
                by_profile[profile]["profile_status"],
                "not_declared_in_manifest_or_workload",
            )


if __name__ == "__main__":
    unittest.main()
