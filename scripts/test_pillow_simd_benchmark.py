"""Guards for the isolated, same-host Pillow-SIMD benchmark cohort."""
from __future__ import annotations

import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from docs_benchmark_view import compare, render_dashboard
from docs_evidence import render_pillow_simd_benchmarks, validate
from run_pillow_simd_benchmark import (
    PILLOW_SIMD_VERSION,
    PILLOW_VERSION,
    WORKLOAD_IDS,
    combine_results,
    workload_contracts,
)


def run(name: str, version: str, suffix: str, offset: float = 0) -> dict:
    targets = [
        {
            "target_profile": profile,
            "target_id": "pillow-rs-python",
            "revision": "a" * 40,
            "dirty": False,
            "runtime": "3.12.10",
            "backend": profile.removeprefix("python-"),
            "features": ["pillow-rs-py/default", "pillow-rs/default"],
        }
        for profile in ("python-cpu", "python-simd")
    ]
    workloads = []
    for index, workload_id in enumerate(WORKLOAD_IDS):
        mode = "RGB" if index % 2 else "L"
        subjects = []
        for subject_id, time in (
            ("pillow", 2.0),
            ("python-cpu", 1.0),
            ("python-simd", 0.5),
        ):
            if subject_id == "pillow":
                execution = {
                    "requested_backend": "pillow",
                    "actual_backend": "pillow",
                    "fallback_reason_counts": {},
                }
            else:
                backend = subject_id.removeprefix("python-")
                execution = {
                    "requested_backend": backend,
                    "actual_backend": backend,
                    "fallback_reason_counts": {},
                    "terminal_complete": True,
                }
            subjects.append(
                {
                    "id": subject_id,
                    "status": "completed",
                    "measurements": [
                        {
                            "metric": "latency",
                            "unit": "millisecond",
                            "sample_count": 5,
                            "statistics": {"median": time + offset, "p95": time + offset},
                        }
                    ],
                    "execution": execution,
                }
            )
        workloads.append(
            {
                "workload_id": workload_id,
                "requirements": [],
                "measurement_policy": {"boundary": "observed_steps", "samples": 5},
                "context": {"size": [1024, 768], "mode": mode},
                "correctness": {"gate": "parity_pass", "outcome": "pass"},
                "subjects": subjects,
            }
        )
    return {
        "schema": "migration-parity/benchmark-result@1",
        "identity": {
            "run_id": suffix,
            "started_at": "2026-10-06T00:00:00Z",
            "finished_at": "2026-10-06T00:01:00Z",
            "manifest": {"path": "manifest.yaml", "sha256": "b" * 64},
            "inputs": [],
            "assets": [],
            "oracles": [
                {
                    "oracle_id": "pillow",
                    "name": name,
                    "version": version,
                    "runtime": "CPython 3.12",
                }
            ],
            "targets": targets,
            "command": {"command_id": "test", "argv": [], "cwd": ".", "timeout_seconds": 30},
        },
        "status": "completed",
        "environment": {
            "machine_id": "runner",
            "os": "Linux x86_64",
            "architecture": "x86_64",
            "cpu": "test-cpu",
            "memory_bytes": 0,
            "power_mode": "unknown",
            "toolchain": "3.12.10",
        },
        "workloads": workloads,
    }


class PillowSimdBenchmarkTests(unittest.TestCase):
    def test_selected_workloads_are_material_full_size_and_parity_backed(self) -> None:
        selected = workload_contracts()
        self.assertEqual(tuple(selected), WORKLOAD_IDS)
        self.assertEqual(len(selected), 9)
        self.assertEqual(
            {item["context"]["mode"] for item in selected.values()},
            {"L", "LA", "RGB", "RGBA"},
        )
        self.assertTrue(
            all(
                item["measurement"]["correctness_gate"] == "parity_pass"
                and item["context"]["size"] == [1024, 768]
                for item in selected.values()
            )
        )

    def test_merge_preserves_same_host_baseline_and_adds_pillow_simd(self) -> None:
        pillow = run("Pillow", PILLOW_VERSION, "normal")
        simd = run("Pillow-SIMD", PILLOW_SIMD_VERSION, "simd", offset=0.25)
        snapshot = combine_results(pillow, simd, "c" * 64)
        validate(snapshot, "appunni-m/pillow-rs")
        self.assertEqual(len(snapshot["rows"]), len(WORKLOAD_IDS) * 4)
        self.assertEqual(snapshot["environment"]["architecture"], "x86_64")
        for workload in WORKLOAD_IDS:
            baseline = next(r for r in snapshot["rows"] if r["workload"] == workload and r["subject"] == "pillow")
            simd_row = next(r for r in snapshot["rows"] if r["workload"] == workload and r["subject"] == "pillow-simd")
            self.assertEqual(compare(simd_row, baseline)[0], 2.0 / 2.25)
        html = render_dashboard(snapshot, {"benchmark": {"kind": "pillow"}})
        self.assertIn("Pillow-SIMD · SSE4", html)

    def test_merge_rejects_mixed_hosts_and_changed_workload_conditions(self) -> None:
        pillow = run("Pillow", PILLOW_VERSION, "normal")
        simd = run("Pillow-SIMD", PILLOW_SIMD_VERSION, "simd")
        mixed_host = copy.deepcopy(simd)
        mixed_host["environment"]["architecture"] = "arm64"
        with self.assertRaisesRegex(ValueError, "different hosts"):
            combine_results(pillow, mixed_host, "c" * 64)
        changed = copy.deepcopy(simd)
        changed["workloads"][0]["context"]["mode"] = "RGBA"
        with self.assertRaisesRegex(ValueError, "conditions differ"):
            combine_results(pillow, changed, "c" * 64)

    def test_public_page_renders_snapshot_data_and_matched_host_notice(self) -> None:
        snapshot = combine_results(
            run("Pillow", PILLOW_VERSION, "normal"),
            run("Pillow-SIMD", PILLOW_SIMD_VERSION, "simd"),
            "c" * 64,
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "out"
            (output / "assets").mkdir(parents=True)
            source = root / "snapshot.json"
            source.write_text(json.dumps(snapshot))
            page = render_pillow_simd_benchmarks(source, output)
            self.assertIn("separate from the Apple ARM results", page)
            self.assertIn("Pillow-SIMD · SSE4", page)
            self.assertTrue((output / "assets/pillow-simd-benchmark.json").is_file())


if __name__ == "__main__":
    unittest.main()
