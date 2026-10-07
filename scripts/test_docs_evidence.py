"""Validate benchmark cohort identity and source-safe aggregation."""
from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

from docs_evidence import SCHEMA, merge_pillow_snapshots, merge_pillow_simd_cohort, validate


REPOSITORY = "owner/repo"


def measurement(subject: str, median: float, machine_id: str) -> dict:
    backend = {"pillow": "pillow", "python-cpu": "cpu", "python-simd": "simd", "pillow-simd": "pillow-simd"}[subject]
    return {
        "workload": "pipeline-op.blur.material-rgb",
        "subject": subject,
        "comparison_group": "default",
        "machine_id": machine_id,
        "status": "completed",
        "median_us": median,
        "p90_us": None,
        "p95_us": None,
        "sample_count": 3,
        "sample_unit": "timed execution",
        "policy": {"correctness_gate": "parity_pass", "boundary": "call"},
        "context": {"mode": "RGB", "size": [1024, 768]},
        "correctness": "parity_pass: pass",
        "requested_backend": backend,
        "actual_backend": backend,
        "terminal_complete": True,
        "fallback_reasons": {},
    }


def snapshot(machine_id: str, revision: str = "a" * 40, subjects=None) -> dict:
    label = "Ubuntu 24.04 · x86_64" if "ubuntu" in machine_id else "macOS 15 · arm64"
    return {
        "schema": SCHEMA,
        "repository": REPOSITORY,
        "revision": revision,
        "measured_at": "2026-10-07T00:00:00Z",
        "clean": True,
        "status": "completed",
        "run_id": f"run-{machine_id}",
        "environment": {"os": label, "architecture": label.split("·")[-1].strip()},
        "machine": {
            "id": machine_id,
            "label": label,
            "os": label.split("·")[0].strip(),
            "architecture": label.split("·")[-1].strip(),
            "cpu": "test CPU",
            "python_version": "3.12.10",
            "rust_toolchain": "1.96.1",
            "image_version": "test-image",
        },
        "policy_status": "test policy",
        "notes": [],
        "rows": [measurement(subject, 20 if subject == "pillow" else 10, machine_id)
                 for subject in (subjects or ("pillow", "python-simd"))],
        "source_sha256": ("b" if "ubuntu" in machine_id else "c") * 64,
    }


class DocsEvidenceTests(unittest.TestCase):
    def test_merge_preserves_same_workload_as_separate_runner_pairs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for item in (snapshot("ubuntu-x64"), snapshot("macos-arm64")):
                path = root / item["machine"]["id"] / "snapshot.json"
                path.parent.mkdir()
                path.write_text(json.dumps(item))
            merged = merge_pillow_snapshots(root, REPOSITORY)
        self.assertEqual({machine["id"] for machine in merged["machines"]}, {"ubuntu-x64", "macos-arm64"})
        self.assertEqual(len(merged["rows"]), 4)
        self.assertEqual({row["machine_id"] for row in merged["rows"]}, {"ubuntu-x64", "macos-arm64"})
        self.assertEqual(merged["schema"], SCHEMA)
        validate(merged, REPOSITORY)

    def test_merge_rejects_runner_artifacts_from_different_source_revisions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for item in (snapshot("ubuntu-x64"), snapshot("macos-arm64", revision="d" * 40)):
                path = root / item["machine"]["id"] / "snapshot.json"
                path.parent.mkdir()
                path.write_text(json.dumps(item))
            with self.assertRaisesRegex(ValueError, "one source revision"):
                merge_pillow_snapshots(root, REPOSITORY)

    def test_pillow_simd_rows_form_a_distinct_paired_runner_cohort(self):
        main = snapshot("ubuntu-x64")
        simd = snapshot("ubuntu-x64", subjects=("pillow", "python-cpu", "python-simd", "pillow-simd"))
        merged = merge_pillow_simd_cohort(main, simd)
        self.assertEqual(len(merged["machines"]), 2)
        simd_machine = merged["machines"][1]
        self.assertIn("Pillow-SIMD paired cohort", simd_machine["label"])
        self.assertEqual(
            {row["machine_id"] for row in merged["rows"] if row["subject"] == "pillow-simd"},
            {simd_machine["id"]},
        )
        validate(merged, REPOSITORY)

    def test_pillow_simd_cohort_requires_the_same_source_revision(self):
        with self.assertRaisesRegex(ValueError, "different source revisions"):
            merge_pillow_simd_cohort(snapshot("ubuntu-x64"), snapshot("ubuntu-x64", revision="d" * 40))

    def test_pillow_simd_cohort_requires_matching_machine_identity(self):
        simd = snapshot("ubuntu-x64", subjects=("pillow", "python-cpu", "python-simd", "pillow-simd"))
        simd["machine"]["cpu"] = "different CPU"
        with self.assertRaisesRegex(ValueError, "runner cpu differs"):
            merge_pillow_simd_cohort(snapshot("ubuntu-x64"), simd)

    def test_pillow_simd_cohort_rejects_unrecorded_machine_identity(self):
        simd = snapshot("ubuntu-x64", subjects=("pillow", "python-cpu", "python-simd", "pillow-simd"))
        simd["machine"]["image_version"] = "not recorded"
        with self.assertRaisesRegex(ValueError, "runner image_version identity is incomplete"):
            merge_pillow_simd_cohort(snapshot("ubuntu-x64"), simd)

    def test_machine_identity_records_python_and_rust_versions_separately(self):
        from docs_evidence import attach_machine_identity

        result = attach_machine_identity(
            {"environment": {"os": "Linux", "architecture": "x86_64", "toolchain": "3.12.10"}, "rows": []},
            runner_id="ubuntu-x64", rust_toolchain="1.96.1",
        )
        self.assertEqual(result["machine"]["python_version"], "3.12.10")
        self.assertEqual(result["machine"]["rust_toolchain"], "1.96.1")


if __name__ == "__main__":
    unittest.main()
