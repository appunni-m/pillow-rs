"""Provenance regressions for the maintained public-input coverage collector."""

import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from scripts.run_migration_rust_coverage import write_coverage_context
from scripts.report_migration_changed_line_coverage import changed_lines, lcov_line_hits


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


if __name__ == "__main__":
    unittest.main()
