"""Guards for the isolated, same-host Pillow-SIMD benchmark cohort."""
from __future__ import annotations

import copy
import gzip
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from docs_benchmark_view import compare, render_dashboard
from docs_evidence import render_pillow_simd_benchmarks, validate
from build_performance_optimization_matrix import (
    classify,
    goal_status,
    operation_matrix_rows,
)
from run_pillow_simd_benchmark import (
    PILLOW_SIMD_VERSION,
    PILLOW_VERSION,
    WORKLOAD_IDS,
    append_github_failure_summary,
    append_github_source_compatibility_summary,
    compare_source_compatibility,
    combine_results,
    run_source_compatibility,
    run_source_cohort,
    validate_run,
    workload_contracts,
    write_parity_failure_diagnostics,
)
from run_migration_benchmark import (
    DEFAULT_MANIFEST,
    load_benchmarks,
    load_manifest,
    load_parity_cases,
)
from run_migration_parity import build_operation_index


def run(
    name: str,
    version: str,
    suffix: str,
    offset: float = 0,
    source_id: str = "pillow",
) -> dict:
    contracts = workload_contracts()
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
        declared_targets = {
            subject["id"]
            for subject in contracts[workload_id]["subjects"]
            if subject["kind"] == "target_profile"
        }
        for subject_id, time in (
            (source_id, 2.0),
            ("python-cpu", 1.0),
            ("python-simd", 0.5),
        ):
            if subject_id != source_id and subject_id not in declared_targets:
                continue
            if subject_id in {"pillow", "pillow-simd"}:
                execution = {
                    "requested_backend": subject_id,
                    "actual_backend": subject_id,
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
                    "kind": (
                        "oracle"
                        if subject_id in {"pillow", "pillow-simd"}
                        else "target_profile"
                    ),
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
    def test_subject_validation_uses_each_workloads_declared_profiles(self) -> None:
        result = run("Pillow", PILLOW_VERSION, "regular-pillow")
        parity = {
            "status": "completed",
            "summary": {"failed": 0},
            "comparisons": [],
            "infrastructure_errors": [],
        }

        validate_run(result, parity, "Pillow", PILLOW_VERSION, list(WORKLOAD_IDS))

        cpu_only_id = "pipeline-op.rotate.material-rgba-nearest-37deg-noise-fill-1024x768"
        cpu_only = next(
            workload
            for workload in result["workloads"]
            if workload["workload_id"] == cpu_only_id
        )
        self.assertEqual(
            {subject["id"] for subject in cpu_only["subjects"]},
            {"pillow", "python-cpu"},
        )

        missing_simd = copy.deepcopy(result)
        declared_both = missing_simd["workloads"][0]
        declared_both["subjects"] = [
            subject
            for subject in declared_both["subjects"]
            if subject["id"] != "python-simd"
        ]
        with self.assertRaisesRegex(ValueError, "python-simd"):
            validate_run(
                missing_simd,
                parity,
                "Pillow",
                PILLOW_VERSION,
                list(WORKLOAD_IDS),
            )

    def test_operation_matrix_includes_verified_cpu_rows_from_both_snapshots(self) -> None:
        full_source = "github-pages/benchmark.json"
        pillow_simd_source = "github-pages/pillow-simd-benchmark.json"

        def workload_row(
            source: str,
            workload: str,
            profile: str,
            speedup: str = "",
        ) -> dict[str, str]:
            return {
                "source_snapshot": source,
                "source_revision": "a" * 40,
                "source_clean": "true",
                "measured_at": "2026-10-13T00:00:00Z",
                "source_json_sha256": "b" * 64,
                "workload": workload,
                "runner": "test-runner",
                "profile": profile,
                "verified_speedup_vs_pillow": speedup,
                "evidence_status": "parity_backend_verified",
            }

        operation = {
            "api_path": "PIL.Image.Image.getchannel",
            "kind": "method",
            "classification": "endpoint",
            "manifest_profiles": {"python-cpu"},
            "workload_profiles": {"python-cpu"},
            "declared_profiles": ["python-cpu"],
            "workload_ids": ["workload.full", "workload.pillow-simd", "workload.missing"],
        }
        rows = [
            workload_row(full_source, "workload.full", "python-cpu", "1.25"),
            workload_row(
                pillow_simd_source,
                "workload.pillow-simd",
                "python-cpu",
                "0.75",
            ),
            # Local observations are not part of either published snapshot.
            workload_row("local-run", "workload.pillow-simd", "python-cpu", "0.1"),
        ]

        result = operation_matrix_rows(
            {"PIL.Image.Image.getchannel": operation},
            rows,
            [],
        )
        cpu = next(row for row in result if row["profile"] == "python-cpu")

        self.assertEqual(cpu["published_workloads"], "2")
        self.assertEqual(cpu["missing_published_workloads"], "workload.missing")
        self.assertEqual(cpu["published_profile_rows"], "2")
        self.assertEqual(cpu["verified_comparisons"], "2")
        self.assertEqual(cpu["below_baseline_comparisons"], "1")
        self.assertEqual(cpu["worst_verified_workload"], "workload.pillow-simd")
        self.assertEqual(cpu["worst_verified_speedup"], "0.750000")
        self.assertEqual(cpu["profile_status"], "target_unmet_in_verified_rows")

    def test_operation_matrix_counts_paired_source_compatibility(self) -> None:
        source = "github-pages/pillow-simd-benchmark.json"

        def row(
            workload: str,
            profile: str,
            evidence: str,
            *,
            runner: str = "test-runner",
            group: str = "default",
            speedup: str = "",
        ) -> dict[str, str]:
            return {
                "source_snapshot": source,
                "source_clean": "true",
                "runner": runner,
                "comparison_group": group,
                "workload": workload,
                "profile": profile,
                "evidence_status": evidence,
                "verified_speedup_vs_pillow_simd": speedup,
            }

        workload_ids = ["compatible", "incompatible", "unproven", "missing"]
        operation = {
            "api_path": "PIL.Image.Image.filter",
            "kind": "method",
            "classification": "endpoint",
            "manifest_profiles": {"python-simd"},
            "workload_profiles": {"python-simd"},
            "declared_profiles": ["python-simd"],
            "workload_ids": workload_ids,
        }
        rows = [
            row(workload, "python-simd", "parity_backend_verified",
                speedup="2" if workload == "compatible" else "")
            for workload in workload_ids
        ]
        rows.extend([
            row("compatible", "pillow-simd", "pillow-simd_reference"),
            row("incompatible", "pillow-simd", "pillow_simd_incompatible"),
            row("unproven", "pillow-simd", "pillow_simd_compatibility_not_proven"),
            # A source from another runner or comparison group cannot fill
            # the missing baseline or override its incompatible counterpart.
            row("missing", "pillow-simd", "pillow-simd_reference", runner="other"),
            row("incompatible", "pillow-simd", "pillow-simd_reference", group="other"),
        ])
        result = operation_matrix_rows({"PIL.Image.Image.filter": operation}, rows, [])
        simd = next(item for item in result if item["profile"] == "python-simd")
        self.assertEqual(simd["pillow_simd_snapshot_workloads"], "4")
        self.assertEqual(simd["verified_pillow_simd_comparisons"], "1")
        self.assertEqual(simd["pillow_simd_incompatible_rows"], "1")
        self.assertEqual(simd["pillow_simd_unproven_rows"], "2")
        self.assertEqual(simd["worst_verified_speedup_vs_pillow_simd"], "2.000000")

    @staticmethod
    def compatibility(outcome: str = "pass") -> dict[str, dict[str, object]]:
        return {
            workload_id: {"outcome": outcome, "differences": []}
            for workload_id in WORKLOAD_IDS
        }

    def test_selected_workloads_are_material_full_size_and_parity_backed(self) -> None:
        selected = workload_contracts()
        self.assertEqual(tuple(selected), WORKLOAD_IDS)
        self.assertEqual(len(selected), 37)
        self.assertEqual(
            {item["context"]["mode"] for item in selected.values()},
            {"L", "LA", "RGB", "RGBA", "CMYK", "F", "I", "YCbCr"},
        )
        manifest = load_manifest(DEFAULT_MANIFEST)
        workloads, _, _ = load_benchmarks(manifest)
        expected_direct_operations = {
            workload_id
            for workload_id, item in workloads.items()
            if workload_id.startswith("pipeline-op.")
            and item["input"]["kind"] == "parity_case"
            and item["measurement"]["correctness_gate"] == "parity_pass"
            and item["context"].get("size") == [1024, 768]
        }
        selected_direct_operations = {
            workload_id for workload_id in selected if workload_id.startswith("pipeline-op.")
        }
        self.assertEqual(len(expected_direct_operations), 32)
        self.assertEqual(selected_direct_operations, expected_direct_operations)
        self.assertTrue(
            all(
                item["measurement"]["correctness_gate"] == "parity_pass"
                and item["context"]["size"] == [1024, 768]
                for item in selected.values()
            )
        )

    def test_merge_preserves_same_host_baseline_and_adds_pillow_simd(self) -> None:
        pillow = run("Pillow", PILLOW_VERSION, "normal")
        simd = run(
            "Pillow",
            PILLOW_VERSION,
            "simd",
            offset=0.25,
            source_id="pillow-simd",
        )
        snapshot = combine_results(
            pillow,
            simd,
            "c" * 64,
            source_compatibility=self.compatibility(),
        )
        validate(snapshot, "appunni-m/pillow-rs")
        contracts = workload_contracts()
        expected_row_count = sum(
            2
            + len(
                {
                    subject["id"]
                    for subject in contracts[workload_id]["subjects"]
                    if subject["kind"] == "target_profile"
                    and subject["id"] in {"python-cpu", "python-simd"}
                }
            )
            for workload_id in WORKLOAD_IDS
        )
        self.assertEqual(len(snapshot["rows"]), expected_row_count)
        self.assertEqual(snapshot["environment"]["architecture"], "x86_64")
        for workload in WORKLOAD_IDS:
            baseline = next(r for r in snapshot["rows"] if r["workload"] == workload and r["subject"] == "pillow")
            simd_row = next(r for r in snapshot["rows"] if r["workload"] == workload and r["subject"] == "pillow-simd")
            self.assertEqual(compare(simd_row, baseline)[0], 2.0 / 2.25)
            self.assertEqual(simd_row["actual_backend"], "pillow-simd")
        html = render_dashboard(snapshot, {"benchmark": {"kind": "pillow"}})
        self.assertIn("Pillow-SIMD · SSE4", html)

    def test_merge_rejects_mixed_hosts_and_changed_workload_conditions(self) -> None:
        pillow = run("Pillow", PILLOW_VERSION, "normal")
        simd = run("Pillow", PILLOW_VERSION, "simd", source_id="pillow-simd")
        mixed_host = copy.deepcopy(simd)
        mixed_host["environment"]["architecture"] = "arm64"
        with self.assertRaisesRegex(ValueError, "different hosts"):
            combine_results(
                pillow, mixed_host, "c" * 64, source_compatibility=self.compatibility()
            )
        changed = copy.deepcopy(simd)
        changed["workloads"][0]["context"]["mode"] = "RGBA"
        with self.assertRaisesRegex(ValueError, "conditions differ"):
            combine_results(
                pillow, changed, "c" * 64, source_compatibility=self.compatibility()
            )

    def test_pillow_simd_incompatibility_keeps_the_row_but_withholds_ratio(self) -> None:
        pillow = run("Pillow", PILLOW_VERSION, "normal")
        simd = run(
            "Pillow", PILLOW_VERSION, "simd", offset=0.25, source_id="pillow-simd"
        )
        compatibility = self.compatibility()
        compatibility[WORKLOAD_IDS[0]] = {
            "outcome": "fail",
            "differences": [{"path": "result.value", "kind": "image_mismatch"}],
        }
        snapshot = combine_results(
            pillow,
            simd,
            "c" * 64,
            source_compatibility=compatibility,
        )
        workload = WORKLOAD_IDS[0]
        baseline = next(
            row
            for row in snapshot["rows"]
            if row["workload"] == workload and row["subject"] == "pillow"
        )
        simd_row = next(
            row
            for row in snapshot["rows"]
            if row["workload"] == workload and row["subject"] == "pillow-simd"
        )
        self.assertEqual(simd_row["correctness"], "Pillow-SIMD compatibility: fail")
        self.assertIsNone(compare(simd_row, baseline)[0])
        self.assertEqual(simd_row["median_us"], 2250)

    def test_parity_failure_names_unmatched_case(self) -> None:
        result = run("Pillow", PILLOW_VERSION, "simd", source_id="pillow-simd")
        parity = {
            "status": "completed",
            "summary": {"failed": 1},
            "comparisons": [
                {
                    "case_id": "case-that-did-not-match",
                    "target_profile": "python-simd",
                    "outcome": "mismatch",
                }
            ],
        }
        with self.assertRaisesRegex(ValueError, "case-that-did-not-match"):
            validate_run(result, parity, "Pillow-SIMD", PILLOW_SIMD_VERSION, list(WORKLOAD_IDS))

    def test_parity_failure_row_cannot_be_hidden_by_zero_summary_count(self) -> None:
        result = run("Pillow", PILLOW_VERSION, "simd", source_id="pillow-simd")
        parity = {
            "status": "completed",
            "summary": {"failed": 0},
            "comparisons": [
                {
                    "case_id": "unaccounted-mismatch",
                    "target_profile": "python-cpu",
                    "outcome": "mismatch",
                }
            ],
        }
        with self.assertRaisesRegex(ValueError, "unaccounted-mismatch"):
            validate_run(result, parity, "Pillow-SIMD", PILLOW_SIMD_VERSION, list(WORKLOAD_IDS))

    def test_parity_failure_reports_exact_byte_mismatch_offsets(self) -> None:
        result = run("Pillow", PILLOW_VERSION, "simd", source_id="pillow-simd")

        def observation(data: str) -> dict:
            return {
                "status": "completed",
                "observations": [
                    {
                        "step_id": "materialize",
                        "status": "ok",
                        "value": {"kind": "bytes", "encoding": "base64", "data": data},
                    }
                ],
            }

        parity = {
            "status": "completed",
            "summary": {"failed": 1},
            "comparisons": [
                {
                    "case_id": "byte-mismatch",
                    "target_profile": "python-simd",
                    "outcome": "mismatch",
                    "source": observation("AQIDBA=="),
                    "target": observation("AQkDBQ=="),
                    "diffs": [
                        {
                            "step_id": "materialize",
                            "path": "materialize.value",
                            "kind": "bytes_mismatch",
                            "message": "exact byte value mismatch",
                        }
                    ],
                }
            ],
        }
        with self.assertRaises(ValueError) as raised:
            validate_run(result, parity, "Pillow-SIMD", PILLOW_SIMD_VERSION, list(WORKLOAD_IDS))

        message = str(raised.exception)
        self.assertIn("byte-mismatch", message)
        self.assertIn("different_bytes_in_shared_range", message)
        self.assertIn("'offset': 1", message)
        self.assertIn("'source': 2", message)
        self.assertIn("'target': 9", message)

    def test_parity_failure_artifact_keeps_only_compact_diagnostics(self) -> None:
        parity = {
            "identity": {"run_id": "parity-run", "oracles": [{"name": "Pillow"}]},
            "comparisons": [
                {
                    "case_id": "byte-mismatch",
                    "target_profile": "python-cpu",
                    "outcome": "mismatch",
                    "source": {
                        "observations": [
                            {
                                "step_id": "materialize",
                                "value": {"kind": "bytes", "encoding": "base64", "data": "AQIDBA=="},
                            }
                        ]
                    },
                    "target": {
                        "observations": [
                            {
                                "step_id": "materialize",
                                "value": {"kind": "bytes", "encoding": "base64", "data": "AQkDBQ=="},
                            }
                        ]
                    },
                    "diffs": [
                        {
                            "step_id": "materialize",
                            "path": "materialize.value",
                            "kind": "bytes_mismatch",
                            "message": "exact byte value mismatch",
                        }
                    ],
                }
            ],
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "failure.json"
            write_parity_failure_diagnostics(path, "Pillow-SIMD", PILLOW_SIMD_VERSION, parity)
            encoded = path.read_text()
            diagnostic = json.loads(encoded)

        self.assertEqual(diagnostic["parity_identity"]["run_id"], "parity-run")
        failure = diagnostic["failed_comparisons"][0]
        self.assertEqual(failure["case_id"], "byte-mismatch")
        self.assertEqual(
            failure["diffs"][0]["byte_comparison"]["different_bytes_in_shared_range"],
            2,
        )
        self.assertNotIn("AQIDBA==", encoded)

    def test_pillow_simd_timing_is_gated_against_regular_pillow(self) -> None:
        result = run("Pillow", PILLOW_VERSION, "simd", source_id="pillow-simd")
        parity = {
            "status": "completed",
            "summary": {"failed": 0},
            "comparisons": [],
        }
        validate_run(
            result,
            parity,
            "Pillow-SIMD",
            PILLOW_SIMD_VERSION,
            list(WORKLOAD_IDS),
        )

    def test_source_cohort_keeps_regular_pillow_as_oracle(self) -> None:
        parity = {
            "status": "completed",
            "identity": {"run_id": "regular-pillow-parity"},
            "summary": {"failed": 0},
            "comparisons": [],
            "infrastructure_errors": [],
        }

        for name, version, source_id in (
            ("Pillow", PILLOW_VERSION, "pillow"),
            ("Pillow-SIMD", PILLOW_SIMD_VERSION, "pillow-simd"),
        ):
            result = run("Pillow", PILLOW_VERSION, source_id, source_id=source_id)
            with self.subTest(source=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                result_path = root / "benchmark.json"
                parity_path = root / "parity.json"
                reused_parity = root / "regular-pillow-parity.json"
                reused_attestation = root / "regular-pillow-parity.attestation.json"
                parity_attestation = root / "parity.attestation.json"
                benchmark_attestation = root / "benchmark.attestation.json"
                source_python = f"/subject/{source_id}"
                observed_commands = []

                def fake_run(command, *, env, **_kwargs):
                    observed_commands.append(command)
                    self.assertEqual(env["MIGRATION_ORACLE_PYTHON"], "/oracle/pillow")
                    self.assertEqual(env["MIGRATION_ORACLE_NAME"], "Pillow")
                    self.assertEqual(env["MIGRATION_BENCHMARK_COHORT"], "pillow-simd")
                    self.assertEqual(env["MIGRATION_BENCHMARK_SOURCE_PYTHON"], source_python)
                    self.assertEqual(env["MIGRATION_BENCHMARK_SOURCE_NAME"], name)
                    result_path.write_text(json.dumps(result))
                    parity_path.write_text(json.dumps(parity))
                    return type("Completed", (), {"returncode": 0, "stdout": "", "stderr": ""})()

                with patch("run_pillow_simd_benchmark.subprocess.run", side_effect=fake_run):
                    _, run_id = run_source_cohort(
                        source_python=source_python,
                        source_name=name,
                        source_version=version,
                        oracle_python="/oracle/pillow",
                        output=result_path,
                        parity_output=parity_path,
                        workload_ids=list(WORKLOAD_IDS),
                        reuse_parity=(reused_parity if source_id == "pillow-simd" else None),
                        reuse_parity_attestation=(
                            reused_attestation if source_id == "pillow-simd" else None
                        ),
                        parity_attestation_output=(
                            parity_attestation if source_id == "pillow" else None
                        ),
                        attestation_output=benchmark_attestation,
                    )
                self.assertEqual(run_id, "regular-pillow-parity")
                command = observed_commands[0]
                self.assertIn("--attestation-output", command)
                self.assertIn(str(benchmark_attestation), command)
                if source_id == "pillow":
                    self.assertIn("--parity-attestation-output", command)
                    self.assertIn(str(parity_attestation), command)
                else:
                    self.assertIn("--reuse-parity-attestation", command)
                    self.assertIn(str(reused_attestation), command)

    def test_source_output_compatibility_uses_the_declared_parity_comparator(self) -> None:
        manifest = load_manifest(DEFAULT_MANIFEST)
        operation_index = build_operation_index(manifest)
        case = {
            "steps": [
                {
                    "step_id": "result",
                    "surface": "PIL.ImageOps",
                    "operation": "grayscale",
                }
            ]
        }

        def result(value: str) -> dict:
            return {
                "status": "completed",
                "observations": [
                    {"step_id": "result", "status": "ok", "value": value}
                ],
            }

        self.assertEqual(
            compare_source_compatibility(
                case, result("same"), result("same"), operation_index
            )[0],
            "pass",
        )
        self.assertEqual(
            compare_source_compatibility(
                case, result("pillow"), result("pillow-simd"), operation_index
            )[0],
            "fail",
        )

    def test_source_compatibility_records_mismatch_and_keeps_target_outputs_stable(self) -> None:
        manifest = load_manifest(DEFAULT_MANIFEST)
        cases, _ = load_parity_cases(manifest)
        case_id = "PIL.ImageOps.grayscale.nuanced.performance-large-rgb-noise-1024x768"
        case = cases[case_id]
        workload_id = WORKLOAD_IDS[19]

        def observations(value: str) -> list[dict]:
            return [
                {"step_id": step_id, "status": "ok", "value": value}
                for step_id in case["observations"]
            ]

        reference_comparisons = [
            {
                "case_id": case_id,
                "target_profile": profile,
                "outcome": "pass",
                "source": {"status": "completed", "observations": observations("pillow")},
                "target": {"status": "completed", "observations": observations("pillow")},
            }
            for profile in ("python-cpu", "python-simd")
        ]

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            reference_path = root / "pillow-parity.json"
            reference_path.write_text(
                json.dumps({"comparisons": reference_comparisons})
            )

            def fake_run(command, *, env, **_kwargs):
                profile = f"python-{env['MIGRATION_TARGET_BACKEND']}"
                artifact = Path(command[command.index("--output") + 1])
                parity = {
                    "status": "completed",
                    "identity": {
                        "run_id": f"compat-{profile}",
                        "oracles": [
                            {"name": "Pillow-SIMD", "version": PILLOW_SIMD_VERSION}
                        ],
                    },
                    "infrastructure_errors": [],
                    "comparisons": [
                        {
                            "case_id": case_id,
                            "target_profile": profile,
                            "outcome": "fail",
                            "source": {
                                "status": "completed",
                                "observations": observations("pillow-simd"),
                            },
                            "target": {
                                "status": "completed",
                                "observations": observations("pillow"),
                            },
                        }
                    ],
                }
                with gzip.open(artifact, "wt", encoding="utf-8") as stream:
                    json.dump(parity, stream)
                return type("Completed", (), {"returncode": 1, "stdout": "", "stderr": ""})()

            with (
                patch("run_pillow_simd_benchmark.load_parity_cases", return_value=({case_id: case}, {})),
                patch(
                    "run_pillow_simd_benchmark.workload_contracts",
                    return_value={
                        workload_id: {
                            "input": {"case_id": case_id},
                            "subjects": [
                                {"kind": "oracle", "id": "pillow"},
                                {"kind": "target_profile", "id": "python-cpu"},
                                {"kind": "target_profile", "id": "python-simd"},
                            ],
                        }
                    },
                ),
                patch("run_pillow_simd_benchmark.subprocess.run", side_effect=fake_run),
            ):
                compatibility, _artifacts = run_source_compatibility(
                    pillow_python="/oracle/pillow",
                    pillow_simd_python="/oracle/pillow-simd",
                    pillow_parity_path=reference_path,
                    workload_ids=[workload_id],
                    output=root / "compatibility.json",
                )

        self.assertEqual(compatibility[workload_id]["outcome"], "fail")
        self.assertEqual(
            compatibility[workload_id]["differences"][0]["kind"],
            "image_mismatch",
        )

    def test_source_compatibility_uses_each_workloads_declared_profiles(self) -> None:
        manifest = load_manifest(DEFAULT_MANIFEST)
        cases, _ = load_parity_cases(manifest)
        workload_id = "pipeline-op.rotate.material-rgba-nearest-37deg-noise-fill-1024x768"
        case_id = workload_contracts()[workload_id]["input"]["case_id"]
        case = cases[case_id]

        def observations() -> list[dict]:
            return [
                {"step_id": step_id, "status": "ok", "value": "same-output"}
                for step_id in case["observations"]
            ]

        reference = {
            "case_id": case_id,
            "target_profile": "python-cpu",
            "outcome": "pass",
            "source": {"status": "completed", "observations": observations()},
            "target": {"status": "completed", "observations": observations()},
        }
        observed_backends = []

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            reference_path = root / "pillow-parity.json"
            reference_path.write_text(json.dumps({"comparisons": [reference]}))

            def fake_run(command, *, env, **_kwargs):
                backend = env["MIGRATION_TARGET_BACKEND"]
                observed_backends.append(backend)
                artifact = Path(command[command.index("--output") + 1])
                parity = {
                    "status": "completed",
                    "identity": {
                        "run_id": f"compat-python-{backend}",
                        "oracles": [
                            {"name": "Pillow-SIMD", "version": PILLOW_SIMD_VERSION}
                        ],
                    },
                    "infrastructure_errors": [],
                    "comparisons": [
                        {
                            "case_id": case_id,
                            "target_profile": f"python-{backend}",
                            "outcome": "pass",
                            "source": {
                                "status": "completed",
                                "observations": observations(),
                            },
                            "target": {
                                "status": "completed",
                                "observations": observations(),
                            },
                        }
                    ],
                }
                with gzip.open(artifact, "wt", encoding="utf-8") as stream:
                    json.dump(parity, stream)
                return type("Completed", (), {"returncode": 0, "stdout": "", "stderr": ""})()

            with patch(
                "run_pillow_simd_benchmark.subprocess.run", side_effect=fake_run
            ):
                compatibility, _artifacts = run_source_compatibility(
                    pillow_python="/oracle/pillow",
                    pillow_simd_python="/oracle/pillow-simd",
                    pillow_parity_path=reference_path,
                    workload_ids=[workload_id],
                    output=root / "compatibility.json",
                )

        self.assertEqual(observed_backends, ["cpu"])
        self.assertEqual(compatibility[workload_id]["outcome"], "pass")

    def test_matrix_requires_explicit_source_compatibility_for_simd_baseline(self) -> None:
        row = {
            "subject": "pillow-simd",
            "status": "completed",
            "median_us": 10,
            "correctness": "Pillow-SIMD compatibility: fail",
        }
        self.assertEqual(classify(row, source_clean=True), "pillow_simd_incompatible")
        self.assertEqual(
            goal_status(None, "pillow_simd_incompatible", 2.0),
            "baseline_incompatible",
        )
        row["correctness"] = "parity_pass: pass"
        self.assertEqual(
            classify(row, source_clean=True),
            "pillow_simd_compatibility_not_proven",
        )
        row["correctness"] = "Pillow-SIMD compatibility: pass"
        self.assertEqual(classify(row, source_clean=True), "pillow-simd_reference")

    def test_github_failure_summary_is_compact_and_optional(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            summary = Path(directory) / "summary.md"
            with patch.dict("os.environ", {"GITHUB_STEP_SUMMARY": str(summary)}):
                append_github_failure_summary("Pillow-SIMD", "parity mismatch: workload-x")
            self.assertIn("Pillow-SIMD benchmark failed", summary.read_text())
            self.assertIn("parity mismatch: workload-x", summary.read_text())

        with patch.dict("os.environ", {}, clear=True):
            append_github_failure_summary("Pillow", "local test failure")

    def test_github_summary_names_incompatible_pillow_simd_workloads(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            summary = Path(directory) / "summary.md"
            with patch.dict("os.environ", {"GITHUB_STEP_SUMMARY": str(summary)}):
                append_github_source_compatibility_summary(
                    {
                        "filter-workload": {"outcome": "fail"},
                        "grayscale-workload": {"outcome": "not_proven"},
                        "matching-workload": {"outcome": "pass"},
                    }
                )
            text = summary.read_text()
            self.assertIn("Pillow-SIMD source compatibility", text)
            self.assertIn("filter-workload", text)
            self.assertIn("grayscale-workload", text)
            self.assertNotIn("matching-workload", text)

    def test_public_page_renders_snapshot_data_and_matched_host_notice(self) -> None:
        snapshot = combine_results(
            run("Pillow", PILLOW_VERSION, "normal"),
            run("Pillow", PILLOW_VERSION, "simd", source_id="pillow-simd"),
            "c" * 64,
            source_compatibility=self.compatibility(),
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
