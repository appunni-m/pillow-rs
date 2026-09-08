"""Provenance regressions for the maintained public-input coverage collector."""

import hashlib
import json
import copy
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from scripts import run_migration_rust_coverage as collector
from scripts.run_migration_rust_coverage import (
    aggregate_backend_coverage, coverage_execution_passed, coverage_input_hashes,
    coverage_supplements, verify_coverage_input_hashes, write_coverage_context,
)
from scripts.report_migration_changed_line_coverage import changed_lines, lcov_line_hits
from scripts.validate_migration_parity_result import coverage_backend_executions


class CoverageContextTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.report = Path(self.directory.name) / "coverage.lcov"
        self.report.write_text("SF:source.rs\nDA:1,1\nend_of_record\n")
        self.hashes = {"source.rs": hashlib.sha256(b"source").hexdigest()}
        self.identity = {
            "run_id": "public-input-run",
            "targets": [{"revision": "measured-revision"}],
            "command": {"argv": ["make", "migration-parity-coverage-rust"]},
            "inputs": [],
        }

    def write(self, *, failed=0, not_run=0, full=False):
        write_coverage_context(
            self.report, self.hashes, "instrumented-build", self.identity,
            {"tests_failed": failed, "plans_not_run": not_run}, full_scope=full,
        )
        return json.loads(self.report.with_name(self.report.name + ".context.json").read_text())

    def test_report_digest_and_selected_scope_preserve_measured_identity(self):
        with patch("scripts.run_migration_rust_coverage.coverage_source_hashes", return_value=self.hashes):
            receipt = self.write()
        self.assertEqual(receipt["report_sha256"], hashlib.sha256(self.report.read_bytes()).hexdigest())
        self.assertEqual(receipt["source_hashes"], self.hashes)
        self.assertEqual(receipt["recorded_revision"], "measured-revision")
        self.assertEqual(receipt["scope"], "selected_tests")
        self.assertEqual(receipt["test_status"], "passed")

    def test_source_change_during_run_cannot_be_certified(self):
        with patch("scripts.run_migration_rust_coverage.coverage_source_hashes", return_value={"source.rs": "changed"}):
            with self.assertRaisesRegex(RuntimeError, "source changed"):
                self.write()
        self.assertFalse(self.report.with_name(self.report.name + ".context.json").exists())

    def test_failed_or_incomplete_execution_cannot_be_certified_as_passed(self):
        with patch("scripts.run_migration_rust_coverage.coverage_source_hashes", return_value=self.hashes):
            for failed, not_run in ((1, 0), (0, 1)):
                with self.subTest(failed=failed, not_run=not_run):
                    self.assertEqual(self.write(failed=failed, not_run=not_run)["test_status"], "failed")

    def test_full_scope_is_explicit(self):
        with patch("scripts.run_migration_rust_coverage.coverage_source_hashes", return_value=self.hashes):
            self.assertEqual(self.write(full=True)["scope"], "full")

    def test_changed_lines_exclude_deletion_only_hunks(self):
        diff = "@@ -1,2 +1,0 @@\n@@ -9 +8 @@\n@@ -30,0 +30,3 @@\n"
        self.assertEqual(changed_lines(diff), {8, 30, 31, 32})

    def test_lcov_retains_zero_hits_and_coalesces_duplicate_instantiations(self):
        report = "SF:source.rs\nDA:5,0\nDA:6,2\nend_of_record\nSF:source.rs\nDA:6,3\nDA:7,0\nend_of_record\n"
        self.assertEqual(lcov_line_hits(report), {"source.rs": {5: 0, 6: 3, 7: 0}})

    def test_external_lcov_records_do_not_hide_repository_zero_hits(self):
        report = "SF:/outside-checkout/dependency.rs\nDA:5,8\nend_of_record\nSF:pillow-rs/src/image.rs\nDA:5,0\nDA:6,2\nend_of_record\n"
        self.assertEqual(lcov_line_hits(report), {
            "/outside-checkout/dependency.rs": {5: 8},
            "pillow-rs/src/image.rs": {5: 0, 6: 2},
        })


class CoverageBackendTests(unittest.TestCase):
    def setUp(self):
        self.plans = [{"plan_id": "plan-one", "selectors": {
            "parity_case_ids": ["case-a", "case-b"], "command_ids": [],
        }}]

    def child(self, backend, *, failed=0, status="completed"):
        passed = 0 if status == "not_run" else 2 - failed
        return {
            "identity": {"run_id": f"run-{backend}", "targets": [{"backend": backend}]}, "status": "completed",
            "infrastructure_errors": [],
            "summary": {"plans_selected": 1, "plans_executed": int(status != "not_run"),
                        "plans_not_run": int(status == "not_run"), "tests_passed": passed, "tests_failed": failed},
            "plans": [{"plan_id": "plan-one", "selected": copy.deepcopy(self.plans[0]["selectors"]),
                       "execution": {"status": status, "tests_passed": passed, "tests_failed": failed}}],
        }

    def test_merge_counts_backend_executions_without_multiplying_unique_plans(self):
        children = {backend: self.child(backend) for backend in ("cpu", "simd", "gpu")}
        summary, plans, executions = aggregate_backend_coverage(self.plans, children)
        self.assertEqual(summary, {"plans_selected": 1, "plans_executed": 1, "plans_not_run": 0, "tests_passed": 6, "tests_failed": 0})
        self.assertEqual(plans["plan-one"]["tests_passed"], 6)
        self.assertEqual([item["backend"] for item in executions], ["cpu", "simd", "gpu"])
        self.assertTrue(coverage_execution_passed(summary, executions))
        merged = [{"plan_id": "plan-one", "selected": self.plans[0]["selectors"], "execution": plans["plan-one"]}]
        coverage_backend_executions(executions, merged, summary)

    def test_earlier_cpu_failure_cannot_be_hidden_by_passing_gpu(self):
        summary, plans, executions = aggregate_backend_coverage(self.plans, {
            "cpu": self.child("cpu", failed=1), "gpu": self.child("gpu"),
        })
        self.assertEqual(summary["tests_failed"], 1)
        self.assertEqual(summary["tests_passed"], 3)
        self.assertEqual(plans["plan-one"]["status"], "failed")
        self.assertFalse(coverage_execution_passed(summary, executions))
        with tempfile.TemporaryDirectory() as temporary:
            report = Path(temporary) / "coverage.lcov"
            report.write_text("SF:source.rs\nDA:1,1\nend_of_record\n")
            identity = {"targets": [{"revision": "revision"}], "run_id": "combined", "command": {}, "inputs": []}
            with patch.object(collector, "coverage_source_hashes", return_value={}):
                write_coverage_context(report, {}, "build", identity, summary, full_scope=False, backend_executions=executions)
            receipt = json.loads(report.with_name(report.name + ".context.json").read_text())
            self.assertEqual(receipt["test_status"], "failed")
            self.assertEqual(receipt["execution"]["backends"][0]["summary"]["tests_failed"], 1)

    def test_incomplete_or_failed_backend_status_cannot_pass_without_failed_test_count(self):
        for status in ("not_run", "failed"):
            with self.subTest(status=status):
                summary, plans, executions = aggregate_backend_coverage(self.plans, {
                    "cpu": self.child("cpu", status=status), "gpu": self.child("gpu"),
                })
                self.assertEqual(summary["plans_selected"], 1)
                self.assertEqual(plans["plan-one"]["status"], status)
                self.assertFalse(coverage_execution_passed(summary, executions))

    def test_equal_counts_with_different_case_or_command_ids_are_rejected(self):
        for field, value in (("parity_case_ids", ["case-a", "case-c"]), ("command_ids", ["other-command"])):
            with self.subTest(field=field):
                cpu = self.child("cpu")
                cpu["plans"][0]["selected"][field] = value
                with self.assertRaisesRegex(RuntimeError, "selected different"):
                    aggregate_backend_coverage(self.plans, {"cpu": cpu, "gpu": self.child("gpu")})

    def test_equal_plan_counts_with_different_plan_ids_are_rejected(self):
        cpu = self.child("cpu")
        cpu["plans"][0]["plan_id"] = "other-plan"
        with self.assertRaisesRegex(RuntimeError, "different plan IDs"):
            aggregate_backend_coverage(self.plans, {"cpu": cpu, "gpu": self.child("gpu")})

    def test_backend_identity_cannot_be_assumed_from_worker_name(self):
        with self.assertRaisesRegex(RuntimeError, "different backend identity"):
            aggregate_backend_coverage(self.plans, {"gpu": self.child("cpu")})

    def test_validator_rejects_backend_counts_omitted_from_merged_totals(self):
        summary, plans, executions = aggregate_backend_coverage(self.plans, {"cpu": self.child("cpu"), "gpu": self.child("gpu")})
        summary["tests_passed"] = 2
        merged = [{"plan_id": "plan-one", "selected": self.plans[0]["selectors"], "execution": plans["plan-one"]}]
        with self.assertRaisesRegex(ValueError, "merged tests_passed"):
            coverage_backend_executions(executions, merged, summary)


class CoverageInputTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name).resolve()
        self.fixture = self.root / "fixtures"
        (self.fixture / "assets").mkdir(parents=True)
        self.manifest = self.fixture / "manifest.yaml"
        self.manifest.write_text("manifest")
        self.root_patch = patch.object(collector, "ROOT", self.root)
        self.fixture_patch = patch.object(collector, "FIXTURE_ROOT", self.fixture)
        self.root_patch.start()
        self.fixture_patch.start()
        self.addCleanup(self.root_patch.stop)
        self.addCleanup(self.fixture_patch.stop)

    def test_referenced_asset_bytes_and_missing_state_are_bound_before_execution(self):
        image = self.fixture / "assets" / "image.png"
        image.write_bytes(b"original-image")
        cases = [{"assets": [{"kind": "ref", "path": "image.png"}, {"kind": "ref", "path": "missing.png"}]}]
        hashes = coverage_input_hashes(self.manifest, [], cases, [])
        self.assertEqual(hashes["fixtures/assets/image.png"], hashlib.sha256(b"original-image").hexdigest())
        self.assertIsNone(hashes["fixtures/assets/missing.png"])
        verify_coverage_input_hashes(hashes)
        image.write_bytes(b"changed-image")
        with self.assertRaisesRegex(RuntimeError, "input changed"):
            verify_coverage_input_hashes(hashes)
        image.write_bytes(b"original-image")
        (self.fixture / "assets" / "missing.png").write_bytes(b"created")
        with self.assertRaisesRegex(RuntimeError, "input changed"):
            verify_coverage_input_hashes(hashes)

    def test_context_cannot_attach_old_input_hashes_to_changed_asset_bytes(self):
        image = self.fixture / "assets" / "image.png"
        image.write_bytes(b"measured")
        hashes = coverage_input_hashes(self.manifest, [], [{"assets": [{"kind": "ref", "path": "image.png"}]}], [])
        report = self.root / "coverage.lcov"
        report.write_text("SF:source.rs\nDA:1,1\nend_of_record\n")
        identity = {"targets": [{"revision": "revision"}], "run_id": "run", "command": {}, "inputs": []}
        summary = {"plans_not_run": 0, "tests_failed": 0}
        image.write_bytes(b"changed")
        with patch.object(collector, "coverage_source_hashes", return_value={}):
            with self.assertRaisesRegex(RuntimeError, "input changed"):
                write_coverage_context(report, {}, "build", identity, summary, full_scope=False, input_hashes=hashes)
            self.assertFalse(report.with_name(report.name + ".context.json").exists())
            image.write_bytes(b"measured")
            write_coverage_context(report, {}, "build", identity, summary, full_scope=False, input_hashes=hashes)
        receipt = json.loads(report.with_name(report.name + ".context.json").read_text())
        self.assertEqual(receipt["input_hashes"], hashes)

    def test_native_json_and_pilfont_companions_are_measured_inputs(self):
        native = self.fixture / "inputs" / "font-native"
        native.mkdir(parents=True)
        source = native / "font.json"
        source.write_text(json.dumps({"cases": [{"inputs": {"assets": {"font": {"kind": "pilfont_ref", "id": "input/pilfont/font.pil"}}}}]}))
        fonts = self.fixture / "assets" / "font" / "pilfont"
        fonts.mkdir(parents=True)
        (fonts / "font.pil").write_bytes(b"metrics")
        image = fonts / "font.png"
        image.write_bytes(b"bitmap")
        scripts = ["run_migration_font_native_cases.py"]
        hashes = coverage_input_hashes(self.manifest, [], [], scripts)
        self.assertIn("fixtures/inputs/font-native/font.json", hashes)
        self.assertIn("fixtures/assets/font/pilfont/font.png", hashes)
        self.assertNotIn("/private/tmp/imagecore-save3.png", hashes)
        image.write_bytes(b"changed-bitmap")
        with self.assertRaisesRegex(RuntimeError, "input changed"):
            verify_coverage_input_hashes(hashes)
        image.write_bytes(b"bitmap")
        (native / "added.json").write_text('{"cases": []}')
        self.assertNotEqual(coverage_input_hashes(self.manifest, [], [], scripts), hashes)

    def test_selected_case_lanes_do_not_inherit_full_native_input_corpus(self):
        plans = [{"selectors": {"command_ids": []}}]
        self.assertEqual(coverage_supplements(plans, full_lane=False), [])
        self.assertEqual(len(coverage_supplements(plans, full_lane=True)), 7)


if __name__ == "__main__":
    unittest.main()
