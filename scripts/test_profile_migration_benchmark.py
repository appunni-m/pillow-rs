"""Regression tests for the managed migration benchmark profiler."""
from __future__ import annotations

import os
import unittest

from profile_migration_benchmark import ROOT, profile_environment
from migration_benchmark_adapter import (
    benchmark_adapter_environment,
    benchmark_adapter_python,
)


class ProfileMigrationBenchmarkTests(unittest.TestCase):
    def test_profile_matches_managed_benchmark_backend_environment(self) -> None:
        inherited = {"PATH": "/usr/bin", "PYTHONPATH": "existing-python-path"}

        for backend in ("cpu", "simd", "gpu"):
            with self.subTest(backend=backend):
                environment = profile_environment(backend, inherited)
                self.assertEqual(environment["MIGRATION_TARGET_BACKEND"], backend)
                self.assertEqual(environment["MIGRATION_STRICT_TARGET_BACKEND"], "1")
                self.assertEqual(
                    environment["MIGRATION_PARITY_BENCHMARK_ADAPTER"], "1"
                )
                self.assertEqual(
                    environment["PYTHONPATH"],
                    os.pathsep.join(
                        [
                            str((ROOT / "pillow-rs-py" / "python").resolve()),
                            "existing-python-path",
                        ]
                    ),
                )

    def test_pillow_profile_maps_to_cpu_target_backend(self) -> None:
        target_python = str((ROOT / "pillow-rs-py" / "python").resolve())
        environment = profile_environment(
            "pillow",
            {"PYTHONPATH": os.pathsep.join([target_python, "oracle-site-packages"])},
        )

        self.assertEqual(environment["MIGRATION_TARGET_BACKEND"], "cpu")
        self.assertEqual(environment["MIGRATION_PARITY_BENCHMARK_ADAPTER"], "1")
        self.assertEqual(environment["MIGRATION_PARITY_TIMED_SOURCE"], "1")
        self.assertEqual(environment["PYTHONPATH"], "oracle-site-packages")

    def test_source_and_target_adapters_use_distinct_python_paths(self) -> None:
        target_python = str((ROOT / "pillow-rs-py" / "python").resolve())
        inherited = {
            "PYTHONPATH": os.pathsep.join(["oracle-site-packages", target_python])
        }

        source = benchmark_adapter_environment("source", "cpu", inherited)
        target = benchmark_adapter_environment("target", "cpu", inherited)

        self.assertEqual(source["PYTHONPATH"], "oracle-site-packages")
        self.assertEqual(
            target["PYTHONPATH"],
            os.pathsep.join([target_python, "oracle-site-packages", target_python]),
        )
        for environment in (source, target):
            self.assertEqual(environment["MIGRATION_STRICT_TARGET_BACKEND"], "1")
            self.assertEqual(environment["MIGRATION_PARITY_BENCHMARK_ADAPTER"], "1")

    def test_adapter_interpreter_matches_managed_source_selection(self) -> None:
        self.assertEqual(
            benchmark_adapter_python(
                "source",
                {
                    "MIGRATION_BENCHMARK_SOURCE_PYTHON": "/opt/pillow-simd/bin/python",
                    "MIGRATION_ORACLE_PYTHON": "/opt/pillow/bin/python",
                },
            ),
            "/opt/pillow-simd/bin/python",
        )
        self.assertEqual(
            benchmark_adapter_python(
                "source", {"MIGRATION_ORACLE_PYTHON": "/opt/pillow/bin/python"}
            ),
            "/opt/pillow/bin/python",
        )

    def test_unknown_backend_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "backend must be"):
            profile_environment("parallel-cpu", {})


if __name__ == "__main__":
    unittest.main()
