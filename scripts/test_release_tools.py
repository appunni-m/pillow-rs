"""Regression tests for immutable Python publication and release recovery."""
from __future__ import annotations
import copy
import hashlib
from pathlib import Path
import tempfile
import unittest
from check_release_recovery import REQUIRED_JOBS, validate
from prepare_pypi_release import missing_files


class RecoveryTests(unittest.TestCase):
    def setUp(self) -> None:
        self.run = dict(id=71, name="Release", path=".github/workflows/release.yml",
                        event="push", head_branch="v0.1.2", head_sha="a" * 40,
                        status="completed", run_attempt=2, conclusion="failure")
        self.jobs = [dict(name=name, run_id=71, head_sha="a" * 40,
                          run_attempt=2, status="completed", conclusion="success")
                     for name in sorted(REQUIRED_JOBS)]

    def check(self, jobs: list[dict]) -> None:
        validate(self.run, jobs, tag="v0.1.2", commit="a" * 40)

    def test_final_job_failure_can_be_recovered_after_all_registries_succeed(self) -> None:
        self.check(self.jobs)

    def test_every_failed_skipped_or_missing_publish_gate_blocks_recovery(self) -> None:
        for index in range(len(self.jobs)):
            for result in ["failure", "skipped", "cancelled", None]:
                jobs = copy.deepcopy(self.jobs)
                jobs[index]["conclusion"] = result
                with self.assertRaises(ValueError):
                    self.check(jobs)
            with self.assertRaises(ValueError):
                self.check(self.jobs[:index] + self.jobs[index + 1:])

    def test_unrelated_runs_attempts_or_commits_cannot_supply_evidence(self) -> None:
        for field, value in [("run_id", 72), ("run_attempt", 1), ("head_sha", "b" * 40)]:
            jobs = copy.deepcopy(self.jobs)
            jobs[0][field] = value
            with self.assertRaises(ValueError):
                self.check(jobs)
        self.run["event"] = "pull_request"
        with self.assertRaises(ValueError):
            self.check(self.jobs)


class PyPIPublicationTests(unittest.TestCase):
    def test_partial_publication_stages_only_missing_identical_version_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            first = Path(directory) / "first.whl"
            second = Path(directory) / "second.whl"
            first.write_bytes(b"verified first wheel")
            second.write_bytes(b"verified second wheel")
            metadata = {"urls": [{"filename": first.name, "digests": {
                "sha256": hashlib.sha256(first.read_bytes()).hexdigest()}}]}
            self.assertEqual(missing_files([first, second], None), [first, second])
            self.assertEqual(missing_files([first, second], metadata), [second])
            self.assertEqual(missing_files([first], metadata), [])
            first.write_bytes(b"rebuilt with different bytes")
            with self.assertRaises(ValueError):
                missing_files([first, second], metadata)

    def test_empty_or_missing_package_never_counts_as_published(self) -> None:
        with self.assertRaises(ValueError):
            missing_files([], None)
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(ValueError):
                missing_files([Path(directory) / "missing.whl"], None)


if __name__ == "__main__":
    unittest.main()
