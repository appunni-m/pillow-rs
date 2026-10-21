"""Focused contracts for benchmark runner environment identity."""
from __future__ import annotations

import importlib
import os
from pathlib import Path
import sys
import unittest
from unittest.mock import patch


SCRIPTS = Path(__file__).resolve().parent
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

benchmark = importlib.import_module("run_migration_benchmark")
parity = importlib.import_module("run_migration_parity")
producer = importlib.import_module("migration_producer_attestation")


class BenchmarkRunnerIdentityTests(unittest.TestCase):
    def test_observed_invocation_context_distinguishes_direct_runner_from_make(self) -> None:
        with patch.dict(os.environ, {}, clear=True), patch.object(
            producer.sys,
            "argv",
            ["scripts/run_migration_benchmark.py", "--workload-id", "resize.small"],
        ), patch.object(producer.sys, "executable", "/venv/bin/python"):
            direct = producer.observed_invocation_context()
        self.assertEqual(
            direct,
            {
                "runner_argv": [
                    "/venv/bin/python",
                    "scripts/run_migration_benchmark.py",
                    "--workload-id",
                    "resize.small",
                ],
                "effective_make_context": None,
            },
        )

        with patch.dict(
            os.environ,
            {
                "PILLOW_RS_OUTER_MAKE_EXECUTABLE": "/usr/bin/make",
                "PILLOW_RS_OUTER_MAKE_GOALS": "migration-parity-benchmark",
            },
            clear=True,
        ), patch.object(
            producer.sys,
            "argv",
            ["scripts/run_migration_benchmark.py", "--workload-id", "resize.small"],
        ), patch.object(producer.sys, "executable", "/venv/bin/python"):
            make = producer.observed_invocation_context()
        self.assertEqual(
            make["effective_make_context"],
            {
                "executable": "/usr/bin/make",
                "goals": ["migration-parity-benchmark"],
            },
        )
        producer.validate_invocation_context(make)

    def test_invalid_or_partial_make_context_is_rejected(self) -> None:
        with patch.dict(
            os.environ,
            {"PILLOW_RS_OUTER_MAKE_EXECUTABLE": "/usr/bin/make"},
            clear=True,
        ):
            with self.assertRaisesRegex(ValueError, "incomplete"):
                producer.observed_invocation_context()

    def test_benchmark_command_identity_uses_effective_argv(self) -> None:
        with patch.object(
            benchmark.sys,
            "argv",
            ["scripts/run_migration_benchmark.py", "--workload-id", "resize.small"],
        ), patch.object(benchmark.sys, "executable", "/venv/bin/python"):
            self.assertEqual(
                benchmark.current_command_argv(),
                [
                    "/venv/bin/python",
                    "scripts/run_migration_benchmark.py",
                    "--workload-id",
                    "resize.small",
                ],
            )

    def test_parity_command_identity_uses_effective_argv(self) -> None:
        with patch.object(
            parity.sys,
            "argv",
            ["scripts/run_migration_parity.py", "--case-id", "case-1"],
        ), patch.object(parity.sys, "executable", "/venv/bin/python"):
            self.assertEqual(
                parity.current_command_argv(),
                [
                    "/venv/bin/python",
                    "scripts/run_migration_parity.py",
                    "--case-id",
                    "case-1",
                ],
            )

    def test_command_identity_records_actual_working_directory(self) -> None:
        with patch.object(benchmark.os, "getcwd", return_value="/tmp/benchmark-run"):
            self.assertEqual(benchmark.current_command_cwd(), "/tmp/benchmark-run")
        with patch.object(parity.os, "getcwd", return_value="/tmp/parity-run"):
            self.assertEqual(parity.current_command_cwd(), "/tmp/parity-run")

    def test_duplicate_parity_source_exception_is_limited_to_benchmark_runs(self) -> None:
        self.assertTrue(
            benchmark.is_benchmark_runner_argv(
                [
                    "/venv/bin/python",
                    "scripts/run_migration_benchmark.py",
                    "--workload-id",
                    "x",
                ]
            )
        )
        self.assertTrue(
            benchmark.is_benchmark_runner_argv(
                ["make", "migration-parity-benchmark"]
            )
        )
        self.assertFalse(
            benchmark.is_benchmark_runner_argv(
                [
                    "/venv/bin/python",
                    "scripts/run_migration_parity.py",
                    "--case-id",
                    "x",
                ]
            )
        )

    def test_uses_stable_runner_id_and_explicit_cpu_model(self) -> None:
        values = {
            "MIGRATION_BENCHMARK_RUNNER_ID": "ubuntu-24.04-x86_64",
            "MIGRATION_BENCHMARK_CPU_MODEL": "AMD Ryzen 5 2600X",
        }
        with patch.dict(os.environ, values, clear=True), patch.multiple(
            benchmark.platform,
            node=lambda: "ephemeral-host-17",
            platform=lambda: "Linux-6.8-x86_64",
            machine=lambda: "x86_64",
            processor=lambda: "generic",
            python_version=lambda: "3.12.10",
        ):
            environment = benchmark.benchmark_environment()

        self.assertEqual(environment["runner_id"], "ubuntu-24.04-x86_64")
        self.assertEqual(environment["cpu"], "AMD Ryzen 5 2600X")
        self.assertEqual(environment["machine_id"], "ephemeral-host-17")
        self.assertEqual(environment["os"], "Linux-6.8-x86_64")
        self.assertEqual(environment["architecture"], "x86_64")

    def test_without_runner_override_does_not_invent_stable_identity(self) -> None:
        with patch.dict(os.environ, {}, clear=True), patch.multiple(
            benchmark.platform,
            node=lambda: "local-host",
            platform=lambda: "Linux-6.8-x86_64",
            machine=lambda: "x86_64",
            processor=lambda: "x86_64",
            python_version=lambda: "3.12.10",
        ):
            environment = benchmark.benchmark_environment()

        self.assertNotIn("runner_id", environment)
        self.assertEqual(environment["cpu"], "x86_64")
        self.assertEqual(environment["machine_id"], "local-host")


if __name__ == "__main__":
    unittest.main()
